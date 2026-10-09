//! Port of the words2num2 **sentence-level API**.
//!
//! Sources (the specification — bugs included):
//! - `words2num2/__init__.py`: `_resolve_lang`, `words2num_sentence`, `convert_sentence`, `sentence_to_words`
//! - `words2num2/converters/sentence.py`: `SentenceConverter`
//! - `words2num2/converters/auto.py`: `UNITS`, `CURRENCIES`, `Quantity`, `auto_parse`, `auto_parse_sentence`
//!
//! Every per-language `Words2Num_*` converter is dispatched **in Rust** by the
//! [`Converter`] abstraction below: `en` uses the hand-written grammar
//! ([`crate::w2n_lang_en`]); every other locale uses the generic reverse-table
//! lookup ([`crate::lookup`]) plus the `_parse_literal` tail — exactly what
//! `Words2Num_Base` does in Python, but without leaving Rust.
//!
//! # Fidelity notes — behaviour reproduced on purpose
//!
//! Verified against the live interpreter. These all look wrong and are all
//! correct ports:
//!
//! * `words2num_sentence("nineteen ninety nine")` used to be `"118"` (the
//!   walker calls `to_cardinal`, 19 + 99). Since a tens word may no longer
//!   follow a smaller number, and the walker now reads a spoken year that
//!   outgrows the cardinal ([`en_year_run`]) and a digit string read one
//!   digit at a time ([`en_digit_run`]), it is `"1999"` — and "four five six
//!   seven" is `"4567"` rather than `"22"`.
//! * `words2num_sentence("minus forty two")` → `"minus 42"`. A run may not
//!   *start* with a connector, and `to_cardinal("minus")` raises, so "minus"
//!   is not a run head. Same for `"a hundred and one dogs"` → `"a 101 dogs"`.
//! * `words2num_sentence("point five")` → `"0.5"`. `to_cardinal("point")`
//!   returns `Decimal(0)` rather than raising, so "point" *is* a valid head.
//! * `words2num_sentence("\"forty-two\"")` used to be `"42\""`: only
//!   *trailing* punctuation was stripped. Leading punctuation is now split off
//!   and kept (`"\"42\""`, #69).
//! * `words2num_sentence("zero point zero zero zero zero zero zero one")`
//!   used to be `"1E-7"` — Python's `Decimal.__str__` flips to scientific
//!   notation once the adjusted exponent drops below -6. Text output is now
//!   positional (`"0.0000001"`, #202, [`W2nValue::text_str`]); the *value*
//!   keeps Python's `str()` ([`py_decimal_str`]).
//! * `auto_parse("$5kn")` raises **KeyError**, not `Words2NumError`: the
//!   regex accepts `[kKmMbBtT][nN]?` but `SCALE_SUFFIXES` only holds
//!   `k K m M b B bn t T tn`. It escapes `auto_parse_sentence` uncaught
//!   because `_replace` only catches `Words2NumError`. See [`W2nError::Key`].
//! * `parse_number_string("0.5", lang="fr")` → `5`. French decimal is `,`,
//!   so the dot is dropped as a stray separator and `"05"` parses as an int.
//! * `_try_word_unit`'s `long_name = info.long` assignment is dead — it is
//!   unconditionally overwritten by the `next(...)` scan below it. Not ported.
//! * `word_units["pound sterling"]` is unreachable: the key holds a space but
//!   the lookup token comes from `rsplit(None, 1)`, so it never contains one.
//! * The `°[CF]?` alternative appears **twice** in `_try_digit_unit`'s regex.
//!   Harmless; the second is dead. Kept in the comment, folded in the code.

mod lexicon;
mod tokens;

use bigdecimal::num_traits::FromPrimitive;
use bigdecimal::BigDecimal;
use num_bigint::BigInt;
use std::collections::HashMap;
use std::str::FromStr;

// ===========================================================================
// Errors
// ===========================================================================

/// The exception kinds this layer can produce, each carrying the *exact*
/// message Python formats. The PyO3 binder maps each variant onto the matching
/// Python exception class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum W2nError {
    /// `words2num2.base.Words2NumError` — a `ValueError` subclass.
    Words2Num(String),
    /// `NotImplementedError` — raised by `_resolve_lang`.
    NotImplemented(String),
    /// `KeyError`. No longer produced: an unknown scale suffix (`"$5kn"`) is a
    /// `Words2Num` error, so `auto_parse_sentence` leaves the span verbatim
    /// instead of aborting (#194). Kept so the binder's mapping stays total.
    /// Python's `KeyError` stringifies as `repr(key)`, hence the odd quoting.
    Key(String),
}

impl W2nError {
    /// The message text Python would carry (`str(exc)`).
    ///
    /// Note `KeyError` is special-cased: `str(KeyError("kn"))` is `"'kn'"`,
    /// not `"kn"`.
    pub fn message(&self) -> String {
        match self {
            W2nError::Words2Num(m) | W2nError::NotImplemented(m) => m.clone(),
            W2nError::Key(k) => py_repr_str(k),
        }
    }
}

// ===========================================================================
// Values — words2num2 returns int | float | Decimal
// ===========================================================================

/// A value as words2num2 produces it.
///
/// `Dec` mirrors Python's `decimal.Decimal` as the (coefficient, exponent)
/// pair that `Decimal.as_tuple()` exposes: `BigDecimal`'s
/// `as_bigint_and_exponent()` returns `(digits, scale)` where the value is
/// `digits * 10^-scale`, i.e. `_int = digits.abs()` and `_exp = -scale`.
/// Keeping the *unnormalised* scale is load-bearing — `Decimal("2.0")` and
/// `Decimal("2")` are equal but stringify differently.
#[derive(Debug, Clone, PartialEq)]
pub enum W2nValue {
    Int(BigInt),
    Float(f64),
    Dec(BigDecimal),
}

impl W2nValue {
    /// Python's `str(value)`.
    pub fn py_str(&self) -> String {
        match self {
            W2nValue::Int(i) => i.to_string(),
            W2nValue::Float(f) => py_float_repr(*f),
            W2nValue::Dec(d) => {
                let (digits, scale) = d.as_bigint_and_exponent();
                py_decimal_str(&digits, scale)
            }
        }
    }

    /// The value as it is written into running text: positional notation,
    /// never an exponent (#202). `py_str` copies Python's `str()`, which turns
    /// `Decimal("0.0000001")` into `"1E-7"` and `1e-07` for a float; a text
    /// normaliser must write `0.0000001`. The value itself (`py_repr`) is
    /// untouched.
    pub fn text_str(&self) -> String {
        match self {
            W2nValue::Int(i) => i.to_string(),
            W2nValue::Float(f) => {
                let r = py_float_repr(*f);
                if r.contains('e') {
                    // Rust's `Display` is the shortest round-trip digits in
                    // positional form (1e-7 -> "0.0000001").
                    format!("{}", f)
                } else {
                    r
                }
            }
            W2nValue::Dec(d) => {
                let (digits, scale) = d.as_bigint_and_exponent();
                decimal_positional_str(&digits, scale)
            }
        }
    }

    /// Python's `repr(value)`. Differs from `str` only for `Decimal`.
    pub fn py_repr(&self) -> String {
        match self {
            W2nValue::Dec(_) => format!("Decimal('{}')", self.py_str()),
            _ => self.py_str(),
        }
    }

    /// `value == 1 or value == -1`, as Python's `value in (1, -1, 1.0, -1.0)`
    /// evaluates it (membership uses `==`, so `Decimal("1")` matches).
    fn is_unit_magnitude(&self) -> bool {
        match self {
            W2nValue::Int(i) => {
                let one = BigInt::from(1);
                *i == one || *i == -one
            }
            W2nValue::Float(f) => *f == 1.0 || *f == -1.0,
            W2nValue::Dec(d) => {
                let one = BigDecimal::from(1);
                *d == one || *d == -one
            }
        }
    }
}

/// Convert an English-grammar value ([`crate::w2n_lang_en::W2nValue`]) into a
/// sentence-layer [`W2nValue`].
///
/// LIMITATION: `PyDec` carries a signed zero (`Decimal('-0.0')`), which
/// `BigDecimal` cannot; a negative-zero decimal therefore loses its sign here.
/// It is unreachable through the sentence walker (a run cannot start with
/// "minus", so a negative decimal never heads a run) and through the tested
/// `auto_parse` paths.
fn en_to_sentence_value(v: crate::w2n_lang_en::W2nValue) -> W2nValue {
    use crate::w2n_lang_en::W2nValue as E;
    match v {
        E::Int(i) => W2nValue::Int(i),
        E::Float(f) => W2nValue::Float(f),
        E::Dec(d) => W2nValue::Dec(d.to_bigdecimal()),
    }
}

// ===========================================================================
// Python string / number formatting primitives
// ===========================================================================

/// Python's `repr()` for `str`, used by every `%r` in the source.
///
/// Quote selection matches CPython: single quotes unless the string contains
/// a single quote and no double quote.
///
/// LIMITATION: CPython escapes non-printable *non-ASCII* (per the unicodedata
/// category) as `\xXX` / `\uXXXX` / `\UXXXXXXXX`. Here non-ASCII passes
/// through verbatim. Every `%r` reachable in this module formats a locale code,
/// a unit token or a type name, so the gap is unreachable in practice.
pub fn py_repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || (c as u32) == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Python's `repr()`/`str()` for `float` (they are the same in Python 3).
///
/// CPython's `format_float_short(..., 'r', ...)`: take the shortest digit
/// string that round-trips, then switch to exponential iff
/// `decpt <= -4 || decpt > 16`, where `decpt` is the decimal point position
/// (`value = 0.<digits> * 10^decpt`). `Py_DTSF_ADD_DOT_0` appends `.0` in the
/// fixed branch only.
///
/// Rust's `{:e}` already yields the shortest round-tripping digits, so it is
/// used purely as a digit/exponent source and then reformatted Python-style.
/// This matters: Rust's own `{}` would print `1e16` as `10000000000000000`
/// and `1e-5` as `0.00001`, where Python prints `1e+16` and `1e-05`.
pub fn py_float_repr(v: f64) -> String {
    if v.is_nan() {
        return "nan".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    let sci = format!("{:e}", v); // e.g. "-1.2345e3", "0e0", "1e-7"
    let (mant, exp) = sci.split_once('e').expect("{:e} always emits an exponent");
    let exp: i32 = exp.parse().expect("{:e} exponent is an integer");
    let neg = mant.starts_with('-');
    let mant = mant.trim_start_matches('-');
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let decpt = exp + 1;
    let sign = if neg { "-" } else { "" };
    let nd = digits.len() as i32;

    if decpt <= -4 || decpt > 16 {
        // Exponential: d[.ddd]e{+,-}XX, exponent at least two digits.
        let mut m = String::from(&digits[..1]);
        if nd > 1 {
            m.push('.');
            m.push_str(&digits[1..]);
        }
        let e = decpt - 1;
        let (esign, eabs) = if e < 0 { ("-", -e) } else { ("+", e) };
        return format!("{}{}e{}{:02}", sign, m, esign, eabs);
    }
    if decpt <= 0 {
        format!("{}0.{}{}", sign, "0".repeat((-decpt) as usize), digits)
    } else if decpt >= nd {
        // ADD_DOT_0
        format!(
            "{}{}{}.0",
            sign,
            digits,
            "0".repeat((decpt - nd) as usize)
        )
    } else {
        format!(
            "{}{}.{}",
            sign,
            &digits[..decpt as usize],
            &digits[decpt as usize..]
        )
    }
}

/// Python's `Decimal.__str__` (from `_pydecimal.__str__`, `eng=False`,
/// `context.capitals = 1`), reconstructed from the coefficient and exponent:
///
/// ```text
/// leftdigits = _exp + len(_int)
/// if _exp <= 0 and leftdigits > -6:  dotplace = leftdigits   # plain
/// else:                              dotplace = 1            # scientific
/// ```
///
/// This is what turns `Decimal("0.0000001")` into `"1E-7"`.
///
/// `digits`/`scale` come from `BigDecimal::as_bigint_and_exponent()`:
/// value = `digits * 10^-scale`, so `_exp = -scale` and `_int = |digits|`.
pub fn py_decimal_str(digits: &BigInt, scale: i64) -> String {
    let neg = digits.sign() == num_bigint::Sign::Minus;
    let int_str = digits.magnitude().to_string(); // _int, unsigned
    let exp: i64 = -scale; // _exp
    let len_int = int_str.chars().count() as i64;
    let leftdigits = exp + len_int;

    let dotplace = if exp <= 0 && leftdigits > -6 {
        leftdigits
    } else {
        1
    };

    let (intpart, fracpart) = if dotplace <= 0 {
        (
            String::from("0"),
            format!(".{}{}", "0".repeat((-dotplace) as usize), int_str),
        )
    } else if dotplace >= len_int {
        (
            format!("{}{}", int_str, "0".repeat((dotplace - len_int) as usize)),
            String::new(),
        )
    } else {
        let cut = dotplace as usize;
        (int_str[..cut].to_string(), format!(".{}", &int_str[cut..]))
    };

    let expo = if leftdigits == dotplace {
        String::new()
    } else {
        format!("E{:+}", leftdigits - dotplace)
    };
    format!(
        "{}{}{}{}",
        if neg { "-" } else { "" },
        intpart,
        fracpart,
        expo
    )
}

/// `format(Decimal, "f")`: the coefficient with the point placed by the
/// exponent, never in scientific notation. Keeps the unnormalised scale, so
/// `Decimal("2.50")` stays `"2.50"`.
pub fn decimal_positional_str(digits: &BigInt, scale: i64) -> String {
    let neg = digits.sign() == num_bigint::Sign::Minus;
    let int_str = digits.magnitude().to_string();
    let body = if scale <= 0 {
        format!("{}{}", int_str, "0".repeat((-scale) as usize))
    } else {
        let scale = scale as usize;
        let padded = if int_str.len() <= scale {
            format!("{}{}", "0".repeat(scale + 1 - int_str.len()), int_str)
        } else {
            int_str
        };
        let cut = padded.len() - scale;
        format!("{}.{}", &padded[..cut], &padded[cut..])
    };
    format!("{}{}", if neg { "-" } else { "" }, body)
}

// ===========================================================================
// Unicode / Python text primitives
// ===========================================================================

/// Starts of every 10-codepoint run in Unicode category `Nd`, which is exactly
/// what Python's `\d` matches for `str` patterns (and what `int()` / `float()`
/// accept). Every `Nd` block is a contiguous ten with digit value 0 at the
/// start, so a block start plus an offset is all that is needed.
///
/// Generated from the local interpreter (`unicodedata.unidata_version` 13.0.0,
/// CPython 3.9). A newer CPython knows a few more blocks; regenerate if the
/// oracle moves. Without this, `auto_parse("٤٢")` would fail where Python
/// returns `42`.
const ND_BLOCKS: [u32; 65] = [
    0x0030, 0x0660, 0x06F0, 0x07C0, 0x0966, 0x09E6, 0x0A66, 0x0AE6, 0x0B66, 0x0BE6, 0x0C66, 0x0CE6,
    0x0D66, 0x0DE6, 0x0E50, 0x0ED0, 0x0F20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0, 0x1A80,
    0x1A90, 0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0,
    0xFF10, 0x104A0, 0x10D30, 0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0,
    0x11650, 0x116C0, 0x11730, 0x118E0, 0x11950, 0x11C50, 0x11D50, 0x11DA0, 0x16A60, 0x16B50,
    0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0, 0x1E950, 0x1FBF0,
];

/// The digit value of `c` if it is in category `Nd`, else `None`.
pub fn digit_value(c: char) -> Option<u32> {
    let cp = c as u32;
    match ND_BLOCKS.binary_search(&cp) {
        Ok(_) => Some(0),
        Err(0) => None,
        Err(i) => {
            let start = ND_BLOCKS[i - 1];
            if cp - start < 10 {
                Some(cp - start)
            } else {
                None
            }
        }
    }
}

/// Python's regex `\d` for `str` patterns.
pub fn is_unicode_digit(c: char) -> bool {
    digit_value(c).is_some()
}

/// Fold every `Nd` digit down to ASCII, which is what `int()` / `float()` do
/// internally before parsing.
fn to_ascii_digits(s: &str) -> String {
    s.chars()
        .map(|c| match digit_value(c) {
            Some(v) if !c.is_ascii_digit() => char::from_digit(v, 10).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// Python's `str.isspace()` per character — equivalently the regex `\s` for
/// `str` patterns. Rust's `char::is_whitespace` agrees except for the four
/// C0 separators `\x1c-\x1f`, which Python counts and Rust does not.
pub fn py_is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Python's `str.isspace()` — all characters are space, and the string is
/// non-empty.
fn py_str_isspace(s: &str) -> bool {
    !s.is_empty() && s.chars().all(py_is_space)
}

/// Python's `str.strip()` (no argument).
fn py_strip(s: &str) -> &str {
    s.trim_matches(py_is_space)
}

/// Python's `str.split()` (no argument): split on runs of whitespace,
/// discarding empty fields.
fn py_split_whitespace(s: &str) -> Vec<&str> {
    s.split(py_is_space).filter(|p| !p.is_empty()).collect()
}

/// Python's `str.rsplit(None, 1)`.
///
/// Trailing whitespace is skipped, the last whitespace-free run becomes the
/// tail, and the head keeps *its own* leading whitespace but loses the
/// separator run — `"  a  b  ".rsplit(None, 1) == ["  a", "b"]`.
fn py_rsplit_once_ws(s: &str) -> Vec<&str> {
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let mut end = chars.len();
    while end > 0 && py_is_space(chars[end - 1].1) {
        end -= 1;
    }
    if end == 0 {
        return Vec::new();
    }
    let mut tail_start = end;
    while tail_start > 0 && !py_is_space(chars[tail_start - 1].1) {
        tail_start -= 1;
    }
    let byte_end = if end == chars.len() {
        s.len()
    } else {
        chars[end].0
    };
    let tail = &s[chars[tail_start].0..byte_end];
    let mut head_end = tail_start;
    while head_end > 0 && py_is_space(chars[head_end - 1].1) {
        head_end -= 1;
    }
    if head_end == 0 {
        return vec![tail];
    }
    vec![&s[..chars[head_end].0], tail]
}

/// Python's `str.rstrip(chars)` for a single-character set.
fn py_rstrip_char(s: &str, ch: char) -> &str {
    s.trim_end_matches(ch)
}

/// `re.sub(r"[\.,;:!\?\"']+$", "", s)` — drop the trailing punctuation run.
///
/// Also the script-native marks and closing brackets/quotes `normalize` folds
/// to a space (#121): "पाँच।" reads 5 and keeps its danda, "five)" its bracket.
fn rstrip_punct(s: &str) -> &str {
    s.trim_end_matches(|c: char| {
        matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'' | '\u{2019}')
            || crate::is_clause_punctuation(c)
    })
}

/// The piece opens with a bracket, quote or other mark `normalize` drops
/// ("(five)", "«six»", "‘seven’"). Re-emitting the value would lose it, so the
/// walker leaves such a piece as written.
fn opens_with_punct(s: &str) -> bool {
    s.chars().next().is_some_and(|c| {
        crate::is_clause_punctuation(c) || matches!(c, '\u{2018}' | '\u{201B}' | '`')
    })
}

/// `re.search(r"[\.,;:!\?\"']+$", s)` → `m.group()`, or `""`.
///
/// The leftmost match of `[...]+$` is exactly the maximal trailing run.
fn trailing_punct(s: &str) -> &str {
    let kept = rstrip_punct(s);
    &s[kept.len()..]
}

/// `re.search(r"[\.,;:!\?]$", tok)` — note this class has **no** quote
/// characters, unlike the strip above: `forty-two"` does not close a run.
fn ends_with_terminal_punct(s: &str) -> bool {
    matches!(
        s.chars().next_back(),
        Some('.') | Some(',') | Some(';') | Some(':') | Some('!') | Some('?')
    )
}

/// `re.findall(r"\S+|\s+", sentence)` — alternating runs of non-space and
/// space. The two branches are complementary, so this is a simple run split
/// and `parts.concat() == sentence` always holds.
fn tokenize(sentence: &str) -> Vec<String> {
    let chars: Vec<char> = sentence.chars().collect();
    let mut parts = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let space = py_is_space(chars[i]);
        let start = i;
        while i < chars.len() && py_is_space(chars[i]) == space {
            i += 1;
        }
        parts.push(chars[start..i].iter().collect());
    }
    parts
}

// ===========================================================================
// `_resolve_lang`
// ===========================================================================

/// Every language key the parser accepts.
///
/// **Derived from the renderer, not maintained by hand.** This used to be a
/// frozen 120-entry array carrying the note *"Must stay in sync with
/// `__init__.py`"* — a file that no longer exists, since the pure-Python
/// implementation it mirrored was replaced by this crate. It drifted badly:
/// the renderer grew to 172 keys and the list did not, with two distinct
/// consequences.
///
/// **28 languages were unreachable.** `resolve_lang` rejected them outright,
/// so `words2num(text, lang="ti")` raised `NotImplementedError: language 'ti'
/// is not supported` — while `supported_langs()` (which reads the renderer's
/// keys) advertised them, and the reverse tables parsed them perfectly. Three
/// answers to one question.
///
/// **8 more were silently routed to a different language.** `resolve_lang`
/// falls back to the first two characters of the code, so an unknown key did
/// not fail, it *matched something else*:
///
/// ```text
/// ban Balinese   -> ba  Bashkir
/// ceb Cebuano    -> ce  Chechen
/// cnh Hakha Chin -> cn  Chinese
/// fil Filipino   -> fi  Finnish
/// kok Konkani    -> ko  Korean
/// miz Mizo       -> mi  Maori
/// pap Papiamento -> pa  Punjabi
/// pli Pali       -> pl  Polish
/// ```
///
/// That failed loudly only by luck — the tables happened to share no words.
/// A collision would have returned a confidently wrong number.
///
/// Deriving the set makes both impossible: it is exactly what
/// [`crate::supported_langs`] reports, so the advertised set and the accepted
/// set cannot disagree again.
pub fn converter_langs() -> &'static [&'static str] {
    static KEYS: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    KEYS.get_or_init(num2words2_core::supported_lang_keys)
        .as_slice()
}

/// The `&'static str` the renderer knows this key by.
///
/// `resolve_lang` hands back an owned `String`; the converter needs a
/// `'static` key to build [`Converter::Table`]. Looking it up here keeps the
/// two in step — before, this was a scan of the frozen array whose miss arm
/// silently fell back to English.
fn static_lang_key(k: &str) -> Option<&'static str> {
    converter_langs().iter().copied().find(|s| *s == k)
}

fn is_known_lang(k: &str) -> bool {
    converter_langs().contains(&k)
}

/// ISO 639-2 three-letter codes (bibliographic and terminological forms) of
/// the two-letter keys, for callers that pass `"eng"`, `"fra"` or `"ger"`.
///
/// This replaces the old "first two characters" fallback, which got `"eng"`
/// right by accident and everything else wrong: `"swedish"` resolved to `sw`
/// (Swahili), `"estonian"` to `es`, `"kazakh"` to `ka` (Georgian) and `"enx"`
/// to `en`, so input in the language the caller meant raised, or a word of the
/// accidental language was read as a number. An explicit table only answers
/// for codes that really name the language.
const ISO_639_2_ALIASES: &[(&str, &str)] = &[
    ("afr", "af"), ("amh", "am"), ("ara", "ar"), ("asm", "as"), ("aze", "az"),
    ("bak", "ba"), ("bel", "be"), ("bul", "bg"), ("bam", "bm"), ("ben", "bn"),
    ("bod", "bo"), ("tib", "bo"), ("bre", "br"), ("bos", "bs"), ("cat", "ca"),
    ("che", "ce"), ("ces", "cs"), ("cze", "cs"), ("cym", "cy"), ("wel", "cy"),
    ("dan", "da"), ("deu", "de"), ("ger", "de"), ("div", "dv"), ("ell", "el"),
    ("gre", "el"), ("eng", "en"), ("epo", "eo"), ("spa", "es"), ("est", "et"),
    ("eus", "eu"), ("baq", "eu"), ("fas", "fa"), ("per", "fa"), ("ful", "ff"),
    ("fin", "fi"), ("fao", "fo"), ("fra", "fr"), ("fre", "fr"), ("glg", "gl"),
    ("guj", "gu"), ("hau", "ha"), ("heb", "he"), ("hin", "hi"), ("hrv", "hr"),
    ("hat", "ht"), ("hun", "hu"), ("hye", "hy"), ("arm", "hy"), ("ind", "id"),
    ("isl", "is"), ("ice", "is"), ("ita", "it"), ("jpn", "ja"), ("jav", "jv"),
    ("kat", "ka"), ("geo", "ka"), ("kik", "ki"), ("kaz", "kk"), ("khm", "km"),
    ("kan", "kn"), ("kor", "ko"), ("kur", "ku"), ("kir", "ky"), ("lat", "la"),
    ("ltz", "lb"), ("lug", "lg"), ("lin", "ln"), ("lao", "lo"), ("lit", "lt"),
    ("lav", "lv"), ("mlg", "mg"), ("mri", "mi"), ("mao", "mi"), ("mkd", "mk"),
    ("mac", "mk"), ("mal", "ml"), ("mon", "mn"), ("mar", "mr"), ("msa", "ms"),
    ("may", "ms"), ("mlt", "mt"), ("mya", "my"), ("bur", "my"), ("nob", "nb"),
    ("nep", "ne"), ("nld", "nl"), ("dut", "nl"), ("nno", "nn"), ("nor", "no"),
    ("oci", "oc"), ("orm", "om"), ("ori", "or"), ("pan", "pa"), ("pol", "pl"),
    ("pus", "ps"), ("por", "pt"), ("roh", "rm"), ("ron", "ro"), ("rum", "ro"),
    ("rus", "ru"), ("kin", "rw"), ("san", "sa"), ("snd", "sd"), ("sin", "si"),
    ("slk", "sk"), ("slo", "sk"), ("slv", "sl"), ("sna", "sn"), ("som", "so"),
    ("sqi", "sq"), ("alb", "sq"), ("srp", "sr"), ("sun", "su"), ("swe", "sv"),
    ("swa", "sw"), ("tam", "ta"), ("tel", "te"), ("tgk", "tg"), ("tha", "th"),
    ("tir", "ti"), ("tuk", "tk"), ("tgl", "tl"), ("tur", "tr"), ("tat", "tt"),
    ("ukr", "uk"), ("urd", "ur"), ("uzb", "uz"), ("vie", "vi"), ("wol", "wo"),
    ("xho", "xh"), ("yid", "yi"), ("yor", "yo"), ("zho", "zh"), ("chi", "zh"),
    ("zul", "zu"),
];

/// The supported key `code` names, ignoring case, or its ISO 639-2 alias.
///
/// Case-insensitive so `"EN"`, `"Fr"` and `"sr_latn"` resolve like `"en"`,
/// `"fr"` and `"sr_Latn"`. Two keys differ only by case (`en_Aero_ICAO` and
/// `en_aero_icao`); both name the same renderer, so taking the first is safe.
fn known_lang_ci(code: &str) -> Option<&'static str> {
    let lower = code.to_lowercase();
    if let Some(k) = converter_langs()
        .iter()
        .copied()
        .find(|k| k.to_lowercase() == lower)
    {
        return Some(k);
    }
    ISO_639_2_ALIASES
        .iter()
        .find(|(alias, _)| *alias == lower)
        .and_then(|(_, key)| static_lang_key(key))
}

/// Resolve a caller's language code to a supported key.
///
/// 1. An exact key (`"en"`, `"sr_Latn"`, `"en_Aero_FAA"`).
/// 2. The same with `-` read as `_` (`"pt-BR"`).
/// 3. A key that matches ignoring case, or an ISO 639-2 code (`"EN"`,
///    `"pt_br"`, `"eng"`, `"fra"`).
/// 4. Unknown trailing subtags dropped one at a time, each retried as in 3:
///    `"en-US"` -> `en`, `"EN_us"` -> `en`, `"sr-Latn-RS"` -> `sr_Latn`.
///
/// Anything else, including language *names* (`"swedish"`, `"german"`), is
/// `NotImplementedError`. There is deliberately no "first two characters"
/// guess: it turned `"swedish"` into Swahili and `"estonian"` into Spanish.
pub fn resolve_lang(lang: &str) -> Result<String, W2nError> {
    if is_known_lang(lang) {
        return Ok(lang.to_string());
    }
    let normalized = lang.replace('-', "_");
    if is_known_lang(&normalized) {
        return Ok(normalized);
    }
    let parts: Vec<&str> = normalized.split('_').collect();
    // Empty fields ("en__US", "_en") are not subtags: no guessing there.
    if !parts.iter().any(|p| p.is_empty()) {
        for n in (1..=parts.len()).rev() {
            if let Some(k) = known_lang_ci(&parts[..n].join("_")) {
                return Ok(k.to_string());
            }
        }
    }
    Err(W2nError::NotImplemented(format!(
        "language {} is not supported",
        py_repr_str(lang)
    )))
}

// ===========================================================================
// Converter — the per-locale dispatch, entirely in Rust
// ===========================================================================

/// `Words2Num_Base.NEGATIVE_WORDS` — every generic locale inherits this; no
/// locale module overrides it.
const BASE_NEGATIVE_WORDS: [&str; 2] = ["minus", "negative"];

/// A per-locale converter, standing in for `CONVERTER_CLASSES[lang]`.
enum Converter {
    /// `Words2Num_EN` — the one hand-written grammar.
    En,
    /// A `Words2Num_Base` subclass; the field is the num2words2 core key its
    /// `LANG` attribute carries (e.g. `"fr"`, `"zh_CN"`).
    Table(&'static str),
}

/// Resolve a `CONVERTER_CLASSES` key (as produced by [`resolve_lang`]) to its
/// [`Converter`].
///
/// Most keys map to the same `LANG`, but three do not — matching the Python
/// registry: `"zh"`/`"cn"` both use `Words2Num_ZH_CN` (`LANG="zh_CN"`) and
/// `"jp"` uses `Words2Num_JA` (`LANG="ja"`).
fn converter_for(resolved: &str) -> Converter {
    match resolved {
        "en" => Converter::En,
        "zh" | "cn" => Converter::Table("zh_CN"),
        "jp" => Converter::Table("ja"),
        // `resolved` came from `resolve_lang`, so it is always a member and
        // the `None` arm is unreachable. It keeps the previous fallback
        // rather than panicking in a library.
        other => Converter::Table(static_lang_key(other).unwrap_or("en")),
    }
}

/// Regional English with its own num2words2 table: en_IN (lakh, crore), en_NG,
/// en_NE, en_NP. Their table holds only the canonical cardinal spelling, so on
/// its own it misses everything the English grammar reads -- decimals ("three
/// point one four"), year pairs ("nineteen ninety nine"), "a hundred", "oh",
/// scales above the table (#193). The table is asked first, so the regional
/// vocabulary keeps its reading (en_IN "one lakh" = 100000), and the English
/// grammar takes what the table declines.
pub(crate) fn en_variant(lang: &str) -> bool {
    matches!(lang, "en_IN" | "en_NG" | "en_NE" | "en_NP")
}

/// The English grammar as a fallback for an [`en_variant`] table: `first` is
/// the table's answer, returned when it parsed (or when `lang` is not a
/// regional English); otherwise the grammar's, and the table's error when both
/// decline.
fn en_variant_fallback(
    lang: &str,
    first: Result<W2nValue, W2nError>,
    grammar: impl FnOnce() -> Result<crate::w2n_lang_en::W2nValue, crate::w2n_lang_en::W2nError>,
) -> Result<W2nValue, W2nError> {
    match first {
        Err(e) if en_variant(lang) => en_convert(grammar()).map_err(|_| e),
        r => r,
    }
}

impl Converter {
    /// `converter.to_cardinal(token)` did not raise? (`_token_is_number_word`).
    fn is_number_word(&self, token: &str) -> bool {
        match self {
            Converter::En => self.to_cardinal(token).is_ok(),
            Converter::Table(lang) => {
                let norm = crate::normalize(token);
                // Cheap gate first: most words of a transcript are not numbers.
                (self.apocope(&norm).is_some()
                    || crate::token_may_be_number(lang, &norm)
                    || lexicon::is_alias_word(lang, &norm))
                    && self.to_cardinal(&norm).is_ok()
                    // he "שני" is 2 only in front of מיליון, ar "وواحد" only after a scale.
                    && (self.apocope(&norm).is_some()
                        || crate::lookup_hundred_prefix_norm(lang, &norm).is_some()
                        || !crate::w2n_forms::is_bound_form(lang, &norm))
            }
        }
    }

    /// A word read as a number only inside a longer one (see [`Self::is_number_part`]).
    fn is_bound(&self, token: &str) -> bool {
        match self {
            Converter::En => false,
            Converter::Table(lang) => {
                let norm = crate::normalize(token);
                self.apocope(&norm).is_none() && crate::w2n_forms::is_bound_form(lang, &norm)
            }
        }
    }

    /// A word that is a number only as part of a longer one: a multiplier form
    /// (he "שני" מיליון), a combining form (kn "ನೂರ", mn "дөчин"), a glued "and"
    /// (ar "وواحد"). It may carry a run on, never make one by itself.
    fn is_number_part(&self, token: &str) -> bool {
        match self {
            Converter::En => false,
            Converter::Table(lang) => {
                let norm = crate::normalize(token);
                crate::w2n_forms::is_number_part(lang, &norm) || crate::w2n_forms::is_bound_form(lang, &norm)
            }
        }
    }

    /// The apocopated form a language uses inside a number ("cincuenta y
    /// **un** centavos", "**un** mil") mapped to the word the reverse table
    /// holds. `None` for a word that is not one, or for English.
    fn apocope(&self, token: &str) -> Option<&'static str> {
        match self {
            Converter::En => None,
            Converter::Table(lang) => apocope_word(lang, token),
        }
    }

    fn to_cardinal(&self, text: &str) -> Result<W2nValue, W2nError> {
        match self {
            Converter::En => en_convert(crate::en_to_cardinal(text)),
            Converter::Table(lang) => {
                en_variant_fallback(lang, base_convert(lang, text, false), || crate::en_to_cardinal(text))
            }
        }
    }

    fn to_ordinal(&self, text: &str) -> Result<W2nValue, W2nError> {
        match self {
            Converter::En => en_convert(crate::en_to_ordinal(text)),
            Converter::Table(lang) => {
                en_variant_fallback(lang, base_convert(lang, text, true), || crate::en_to_ordinal(text))
            }
        }
    }

    /// Is `token` (one part, hyphens allowed) an ordinal: "fifteenth",
    /// "twenty-first", fr "vingt-troisième", de "zweiten"?
    fn is_ordinal_word(&self, token: &str) -> bool {
        if holds_digit(token) {
            return false; // "2" is a literal, not the ordinal table's business
        }
        match self {
            Converter::En => token
                .split('-')
                .last()
                .is_some_and(|w| crate::w2n_lang_en::ordinal_cardinal(w).is_some()),
            // A word that is also a cardinal is not an ordinal: vi writes the ordinal
            // as "thứ" + cardinal, so its ordinal table holds every cardinal too.
            Converter::Table(lang) => {
                let norm = crate::normalize(token);
                matches!(crate::lookup(lang, &norm, true, &[]), Ok(Some(_))) && !self.is_number_word(&norm)
            }
        }
    }

    /// The num2words2 key this converter renders with.
    fn n2w_key(&self) -> &'static str {
        match self {
            Converter::En => "en",
            Converter::Table(lang) => lang,
        }
    }

    fn to_year(&self, text: &str) -> Result<W2nValue, W2nError> {
        match self {
            Converter::En => en_convert(crate::en_to_year(text)),
            Converter::Table(lang) => {
                en_variant_fallback(lang, table_year(lang, text), || crate::en_to_year(text))
            }
        }
    }

    /// `Words2Num_Base.to_ordinal_num` — EN inherits it unchanged.
    ///
    /// A language whose renderer writes `ordinal_num` in Roman numerals (la)
    /// or in words (ar, bn) also reads those forms back (#127, #246).
    fn to_ordinal_num(&self, text: &str) -> Result<W2nValue, W2nError> {
        let key = self.n2w_key();
        let err = match base_ordinal_num(text, key) {
            Ok(v) => return Ok(v),
            Err(e) => e,
        };
        if text.chars().any(is_unicode_digit) {
            return Err(err);
        }
        match ordinal_num_shape(key) {
            OrdinalNumShape::Roman => roman_value(text)
                .map(|n| W2nValue::Int(BigInt::from(n)))
                .ok_or(err),
            OrdinalNumShape::Words => self.to_ordinal(text).map_err(|_| err),
            OrdinalNumShape::Figures => Err(err),
        }
    }

    /// `Words2Num_Base.to_currency` == `self.to_cardinal` (polymorphic, so EN
    /// currency uses EN cardinal).
    fn to_currency(&self, text: &str) -> Result<W2nValue, W2nError> {
        self.to_cardinal(text)
    }

    /// Digit-sequence mode: spoken digit-by-digit spellings ("two seven five"
    /// → 275, "sieben acht" → 78) are concatenated, not summed. The cardinal
    /// grammar mis-parses these (e.g. "two seven five" → 14); dictated house
    /// numbers, zip codes and phone-style runs are commonly spelled this way.
    ///
    /// The run is read left to right, one digit word (0-9 in the target
    /// language) at a time:
    ///
    /// * a digit word may span several tokens -- lus/miz zero is "a awmlo"
    ///   (#106) -- so up to three tokens are tried, shortest first;
    /// * a hyphenated token is first read whole -- Wolof 6-9 are the single
    ///   words "juróom-benn".."juróom-ñeent" (#50) -- and only split on its
    ///   hyphens ("one-two-three") when that fails;
    /// * English (and its regional variants) also takes "double"/"triple"
    ///   before a digit ("double zero seven" → 007) and "o" as zero, as the
    ///   sentence walker's digit runs do (#190).
    ///
    /// Anything else (a non-digit, or 10+) makes this decline with `Err`, so
    /// the sentence walker falls back to its longest cardinal parse and never
    /// turns a real cardinal into a digit string. A run of any length is read:
    /// the digits are concatenated into a `BigInt`, so nothing overflows (#215).
    ///
    /// The result is an `int`, so leading zeros are dropped: "zero zero seven"
    /// is 7 (#195, a deliberate choice -- the return type stays numeric). To
    /// keep them, convert a sentence instead: `words2num_sentence` writes a
    /// digit run as its digit string ("call 007").
    fn to_digits(&self, text: &str) -> Result<W2nValue, W2nError> {
        let toks = py_split_whitespace(text);
        if toks.is_empty() {
            return Err(W2nError::Words2Num("empty digit run".to_string()));
        }
        let english = match self {
            Converter::En => true,
            Converter::Table(lang) => en_variant(lang),
        };
        let not_digit = || W2nError::Words2Num("not a single digit".to_string());
        // One digit word -> its ASCII digit.
        let digit = |word: &str| -> Option<char> {
            if english && word.to_lowercase() == "o" {
                return Some('0');
            }
            match self.to_cardinal(word) {
                // A single decimal digit stringifies to exactly one ASCII digit
                // (0-9); anything else (negatives, 10+, floats) declines.
                Ok(W2nValue::Int(d)) => {
                    let ds = d.to_string();
                    let b = ds.as_bytes();
                    (b.len() == 1 && b[0].is_ascii_digit()).then(|| b[0] as char)
                }
                _ => None,
            }
        };
        let mut s = String::with_capacity(toks.len());
        let mut repeat = 1usize;
        let mut k = 0usize;
        while k < toks.len() {
            let tok = toks[k];
            if english && matches!(tok.to_lowercase().as_str(), "double" | "triple") {
                if repeat > 1 {
                    return Err(not_digit()); // "double triple"
                }
                repeat = if tok.eq_ignore_ascii_case("double") { 2 } else { 3 };
                k += 1;
                continue;
            }
            let mut read: Option<(String, usize)> = None;
            for len in 1..=3.min(toks.len() - k) {
                if let Some(d) = digit(&toks[k..k + len].join(" ")) {
                    read = Some((d.to_string(), len));
                    break;
                }
            }
            if read.is_none() && tok.contains('-') {
                let subs: Vec<&str> = tok.split('-').filter(|t| !t.is_empty()).collect();
                let ds: Option<String> = subs.iter().map(|t| digit(t)).collect();
                read = ds.filter(|d| !d.is_empty()).map(|d| (d, 1));
            }
            let (ds, len) = read.ok_or_else(not_digit)?;
            if repeat > 1 && ds.len() != 1 {
                return Err(not_digit()); // "double one-two"
            }
            for _ in 0..repeat {
                s.push_str(&ds);
            }
            repeat = 1;
            k += len;
        }
        if repeat > 1 {
            return Err(not_digit()); // a trailing "double"
        }
        let n: BigInt = s
            .parse()
            .map_err(|_| W2nError::Words2Num("bad digit run".to_string()))?;
        Ok(W2nValue::Int(n))
    }

    /// Dispatch `getattr(converter, "to_{to}")(token, **kwargs)`.
    ///
    /// `has_kwargs` models the sentence walker passing `**kwargs` through: the
    /// standard converters accept none, so Python raises `TypeError` there,
    /// which the walker swallows as "not a number word". An unknown `to`
    /// raises `AttributeError` in Python (`getattr` miss) — also swallowed.
    /// Both are represented here as an `Err` the caller drops.
    fn convert(&self, to: &str, text: &str, has_kwargs: bool) -> Result<W2nValue, W2nError> {
        if has_kwargs {
            // TypeError: to_*() takes no keyword arguments.
            return Err(W2nError::Words2Num(
                "unexpected keyword argument".to_string(),
            ));
        }
        match to {
            "cardinal" => self.to_cardinal(text),
            "ordinal" => self.to_ordinal(text),
            "year" => self.to_year(text),
            "ordinal_num" => self.to_ordinal_num(text),
            "currency" => self.to_currency(text),
            "digits" => self.to_digits(text),
            // AttributeError: converter has no to_<to>.
            _ => Err(W2nError::NotImplemented(format!(
                "'Words2Num' object has no attribute 'to_{}'",
                to
            ))),
        }
    }
}

/// Wrap the English grammar's `Result` into the sentence-layer types.
fn en_convert(
    r: Result<crate::w2n_lang_en::W2nValue, crate::w2n_lang_en::W2nError>,
) -> Result<W2nValue, W2nError> {
    match r {
        Ok(v) => Ok(en_to_sentence_value(v)),
        Err(e) => Err(W2nError::Words2Num(e.msg)),
    }
}

/// Port of `Words2Num_Base._convert`: reverse-table lookup, then the
/// sign/digit/error tail (`_parse_literal`).
/// Apocopated number words per language, canonical spelling on the right:
/// es/gl "un" / "una" / "veintiún" (num2words renders 1 as "uno", 21 as
/// "veintiuno"; speech drops the vowel before a noun or a scale word).
fn apocope_word(lang: &str, token: &str) -> Option<&'static str> {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    match (base, token) {
        ("es" | "gl", "un" | "una") => Some("uno"),
        ("es", "veintiun" | "veintiún" | "veintiuna") => Some("veintiuno"),
        // fr agrees "un" with a feminine noun: "cinquante et une personnes",
        // "vingt et une heures". num2words only renders the masculine.
        ("fr", "une") => Some("un"),
        // it "un milione", de "eine Million": the article-one in front of a scale word.
        ("it", "un" | "una") => Some("uno"),
        ("de", "ein" | "eine" | "einen" | "einem" | "einer") => Some("eine"),
        _ => None,
    }
}

/// Canonicalize apocopes inside a number: "cincuenta y un" -> "cincuenta y
/// uno", "un mil" -> "mil" (num2words spells 1000 without a unit). A lone
/// "un" / "una" is left alone: on its own it is an article, not a count.
fn canonical_apocopes_norm(lang: &str, norm: &str) -> Option<String> {
    let toks: Vec<&str> = norm.split_whitespace().collect();
    if !toks.iter().any(|t| apocope_word(lang, t).is_some()) {
        return None;
    }
    if toks.len() == 1 && matches!(toks[0], "un" | "una" | "une" | "ein" | "eine" | "einen" | "einem" | "einer") {
        return None;
    }
    let mut out: Vec<&str> = toks.iter().map(|t| apocope_word(lang, t).unwrap_or(t)).collect();
    if out.len() >= 2 && out[0] == "uno" && crate::scale_words(lang).iter().any(|(w, _)| w == out[1]) {
        out.remove(0);
    }
    let canon = out.join(" ");
    if canon == norm {
        return None; // already canonical (de "eine" maps to itself): no second pass
    }
    Some(canon)
}

fn base_convert(lang: &str, text: &str, ordinal: bool) -> Result<W2nValue, W2nError> {
    // Normalized once here; every reading below works on `norm`.
    let norm = crate::normalize(text);
    let supported = crate::is_supported_lang(lang);
    // The sign is stripped once, here, and applied to the whole value every
    // reading composes: "moins un million mille" is -(1001000), not
    // -1 × 10^6 + 1000, and "minus zwölftausend…" reaches the scale composer
    // (#61). A glued prefix (th "ติดลบเจ็ด", tr "eksiyedi", #124) yields to an
    // unsigned reading of the whole token, should one exist.
    if supported {
        if let Some((rest, glued)) = crate::strip_sign(lang, &norm) {
            if glued {
                if let Ok(v) = base_convert_unsigned(lang, &norm, ordinal) {
                    return Ok(v);
                }
            }
            if let Some(v) = base_convert_unsigned(lang, rest, ordinal).ok().and_then(negate) {
                return Ok(v);
            }
        }
    }
    base_convert_unsigned(lang, &norm, ordinal)
}

/// `-v` for an int or float reading. The table path never yields a decimal;
/// should one appear it is declined rather than returned unsigned.
fn negate(v: W2nValue) -> Option<W2nValue> {
    match v {
        W2nValue::Int(i) => Some(W2nValue::Int(-i)),
        W2nValue::Float(f) => Some(W2nValue::Float(-f)),
        W2nValue::Dec(_) => None,
    }
}

/// [`base_convert`] on normalized text with any sign already stripped.
fn base_convert_unsigned(lang: &str, norm: &str, ordinal: bool) -> Result<W2nValue, W2nError> {
    let first = base_convert_as_written(lang, norm, ordinal);
    if first.is_ok() || ordinal || !crate::is_supported_lang(lang) {
        return first;
    }
    // Declined forms and spelling variants the renderer does not write (ru
    // "трёх", ar "عشرين", el "εννιά", ro "șaișpe", cs "pětadvacet", vi "ba
    // mươi tư"): read through the language's aliases, only once every reading
    // as written has failed (#74, #77, #78, #179, #180). The sign is already
    // off ("minus трёх" is -3).
    if let Some(rewritten) = lexicon::alias_rewrite(lang, norm) {
        if let Ok(v) = base_convert_as_written(lang, &rewritten, false) {
            return Ok(v);
        }
    }
    first
}

/// [`base_convert_unsigned`] on the text exactly as written, no aliases.
fn base_convert_as_written(lang: &str, norm: &str, ordinal: bool) -> Result<W2nValue, W2nError> {
    let norm = norm.to_string();
    let supported = crate::is_supported_lang(lang);
    if !ordinal && supported {
        if let Some(canon) = canonical_apocopes_norm(lang, &norm) {
            if let Ok(v) = base_convert(lang, &canon, false) {
                return Ok(v);
            }
        }
    }
    // `_rust_lookup`: guarded on `LANG in _RUST_LANGS`, and any error from the
    // core is swallowed to `None` (`except Exception: return None`).
    if supported {
        if let Ok(Some(v)) = crate::lookup_norm(lang, &norm, ordinal, &[]) {
            return Ok(W2nValue::Int(BigInt::from(v)));
        }
        if ordinal {
            // A compound ordinal the renderer never writes (cs "dvacátý
            // první"): every element read through the ordinal table.
            if let Some(v) = crate::ord_forms::compose(lang, &norm) {
                return Ok(W2nValue::Int(BigInt::from(v)));
            }
            // Above the table: a cardinal head + an ordinal tail, checked
            // against the renderer (#125).
            if let Some(v) = crate::ord_compose::compose(lang, &norm) {
                return Ok(W2nValue::Int(BigInt::from(v)));
            }
            // A compound ordinal spoken in parts where num2words glues it:
            // es "vigésimo cuarto" is "vigesimocuarto", tr "kırk ikinci" is
            // "kırkikinci" (#70). Only a genuine table hit counts.
            if norm.contains(' ') {
                let despaced: String = norm.split_whitespace().collect();
                if let Ok(Some(v)) = crate::lookup_norm(lang, &despaced, true, &[]) {
                    return Ok(W2nValue::Int(BigInt::from(v)));
                }
            }
        }
        // Composition multi-échelle pour les valeurs hors table (> 10001) :
        // « soixante-neuf mille huit » → 69008. Uniquement en cardinal.
        if !ordinal {
            // The bare hundred prefix num2words never renders alone (es
            // "ciento"): 100, so the walker can open a run on it and grow it
            // into the table hit "ciento cincuenta y cuatro".
            if let Some(v) = crate::lookup_hundred_prefix_norm(lang, &norm) {
                return Ok(W2nValue::Int(BigInt::from(v)));
            }
            if let Some(v) = crate::parse_scaled_norm(lang, &norm) {
                return Ok(W2nValue::Int(BigInt::from(v)));
            }
            // Compound spoken as separate tokens where num2words renders it
            // glued: "mille novecento ottantotto" → the canonical
            // "millenovecentottantotto". Only when de-spacing actually changes
            // the string (multi-token input), and only if that glued form is a
            // genuine table hit — so it never invents a value.
            if norm.contains(' ') {
                let despaced: String = norm.split_whitespace().collect();
                if let Ok(Some(v)) =
                    crate::lookup_norm(lang, &despaced, false, &[])
                {
                    return Ok(W2nValue::Int(BigInt::from(v)));
                }
                // Beyond the table: tr writes "üç yüz bin" spaced where
                // num2words2 glues "üçyüzbin" (300000). Kept only when the
                // glued form is exactly the renderer's spelling of the value
                // (#75).
                if let Some(v) = crate::parse_scaled_norm(lang, &despaced) {
                    if lexicon::renders_as(lang, &BigInt::from(v), &despaced) {
                        return Ok(W2nValue::Int(BigInt::from(v)));
                    }
                }
            }
            // Spoken "year" readings (two 2-digit groups, or an explicit/glued
            // hundred) that are not num2words' canonical spelling.
            if let Some(v) = crate::parse_year_norm(lang, &norm) {
                return Ok(W2nValue::Int(BigInt::from(v)));
            }
            // Scale words in an oblique, dual or glued form (kn "ಸಾವಿರದ", ar
            // "مليونان", da "tolvtusinde"), respelled for the scale split.
            if let Some(respelled) = crate::w2n_forms::scale_spelling(lang, &norm) {
                if let Ok(v) = base_convert_unsigned(lang, &respelled, false) {
                    return Ok(v);
                }
            }
        }
    }
    parse_literal_norm(&norm)
}

/// `to="year"` for a reverse-table language. Python's `Words2Num_Base.to_year`
/// is `to_cardinal`, but many renderers spell a year apart from the cardinal
/// (a year word, digit by digit, an era: see `lib.rs`, "Year mode"). In
/// order: the renderer's own year spelling; the cardinal; the cardinal with
/// the year word dropped (ja "千九百五年"), or with ta's combining thousand;
/// a digit-by-digit year (zh "一九〇五年"). The cardinal's error is kept
/// when nothing reads.
fn table_year(lang: &str, text: &str) -> Result<W2nValue, W2nError> {
    let norm = crate::normalize(text);
    if let Some(v) = crate::lookup_year_norm(lang, &norm) {
        return Ok(W2nValue::Int(BigInt::from(v)));
    }
    let err = match base_convert(lang, text, false) {
        Ok(v) => return Ok(v),
        Err(e) => e,
    };
    let variants = crate::year_variants_norm(lang, &norm);
    for v in std::iter::once(&norm).chain(variants.iter()) {
        if let Some(y) = crate::year_digit_string_norm(lang, v) {
            return Ok(W2nValue::Int(BigInt::from(y)));
        }
        if *v == norm {
            continue;
        }
        if let Some(y) = crate::lookup_year_norm(lang, v) {
            return Ok(W2nValue::Int(BigInt::from(y)));
        }
        // A negative cardinal with the year word stripped is not trusted:
        // as/ba render a BC year as "<year word> <minus> <number>", and the
        // table path's sign handling above the table is #61's to fix.
        match base_convert(lang, v, false) {
            Ok(W2nValue::Int(y)) if y.sign() == num_bigint::Sign::Minus => {}
            Ok(y) => return Ok(y),
            Err(_) => {}
        }
    }
    Err(err)
}

/// Port of `Words2Num_Base._parse_literal` — a bare digit string, a leading
/// sign word, or genuinely unparseable input.
///
/// `errmsg_unparseable` is `"cannot parse %r as a number"`.
fn parse_literal_norm(normalized: &str) -> Result<W2nValue, W2nError> {
    let unparseable = |s: &str| W2nError::Words2Num(format!("cannot parse {} as a number", py_repr_str(s)));
    if normalized.is_empty() {
        return Err(unparseable(normalized));
    }
    for neg in BASE_NEGATIVE_WORDS {
        if let Some(rest) = normalized
            .strip_prefix(neg)
            .and_then(|r| r.strip_prefix(' '))
        {
            return finish_parse_literal(rest, -1);
        }
        if normalized == neg {
            return Err(unparseable(normalized));
        }
    }
    finish_parse_literal(normalized, 1)
}

/// The `try: … except ValueError: pass; raise` tail of `_parse_literal`.
fn finish_parse_literal(normalized: &str, sign: i64) -> Result<W2nValue, W2nError> {
    if normalized.contains('.') {
        if let Some(f) = py_float(normalized) {
            return Ok(W2nValue::Float(if sign < 0 { -f } else { f }));
        }
    } else if let Some(i) = py_int(normalized) {
        return Ok(W2nValue::Int(if sign < 0 { -i } else { i }));
    }
    Err(W2nError::Words2Num(format!(
        "cannot parse {} as a number",
        py_repr_str(normalized)
    )))
}

/// Port of `Words2Num_Base.to_ordinal_num`, for the language `lang` (a
/// num2words2 key).
///
/// ```python
/// m = re.search(r"-?\d+", text)
/// if not m: raise Words2NumError("cannot parse %r as a number" % text)
/// return int(m.group())
/// ```
///
/// The Python regex is too loose for the forms the renderers write:
/// - a hyphen after a letter is a prefix, not a sign: id "ke-21", fil
///   "ika-21", zu "we-100" are 21 / 21 / 100 (#63);
/// - a thousands-grouped figure is one number: en "1,000th", de "1.000.",
///   fr "1 000e" are all 1000; a decimal ("2.5th") has no rank and raises (#64);
/// - figures glued to a Latin word ("covid19") are not a rank, unless the
///   language writes its ordinal that way (xh "we21") (#64).
fn base_ordinal_num(text: &str, lang: &str) -> Result<W2nValue, W2nError> {
    let fail = || W2nError::Words2Num(unparseable_msg(text));
    let chars: Vec<char> = text.chars().collect();
    let Some(start) = chars.iter().position(|&c| is_unicode_digit(c)) else {
        return Err(fail());
    };
    // `-?`: a '-' is the sign only where a sign can stand — at the start or
    // after a space / punctuation, never glued to a word.
    let negative = start > 0
        && chars[start - 1] == '-'
        && (start == 1 || !chars[start - 2].is_alphanumeric());
    let before = if negative { start.checked_sub(2) } else { start.checked_sub(1) };
    if before.is_some_and(|b| chars[b].is_ascii_alphabetic()) && !glues_latin_prefix(lang) {
        return Err(fail());
    }
    let mut j = start;
    while j < chars.len() && is_unicode_digit(chars[j]) {
        j += 1;
    }
    let mut digits: String = chars[start..j].iter().collect();
    // Thousands groups: a separator followed by exactly three digits, only
    // after a leading group of one to three digits.
    if j - start <= 3 {
        while j + 4 <= chars.len() {
            let is_sep = matches!(
                chars[j],
                ',' | '.' | ' ' | '\'' | '\u{00A0}' | '\u{202F}' | '\u{2009}'
            );
            let group = chars[j + 1..j + 4].iter().all(|&c| is_unicode_digit(c));
            let closed = chars.get(j + 4).is_none_or(|&c| !is_unicode_digit(c));
            if !(is_sep && group && closed) {
                break;
            }
            digits.extend(&chars[j + 1..j + 4]);
            j += 4;
        }
    }
    // A decimal point or comma followed by a digit is a fraction: no rank.
    if matches!(chars.get(j), Some(',' | '.')) && chars.get(j + 1).copied().is_some_and(is_unicode_digit) {
        return Err(fail());
    }
    let value = py_int(&digits).ok_or_else(fail)?;
    Ok(W2nValue::Int(if negative { -value } else { value }))
}

/// Does the renderer write this language's ordinal figures glued after a
/// Latin-letter prefix (xh "we21")? Then a letter before the figures is the
/// ordinal marker, not part of a word like "covid19".
fn glues_latin_prefix(lang: &str) -> bool {
    num2words2_core::get_lang_by_key(lang)
        .and_then(|l| l.to_ordinal_num(&BigInt::from(21)).ok())
        .and_then(|s| s.find("21").map(|at| s[..at].chars().last()))
        .flatten()
        .is_some_and(|c| c.is_ascii_alphabetic())
}

/// A Roman numeral (la renders its `ordinal_num` as "XXI", "MMXXIV"), read
/// only in its canonical spelling: the value must re-render to the same
/// letters, so "IIII" or "VX" raise.
fn roman_value(text: &str) -> Option<i64> {
    let t = text.trim().trim_end_matches('.').to_ascii_uppercase();
    if t.is_empty() || t.len() > 15 || !t.chars().all(|c| "IVXLCDM".contains(c)) {
        return None;
    }
    let val = |c: char| match c {
        'I' => 1,
        'V' => 5,
        'X' => 10,
        'L' => 50,
        'C' => 100,
        'D' => 500,
        _ => 1000,
    };
    let cs: Vec<i64> = t.chars().map(val).collect();
    let mut total = 0i64;
    for (k, &v) in cs.iter().enumerate() {
        if cs.get(k + 1).is_some_and(|&n| n > v) {
            total -= v;
        } else {
            total += v;
        }
    }
    (1..4000).contains(&total).then_some(total).filter(|&n| to_roman(n) == t)
}

fn to_roman(mut n: i64) -> String {
    const R: [(i64, &str); 13] = [
        (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"),
        (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
    ];
    let mut out = String::new();
    for (v, s) in R {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    out
}

/// How the renderer writes `to_ordinal_num` for this language, probed on 21:
/// in figures (the common case), in Roman numerals (la "XXI"), or in words
/// (ar "الأول والعشرون", bn "একুশতম").
#[derive(Clone, Copy, PartialEq, Eq)]
enum OrdinalNumShape {
    Figures,
    Roman,
    Words,
}

fn ordinal_num_shape(lang: &str) -> OrdinalNumShape {
    let rendered = num2words2_core::get_lang_by_key(lang)
        .and_then(|l| l.to_ordinal_num(&BigInt::from(21)).ok());
    match rendered {
        Some(s) if s == "XXI" => OrdinalNumShape::Roman,
        Some(s) if !s.chars().any(is_unicode_digit) => OrdinalNumShape::Words,
        _ => OrdinalNumShape::Figures,
    }
}

fn unparseable_msg(text: &str) -> String {
    format!("cannot parse {} as a number", py_repr_str(text))
}

/// Python's `int(str)` over a `_normalize`-shaped string (optional sign, then
/// Unicode `Nd` digits). Returns `None` where Python raises `ValueError`.
fn py_int(s: &str) -> Option<BigInt> {
    let t = s.trim_matches(py_is_space);
    let (neg, body) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    if body.is_empty() || !body.chars().all(is_unicode_digit) {
        return None;
    }
    let ascii = to_ascii_digits(body);
    let v = BigInt::from_str(&ascii).ok()?;
    Some(if neg { -v } else { v })
}

/// Python's `float(str)` over a `_normalize`-shaped decimal string.
fn py_float(s: &str) -> Option<f64> {
    let t = s.trim_matches(py_is_space);
    let ascii = to_ascii_digits(t);
    ascii.parse::<f64>().ok()
}

/// `from .. import words2num; words2num(text, lang=lang)` — resolve the locale
/// and run its `to_cardinal`.
fn call_words2num(text: &str, lang: &str) -> Result<W2nValue, W2nError> {
    let resolved = resolve_lang(lang)?;
    converter_for(&resolved).to_cardinal(text)
}

/// Port of the public `words2num2.words2num(text, lang, to)` — the single-token
/// entry point, dispatch and all.
///
/// ```python
/// def words2num(text, lang="en", to="cardinal", **kwargs):
///     resolved = _resolve_lang(lang)
///     converter = CONVERTER_CLASSES[resolved]
///     if to not in CONVERTER_TYPES:
///         raise NotImplementedError("conversion type %r unsupported" % to)
///     return getattr(converter, "to_{}".format(to))(text, **kwargs)
/// ```
///
/// Returns the English grammar's [`crate::w2n_lang_en::W2nValue`] rather than
/// this module's [`W2nValue`], so the `en` decimal path keeps its `PyDec`
/// backing: a signed-zero decimal (`Decimal('-0.0')`) and the exact
/// 28-significant-digit `str()` both survive, neither of which `BigDecimal` can
/// carry. The 119 reverse-table locales only ever produce an `int` or `float`,
/// so mapping their result back through [`sentence_to_en_value`] is lossless.
pub fn words2num(
    text: &str,
    lang: &str,
    to: &str,
) -> Result<crate::w2n_lang_en::W2nValue, W2nError> {
    let resolved = resolve_lang(lang)?;
    let converter = converter_for(&resolved);

    check_converter_type(to)?;

    // `getattr(converter, "to_{to}")(text)`. The English grammar path returns
    // its native value directly; every other path is `int`/`float` and is
    // promoted to the English value type.
    // An amount spoken in words: "twelve euros, thirty-four cents" -> 12.34.
    // Text that names no currency is read as a cardinal below.
    if to == "currency" {
        if let Some(amount) = currency_amount(&converter, &resolved, text) {
            return amount;
        }
    }

    match &converter {
        Converter::En => match to {
            // Without a currency word, to="currency" is the cardinal.
            "cardinal" | "currency" => {
                crate::en_to_cardinal(text).map_err(|e| W2nError::Words2Num(e.msg))
            }
            "ordinal" => crate::en_to_ordinal(text).map_err(|e| W2nError::Words2Num(e.msg)),
            "year" => crate::en_to_year(text).map_err(|e| W2nError::Words2Num(e.msg)),
            "digits" => converter.to_digits(text).map(sentence_to_en_value),
            // `Words2Num_EN` inherits `Words2Num_Base.to_ordinal_num`.
            _ => base_ordinal_num(text, "en").map(sentence_to_en_value),
        },
        Converter::Table(lang) => {
            // Regional English (#193): the English grammar answers what the
            // table declines, keeping its own value type (a `Decimal` for
            // "three point one four").
            let grammar = |e: W2nError| -> Result<crate::w2n_lang_en::W2nValue, W2nError> {
                if !en_variant(lang) {
                    return Err(e);
                }
                match to {
                    "cardinal" | "currency" => crate::en_to_cardinal(text),
                    "ordinal" => crate::en_to_ordinal(text),
                    "year" => crate::en_to_year(text),
                    _ => return Err(e),
                }
                .map_err(|_| e)
            };
            let table = match to {
                // A decimal spoken in words, read as the sentence walker
                // reads it: fr "trois virgule un quatre" -> 3.14 (#191).
                "cardinal" => base_convert(lang, text, false).or_else(|e| {
                    table_decimal(&converter, &resolved, text)
                        .and_then(|(int, _, frac)| BigDecimal::from_str(&format!("{}.{}", int, frac)).ok())
                        .map(W2nValue::Dec)
                        .ok_or(e)
                }),
                // A currency amount was read above; what is left is the cardinal.
                "currency" => base_convert(lang, text, false),
                "year" => table_year(lang, text),
                "ordinal" => base_convert(lang, text, true),
                "digits" => converter.to_digits(text),
                _ => converter.to_ordinal_num(text),
            };
            match table {
                Ok(v) => Ok(sentence_to_en_value(v)),
                Err(e) => grammar(e),
            }
        }
    }
}

/// `to="currency"`: the amount as a `Decimal`, unit plus subunit (`12.34`), via
/// [`crate::w2n_currency::parse_amount`]. `None` when the text names no
/// currency, so the caller keeps the plain cardinal reading.
fn currency_amount(
    converter: &Converter,
    resolved: &str,
    text: &str,
) -> Option<Result<crate::w2n_lang_en::W2nValue, W2nError>> {
    let cardinal = |words: &str| -> Option<BigDecimal> {
        match converter.to_cardinal(words).ok()? {
            W2nValue::Int(i) => Some(BigDecimal::from(i)),
            W2nValue::Dec(d) => Some(d),
            W2nValue::Float(f) => BigDecimal::from_str(&py_float_repr(f)).ok(),
        }
    };
    let amount = crate::w2n_currency::parse_amount(text, resolved, &cardinal)?;
    Some(
        amount
            .map(|d| crate::w2n_lang_en::W2nValue::Dec(crate::w2n_lang_en::PyDec::from_bigdecimal(&d)))
            .map_err(W2nError::Words2Num),
    )
}

/// Promote a reverse-table / `ordinal_num` result into the English grammar's
/// value type. Those paths never yield a `Decimal`, so the `Dec` arm is
/// unreachable in practice; it is mapped losslessly (rather than panicked on —
/// the crate builds `panic = "abort"`) via [`crate::w2n_lang_en::PyDec`].
fn sentence_to_en_value(v: W2nValue) -> crate::w2n_lang_en::W2nValue {
    use crate::w2n_lang_en::W2nValue as EnV;
    match v {
        W2nValue::Int(i) => EnV::Int(i),
        W2nValue::Float(f) => EnV::Float(f),
        W2nValue::Dec(d) => EnV::Dec(crate::w2n_lang_en::PyDec::from_bigdecimal(&d)),
    }
}

// ===========================================================================
// `words2num_sentence` / `convert_sentence` / `sentence_to_words`
// ===========================================================================

/// A token that already holds a digit ("850", "850-820-9095", "$426", "94504")
/// is a literal: it never starts or extends a run. Without this, adjacent
/// numerals were summed ("calling from 850 820 9095" came out as "10765").
fn holds_digit(token: &str) -> bool {
    token.chars().any(|c| c.is_ascii_digit())
}

/// `SentenceConverter._starts_run` — a run must open with a real number word
/// or with a decimal separator, never with `"and"` / `"minus"` / `"a"`.
///
/// **Why the decimal separator is listed explicitly.** `is_number_word` is
/// `to_cardinal(token).is_ok()`, so before
/// revdotcom/words2num#5 was ported, `"point"` qualified as a run head only
/// because `to_cardinal("point")` silently returned `Decimal('0')` — the very
/// bug that port fixes. Once a dangling separator raises, the accident that
/// made `"point five"` parse in a sentence disappears with it.
///
/// `"point five"` is a real spoken number and `words2num("point five")`
/// returns `0.5`, so the walker has to reach it too. Naming the separators
/// here makes that a property of the walker rather than a side effect of a
/// bug, and the guarantee is pinned by the `"point is a valid head"` case in
/// [`tests::walker_matches_python`].
///
/// Only separators are promoted. The rest of [`INCLUDABLE_EN`] stays
/// run-internal: a run still may not open with `"and"`, `"minus"`,
/// `"negative"`, `"a"` or `"an"`.
fn starts_run(converter: &Converter, token: &str, decimal_heads: &[&str]) -> bool {
    if token.is_empty() || holds_digit(token) {
        return false;
    }
    if decimal_heads.contains(&token) {
        return true;
    }
    let dehyphened = token.replace('-', " ");
    py_split_whitespace(&dehyphened)
        .iter()
        .any(|sub| converter.is_number_word(sub))
}

/// The decimal separator words that may open a run, by resolved language.
///
/// English only: the non-English locales go through the reverse-table
/// converter, whose separator handling this change does not touch.
fn decimal_heads_for(resolved: &str) -> &'static [&'static str] {
    if resolved == "en" || en_variant(resolved) {
        &["point", "dot"]
    } else {
        &[]
    }
}

/// `SentenceConverter._is_candidate` — cheap pre-filter for run growth.
fn is_candidate(converter: &Converter, token: &str, includable: &[&str]) -> bool {
    if token.is_empty() || holds_digit(token) {
        return false;
    }
    if is_includable(token, includable) || converter.apocope(token).is_some() {
        return true;
    }
    // vi "tư" is 4 only after "mươi", ar "وعشرين" only after a unit: not
    // number words on their own, but they may complete a run (#74, #77).
    if let Converter::Table(lang) = converter {
        if lexicon::is_alias_word(lang, &crate::normalize(token)) {
            return true;
        }
    }
    let dehyphened = token.replace('-', " ");
    py_split_whitespace(&dehyphened)
        .iter()
        .any(|sub| converter.is_number_word(sub) || converter.is_number_part(sub))
}

/// May a run END on this token? A connector ("and", "a", "point", es "y",
/// fr "et") is includable inside a run but is not a number on its own, so a
/// run that stops on one leaves it out — the walker then emits the connector
/// as a word instead of swallowing it.
fn candidate_is_number(converter: &Converter, token: &str, includable: &[&str]) -> bool {
    if !is_includable(token, includable) {
        return true;
    }
    // en "point" parses as Decimal(0) on its own; still a connector here.
    let dehyphened = token.replace('-', " ");
    py_split_whitespace(&dehyphened)
        .iter()
        .any(|sub| converter.is_number_word(sub) && !includable.contains(sub))
}

/// Is `token` one of the run-internal words? The derived connectors are
/// normalised (ro "si", vi "le"), the hand list is not (ro "și").
fn is_includable(token: &str, includable: &[&str]) -> bool {
    includable.contains(&token) || {
        let norm = crate::normalize(token);
        includable.contains(&norm.as_str())
    }
}

/// A run that crosses a connector only the renderer lists ([`lexicon::
/// derived_connectors`]) must read as one number the way the renderer joins
/// them: "fjörutíu og tveir" is 40 + 2, "douăzeci de mii" 20 × 1000, and
/// num2words2 spells either. "fem og seks" (five and six) and sw "mbili na
/// tatu" (two and three) are two numbers, and stay two (#65). The hand-listed
/// connectors keep their behaviour.
fn connector_join_ok(converter: &Converter, stripped: &str, total: &W2nValue, hand: &[&str]) -> bool {
    let Converter::Table(lang) = converter else { return true };
    let derived = lexicon::derived_connectors(lang);
    let norm = crate::normalize(stripped).replace('-', " ");
    let words: Vec<&str> = norm.split_whitespace().collect();
    let Some(p) = words
        .iter()
        .rposition(|w| derived.iter().any(|d| d == w) && !hand.contains(w))
    else {
        return true;
    };
    if p == 0 || p + 1 == words.len() {
        return true;
    }
    let W2nValue::Int(t) = total else { return true };
    if lexicon::renders_as(lang, t, &norm) {
        return true;
    }
    let int = |ws: &[&str]| match converter.to_cardinal(&ws.join(" ")) {
        Ok(W2nValue::Int(v)) => Some(v),
        _ => None,
    };
    let (Some(l), Some(r)) = (int(&words[..p]), int(&words[p + 1..])) else {
        return false;
    };
    let zero = BigInt::from(0);
    (*t == &l + &r && r < l && &l % BigInt::from(10) == zero) || *t == &l * &r
}

/// The scale a token closes with a comma: "thousand," -> 1000, "million," ->
/// 1000000, pt "mil," -> 1000. `None` for any other token or punctuation.
fn comma_after_scale(converter: &Converter, tok: &str) -> Option<BigInt> {
    if trailing_punct(tok) != "," {
        return None;
    }
    let clean = rstrip_punct(tok).to_lowercase().replace('-', " ");
    let last = *py_split_whitespace(&clean).last()?;
    if !is_scale_word(converter, last, 1_000) {
        return None;
    }
    match converter.to_cardinal(last).ok()? {
        W2nValue::Int(v) => Some(v),
        _ => None,
    }
}

/// After each comma the run crossed, the words up to the next comma (or the
/// run end `j`) must be one group of the number: at least a hundred and below
/// the scale before the comma. "twelve thousand, three hundred" is 12300 the
/// way num2words2 writes it; "in two thousand, five people came" keeps 2000
/// and 5 apart (#67).
fn comma_groups_ok(converter: &Converter, parts: &[String], j: usize, commas: &[(usize, BigInt)]) -> bool {
    for (k, (at, scale)) in commas.iter().enumerate() {
        let end = commas.get(k + 1).map_or(j, |(next, _)| *next);
        if end <= *at {
            continue; // the run ends on this comma: nothing after it yet
        }
        let group = rstrip_punct(py_strip(&parts[at + 1..=end].concat())).to_string();
        match converter.to_cardinal(&group) {
            Ok(W2nValue::Int(v)) if v >= BigInt::from(100) && v < *scale => {}
            _ => return false,
        }
    }
    true
}

/// One word of an English number run: the word itself (lowercased, hyphens
/// split), the index of the sentence part it came from, and whether it is the
/// last word of that part — a reading may only stop on a part boundary, never
/// halfway through "ninety-nine".
struct EnWord {
    word: String,
    part: usize,
    last: bool,
}

/// The consecutive English number words starting at part `i`: units, teens,
/// tens and "oh"/"o", each part split on hyphens. Stops at the first part
/// that is not wholly made of such words, and after one that ends in
/// terminal punctuation — the same run boundaries the cardinal walk uses.
fn en_number_words(parts: &[String], i: usize) -> Vec<EnWord> {
    use crate::w2n_lang_en::{tens_value, unit_value};
    let mut words = Vec::new();
    let mut j = i;
    // Called at every run start: without a bound, a text made of number words
    // is rescanned to its end from each position (#186).
    while j < parts.len() && words.len() < MAX_DIGIT_RUN_WORDS {
        let tok = &parts[j];
        if py_str_isspace(tok) {
            j += 1;
            continue;
        }
        let clean = rstrip_punct(tok).to_lowercase().replace('-', " ");
        let subs = py_split_whitespace(&clean);
        if subs.is_empty()
            || !subs.iter().all(|w| {
                matches!(*w, "o" | "double" | "triple")
                    || unit_value(w).is_some()
                    || tens_value(w).is_some()
            })
        {
            break;
        }
        let n = subs.len();
        for (k, w) in subs.iter().enumerate() {
            words.push(EnWord {
                word: w.to_string(),
                part: j,
                last: k + 1 == n,
            });
        }
        if ends_with_terminal_punct(tok) {
            break;
        }
        j += 1;
    }
    words
}

/// A digit string read one digit at a time — an account number, ZIP code or
/// phone number: "four five six seven" -> "4567", "zero six one two" ->
/// "0612". Consecutive single-digit words (zero, one..nine), no tens or scale
/// word among them; "double"/"triple" repeat the digit after them ("double
/// zero seven" -> "007"). A lone digit word ("press one") is left to the
/// cardinal grammar, as is "twenty five". The digits are concatenated, never
/// summed, and a leading zero is kept — which is why this returns the string
/// rather than a value. Also returns the last part index covered.
///
/// Three digits at least, or two with a leading zero ("zero six", "oh
/// seven"): two digit words are how a speaker gives a range, "two three
/// minutes" is 2 or 3 minutes, never 23 (#51). "oh"/"o" is 0 only next to
/// another digit word ("room oh seven" -> "room 07"); on its own it is the
/// interjection (#79), which the walker leaves alone.
fn en_digit_run(parts: &[String], i: usize) -> Option<(String, usize)> {
    use crate::w2n_lang_en::unit_value;
    let mut digits = String::new();
    let mut end: Option<(usize, usize)> = None;
    let mut repeat = 1usize;
    for (k, w) in en_number_words(parts, i).iter().enumerate() {
        let d = match w.word.as_str() {
            "oh" | "o" => 0,
            "nought" | "naught" if k == 0 => break,
            "double" | "triple" => {
                if repeat > 1 {
                    break;
                }
                repeat = if w.word == "double" { 2 } else { 3 };
                continue;
            }
            other => match unit_value(other) {
                Some(v) if v <= 9 => v,
                _ => break,
            },
        };
        for _ in 0..repeat {
            digits.push(char::from(b'0' + d as u8));
        }
        repeat = 1;
        if w.last {
            end = Some((digits.len(), w.part));
        }
    }
    let (len, part) = end?;
    digits.truncate(len);
    if !is_digit_string(&digits) {
        return None;
    }
    Some((digits, part))
}

/// Is a string of concatenated digit words a dictated digit string? Three
/// digits or more, or two with a leading zero ("06"). Two digit words without
/// one are a range ("deux trois minutes", "two three minutes"): 2 or 3, not
/// 23 (#51).
fn is_digit_string(digits: &str) -> bool {
    digits.len() >= 3 || (digits.len() == 2 && digits.starts_with('0'))
}

/// A run never grows past this many words. No real cardinal comes close
/// ("one million, two hundred and thirty-four thousand, five hundred and
/// sixty-seven" is 13); without the bound, a text made of number words that do
/// not combine ("zweitausend zweitausend ...") re-parsed an ever-longer run from
/// every start, cubic in its length (#186).
const MAX_RUN_WORDS: usize = 40;

/// A run stops growing after this many consecutive words that did not make
/// the run parse (#186). A connector or a "cents"-type word needs one or two;
/// where the multiplier follows the scale word (ha "miliyan dubu ɗari biyu
/// talatin da huɗu ...") the partial runs fail for up to six words before the
/// group closes.
const MAX_FAILED_EXTENSIONS: usize = 8;

/// The longest digit dictation read at once (an IBAN is 34 characters). A
/// longer one is cut into consecutive strings rather than rescanned from
/// every position (#186).
const MAX_DIGIT_RUN_WORDS: usize = 64;

/// The digit-string reading for a reverse-table language: fr "sept cinq
/// zéro un zéro" -> "75010", es "seis uno nueve ocho cero" -> "61980", fr
/// "zéro six" -> "06". Same rules as [`en_digit_run`]: two or more
/// consecutive single-digit words, each a whole part, the run closing on
/// terminal punctuation. A run that opens on the article-number ("un",
/// "uno", "um") needs three digits — "un deux" is two words, "un deux
/// trois" a dictation.
fn table_digit_run(
    converter: &Converter,
    resolved: &str,
    parts: &[String],
    i: usize,
) -> Option<(String, usize)> {
    let mut digits = String::new();
    let mut end: Option<(usize, usize)> = None;
    let mut j = i;
    let mut article_head = false;
    while j < parts.len() && digits.len() < MAX_DIGIT_RUN_WORDS {
        let tok = &parts[j];
        if py_str_isspace(tok) {
            j += 1;
            continue;
        }
        let clean = rstrip_punct(tok).to_lowercase();
        // "cinq 5": a token in figures is a literal, not a digit word (#68).
        // ko native numerals count, they are never dictated (#178).
        if clean.contains('-')
            || holds_digit(&clean)
            || (resolved == "ko" && lexicon::ko_native(&clean).is_some())
        {
            break;
        }
        let article = is_article_word(resolved, &clean);
        // ar "ثلاثة وخمسة" is "three and five", not the digits 3 5.
        if !article && converter.is_bound(&clean) {
            break;
        }
        let d = match converter.to_cardinal(&clean) {
            Ok(W2nValue::Int(v)) if v >= BigInt::from(0) && v <= BigInt::from(9) => v.to_string(),
            // es "un", fr "une": not in the table on their own, but 1 here.
            _ if article => "1".to_string(),
            _ => break,
        };
        if digits.is_empty() {
            article_head = article;
        }
        digits.push_str(&d);
        end = Some((digits.len(), j));
        if ends_with_terminal_punct(tok) {
            break;
        }
        j += 1;
    }
    let (len, part) = end?;
    digits.truncate(len);
    if !is_digit_string(&digits) || (article_head && len < 3) {
        return None;
    }
    Some((digits, part))
}

/// Is `run` exactly two words, each a single digit (0-9) on its own, without a
/// leading zero? Then it is a range ("deux trois minutes"), and the run must
/// not read it as one number (#51).
fn two_digit_words(converter: &Converter, run: &[String]) -> bool {
    let words: Vec<String> = run
        .iter()
        .filter(|t| !py_str_isspace(t))
        .map(|t| rstrip_punct(t).to_lowercase())
        .collect();
    if words.len() != 2 {
        return false;
    }
    let digit = |w: &str| match converter.to_cardinal(w) {
        Ok(W2nValue::Int(v)) if v >= BigInt::from(0) && v <= BigInt::from(9) => v.to_string().parse::<u8>().ok(),
        _ => None,
    };
    matches!((digit(&words[0]), digit(&words[1])), (Some(a), Some(_)) if a != 0)
}

/// A spoken year that is not one English cardinal: "nineteen" or "twenty"
/// followed by "oh" + digit ("twenty oh five" = 2005), a teen ("twenty
/// nineteen" = 2019), or a tens word with an optional unit ("nineteen ninety
/// nine" = 1999, "twenty twenty" = 2020). This is the pair reading of
/// `to="year"`, applied by the cardinal walk only when it reaches further
/// than the cardinal did — so "twenty five" stays 25, and "two thousand
/// twenty five" never comes here. Returns the year and the last part index.
fn en_year_run(parts: &[String], i: usize) -> Option<(i64, usize)> {
    use crate::w2n_lang_en::{tens_value, unit_value};
    let words = en_number_words(parts, i);
    let word = |k: usize| words.get(k).map(|w| w.word.as_str());
    let century = match word(0)? {
        "nineteen" => 1900,
        "twenty" => 2000,
        _ => return None,
    };
    let digit = |k: usize| unit_value(word(k)?).filter(|v| (1..=9).contains(v));
    // Candidate readings as (word count, value); the longest one that ends
    // on a part boundary wins.
    let mut readings: Vec<(usize, i64)> = Vec::new();
    let second = word(1)?;
    if second == "oh" || second == "o" {
        if let Some(u) = digit(2) {
            readings.push((3, century + u));
        }
    } else if let Some(t) = unit_value(second).filter(|v| (10..=19).contains(v)) {
        readings.push((2, century + t));
    } else if let Some(t) = tens_value(second) {
        readings.push((2, century + t));
        if let Some(u) = digit(2) {
            readings.push((3, century + t + u));
        }
    }
    readings
        .into_iter()
        .rev()
        .find(|&(n, _)| words[n - 1].last)
        .map(|(n, year)| (year, words[n - 1].part))
}

/// `SentenceConverter.INCLUDABLE` — tokens allowed *inside* a run though they
/// are not numbers. Keyed by the **resolved** code, so only exactly `"en"`
/// gets them; `"en_IN"` and the other English locales get the connector
/// num2words2 writes ("and") through [`lexicon::derived_connectors`].
const INCLUDABLE_EN: [&str; 7] = ["and", "point", "dot", "minus", "negative", "a", "an"];

/// Connector words allowed INSIDE a number run for non-English locales — the
/// analogue of English "and". Without them the walker breaks a run at the
/// connector ("sesenta **y** nueve mil ocho" stopped at "sesenta"), so
/// composed cardinals above the reverse table never reached the parser. Keyed
/// by resolved language prefix. This hand list holds the decimal words and
/// the eleven languages it always covered; the walker adds every connector
/// the renderer writes ([`lexicon::derived_connectors`], #65).
///
/// The words of a spoken decimal are added from the renderer
/// ([`crate::w2n_decimal::run_words`]): sv "komma", ko "점", hi "दशमलव", ru
/// "целых" — so the run grows across the separator (#107, #108).
fn includable_for(resolved: &str) -> &'static [&'static str] {
    static CACHE: std::sync::OnceLock<std::sync::RwLock<HashMap<String, &'static [&'static str]>>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(l) = cache.read().ok().and_then(|m| m.get(resolved).copied()) {
        return l;
    }
    let mut words: Vec<&'static str> = hand_includable(resolved).to_vec();
    for w in crate::w2n_decimal::run_words(converter_for(resolved).n2w_key()) {
        if !words.contains(&w) {
            words.push(w);
        }
    }
    let built: &'static [&'static str] = Box::leak(words.into_boxed_slice());
    if let Ok(mut m) = cache.write() {
        m.entry(resolved.to_string()).or_insert(built);
    }
    built
}

fn hand_includable(resolved: &str) -> &'static [&'static str] {
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    match base {
        "es" | "gl" => &["y", "e", "coma", "punto"],
        "pt" => &["e", "vírgula", "virgula", "ponto"],
        "it" => &["e", "virgola", "punto"],
        "fr" => &["et", "virgule"],
        "ca" => &["i", "coma"],
        "de" => &["und", "komma"],
        "nl" | "af" => &["en", "komma"],
        // ro "două mii de lei": "de" stays with the noun, never ends the number.
        "ro" => &["si", "și", "de", "virgulă", "virgula"],
        "pl" => &["i", "przecinek"],
        // fa "و", da/nb "og", ha "da": a run may hold the connector but not end on it.
        _ => crate::w2n_forms::connectors(resolved),
    }
}

/// The spoken decimal separator of a language and the character it stands
/// for: fr "virgule" → ",", es "punto" → ".", "coma" → ",". English "point"
/// belongs to its own grammar. Every other language's separator is derived
/// from the renderer ([`crate::w2n_decimal`]) and writes the locale's decimal
/// character; this table only says which character a word stands for where
/// a language has two (es "punto" and "coma").
fn decimal_word(resolved: &str, word: &str) -> Option<char> {
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    match (base, word) {
        ("fr", "virgule")
        | ("es" | "gl" | "ca", "coma")
        | ("pt", "virgula")
        | ("it", "virgola")
        | ("de" | "nl" | "af", "komma")
        | ("ro", "virgula")
        | ("pl", "przecinek") => Some(','),
        ("es" | "gl" | "pt" | "it", "punto" | "ponto") => Some('.'),
        _ => None,
    }
}

/// A decimal spoken in a reverse-table language: "trois virgule cinq" →
/// ("3", ',', "5"), "dos coma cero cinco" → ("2", ',', "05"), "trois virgule
/// cent vingt-cinq" → ("3", ',', "125"), ru "три целых четырнадцать сотых" →
/// ("3", ',', "14"). The shapes are derived from the renderer, see
/// [`crate::w2n_decimal::read`]; a derived separator writes the locale's
/// decimal character. `None` when it is not one.
fn table_decimal(
    converter: &Converter,
    resolved: &str,
    text: &str,
) -> Option<(String, char, String)> {
    let default_sep = crate::w2n_formats::locale_format(resolved)
        .and_then(|f| f.decimal.chars().next())
        .unwrap_or('.');
    let cardinal = |words: &str| match converter.to_cardinal(words).ok()? {
        W2nValue::Int(i) => Some(i),
        _ => None,
    };
    crate::w2n_decimal::read(
        converter.n2w_key(),
        text,
        &|w| decimal_word(resolved, w),
        default_sep,
        &cardinal,
    )
}

/// "pour cent" / "por ciento" / "per cent": the second word is the number 100
/// on its own, but after the preposition it is the percent sign. Keyed by
/// language prefix; the walker leaves the pair as words ("vingt pour cent" →
/// "20 pour cent", never "20 pour 100").
fn percent_phrase(resolved: &str) -> Option<(&'static str, &'static str)> {
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    match base {
        "fr" => Some(("pour", "cent")),
        "es" | "gl" => Some(("por", "ciento")),
        "pt" => Some(("por", "cento")),
        "it" => Some(("per", "cento")),
        "en" => Some(("per", "cent")),
        "ca" => Some(("per", "cent")),
        // el "τοις εκατό", vi "phần trăm": the everyday words for percent (#82).
        // Normalised, as the walker compares them.
        "el" => Some(("τοις", "εκατο")),
        "vi" => Some(("phan", "tram")),
        _ => None,
    }
}

/// The previous non-blank part before `i`, lowercased and without trailing
/// punctuation.
fn prev_word(parts: &[String], i: usize) -> Option<String> {
    parts[..i]
        .iter()
        .rev()
        .find(|t| !py_str_isspace(t))
        .map(|t| rstrip_punct(t).to_lowercase())
}

/// The next non-blank part after `i`, lowercased and without trailing
/// punctuation.
fn next_word(parts: &[String], i: usize) -> Option<String> {
    parts[i + 1..]
        .iter()
        .find(|t| !py_str_isspace(t))
        .map(|t| rstrip_punct(t).to_lowercase())
}

/// Month names, for the date reading of "first"/"second" ("the first of
/// march", "le premier avril"). Diacritics stripped, as `normalize` does.
pub(crate) fn months(resolved: &str) -> &'static str {
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    match base {
        "en" => {
            "january february march april may june july august september october november \
             december jan feb mar apr jun jul aug sep sept oct nov dec"
        }
        "fr" => {
            "janvier fevrier mars avril mai juin juillet aout septembre octobre novembre decembre"
        }
        "es" => {
            "enero febrero marzo abril mayo junio julio agosto septiembre setiembre octubre \
             noviembre diciembre"
        }
        "de" => {
            "januar janner februar marz april mai juni juli august september oktober november \
             dezember"
        }
        "it" => {
            "gennaio febbraio marzo aprile maggio giugno luglio agosto settembre ottobre \
             novembre dicembre"
        }
        "pt" => {
            "janeiro fevereiro marco abril maio junho julho agosto setembro outubro novembro \
             dezembro"
        }
        "nl" => {
            "januari februari maart april mei juni juli augustus september oktober november \
             december"
        }
        "ca" => "gener febrer marc abril maig juny juliol agost setembre octubre novembre desembre",
        _ => "",
    }
}

fn is_month(resolved: &str, word: Option<&str>) -> bool {
    word.is_some_and(|w| {
        let norm = crate::normalize(w);
        months(resolved).split_whitespace().any(|m| m == norm)
    })
}

/// The indefinite article / "one" in front of a single ordinal makes it a
/// fraction ("a fifth of", "un dixième"), which stays in words.
fn is_indefinite(resolved: &str, word: Option<&str>) -> bool {
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    let Some(w) = word else { return false };
    let list: &[&str] = match base {
        "en" => &["a", "an", "one"],
        "fr" => &["un", "une"],
        "es" | "gl" | "ca" => &["un", "una", "uno"],
        "it" => &["un", "uno", "una"],
        "pt" => &["um", "uma"],
        "de" => &["ein", "eine", "einen", "einem", "einer", "eines"],
        "nl" => &["een"],
        _ => &[],
    };
    list.contains(&w)
}

/// "first" and "second" (and their translations) are not only ranks: "first
/// of all", "wait a second", "la première fois", "un momento, segundo". On
/// their own they stay in words unless a month sits next to them.
fn is_ambiguous_ordinal(resolved: &str, word: &str) -> bool {
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    let mut w = crate::normalize(word);
    if matches!(base, "es" | "pt" | "gl") && w.ends_with('s') {
        w.pop(); // "las primeras"
    }
    let list: &[&str] = match base {
        "en" => &["first", "second"],
        "es" | "gl" => &["primero", "primer", "primera", "segundo", "segunda"],
        "it" => &["primo", "prima", "secondo", "seconda"],
        "pt" => &["primeiro", "primeira", "segundo", "segunda"],
        "de" => &["erste", "ersten", "erster", "erstes", "erstem"],
        "nl" => &["eerste"],
        "ca" => &[
            "primer", "primera", "segon", "segona",
            // Also the collective nouns "a ten / dozen / hundred / thousand".
            "desena", "desenes", "dotzena", "dotzenes", "centena", "centenes", "milena", "milenes",
            // And the time unit "seconds", the fraction "quarters".
            "segons", "segones", "quarts", "quartes",
        ],
        // The weekdays Monday-Thursday are the feminine ordinals 2nd-5th.
        "el" => &[
            // "πρώτα" is also "first(ly)", "δεύτερο" the time unit.
            "πρωτα", "δευτερο",
            "δευτερα", "δευτερας", "τριτη", "τριτης", "τεταρτη", "τεταρτης", "πεμπτη", "πεμπτης",
        ],
        // "secondes" is also the time unit ("quelques secondes").
        "fr" => &["premier", "premiere", "second", "seconde", "secondes", "seconds"],
        // "(wait a) second"
        "he" => &["שנייה", "שניה"],
        // "annað" / "önnur" are also "other".
        "is" => &[
            "onnur", "annað", "annan", "annarri", "annars", "aðra", "aðrir", "aðrar", "oðrum", "oðru",
        ],
        // "kwanza" is also the adverb "first(ly)".
        "sw" => &["kwanza", "pili"],
        // The inflected 2nd that is also the everyday "other": da "noget andet",
        // nb "en annen", sv "de andre", et "teise", fi "toisen".
        "da" => &["andet"],
        "nb" | "no" => &["annen", "annet"],
        "sv" => &["andre"],
        "et" => &["teise", "teist", "teised", "teisel", "teisele", "teises", "teiseks"],
        "fi" => &["toisen", "toista", "toisena", "toiset", "toisessa", "toiseen", "toisella"],
        _ => &[],
    };
    list.contains(&w.as_str())
}

/// How a language writes the ordinal of `n` in figures: en 21st, fr 1er /
/// 1re / 2e, es 3º / 3ª, de 2., nl 15e. `feminine` is the agreement the
/// spoken form carried ("la vigésima", "la première"). Falls back to
/// num2words2's `ordinal_num`, and to `None` (keep the words) where that has
/// no rendering either.
fn ordinal_figures(resolved: &str, n2w_key: &str, n: &BigInt, feminine: bool) -> Option<String> {
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    let suffix = match base {
        "en" => {
            let v = (n % BigInt::from(100))
                .to_string()
                .parse::<u32>()
                .unwrap_or(0);
            match (v % 10, v) {
                (_, 11..=13) => "th",
                (1, _) => "st",
                (2, _) => "nd",
                (3, _) => "rd",
                _ => "th",
            }
        }
        "fr" => match (*n == BigInt::from(1), feminine) {
            (true, false) => "er",
            (true, true) => "re",
            _ => "e",
        },
        "es" | "pt" | "it" | "gl" => {
            if feminine {
                "ª"
            } else {
                "º"
            }
        }
        "de" => ".",
        "nl" => "e",
        _ => {
            // Only when the language marks it (tr "1'inci"); a bare number would lose
            // the rank (ru / pl render just the figure), so the words stay.
            return num2words2_core::get_lang_by_key(n2w_key)
                .and_then(|l| l.to_ordinal_num(n).ok())
                .filter(|figures| *figures != n.to_string());
        }
    };
    Some(format!("{}{}", n, suffix))
}

/// "two thirds", "three quarters", fr "deux tiers", es "tres cuartos": a
/// fraction word (a plural ordinal, or the irregular half / third /
/// quarter) after a cardinal makes it a fraction, and the cardinal stays in
/// words with it.
fn fraction_follows(converter: &Converter, resolved: &str, word: Option<&str>) -> bool {
    let Some(w) = word else { return false };
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    let norm = crate::normalize(w);
    let irregular = match base {
        "en" => "half halves quarter quarters",
        "fr" => "demi demis demie demies tiers quart quarts",
        "es" | "gl" => "medio medios media medias tercio tercios cuarto cuartos",
        "pt" => "meio meios meia meias terco tercos quarto quartos",
        "it" => "mezzo mezzi mezza mezze terzo terzi quarto quarti",
        "ca" => "mig mitjos mitja mitges terc tercos quart quarts",
        "de" => "halb halbe drittel viertel",
        "nl" => "half halve derde kwart",
        _ => "",
    };
    if irregular.split_whitespace().any(|f| f == norm) {
        return true;
    }
    match converter {
        // "ten seconds" is time; "two firsts" is not a fraction either.
        Converter::En => norm.strip_suffix('s').is_some_and(|stem| {
            !matches!(stem, "second" | "first") && crate::w2n_lang_en::ordinal_cardinal(stem).is_some()
        }),
        // A plural ordinal ("cinquièmes", "quintos"): the ordinal table holds
        // the singular.
        Converter::Table(lang) => {
            let denominator = |t: &str| {
                matches!(base_convert(lang, t, true), Ok(W2nValue::Int(n)) if n >= BigInt::from(3))
            };
            // bg "две стотни", el "δύο τρίτα": an inflected ordinal after a
            // cardinal is a fraction's denominator, as in `ordinal_rendering`.
            norm.strip_suffix('s').is_some_and(denominator)
                // bg plural "стотни", "трети": the feminine singular is "стотна", "трета".
                || (base == "bg" && norm.strip_suffix('и').is_some_and(|t| denominator(&format!("{t}а"))))
                || (matches!(base, "fr" | "ca" | "el" | "bg")
                    && !is_canonical_ordinal(converter, w)
                    && denominator(&norm))
        }
    }
}

/// The word two parts before / after `i`, skipping blanks.
fn prev_word2(parts: &[String], i: usize) -> Option<String> {
    let mut it = parts[..i].iter().rev().filter(|t| !py_str_isspace(t));
    it.next()?;
    it.next().map(|t| rstrip_punct(t).to_lowercase())
}

fn next_word2(parts: &[String], i: usize) -> Option<String> {
    let mut it = parts[i + 1..].iter().filter(|t| !py_str_isspace(t));
    it.next()?;
    it.next().map(|t| rstrip_punct(t).to_lowercase())
}

/// The ordinal reading of a run in plain mode. A table language whose
/// ordinal opens with a particle (ro "al douăzeci și unulea") has it just
/// before the run, so the reading may take that word along (#70); the
/// rendering then drops it from the figures ("al 21-lea").
fn ordinal_reading(converter: &Converter, parts: &[String], i: usize, run: &str) -> Option<W2nValue> {
    if let Ok(v) = converter.to_ordinal(run) {
        return Some(v);
    }
    let Converter::Table(lang) = converter else { return None };
    if !ordinal_opens_with_particle(lang) {
        return None;
    }
    let raw = parts[..i].iter().rev().find(|t| !py_str_isspace(t))?;
    let prev = raw.to_lowercase();
    if prev.is_empty()
        || !trailing_punct(raw).is_empty()
        || holds_digit(&prev)
        || converter.is_number_word(&prev)
    {
        return None;
    }
    converter.to_ordinal(&format!("{prev} {run}")).ok()
}

/// Does the renderer open its ordinals with a word that is not a number (ro
/// "al doilea")? Probed on 2; only then may an ordinal reading take the word
/// before a run.
fn ordinal_opens_with_particle(lang: &str) -> bool {
    let Some(l) = num2words2_core::get_lang_by_key(lang) else { return false };
    let Some(two) = l.to_ordinal(&BigInt::from(2)).ok().map(|w| crate::normalize(&w)) else {
        return false;
    };
    match two.split_once(' ') {
        Some((first, _)) => !matches!(crate::lookup(lang, first, false, &[]), Ok(Some(_))),
        None => false,
    }
}

/// Does the renderer write a compound ordinal as words + the ordinal of the
/// unit ("двадцать первый", "dwudziesty pierwszy")? Probed on 21.
fn unit_ordinal_compounds(lang: &str) -> bool {
    let Some(l) = num2words2_core::get_lang_by_key(lang) else { return false };
    let render = |n: i64| l.to_ordinal(&BigInt::from(n)).ok().map(|w| crate::normalize(&w));
    match (render(21), render(1)) {
        (Some(r21), Some(r1)) => !r1.is_empty() && r21.ends_with(&format!(" {r1}")),
        _ => false,
    }
}

/// The part index of an ordinal word right after the cardinal run ending at
/// `end` that makes the two one compound ordinal ("двадцать" + "первый":
/// the unit 1 is below the 10s of 20), in a language that composes its
/// ordinals that way. `None` otherwise.
fn split_compound_ordinal(
    converter: &Converter,
    parts: &[String],
    end: usize,
    value: Option<&W2nValue>,
) -> Option<usize> {
    let Converter::Table(lang) = converter else { return None };
    let Some(W2nValue::Int(c)) = value else { return None };
    if ends_with_terminal_punct(&parts[end]) || *c <= BigInt::from(0) {
        return None;
    }
    let k = (end + 1..parts.len()).find(|&k| !py_str_isspace(&parts[k]))?;
    let word = rstrip_punct(&parts[k]).to_lowercase();
    if !converter.is_ordinal_word(&word) {
        return None;
    }
    let Ok(W2nValue::Int(o)) = converter.to_ordinal(&word) else { return None };
    // The largest power of ten dividing the cardinal: 20 -> 10, 300 -> 100.
    let mut place = BigInt::from(1);
    while c % (&place * 10) == BigInt::from(0) {
        place *= 10;
    }
    (o > BigInt::from(0) && o < place && unit_ordinal_compounds(lang)).then_some(k)
}

/// The spoken ordinal agreed with its noun (hi "पहली", tr "ikinciye") where
/// the renderer writes one form: carry that ending over to the figures, so
/// "पहली बार" is "१ली बार" and not the masculine "१ला बार" (#153). Only a
/// short ending swap on a shared stem is carried over; anything else keeps
/// the renderer's figures.
fn agree_figures(n2w_key: &str, n: &BigInt, spoken: &str, figures: String) -> String {
    use unicode_normalization::UnicodeNormalization;
    if !figures.chars().any(is_unicode_digit) {
        return figures;
    }
    let Some(canonical) = num2words2_core::get_lang_by_key(n2w_key)
        .and_then(|l| l.to_ordinal(n).ok())
        .map(|w| crate::normalize(&w))
    else {
        return figures;
    };
    let canon: Vec<char> = canonical.rsplit(' ').next().unwrap_or("").chars().collect();
    let norm: Vec<char> = crate::normalize(spoken).chars().collect();
    let raw: Vec<char> = spoken.nfc().collect::<String>().to_lowercase().chars().collect();
    if raw.len() != norm.len() {
        return figures; // the normalized spelling does not line up with the raw one
    }
    let common = canon.iter().zip(&norm).take_while(|(a, b)| a == b).count();
    let (k, s) = (&canon[common..], &raw[common..]);
    if common < 2 || k.len() > 3 || s.len() > 3 || k == &norm[common..] {
        return figures;
    }
    let k: String = k.iter().collect();
    let s: String = s.iter().collect();
    // With nothing to swap (tr "ikinci" + "ye"), the ending is only appended
    // to figures that end in a letter ("2'nci" -> "2'nciye"); hu "1." stays
    // "1.", never "1.t".
    if k.is_empty() && !figures.chars().last().is_some_and(char::is_alphabetic) {
        return figures;
    }
    match figures.strip_suffix(k.as_str()) {
        Some(stem) if !k.is_empty() || !s.is_empty() => format!("{stem}{s}"),
        _ => figures,
    }
}

/// The figures for an ordinal run (`parts[i..=end]`, value `v`), or `None`
/// when it stays in words: a lone "first"/"second" away from a month, or a
/// fraction ("a fifth", "un dixième"). A compound ("twenty first", "one
/// hundred and fifth", "vingt-troisième") is always a rank.
fn ordinal_rendering(
    converter: &Converter,
    resolved: &str,
    parts: &[String],
    i: usize,
    end: usize,
    v: &W2nValue,
) -> Option<String> {
    let W2nValue::Int(n) = v else { return None };
    let run = rstrip_punct(py_strip(&parts[i..=end].concat())).to_lowercase();
    let words: Vec<&str> = run
        .split(&[' ', '-'][..])
        .filter(|w| !w.is_empty())
        .collect();
    let single = words.len() == 1;
    let prev = prev_word(parts, i);
    if single && is_indefinite(resolved, prev.as_deref()) {
        return None;
    }
    // it "cinquanta centesimi": the cent, not the hundredth, after a number.
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    if single
        && crate::w2n_currency::is_subunit_word(base, &crate::normalize(words[0]))
        && prev.as_deref().is_some_and(|p| {
            p.chars().next().is_some_and(|c| c.is_ascii_digit()) || converter.is_number_word(p)
        })
    {
        return None;
    }
    // es/pt "cuartos", "tercios": the plural of an ordinal is a fraction.
    if single && matches!(base, "es" | "pt" | "gl") && crate::normalize(words[0]).ends_with('s') {
        return None;
    }
    if single && is_ambiguous_ordinal(resolved, words[0]) {
        let next = next_word(parts, end);
        let prev2 = prev_word2(parts, i);
        let next2 = next_word2(parts, end);
        let of = matches!(
            next.as_deref(),
            Some("of" | "de" | "du" | "di" | "van" | "des")
        );
        let the = matches!(
            prev.as_deref(),
            Some("the" | "le" | "la" | "el" | "il" | "am" | "den" | "dia" | "o" | "lo")
        );
        // "the first of the month", "le premier du mois", "el primero de cada mes".
        let of_the = of
            && next2.as_deref().is_some_and(|w| {
                "the each every next this last month mois chaque mes cada"
                    .split_whitespace()
                    .any(|d| d == w)
            });
        let date = is_month(resolved, next.as_deref())
            || (of && is_month(resolved, next2.as_deref()))
            || of_the
            || is_month(resolved, prev.as_deref())
            || (the && is_month(resolved, prev2.as_deref()));
        if !date {
            return None;
        }
    }
    // fr "trois cinquièmes", ca "tres quarts", el "δύο τρίτα", bg "две стотни": an
    // inflected ordinal right after a number is a fraction's denominator.
    if single
        && matches!(base, "fr" | "ca" | "el" | "bg")
        && !is_canonical_ordinal(converter, words[0])
        && prev.as_deref().is_some_and(|p| {
            p.chars().next().is_some_and(|c| c.is_ascii_digit()) || converter.is_number_word(p)
        })
    {
        return None;
    }
    let last = crate::normalize(words[words.len() - 1]);
    let feminine = match base {
        "fr" => last == "premiere" || last == "premieres",
        "es" | "pt" | "gl" => last.ends_with('a') || last.ends_with("as"),
        // it: the feminine plural ends in -e ("le prime volte"), the masculine in -i.
        "it" => last.ends_with('a') || last.ends_with('e'),
        _ => false,
    };
    let figures = ordinal_figures(resolved, converter.n2w_key(), n, feminine)?;
    // ro "al douăzeci și unulea" read with the "al" before the run: the
    // figures "al 21-lea" must not repeat it.
    let figures = match prev.as_deref() {
        Some(p) if !p.is_empty() => figures
            .strip_prefix(&format!("{p} "))
            .map(str::to_string)
            .unwrap_or(figures),
        _ => figures,
    };
    match base {
        // these spell their own agreement above
        "en" | "es" | "pt" | "it" | "gl" | "de" | "nl" => Some(figures),
        // hand-written agreement: fr plural "2es", ca "5a", el "1η", bg "1-ва"
        "fr" | "ca" | "el" | "bg" => Some(agree_figure(base, &last, figures)),
        _ => Some(agree_figures(converter.n2w_key(), n, words[words.len() - 1], figures)),
    }
}

/// Is `word` the ordinal exactly as num2words2 renders it (fr "cinquième",
/// not the plural "cinquièmes")? The inflected forms reach the table through
/// `ord_forms`; the rendered form is the one a renderer would print back.
fn is_canonical_ordinal(converter: &Converter, word: &str) -> bool {
    let Converter::Table(lang) = converter else { return true };
    let Some(r) = num2words2_core::get_lang_by_key(lang) else { return true };
    let norm = crate::normalize(word);
    match crate::lookup(lang, &norm, true, &[]) {
        Ok(Some(v)) => r
            .to_ordinal(&BigInt::from(v))
            .is_ok_and(|w| crate::normalize(&w) == norm),
        _ => true,
    }
}

/// The figures of an ordinal agree with the spoken form where the language
/// marks it: fr plural "2es" / "1ers", ca "5a" / "1a" for the feminine, el
/// "1η" / "1ο" (the abbreviation is the figure + the word's ending), bg
/// "1-ва" / "1-во" / "1-вия". `fig` is the masculine singular figure.
fn agree_figure(base: &str, last: &str, fig: String) -> String {
    match base {
        "fr" if last.ends_with('s') => format!("{fig}s"),
        "ca" => {
            let n: String = fig.chars().take_while(char::is_ascii_digit).collect();
            if last.ends_with("es") {
                format!("{n}es") // primeres, cinquenes
            } else if last.ends_with('a') {
                format!("{n}a") // primera, cinquena
            } else if last.ends_with("ens") {
                format!("{n}ns") // cinquens
            } else if last.ends_with('s') {
                format!("{fig}s") // primers -> 1rs, segons -> 2ns
            } else {
                fig
            }
        }
        "el" => {
            let n: String = fig.chars().take_while(char::is_ascii_digit).collect();
            let ending = ["ους", "ος", "ου", "ον", "οι", "ες", "ης", "ων", "η", "ο", "α"]
                .into_iter()
                .find(|e| last.ends_with(e));
            match ending {
                Some(e) => format!("{n}{e}"),
                None => fig,
            }
        }
        "bg" => {
            // The masculine figure is "N-" + the last two letters ("1-ви"); swap its
            // final -и for the spoken ending.
            let ending = ["ият", "ия", "ата", "ото", "ите", "а", "о"]
                .into_iter()
                .find(|e| last.ends_with(e));
            match (ending, fig.strip_suffix('и')) {
                (Some(e), Some(stem)) if fig.contains('-') => format!("{stem}{e}"),
                _ => fig,
            }
        }
        _ => fig,
    }
}

/// The words that are both the number 1 and an article or pronoun: en "one"
/// ("no one", "one of them"), fr "un"/"une" ("un instant"), es "un"/"una"/
/// "uno", pt "um"/"uma", it "un"/"uno"/"una", nl "een", de "ein" and its
/// declensions, ca "un"/"una". On their own they stay words; see
/// [`article_is_count`] for when they are the number.
fn article_words(resolved: &str) -> &'static str {
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    match base {
        "en" => "one",
        "fr" => "un une",
        "es" | "gl" | "ca" => "un una uno",
        "it" => "un uno una",
        "pt" => "um uma",
        "nl" => "een",
        "de" => "ein eine einen einem einer eines",
        // The indefinite article is the word for one (#83): tr "bir gün", hu
        // "egy nap", sv "ett par", da/no "en dag", el "ένα παιδί"; fa / hi /
        // bn / vi use the numeral as the indefinite determiner ("یک روز",
        // "एक दिन", "một người"). ro "un" / "o" (#183). Normalised.
        "tr" => "bir",
        "hu" => "egy",
        "sv" => "en ett",
        "da" | "dk" => "en et",
        "no" | "nb" | "nn" => "en ei et ett",
        "el" => "ενας ενα μια μιας ενος εναν",
        "fa" => "یک",
        "hi" => "एक",
        "bn" => "এক",
        "vi" => "mot",
        "ro" => "un o",
        _ => "",
    }
}

pub(super) fn is_article_word(resolved: &str, word: &str) -> bool {
    let norm = crate::normalize(word);
    article_words(resolved)
        .split_whitespace()
        .any(|w| w == word || w == norm)
}

/// Number words that are also a common word of the language, and whose
/// number reading needs another number word next to them (the maintainer's
/// policy, #80): it "sei" (you are), fr "neuf" (new), nl "acht" (attention,
/// "acht geven"), ca "nou" (new), ro "nouă" (new), tr "yedi" (ate) and "yüz"
/// (face). "dix neuf", "sei e trenta" still convert; "tu sei bravo",
/// "quoi de neuf" stay words. Normalised.
fn homograph_words(resolved: &str) -> &'static str {
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    match base {
        "it" => "sei",
        "fr" => "neuf",
        "nl" | "af" => "acht",
        "ca" => "nou",
        "ro" => "noua",
        "tr" => "yedi yuz",
        _ => "",
    }
}

/// Is the word at `i` (as written, `piece`) a homograph of a number word? de
/// "Acht" capitalised is the noun ("in Acht nehmen", "Acht geben"), the
/// number is lower case.
fn is_homograph(resolved: &str, parts: &[String], i: usize, piece: &str) -> bool {
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    let word = rstrip_punct(piece);
    if base == "ko" {
        // 만일 ("if") also reads as the glued Sino 10001 (만 일).
        return lexicon::ko_native_needs_counter(word) || word == "만일";
    }
    if base == "de" {
        let sentence_start = prev_word(parts, i).is_none()
            || parts[..i]
                .iter()
                .rev()
                .find(|t| !py_str_isspace(t))
                .is_some_and(|t| matches!(t.chars().last(), Some('.' | '!' | '?' | ':')));
        return word == "Acht" && !sentence_start;
    }
    let norm = crate::normalize(word);
    homograph_words(resolved).split_whitespace().any(|w| w == norm)
}

/// Units, currencies and time words a lone article-number counts: "one
/// dollar", "un euro", "one percent", "one o'clock", "une heure". Compared
/// after `normalize` (no diacritics). "second" is left out on purpose ("one
/// second" is a pause, "une seconde" too).
fn unit_words(resolved: &str) -> &'static str {
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    match base {
        "en" => {
            "dollar dollars cent cents euro euros pound pounds buck bucks grand percent hour \
             hours minute minutes day days week weeks month months year years oclock am pm \
             kilo kilos kilogram kilograms kilometer kilometers kilometre kilometres km \
             meter meters metre metres mile miles gram grams liter liters litre litres gallon \
             gallons ounce ounces inch inches foot feet degree degrees k"
        }
        "fr" => {
            "euro euros dollar dollars centime centimes heure heures minute minutes jour jours \
             semaine semaines mois an ans annee annees kilo kilos kilometre kilometres km metre \
             metres gramme grammes litre litres degre degres"
        }
        "es" | "gl" => {
            "dolar dolares euro euros centavo centavos centimo centimos peso pesos hora horas \
             minuto minutos dia dias semana semanas mes meses ano anos kilo kilos kilometro \
             kilometros metro metros gramo gramos litro litros grado grados"
        }
        "pt" => {
            "dolar dolares euro euros centavo centavos real reais hora horas minuto minutos \
             dia dias semana semanas mes meses ano anos quilo quilos quilometro quilometros \
             metro metros grama gramas litro litros grau graus"
        }
        "it" => {
            "euro dollaro dollari centesimo centesimi ora ore minuto minuti giorno giorni \
             settimana settimane mese mesi anno anni chilo chili chilometro chilometri metro \
             metri grammo grammi litro litri grado gradi"
        }
        "nl" => {
            "euro dollar cent procent uur minuut minuten dag dagen week weken maand maanden \
             jaar jaren kilo kilometer meter gram liter graad graden"
        }
        "de" => {
            "euro dollar cent prozent stunde stunden minute minuten tag tage woche wochen \
             monat monate jahr jahre kilo kilometer meter gramm liter grad"
        }
        "ca" => "euro euros dolar dolars centim centims hora hores minut minuts dia dies setmana setmanes mes mesos any anys",
        // #83. No "day" word: "bir gün", "egy nap", "एक दिन" are "one day"
        // the article way ("once upon a time"). Letters `normalize` cannot
        // strip (tr ı, da/no ø, vi đ) are written as such.
        "tr" => {
            "lira liralık kurus dolar euro avro sterlin saat dakika hafta yıl sene kilo \
             kilogram gram metre kilometre litre derece yuzde"
        }
        "hu" => {
            "forint euro dollar ora perc ev honap kilo kilogramm gramm meter kilometer liter \
             fok szazalek"
        }
        "sv" => {
            "krona kronor öre euro dollar timme timmar minut minuter vecka veckor manad \
             manader ar kilo kilometer meter gram liter grad grader procent"
        }
        "da" | "dk" => {
            "krone kroner øre euro dollar time timer minut minutter uge uger maned maneder ar \
             kilo kilometer meter gram liter grad grader procent"
        }
        "no" | "nb" | "nn" => {
            "krone kroner øre euro dollar time timer minutt minutter uke uker maned maneder ar \
             kilo kilometer meter gram liter grad grader prosent"
        }
        "el" => {
            "ευρω δολαριο δολαρια λεπτο λεπτα ωρα ωρες εβδομαδα μηνα μηνας χρονο χρονια ετος \
             κιλο κιλα μετρο χιλιομετρο λιτρο βαθμο τοις"
        }
        "fa" => "تومان ریال دلار یورو ساعت دقیقه هفته ماه سال کیلو متر کیلومتر لیتر درصد گرم",
        "hi" => {
            "रुपये रुपया रुपए डॉलर यूरो घंटा घंटे मिनट हफ्ता हफ़्ता महीना साल किलो मीटर किलोमीटर \
             लीटर प्रतिशत ग्राम"
        }
        "bn" => "টাকা ডলার ইউরো ঘণ্টা মিনিট সপ্তাহ মাস বছর কিলো মিটার কিলোমিটার লিটার শতাংশ গ্রাম",
        "vi" => "đồng đô euro gio phut tuan thang nam ky kilo can met km lit phan",
        "ro" => "leu lei ban bani euro dolar ora ore minut minute saptamana luna an ani kilogram metru litru grad procent",
        _ => "",
    }
}

/// Words that label the number after them: "press one", "option one",
/// "number one", "chapter one", fr "tapez un", es "marque uno".
fn label_words(resolved: &str) -> &'static str {
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    match base {
        "en" => {
            "number option press dial select choose enter step page room floor line level \
             chapter part section extension ext channel gate terminal platform zone phase \
             round item question unit exhibit table figure seat car bus route grade day week \
             plan tier code pin"
        }
        "fr" => {
            "numero option tapez appuyez composez faites etape page chambre ligne niveau \
             chapitre partie section poste quai zone phase question article siege salle voie \
             porte jour semaine bus code"
        }
        "es" | "gl" => {
            "numero opcion marque pulse presione oprima digite paso pagina habitacion linea \
             nivel capitulo parte seccion extension anden zona fase pregunta articulo asiento \
             sala puerta dia semana codigo"
        }
        "pt" => {
            "numero opcao digite tecle pressione passo pagina quarto linha nivel capitulo \
             parte secao ramal zona fase pergunta artigo assento sala porta dia semana codigo"
        }
        "it" => {
            "numero opzione premi prema digiti passo pagina camera linea livello capitolo \
             parte sezione interno zona fase domanda articolo posto sala porta giorno \
             settimana codice"
        }
        "nl" => {
            "nummer optie toets druk stap pagina kamer lijn niveau hoofdstuk deel sectie zone \
             fase vraag artikel stoel zaal deur dag week code"
        }
        "de" => {
            "nummer option drucken wahlen schritt seite zimmer linie ebene kapitel teil \
             abschnitt zone phase frage artikel platz saal tur tag woche code"
        }
        _ => "",
    }
}

/// Is the lone article-number at `i` a count? Yes in front of a unit or
/// currency, in front of the percent phrase, after a label word or after a
/// month ("december one"). "one of them", "no one", "un instant", "one
/// second" stay words.
fn article_is_count(resolved: &str, parts: &[String], i: usize, next_is_number: bool) -> bool {
    // "o'clock" and "a.m." normalise to "o clock" / "a m": compared unspaced.
    let key = |w: String| crate::normalize(&w).replace(' ', "");
    let next = next_word(parts, i).map(key);
    let next2 = next_word2(parts, i).map(key);
    let prev = prev_word(parts, i).map(key);
    let labelled = prev.as_deref().is_some_and(|p| {
        label_words(resolved).split_whitespace().any(|l| l == p) || is_month(resolved, Some(p))
    });
    let punct = trailing_punct(&parts[i]);
    if !punct.is_empty() && punct != "," {
        return labelled;
    }
    // "one, two, three", "le un janvier".
    if next_is_number || is_month(resolved, next.as_deref()) {
        return true;
    }
    if let Some(n) = next.as_deref() {
        if unit_words(resolved)
            .split_whitespace()
            .any(|u| u == n || crate::normalize(u) == n)
        {
            return true;
        }
        if percent_phrase(resolved)
            .is_some_and(|(prep, word)| n == prep && next2.as_deref() == Some(word))
        {
            return true;
        }
        if n == "percent" || n == "pourcent" {
            return true;
        }
    }
    labelled
}

/// Is `word` a scale word of `resolved` with magnitude >= `min`: en "thousand",
/// fr "millions", de "millionen"?
fn is_scale_word(converter: &Converter, word: &str, min: i64) -> bool {
    match converter {
        Converter::En => {
            let mag = match word {
                "hundred" => 100,
                "thousand" => 1_000,
                _ if crate::w2n_lang_en::is_scale_word(word) => 1_000_000,
                _ => return false,
            };
            mag >= min
        }
        Converter::Table(lang) => crate::is_scale_word_of(lang, &crate::normalize(word), min),
    }
}

/// A scale word on its own is a quantity, not a number: "a couple of thousand",
/// "hundreds of", "des millions", "millones de personas", "half a million".
/// fr "mille" / es "mil" / it "mille" / de "tausend" are the number 1000 on their
/// own (no article in speech), so only million and up count there.
fn lone_scale_word(converter: &Converter, word: &str) -> bool {
    let min = match converter {
        Converter::En => 100,
        Converter::Table(_) => 1_000_000,
    };
    is_scale_word(converter, word, min)
}

/// en "the nineteen nineties": a decade word after a number keeps it in words.
fn en_decade(word: Option<&str>) -> bool {
    word.is_some_and(|w| {
        "twenties thirties forties fifties sixties seventies eighties nineties"
            .split_whitespace()
            .any(|d| d == w)
    })
}

/// The `to=` modes [`words2num`] and [`words2num_sentence`] accept.
pub const CONVERTER_TYPES: [&str; 6] =
    ["cardinal", "ordinal", "ordinal_num", "year", "currency", "digits"];

/// An unknown `to` is a `NotImplementedError`, distinct from the reverse table
/// declining a word. Shared by the single-token and the sentence entry points
/// so a typo (`to="cardnal"`) raises in both, rather than the sentence walker
/// treating every run as "not a number" and handing the text back unchanged.
fn check_converter_type(to: &str) -> Result<(), W2nError> {
    if CONVERTER_TYPES.contains(&to) {
        Ok(())
    } else {
        Err(W2nError::NotImplemented(format!(
            "conversion type {} unsupported",
            py_repr_str(to)
        )))
    }
}

/// Port of `words2num2.words2num_sentence` → `SentenceConverter.convert`.
///
/// Walks the sentence and, at each position that opens with a real number
/// word, grows the longest run of tokens the per-language converter accepts.
///
/// `has_kwargs` is whether the Python caller passed extra keyword arguments
/// through (`kwargs or None` was truthy). The standard converters accept none,
/// so any such call fails every conversion — matching Python's swallowed
/// `TypeError`.
pub fn words2num_sentence(
    sentence: &str,
    lang: &str,
    to: &str,
    has_kwargs: bool,
) -> Result<String, W2nError> {
    words2num_sentence_opts(sentence, lang, to, has_kwargs, false)
}

/// [`words2num_sentence`] with the currency fold: amounts spoken as number +
/// currency word (+ subunit) are written the way the language writes that
/// currency (`$1,355.28`, `43,20 $`, `20,50 €`). See [`crate::w2n_currency`].
pub fn words2num_sentence_opts(
    sentence: &str,
    lang: &str,
    to: &str,
    has_kwargs: bool,
    currency: bool,
) -> Result<String, W2nError> {
    words2num_sentence_full(sentence, lang, to, has_kwargs, currency, false)
}

/// [`words2num_sentence_opts`] with the date pass: day + month (+ year) spans
/// written the way the language writes a date ("March 12, 2026", "12 March
/// 2026", "am 12. März 2026"). See [`crate::w2n_date`].
pub fn words2num_sentence_full(
    sentence: &str,
    lang: &str,
    to: &str,
    has_kwargs: bool,
    currency: bool,
    date: bool,
) -> Result<String, W2nError> {
    let out = words2num_sentence_folded(sentence, lang, to, has_kwargs, currency)?;
    Ok(if date { crate::w2n_date::fold_dates(&out, lang) } else { out })
}

fn words2num_sentence_folded(
    sentence: &str,
    lang: &str,
    to: &str,
    has_kwargs: bool,
    currency: bool,
) -> Result<String, W2nError> {
    // Validate up front (#201): the walk swallows every per-run error, so an
    // unsupported `lang` or `to` would otherwise come back as the input text.
    resolve_lang(lang)?;
    check_converter_type(to)?;
    if currency {
        let resolved = resolve_lang(lang)?;
        // Walked piece by piece: fr "trente-quatre cents" after "dollars et"
        // is 34 US cents, not 3400 (see `pieces_for_walk`).
        let mut out = String::new();
        for (piece, walk) in crate::w2n_currency::pieces_for_walk(sentence, &resolved) {
            if walk {
                out.push_str(&words2num_sentence_walk(&piece, lang, to, has_kwargs)?);
            } else {
                out.push_str(&piece);
            }
        }
        return Ok(crate::w2n_currency::fold_currency(&out, &resolved));
    }
    words2num_sentence_walk(sentence, lang, to, has_kwargs)
}

fn words2num_sentence_walk(
    sentence: &str,
    lang: &str,
    to: &str,
    has_kwargs: bool,
) -> Result<String, W2nError> {
    let resolved = resolve_lang(lang)?;
    let converter = converter_for(&resolved);
    // Regional English reads with the English grammar where its table
    // declines (#193), so it takes the English run rules too.
    let en_grammar = matches!(converter, Converter::En) || en_variant(&resolved);
    let hand_includable: &[&str] = if en_grammar {
        &INCLUDABLE_EN
    } else {
        includable_for(&resolved)
    };
    // The hand list (decimal words, the English "a"/"minus") plus every
    // connector the renderer writes between number words (#65).
    let mut includable_owned: Vec<&str> = hand_includable.to_vec();
    if let Converter::Table(lang) = converter {
        for w in lexicon::derived_connectors(lang) {
            if !includable_owned.contains(&w.as_str()) {
                includable_owned.push(w.as_str());
            }
        }
    }
    let includable: &[&str] = &includable_owned;
    let decimal_heads = decimal_heads_for(&resolved);

    // The digit-string and spoken-year readings are English-only and belong
    // to the cardinal walk: `to="year"` keeps its own pair reading, and any
    // extra keyword argument fails every conversion (see `has_kwargs`).
    let en_cardinal = en_grammar && to == "cardinal" && !has_kwargs;
    // `to="digits"` writes a digit run as its digit string too, leading zeros
    // and "double"/"triple" included ("call double zero seven" -> "call
    // 007"), where the single-token `to_digits` can only return an int (#190, #195).
    let digits_mode = to == "digits" && !has_kwargs;
    let en_digits = en_cardinal || (en_grammar && digits_mode);
    // Plain cardinal mode (any language): ordinals are read too, and written
    // in figures ("15th", "1er", "2e") when they are ranks.
    let plain = to == "cardinal" && !has_kwargs;

    let percent = percent_phrase(&resolved);

    let (parts, glue) = tokens::split_parts(sentence, &converter, &resolved);
    // The separator to write before a number that follows a proclitic part
    // (he "ב-1999") and after one that precedes a case ending (tr "3'te").
    let sep_before = |i: usize| match i.checked_sub(1).map(|k| glue[k]) {
        Some(tokens::Glue::Prefix(sep)) => sep,
        _ => "",
    };
    let sep_after = |end: usize| match glue.get(end + 1) {
        Some(tokens::Glue::Suffix(sep)) => *sep,
        _ => "",
    };
    let n = parts.len();
    let mut out = String::new();
    let mut i = 0usize;
    // The run just emitted was a decimal ("two point five"): a scale word
    // right after it stays a word ("2.5 million"), it is not a new number.
    let mut after_decimal = false;

    while i < n {
        let piece = &parts[i];
        if py_str_isspace(piece) || glue[i] != tokens::Glue::Free {
            // A proclitic or case ending cut off a word is never a number.
            out.push_str(piece);
            i += 1;
            continue;
        }
        if opens_with_punct(piece) {
            out.push_str(piece);
            i += 1;
            continue;
        }
        let head = rstrip_punct(piece).to_lowercase();
        // Most words of a transcript are not numbers: decide that with one cheap
        // check before anything that looks at the neighbours or the ordinal table.
        // en "oh" / "o": the interjection, a number only next to another digit
        // word ("room oh seven"), which the digit run reads (#79).
        let head_is_oh = en_grammar && matches!(head.as_str(), "oh" | "o");
        let starts = starts_run(&converter, &head, decimal_heads)
            && !(head_is_oh && !(en_cardinal && en_digit_run(&parts, i).is_some()));
        // kn "ನೂರ", he "שני", mn "дөчин": a number only with what follows, so it
        // opens a run only in front of another number word, and alone stays a word.
        let part_head = !starts
            && !ends_with_terminal_punct(piece)
            && !includable.contains(&head.as_str())
            && converter.is_number_part(&head)
            && next_word(&parts, i).is_some_and(|t| {
                !includable.contains(&t.as_str()) && is_candidate(&converter, &t, includable)
            });
        let head_is_number = starts || part_head;
        let article_number = plain && is_article_word(&resolved, &head);
        let head_is_apocope = converter.apocope(&head).is_some();
        let head_is_article = en_cardinal && matches!(head.as_str(), "a" | "an");
        let head_is_repeat = en_digits && matches!(head.as_str(), "double" | "triple");
        let head_is_scale = after_decimal && is_scale_word(&converter, &head, 1_000);
        let head_is_percent = percent.is_some_and(|(_, word)| crate::normalize(&head) == word);
        let ordinal_head = plain && !head_is_number && converter.is_ordinal_word(&head);
        after_decimal = false;
        if !head_is_number
            && !article_number
            && !head_is_apocope
            && !head_is_article
            && !head_is_repeat
            && !ordinal_head
        {
            out.push_str(piece);
            i += 1;
            continue;
        }
        let next = next_word(&parts, i);
        // "one second", "one third": the English grammar reads an ordinal as a
        // number word, but it does not count the article in front of it.
        let next_is_number = next.as_deref().is_some_and(|t| {
            starts_run(&converter, t, decimal_heads)
                && !converter.is_ordinal_word(t)
                && converter.is_number_word(t)
        });
        let next_starts_run = !ends_with_terminal_punct(piece)
            && next.as_deref().is_some_and(|t| starts_run(&converter, t, decimal_heads));
        // A run must START with a real number word, or with an apocope
        // ("un" in es "un mil") followed by one, or — in English — with
        // "a"/"an" in front of a scale word ("a hundred dollars").
        let apocope_head = head_is_apocope && next_starts_run;
        let article_head = (head_is_article
            && !ends_with_terminal_punct(piece)
            && next
                .as_deref()
                .is_some_and(crate::w2n_lang_en::is_scale_word)
            // "half a million", "a quarter of a million": a quantity, kept in words.
            && !matches!(prev_word(&parts, i).as_deref(), Some("half" | "quarter")))
            // The article-one in front of a scale word is the number: sv "en
            // miljon", no "ett tusen", ro "o mie", "un milion" (#183).
            || (article_number
                && !head_is_apocope
                && !matches!(converter, Converter::En)
                && !ends_with_terminal_punct(piece)
                && next.as_deref().is_some_and(|t| is_scale_word(&converter, t, 100)));
        // "vingt pour cent": "cent" after "pour" is the percent sign, not 100.
        let percent_tail = head_is_percent
            && percent.is_some_and(|(prep, _)| {
                prev_word(&parts, i).is_some_and(|p| crate::normalize(&p) == prep)
            });
        // "two point five million", "deux virgule cinq millions": the scale word stays a word.
        let decimal_scale = head_is_scale && !next_starts_run;
        // en "double zero seven".
        let repeat_head = head_is_repeat && en_digit_run(&parts, i).is_some();
        if (!head_is_number && !apocope_head && !article_head && !ordinal_head && !repeat_head)
            || percent_tail
            || decimal_scale
        {
            // es "un dólar": "un" is not a number word on its own, but here
            // it counts.
            if article_number
                && !apocope_head
                && article_is_count(&resolved, &parts, i, next_is_number)
            {
                out.push('1');
                out.push_str(trailing_punct(piece));
            } else {
                out.push_str(piece);
            }
            i += 1;
            continue;
        }

        // A digit string read one digit at a time is concatenated, not
        // summed: "four five six seven" is 4567, never 22.
        let digit_run = if en_digits {
            en_digit_run(&parts, i)
        } else if plain || digits_mode {
            table_digit_run(&converter, &resolved, &parts, i)
        } else {
            None
        };
        if let Some((digits, end)) = digit_run {
            let run = parts[i..=end].concat();
            out.push_str(sep_before(i));
            out.push_str(&digits);
            out.push_str(trailing_punct(&run));
            out.push_str(sep_after(end));
            i = end + 1;
            continue;
        }

        // Grow a number run starting at i.
        let mut best_value: Option<W2nValue> = None;
        let mut best_end = i;
        let mut best_ordinal = false;
        // The spoken decimal separator of a table-language decimal run.
        let mut best_sep: Option<char> = None;
        let mut j = i;
        let mut words = 0usize;
        let mut failed = 0usize;
        // The commas the run crossed after a scale word, with that scale (#67).
        let mut commas: Vec<(usize, BigInt)> = Vec::new();
        while j < n {
            let tok = &parts[j];
            if py_str_isspace(tok) {
                j += 1;
                continue;
            }
            let clean = rstrip_punct(tok).to_lowercase();
            if clean.is_empty()
                || opens_with_punct(tok)
                || tokens::is_punct_part(&clean)
                || glue[j] != tokens::Glue::Free
            {
                // a lone punctuation part ("quarante-deux ?", the ")" of
                // "two)") never extends a run
                break;
            }
            // A token in figures is a literal: "nineteen 90" is not 109, "fifty
            // 50" not 100 (#68). The parse would accept it through the literal
            // tail, so this cannot be left to the parse.
            if holds_digit(&clean) {
                break;
            }
            words += 1;
            if words > MAX_RUN_WORDS || failed >= MAX_FAILED_EXTENSIONS {
                break; // #186
            }
            // A native Korean numeral is one word: "하나 둘 셋" counts 1 2 3,
            // and "삼십오 세" is 35 and the age counter (#178).
            if j > i
                && resolved == "ko"
                && (lexicon::ko_native(&clean).is_some() || lexicon::ko_native(&head).is_some())
            {
                break;
            }
            // A token that is not a number word on its own may still complete
            // the run: fr "cents" / "vingts" only exist inside "deux cents" /
            // "quatre vingts". So the run is tried *with* it before giving up.
            // The head was already accepted (an article in front of a scale
            // word is not a number word on its own: sv "en miljon").
            let candidate = j == i || is_candidate(&converter, &clean, includable);
            let run = parts[i..=j].concat();
            let stripped = rstrip_punct(py_strip(&run)).to_string();
            // The standard converters never return `None`, so a successful
            // parse always both records the value and advances best_end. A
            // raised error is Python's `except Exception` — swallow and keep
            // growing.
            // The cardinal reading; failing that, in plain mode, the ordinal
            // one ("vingt-troisième", "fifteenth"). The English grammar reads
            // a trailing ordinal as its cardinal, so the last word tells.
            let hit = match converter.convert(to, &stripped, has_kwargs) {
                // cs "tři sta": "sta" is also an inflected ordinal ("stá"), but
                // the run reads as an ordinal only if its ordinal reading agrees.
                Ok(v) => {
                    let ordinal = plain
                        && converter.is_ordinal_word(&clean)
                        && (matches!(converter, Converter::En)
                            || converter.to_ordinal(&stripped).is_ok_and(|o| o == v));
                    Some((v, ordinal))
                }
                Err(_) if plain && !en_cardinal => {
                    // "trois virgule cinq": a decimal spoken in a table language.
                    let dec = table_decimal(&converter, &resolved, &stripped).and_then(
                        |(int, sep, frac)| {
                            BigDecimal::from_str(&format!("{}.{}", int, frac))
                                .ok()
                                .map(|d| (W2nValue::Dec(d), sep))
                        },
                    );
                    match dec {
                        Some((v, sep)) => {
                            best_sep = Some(sep);
                            Some((v, false))
                        }
                        None => ordinal_reading(&converter, &parts, i, &stripped).map(|v| (v, true)),
                    }
                }
                Err(_) if plain => ordinal_reading(&converter, &parts, i, &stripped).map(|v| (v, true)),
                Err(_) => None,
            };
            // "deux trois", "two three": two digit words are a range, not one
            // number (#51). A dictated string goes through the digit run.
            let hit = hit.filter(|_| !two_digit_words(&converter, &parts[i..=j]));
            let hit = hit.filter(|(v, _)| connector_join_ok(&converter, &stripped, v, hand_includable));
            let hit = hit.filter(|_| comma_groups_ok(&converter, &parts, j, &commas));
            match hit {
                // A run never ends on a connector: "one and a half" is "1 and
                // a half", not "1 half"; "five point" keeps its "point".
                Some((v, ordinal)) if candidate_is_number(&converter, &clean, includable) => {
                    best_value = Some(v);
                    best_end = j;
                    best_ordinal = ordinal;
                    failed = 0;
                }
                Some(_) => failed += 1,
                None if !candidate => break,
                None => failed += 1,
            }
            // A token ending in terminal punctuation closes the run — except
            // the comma num2words2 itself writes after a scale word ("twelve
            // thousand, three hundred and forty-five"), which the run may cross
            // if what follows is the next lower group (#67).
            if ends_with_terminal_punct(tok) {
                match comma_after_scale(&converter, tok) {
                    Some(scale) if j > i => commas.push((j, scale)),
                    _ => break,
                }
            }
            j += 1;
        }

        // A spoken year outgrows the cardinal reading: "twenty twenty five"
        // is the cardinal "twenty" and then a dangling "twenty five", but the
        // year 2025.
        if en_cardinal {
            if let Some((year, end)) = en_year_run(&parts, i) {
                if best_value.is_none() || end > best_end {
                    best_value = Some(W2nValue::Int(BigInt::from(year)));
                    best_end = end;
                }
            }
        }

        // ru "двадцать первый", pl "dwadzieścia pierwszy": a cardinal followed
        // by the ordinal of a smaller unit is one compound ordinal. When it
        // did not read as one, converting the cardinal alone would split it
        // into "20 первый"; the whole compound stays in words (#70).
        if plain && !best_ordinal {
            if let Some(k) = split_compound_ordinal(&converter, &parts, best_end, best_value.as_ref()) {
                out.push_str(&parts[i..=k].concat());
                i = k + 1;
                continue;
            }
        }

        if let Some(v) = best_value {
            // Preserve trailing punctuation that was stripped during parse.
            let run = parts[i..=best_end].concat();
            let trailing = trailing_punct(&run);
            let rendered = if best_ordinal {
                ordinal_rendering(&converter, &resolved, &parts, i, best_end, &v)
            } else if plain
                && !best_ordinal
                && fraction_follows(
                    &converter,
                    &resolved,
                    next_word(&parts, best_end).as_deref(),
                )
            {
                // "two thirds", "deux tiers": a fraction, kept in words.
                None
            } else if article_number
                && best_end == i
                && !article_is_count(&resolved, &parts, i, next_is_number)
            {
                // A lone "one" / "un" / "um" is an article or a pronoun.
                None
            } else if part_head && best_end == i {
                None
            } else if plain
                && best_end == i
                && resolved == "ko"
                && is_homograph(&resolved, &parts, i, piece)
            {
                // ko 세 개 (three things) but 네 (yes), 삼십오 세 (35 years old).
                (lexicon::ko_counter_follows(&head, next.as_deref())).then(|| v.text_str())
            } else if plain
                && best_end == i
                && is_homograph(&resolved, &parts, i, piece)
                && !next_is_number
                // "sei e trenta": a connector, then a number.
                && !(next.as_deref().is_some_and(|t| is_includable(t, includable))
                    && next_word2(&parts, i).is_some_and(|t| starts_run(&converter, &t, decimal_heads)))
                && !prev_word(&parts, i).is_some_and(|p| starts_run(&converter, &p, decimal_heads))
            {
                // "tu sei bravo", "quoi de neuf": a lone homograph is the word (#80).
                None
            } else if plain && best_end == i && lone_scale_word(&converter, &head) {
                // "a couple of thousand", "des millions": a quantity, not a number.
                None
            } else if en_cardinal && en_decade(next_word(&parts, best_end).as_deref()) {
                // "the nineteen nineties".
                None
            } else if let (W2nValue::Dec(_), Some(sep)) = (&v, best_sep) {
                Some(v.text_str().replacen('.', &sep.to_string(), 1))
            } else {
                Some(v.text_str())
            };
            match rendered {
                Some(text) => {
                    after_decimal = matches!(v, W2nValue::Dec(_)) && trailing.is_empty();
                    out.push_str(sep_before(i));
                    out.push_str(&text);
                    out.push_str(trailing);
                    out.push_str(sep_after(best_end));
                }
                None => out.push_str(&run),
            }
            i = best_end + 1;
        } else {
            out.push_str(piece);
            i += 1;
        }
    }
    Ok(out)
}

/// `convert_sentence = words2num_sentence` (alias, parity with num2words2).
pub fn convert_sentence(
    sentence: &str,
    lang: &str,
    to: &str,
    has_kwargs: bool,
) -> Result<String, W2nError> {
    words2num_sentence(sentence, lang, to, has_kwargs)
}

/// `sentence_to_words = words2num_sentence` (alias, parity with num2words2).
pub fn sentence_to_words(
    sentence: &str,
    lang: &str,
    to: &str,
    has_kwargs: bool,
) -> Result<String, W2nError> {
    words2num_sentence(sentence, lang, to, has_kwargs)
}

// ===========================================================================
// Registries — `UNITS`, `CURRENCIES`, `SCALE_SUFFIXES`
// ===========================================================================

/// `converters.auto.UnitInfo`.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitInfo {
    pub short: &'static str,
    pub long: &'static str,
    pub kind: &'static str,
    pub confidence: f64,
}

const fn ui(short: &'static str, long: &'static str, kind: &'static str, confidence: f64) -> UnitInfo {
    UnitInfo {
        short,
        long,
        kind,
        confidence,
    }
}

/// `converters.auto.UNITS`.
///
/// A slice, not a map: insertion order is observable through
/// `next((u.long for u in UNITS.values() if u.short == short), tail)` in
/// `_try_word_unit`, which takes the *first* entry with a matching short form
/// (`"l"` before `"L"` → `"liter"`; `"µm"` before `"um"` → `"micrometer"`;
/// `"°C"` before `"C"` → `"degree celsius"`).
///
/// The `µ` here is U+00B5 MICRO SIGN, matching the source (not U+03BC).
pub static UNITS: &[(&str, UnitInfo)] = &[
    // Length
    ("mm", ui("mm", "millimeter", "length", 1.0)),
    ("cm", ui("cm", "centimeter", "length", 1.0)),
    ("dm", ui("dm", "decimeter", "length", 1.0)),
    ("m", ui("m", "meter", "length", 0.6)),
    ("km", ui("km", "kilometer", "length", 1.0)),
    ("in", ui("in", "inch", "length", 0.5)),
    ("ft", ui("ft", "foot", "length", 1.0)),
    ("yd", ui("yd", "yard", "length", 1.0)),
    ("mi", ui("mi", "mile", "length", 1.0)),
    ("nm", ui("nm", "nanometer", "length", 1.0)),
    ("\u{b5}m", ui("\u{b5}m", "micrometer", "length", 1.0)),
    ("um", ui("\u{b5}m", "micrometer", "length", 1.0)),
    // Mass
    ("mg", ui("mg", "milligram", "mass", 1.0)),
    ("g", ui("g", "gram", "mass", 0.6)),
    ("kg", ui("kg", "kilogram", "mass", 1.0)),
    ("t", ui("t", "tonne", "mass", 0.5)),
    ("lb", ui("lb", "pound", "mass", 1.0)),
    ("lbs", ui("lb", "pound", "mass", 1.0)),
    ("oz", ui("oz", "ounce", "mass", 1.0)),
    // Temperature
    ("\u{b0}", ui("\u{b0}", "degree", "temperature", 0.7)),
    ("\u{b0}C", ui("\u{b0}C", "degree celsius", "temperature", 1.0)),
    ("\u{b0}F", ui("\u{b0}F", "degree fahrenheit", "temperature", 1.0)),
    ("K", ui("K", "kelvin", "temperature", 0.6)),
    ("C", ui("\u{b0}C", "degree celsius", "temperature", 0.5)),
    ("F", ui("\u{b0}F", "degree fahrenheit", "temperature", 0.5)),
    // Time
    ("ms", ui("ms", "millisecond", "time", 1.0)),
    ("s", ui("s", "second", "time", 0.6)),
    ("sec", ui("s", "second", "time", 1.0)),
    ("min", ui("min", "minute", "time", 1.0)),
    ("h", ui("h", "hour", "time", 0.7)),
    ("hr", ui("h", "hour", "time", 1.0)),
    ("hrs", ui("h", "hour", "time", 1.0)),
    ("d", ui("d", "day", "time", 0.5)),
    // Volume
    ("ml", ui("ml", "milliliter", "volume", 1.0)),
    ("cl", ui("cl", "centiliter", "volume", 1.0)),
    ("dl", ui("dl", "deciliter", "volume", 1.0)),
    ("l", ui("L", "liter", "volume", 0.7)),
    ("L", ui("L", "liter", "volume", 1.0)),
    ("gal", ui("gal", "gallon", "volume", 1.0)),
    // Percent
    ("%", ui("%", "percent", "percent", 1.0)),
];

/// `UNITS.get(key)`.
pub fn units_get(key: &str) -> Option<&'static UnitInfo> {
    UNITS.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

/// `converters.auto.CurrencyInfo`.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrencyInfo {
    pub code: &'static str,
    pub symbol: &'static str,
    pub long: &'static str,
}

const fn ci(code: &'static str, symbol: &'static str, long: &'static str) -> CurrencyInfo {
    CurrencyInfo { code, symbol, long }
}

/// `converters.auto.CURRENCIES`.
///
/// Note `"EUR"` maps to long `"euro"` while `"USD"` maps to `"US dollar"` but
/// the `"$"` symbol maps to `"dollar"` — so `auto_parse_sentence("$5", expand=True)`
/// says "dollars" and `auto_parse_sentence("5 USD", expand=True)` says
/// "US dollars". Faithful to the source.
pub static CURRENCIES: &[(&str, CurrencyInfo)] = &[
    ("$", ci("USD", "$", "dollar")),
    ("\u{20ac}", ci("EUR", "\u{20ac}", "euro")),
    ("\u{a3}", ci("GBP", "\u{a3}", "pound")),
    ("\u{a5}", ci("JPY", "\u{a5}", "yen")),
    ("\u{20b9}", ci("INR", "\u{20b9}", "rupee")),
    ("\u{20bd}", ci("RUB", "\u{20bd}", "ruble")),
    ("\u{20a9}", ci("KRW", "\u{20a9}", "won")),
    ("\u{20ba}", ci("TRY", "\u{20ba}", "lira")),
    ("USD", ci("USD", "$", "US dollar")),
    ("EUR", ci("EUR", "\u{20ac}", "euro")),
    ("GBP", ci("GBP", "\u{a3}", "pound sterling")),
    ("JPY", ci("JPY", "\u{a5}", "yen")),
    ("CHF", ci("CHF", "CHF", "Swiss franc")),
    ("CAD", ci("CAD", "$", "Canadian dollar")),
    ("AUD", ci("AUD", "$", "Australian dollar")),
    ("CNY", ci("CNY", "\u{a5}", "yuan")),
    ("INR", ci("INR", "\u{20b9}", "rupee")),
    ("BRL", ci("BRL", "R$", "real")),
    ("MXN", ci("MXN", "$", "Mexican peso")),
    ("RUB", ci("RUB", "\u{20bd}", "ruble")),
    ("KRW", ci("KRW", "\u{20a9}", "won")),
];

/// `CURRENCIES.get(key)`.
pub fn currencies_get(key: &str) -> Option<&'static CurrencyInfo> {
    CURRENCIES.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

/// The eight symbols in the `[$€£¥₹₽₩₺]` character class.
const CURRENCY_SYMBOLS: [char; 8] = [
    '$',
    '\u{20ac}',
    '\u{a3}',
    '\u{a5}',
    '\u{20b9}',
    '\u{20bd}',
    '\u{20a9}',
    '\u{20ba}',
];

fn is_currency_symbol(c: char) -> bool {
    CURRENCY_SYMBOLS.contains(&c)
}

/// `converters.auto.SCALE_SUFFIXES`, as a power of ten. The regex admits
/// `[kKmMbBtT][nN]?`; the `n` is the financial-press spelling of the large
/// scales in any case (`$5Mn`, `$4.1bn`, `$2TN`), so it is accepted on m/b/t.
/// `"kn"` is not an abbreviation of thousand (it is the kuna): `None`, which
/// the caller turns into a `Words2NumError` (#194).
fn scale_suffix(s: &str) -> Option<u32> {
    let mut it = s.chars();
    let head = it.next()?;
    let n = match it.next() {
        None => false,
        Some('n' | 'N') if it.next().is_none() => true,
        Some(_) => return None,
    };
    match head.to_ascii_lowercase() {
        'k' if !n => Some(3),
        'm' => Some(6),
        'b' => Some(9),
        't' => Some(12),
        _ => None,
    }
}

// ===========================================================================
// `Quantity`
// ===========================================================================

/// `converters.auto.Quantity`.
#[derive(Debug, Clone, PartialEq)]
pub struct Quantity {
    pub value: W2nValue,
    pub unit: Option<String>,
    pub unit_long: Option<String>,
    pub kind: Option<String>,
    pub confidence: f64,
    pub raw: String,
}

impl Quantity {
    fn bare(value: W2nValue, raw: &str) -> Quantity {
        Quantity {
            value,
            unit: None,
            unit_long: None,
            kind: None,
            confidence: 1.0,
            raw: raw.to_string(),
        }
    }

    /// `Quantity.__repr__`.
    ///
    /// ```python
    /// if self.unit:
    ///     return "Quantity(value={!r}, unit={!r}, kind={!r}, confidence={})".format(...)
    /// return "Quantity(value={!r})".format(self.value)
    /// ```
    ///
    /// `if self.unit` is a *truthiness* test, so an empty-string unit takes
    /// the short branch. `confidence` uses `{}` (i.e. `str`), not `{!r}`.
    pub fn py_repr(&self) -> String {
        match &self.unit {
            Some(u) if !u.is_empty() => format!(
                "Quantity(value={}, unit={}, kind={}, confidence={})",
                self.value.py_repr(),
                py_repr_str(u),
                match &self.kind {
                    Some(k) => py_repr_str(k),
                    None => "None".to_string(),
                },
                py_float_repr(self.confidence),
            ),
            _ => format!("Quantity(value={})", self.value.py_repr()),
        }
    }
}

// ===========================================================================
// `parse_number_string` — one implementation, in `crate::w2n_formats`
// ===========================================================================

/// [`crate::w2n_formats::parse_number_string`] in this module's value and error
/// types. There used to be a private copy of the whole `formats.py` port here;
/// two copies meant two places to fix every separator rule.
fn parse_number_string(
    s: &str,
    thousands_sep: Option<&str>,
    decimal_sep: Option<&str>,
    lang: Option<&str>,
) -> Result<W2nValue, W2nError> {
    crate::w2n_formats::parse_number_string(s, thousands_sep, decimal_sep, lang)
        .map(formats_value)
        .map_err(|e| W2nError::Words2Num(e.0))
}

fn formats_value(v: crate::w2n_formats::W2nValue) -> W2nValue {
    use crate::w2n_formats::W2nValue as F;
    match v {
        F::Int(i) => W2nValue::Int(i),
        F::Float(f) => W2nValue::Float(f),
        F::Dec(d) => W2nValue::Dec(d),
    }
}

// ===========================================================================
// `auto_parse` internals
// ===========================================================================

/// The `[\d.,'\xa0   _]` class shared by every `auto.py` regex.
fn is_num_class(c: char) -> bool {
    is_unicode_digit(c)
        || matches!(
            c,
            '.' | ',' | '\'' | '\u{a0}' | '\u{202f}' | '\u{2009}' | ' ' | '_'
        )
}

/// `[.,'_\xa0   ]` — the same eight non-digit characters, used as
/// the internal separator in `auto_parse_sentence`'s `NUM`.
fn is_num_sep(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | '\'' | '_' | '\u{a0}' | '\u{202f}' | '\u{2009}' | ' '
    )
}

/// A tiny cursor over `&[char]`, since these regexes must be matched by
/// character (the classes contain non-ASCII) and never by byte.
struct Cursor<'a> {
    c: &'a [char],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn new(c: &'a [char]) -> Self {
        Cursor { c, i: 0 }
    }
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn at(&self, k: usize) -> Option<char> {
        self.c.get(self.i + k).copied()
    }
    fn eat_if(&mut self, f: impl Fn(char) -> bool) -> bool {
        if self.peek().is_some_and(&f) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    /// `\s*`
    fn skip_ws(&mut self) {
        while self.eat_if(py_is_space) {}
    }
    fn take_while(&mut self, f: impl Fn(char) -> bool) -> String {
        let start = self.i;
        while self.peek().is_some_and(&f) {
            self.i += 1;
        }
        self.c[start..self.i].iter().collect()
    }
    fn at_end(&self) -> bool {
        self.i >= self.c.len()
    }
}

/// `([-+]?[\d.,'\xa0   _]+)`, greedy.
///
/// No backtracking is needed anywhere this is used: the only characters the
/// greedy run could give back that a following `\s*` would accept are the four
/// space-likes, and `\s*` would then consume them and land on the very same
/// index. Every continuation is therefore identical.
fn eat_signed_num_class(cur: &mut Cursor) -> Option<String> {
    let start = cur.i;
    cur.eat_if(|c| c == '-' || c == '+');
    let body = cur.take_while(is_num_class);
    if body.is_empty() {
        cur.i = start;
        return None;
    }
    Some(cur.c[start..cur.i].iter().collect())
}

/// `([A-Z]{3})`
fn eat_iso_code(cur: &mut Cursor) -> Option<String> {
    let mut out = String::new();
    for k in 0..3 {
        match cur.at(k) {
            Some(c) if c.is_ascii_uppercase() => out.push(c),
            _ => return None,
        }
    }
    cur.i += 3;
    Some(out)
}

/// `converters.auto._try_currency_prefix`
///
/// ```python
/// m = re.match(r"^\s*([$€£¥₹₽₩₺])\s*([-+]?[\d.,'\xa0   _]+)"
///              r"\s*([kKmMbBtT][nN]?)?\s*$", text)
/// ...
/// m = re.match(r"^\s*([A-Z]{3})\s*([-+]?[\d.,'\xa0   _]+)\s*$", text)
/// if m and m.group(1) in CURRENCIES: ...
/// ```
fn try_currency_prefix(text: &[char]) -> Option<(String, String, Option<String>)> {
    // Symbol prefix.
    {
        let mut cur = Cursor::new(text);
        cur.skip_ws();
        if let Some(sym) = cur.peek().filter(|c| is_currency_symbol(*c)) {
            cur.i += 1;
            cur.skip_ws();
            if let Some(num) = eat_signed_num_class(&mut cur) {
                cur.skip_ws();
                let mut scale = None;
                if cur.peek().is_some_and(|c| "kKmMbBtT".contains(c)) {
                    let mut s = String::new();
                    s.push(cur.peek().unwrap());
                    cur.i += 1;
                    if cur.peek().is_some_and(|c| c == 'n' || c == 'N') {
                        s.push(cur.peek().unwrap());
                        cur.i += 1;
                    }
                    scale = Some(s);
                }
                cur.skip_ws();
                if cur.at_end() {
                    return Some((sym.to_string(), py_strip(&num).to_string(), scale));
                }
            }
        }
    }
    // 3-letter ISO code prefix (USD, EUR, ...).
    {
        let mut cur = Cursor::new(text);
        cur.skip_ws();
        if let Some(code) = eat_iso_code(&mut cur) {
            cur.skip_ws();
            if let Some(num) = eat_signed_num_class(&mut cur) {
                cur.skip_ws();
                if cur.at_end() && currencies_get(&code).is_some() {
                    return Some((code, py_strip(&num).to_string(), None));
                }
            }
        }
    }
    None
}

/// `converters.auto._try_currency_suffix`
fn try_currency_suffix(text: &[char]) -> Option<(String, String)> {
    // Symbol suffix.
    {
        let mut cur = Cursor::new(text);
        cur.skip_ws();
        if let Some(num) = eat_signed_num_class(&mut cur) {
            cur.skip_ws();
            if let Some(sym) = cur.peek().filter(|c| is_currency_symbol(*c)) {
                cur.i += 1;
                cur.skip_ws();
                if cur.at_end() {
                    return Some((sym.to_string(), py_strip(&num).to_string()));
                }
            }
        }
    }
    // 3-letter ISO code suffix.
    {
        let mut cur = Cursor::new(text);
        cur.skip_ws();
        if let Some(num) = eat_signed_num_class(&mut cur) {
            cur.skip_ws();
            if let Some(code) = eat_iso_code(&mut cur) {
                cur.skip_ws();
                if cur.at_end() && currencies_get(&code).is_some() {
                    return Some((code, py_strip(&num).to_string()));
                }
            }
        }
    }
    None
}

/// `converters.auto._quantity_currency`
fn quantity_currency(value: W2nValue, sym: &str, raw: &str) -> Quantity {
    let info = currencies_get(sym).expect("every symbol the regex matches is a CURRENCIES key");
    Quantity {
        value,
        unit: Some(info.code.to_string()),
        unit_long: Some(info.long.to_string()),
        kind: Some("currency".to_string()),
        confidence: 1.0,
        raw: raw.to_string(),
    }
}

/// `converters.auto._apply_scale`, exact: the decimal point is shifted, never
/// multiplied in binary floating point, so `$4.1bn` is 4100000000 and not
/// 4099999999.9999995 (#71). A whole result is an `int`; `$1.2345k` stays the
/// `Decimal` 1234.5.
fn apply_scale(value: W2nValue, scale_str: &str) -> Result<W2nValue, W2nError> {
    let exp = scale_suffix(scale_str).ok_or_else(|| {
        W2nError::Words2Num(format!("unknown scale suffix {}", py_repr_str(scale_str)))
    })?;
    let pow = |n: u32| BigInt::from(10u8).pow(n);
    match value {
        W2nValue::Int(i) => Ok(W2nValue::Int(i * pow(exp))),
        // Unreachable: digit decimals parse to `Dec`. Kept total.
        W2nValue::Float(f) => Ok(W2nValue::Float(f * 10f64.powi(exp as i32))),
        W2nValue::Dec(d) => {
            let (digits, scale) = d.normalized().as_bigint_and_exponent();
            let scale = scale - i64::from(exp);
            Ok(if scale <= 0 {
                W2nValue::Int(digits * pow(u32::try_from(-scale).unwrap_or(0)))
            } else {
                W2nValue::Dec(BigDecimal::new(digits, scale))
            })
        }
    }
}

/// The other units a short token can stand for, as [`UNITS`] keys: "m" is a
/// metre, a minute ("5m" on a timer) or a mile; a bare "°" is Celsius or
/// Fahrenheit. Only these readings can be chosen with `prefer`.
fn alternative_readings(token: &str) -> &'static [&'static str] {
    match token {
        "m" | "M" => &["min", "mi"],
        "\u{b0}" => &["\u{b0}C", "\u{b0}F"],
        _ => &[],
    }
}

/// `converters.auto._resolve_unit`
///
/// Returns `(info, confidence)`; `None` means the token is not a unit at all.
///
/// `prefer` maps a token to the unit the caller means by it
/// (`{"m": "minute"}`, by long or short form). When that unit is a reading of
/// the token (see [`alternative_readings`]) the whole preferred unit is
/// returned — short form, long form and kind, so `"5m"` becomes `5 min`, a
/// time (#203). A target that is not a reading of the token (`{"m": "kg"}`) or
/// not a unit at all is ignored.
fn resolve_unit(unit_str: &str, prefer: &HashMap<String, String>) -> Option<(&'static UnitInfo, f64)> {
    // Direct hit, then a lowercased retry (so "°C"/"K" keep their casing).
    let info = units_get(unit_str).or_else(|| units_get(&unit_str.to_lowercase()))?;

    // prefer-override is keyed on the ORIGINAL token, not the lowercased one.
    if let Some(target) = prefer.get(unit_str).filter(|t| !t.is_empty()) {
        let wanted = UNITS
            .iter()
            .find(|(_, u)| u.long == target || u.short == target);
        if let Some((key, u)) = wanted {
            let reading = u.short == info.short || alternative_readings(unit_str).contains(key);
            if reading {
                // Confidence reflects the caller stepping in.
                return Some((u, info.confidence.max(0.9)));
            }
        }
    }
    Some((info, info.confidence))
}

/// `converters.auto._try_digit_unit`
///
/// ```python
/// m = re.match(r"^\s*([-+]?[\d.,'\xa0   _]+)\s*(°[CF]?|°[CF]?|µm|"
///              r"[a-zA-Z]+|%)\s*$", text)
/// ```
/// The `°[CF]?` branch is duplicated in the source; the second is dead.
fn try_digit_unit(
    text: &[char],
    prefer: &HashMap<String, String>,
    thousands_sep: Option<&str>,
    decimal_sep: Option<&str>,
    lang: &str,
) -> Result<Option<Quantity>, W2nError> {
    let mut cur = Cursor::new(text);
    cur.skip_ws();
    let Some(num) = eat_signed_num_class(&mut cur) else {
        return Ok(None);
    };
    cur.skip_ws();

    // Ordered alternation: °[CF]? | °[CF]? | µm | [a-zA-Z]+ | %
    let unit: String = if cur.peek() == Some('\u{b0}') {
        let mut s = String::from('\u{b0}');
        cur.i += 1;
        if let Some(c) = cur.peek().filter(|c| *c == 'C' || *c == 'F') {
            s.push(c);
            cur.i += 1;
        }
        s
    } else if cur.peek() == Some('\u{b5}') && cur.at(1) == Some('m') {
        cur.i += 2;
        String::from("\u{b5}m")
    } else if cur.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
        cur.take_while(|c| c.is_ascii_alphabetic())
    } else if cur.peek() == Some('%') {
        cur.i += 1;
        String::from("%")
    } else {
        return Ok(None);
    };
    cur.skip_ws();
    if !cur.at_end() {
        return Ok(None);
    }

    let num_str = py_strip(&num).to_string();
    let unit_str = py_strip(&unit).to_string();

    let Some((info, conf)) = resolve_unit(&unit_str, prefer) else {
        return Ok(None);
    };
    let value = match parse_number_string(&num_str, thousands_sep, decimal_sep, Some(lang)) {
        Ok(v) => v,
        Err(W2nError::Words2Num(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    Ok(Some(Quantity {
        value,
        unit: Some(info.short.to_string()),
        unit_long: Some(info.long.to_string()),
        kind: Some(info.kind.to_string()),
        confidence: conf,
        raw: String::new(),
    }))
}

/// `converters.auto._try_word_unit`'s `word_units` table → `(short, kind)`.
///
/// `"pound sterling"` is present in the source but unreachable: the lookup key
/// comes from `rsplit(None, 1)` and so can never contain a space. Kept for
/// fidelity of the table, not of the behaviour.
fn word_unit(tail: &str) -> Option<(&'static str, &'static str)> {
    Some(match tail {
        "millimeter" | "millimetre" => ("mm", "length"),
        "centimeter" | "centimetre" => ("cm", "length"),
        "meter" | "metre" => ("m", "length"),
        "kilometer" | "kilometre" => ("km", "length"),
        "inch" => ("in", "length"),
        "foot" | "feet" => ("ft", "length"),
        "yard" => ("yd", "length"),
        "mile" => ("mi", "length"),
        "milligram" => ("mg", "mass"),
        "gram" => ("g", "mass"),
        "kilogram" => ("kg", "mass"),
        "tonne" | "ton" => ("t", "mass"),
        "pound" => ("lb", "mass"),
        "ounce" => ("oz", "mass"),
        "second" => ("s", "time"),
        "minute" => ("min", "time"),
        "hour" => ("h", "time"),
        "day" => ("d", "time"),
        "millisecond" => ("ms", "time"),
        "milliliter" | "millilitre" => ("ml", "volume"),
        "liter" | "litre" => ("L", "volume"),
        "gallon" => ("gal", "volume"),
        "percent" => ("%", "percent"),
        "degree" => ("\u{b0}", "temperature"),
        "celsius" => ("\u{b0}C", "temperature"),
        "fahrenheit" => ("\u{b0}F", "temperature"),
        "kelvin" => ("K", "temperature"),
        // currency words
        "dollar" => ("USD", "currency"),
        "euro" => ("EUR", "currency"),
        "pound sterling" => ("GBP", "currency"), // unreachable, see above
        "yen" => ("JPY", "currency"),
        "rupee" => ("INR", "currency"),
        "yuan" => ("CNY", "currency"),
        "franc" => ("CHF", "currency"),
        "ruble" => ("RUB", "currency"),
        "won" => ("KRW", "currency"),
        _ => return None,
    })
}

/// `converters.auto._try_word_unit` — a word-form number plus a unit word.
fn try_word_unit(
    text: &str,
    lang: &str,
    prefer: &HashMap<String, String>,
) -> Result<Option<Quantity>, W2nError> {
    let parts = py_rsplit_once_ws(text);
    if parts.len() != 2 {
        return Ok(None);
    }
    let head = parts[0];
    let raw_tail = parts[1];
    // `parts[1].lower().rstrip("s")` — rstrip drops EVERY trailing "s".
    let tail = py_rstrip_char(&raw_tail.to_lowercase(), 's').to_string();

    let (short, kind) = match word_unit(&tail) {
        Some(v) => v,
        None => {
            // Fall back to short-form tokens (kg, cm, °C …) so mixed forms
            // like "forty-two kg" work. NB: `_resolve_unit` is handed the
            // ORIGINAL tail, not the lowercased/rstripped one — which is why
            // "five lbs" works (UNITS has "lbs") but "forty-two kgs" does not.
            match resolve_unit(raw_tail, prefer) {
                Some((info, _conf)) => (info.short, info.kind),
                None => return Ok(None),
            }
        }
    };

    // `words2num(head, lang=lang)` — a raised Exception (unparseable /
    // unsupported locale) is swallowed to "not a unit expression".
    let value = match call_words2num(head, lang) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    if kind == "currency" {
        let info = currencies_get(short).expect("every currency short is a CURRENCIES key");
        return Ok(Some(Quantity {
            value,
            unit: Some(info.code.to_string()),
            unit_long: Some(info.long.to_string()),
            kind: Some("currency".to_string()),
            confidence: 1.0,
            raw: String::new(),
        }));
    }
    // `next((u.long for u in UNITS.values() if u.short == short), tail)` —
    // first match in insertion order, falling back to the bare tail.
    let long_name = UNITS
        .iter()
        .find(|(_, u)| u.short == short)
        .map(|(_, u)| u.long.to_string())
        .unwrap_or_else(|| tail.clone());
    Ok(Some(Quantity {
        value,
        unit: Some(short.to_string()),
        unit_long: Some(long_name),
        kind: Some(kind.to_string()),
        confidence: 1.0,
        raw: String::new(),
    }))
}

// ===========================================================================
// `auto_parse`
// ===========================================================================

/// Port of `converters.auto.auto_parse`.
///
/// Tries, in order: currency prefix, currency suffix, digit+unit, word+unit,
/// bare digit number, bare word number.
///
/// The `isinstance(text, str)` guard has no analogue here — `text` is `&str`.
/// A binding that accepts `PyAny` must reproduce
/// `Words2NumError("expected str, got %r" % type(text).__name__)` itself.
///
/// The language and the separator overrides are checked before anything is
/// parsed: an unsupported `lang` raises `NotImplementedError` like
/// `words2num` does (#204), and `thousands_sep == decimal_sep` is refused
/// (#188). Digit decimals are exact `Decimal`s (#71).
pub fn auto_parse(
    text: &str,
    lang: &str,
    prefer: &HashMap<String, String>,
    thousands_sep: Option<&str>,
    decimal_sep: Option<&str>,
) -> Result<Quantity, W2nError> {
    let resolved = resolve_lang(lang)?;
    crate::w2n_formats::check_separators(thousands_sep, decimal_sep)
        .map_err(|e| W2nError::Words2Num(e.0))?;
    let lang = resolved.as_str();
    let raw = text;
    let text = py_strip(text);
    if text.is_empty() {
        return Err(W2nError::Words2Num("empty input".to_string()));
    }
    let chars: Vec<char> = text.chars().collect();

    // 1. Currency-prefix form: $12.50 / €12,50 / $5m
    if let Some((sym, num_str, scale)) = try_currency_prefix(&chars) {
        // NB: only Words2NumError is swallowed here. A KeyError out of
        // _apply_scale ("$5kn") escapes auto_parse entirely.
        match (|| -> Result<Quantity, W2nError> {
            let mut value = parse_number_string(&num_str, thousands_sep, decimal_sep, Some(lang))?;
            if let Some(s) = scale.as_deref().filter(|s| !s.is_empty()) {
                value = apply_scale(value, s)?;
            }
            Ok(quantity_currency(value, &sym, raw))
        })() {
            Ok(q) => return Ok(q),
            Err(W2nError::Words2Num(_)) => {}
            Err(e) => return Err(e),
        }
    }

    // 2. Currency-suffix form: 12.50 € / 45 USD
    if let Some((sym, num_str)) = try_currency_suffix(&chars) {
        match parse_number_string(&num_str, thousands_sep, decimal_sep, Some(lang)) {
            Ok(value) => return Ok(quantity_currency(value, &sym, raw)),
            Err(W2nError::Words2Num(_)) => {}
            Err(e) => return Err(e),
        }
    }

    // 3. Number + unit suffix (digit form): "5cm", "20°C", "42%", "3.5 km"
    if let Some(mut res) = try_digit_unit(&chars, prefer, thousands_sep, decimal_sep, lang)? {
        res.raw = raw.to_string();
        return Ok(res);
    }

    // 4. Word-form number + unit: "forty-two kg", "twenty-three percent"
    if let Some(mut res) = try_word_unit(text, lang, prefer)? {
        res.raw = raw.to_string();
        return Ok(res);
    }

    // 5. Pure number — digit form, then word form.
    match parse_number_string(text, thousands_sep, decimal_sep, Some(lang)) {
        Ok(value) => return Ok(Quantity::bare(value, raw)),
        Err(W2nError::Words2Num(_)) => {}
        Err(e) => return Err(e),
    }

    match call_words2num(text, lang) {
        Ok(v) => Ok(Quantity::bare(v, raw)),
        // raise Words2NumError("could not auto-parse %r: %s" % (raw, exc))
        Err(e) => Err(W2nError::Words2Num(format!(
            "could not auto-parse {}: {}",
            py_repr_str(raw),
            e.message()
        ))),
    }
}

// ===========================================================================
// `auto_parse_sentence`
// ===========================================================================

/// `NUM = r"[-+]?\d+(?:[.,'_\xa0   ]\d+)*"`
///
/// Note this is stricter than the `[\d.,'… _]+` class used inside
/// `auto_parse`: a separator only counts when digits immediately follow, which
/// is what keeps the regex from swallowing the `", "` after `"$12.50"`.
fn eat_num(cur: &mut Cursor) -> Option<String> {
    let start = cur.i;
    cur.eat_if(|c| c == '-' || c == '+');
    if !cur.peek().is_some_and(is_unicode_digit) {
        cur.i = start;
        return None;
    }
    cur.take_while(is_unicode_digit);
    // (?:sep \d+)* — greedy; each round needs digits after the separator.
    loop {
        let save = cur.i;
        if !cur.eat_if(is_num_sep) {
            break;
        }
        if !cur.peek().is_some_and(is_unicode_digit) {
            cur.i = save;
            break;
        }
        cur.take_while(is_unicode_digit);
    }
    Some(cur.c[start..cur.i].iter().collect())
}

/// One attempt of `auto_parse_sentence`'s pattern at `pos`; returns the end
/// index of the match.
///
/// ```text
/// (?: [$€£¥₹₽₩₺]\s* NUM (?:\s*[kKmMbBtT][nN]?)?
///   | NUM (?:\s*(?: °[CF]? | % | [$€£¥₹₽₩₺] | [A-Z]{3} | [a-zA-Z µ]+ ))?
/// )
/// ```
/// Alternation is ordered, so `[A-Z]{3}` wins over `[a-zA-Z]+` — which is why
/// `"45USDX"` matches only `"45USD"` and leaves the `X` behind.
fn match_quantity_at(chars: &[char], pos: usize) -> Option<usize> {
    // Alternative 1 — currency symbol prefix.
    {
        let mut cur = Cursor {
            c: chars,
            i: pos,
        };
        if cur.peek().is_some_and(is_currency_symbol) {
            cur.i += 1;
            cur.skip_ws();
            if eat_num(&mut cur).is_some() {
                let save = cur.i;
                cur.skip_ws();
                if cur.eat_if(|c| "kKmMbBtT".contains(c)) {
                    cur.eat_if(|c| c == 'n' || c == 'N');
                } else {
                    cur.i = save;
                }
                return Some(cur.i);
            }
        }
    }
    // Alternative 2 — number with an optional trailing unit/currency token.
    {
        let mut cur = Cursor {
            c: chars,
            i: pos,
        };
        if eat_num(&mut cur).is_some() {
            let save = cur.i;
            cur.skip_ws();
            let matched = if cur.peek() == Some('\u{b0}') {
                cur.i += 1;
                cur.eat_if(|c| c == 'C' || c == 'F');
                true
            } else if cur.peek() == Some('%') || cur.peek().is_some_and(is_currency_symbol) {
                // `% | [$€£¥₹₽₩₺]` — both consume exactly one character.
                cur.i += 1;
                true
            } else if (0..3).all(|k| cur.at(k).is_some_and(|c| c.is_ascii_uppercase())) {
                cur.i += 3;
                true
            } else if cur
                .peek()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '\u{b5}')
            {
                cur.take_while(|c| c.is_ascii_alphabetic() || c == '\u{b5}');
                true
            } else {
                false
            };
            if !matched {
                cur.i = save;
            }
            return Some(cur.i);
        }
    }
    None
}

/// `converters.auto._format_number`
///
/// ```python
/// if isinstance(value, float) and value.is_integer(): return str(int(value))
/// return str(value)
/// ```
///
/// A `Decimal` is written plainly, without trailing fraction zeros or an
/// exponent: `12.50` → `12.5`, `12345.00` → `12345` — what the float this
/// used to be printed.
///
/// A float is written positionally too: Python's `str()` writes `1e-07`, which
/// has no place in rewritten text (#202).
fn format_number(value: &W2nValue) -> String {
    if let W2nValue::Dec(d) = value {
        return d.normalized().to_plain_string();
    }
    if let W2nValue::Float(f) = value {
        // float.is_integer() is False for inf/nan.
        if f.is_finite() && f.fract() == 0.0 {
            if *f == 0.0 {
                return "0".to_string(); // int(-0.0) == 0
            }
            if let Some(i) = BigInt::from_f64(*f) {
                return i.to_string();
            }
        }
    }
    value.text_str()
}

/// `converters.auto._IRREGULAR_PLURALS`
fn irregular_plural(long_form: &str) -> Option<&'static str> {
    Some(match long_form {
        "foot" => "feet",
        "inch" => "inches",
        "pound sterling" => "pounds sterling",
        "degree celsius" => "degrees celsius",
        "degree fahrenheit" => "degrees fahrenheit",
        "Swiss franc" => "Swiss francs",
        "Canadian dollar" => "Canadian dollars",
        "Australian dollar" => "Australian dollars",
        "Mexican peso" => "Mexican pesos",
        "US dollar" => "US dollars",
        _ => return None,
    })
}

/// `converters.auto._UNCOUNTABLE`
fn is_uncountable(long_form: &str) -> bool {
    matches!(long_form, "yen" | "yuan" | "won" | "kelvin" | "percent")
}

/// Port of `converters.auto.pluralize`.
pub fn pluralize(long_form: Option<&str>, value: &W2nValue) -> Option<String> {
    let long_form = long_form?;
    // `if value in (1, -1, 1.0, -1.0)` — membership compares with ==.
    if value.is_unit_magnitude() {
        return Some(long_form.to_string());
    }
    if is_uncountable(long_form) {
        return Some(long_form.to_string());
    }
    if let Some(p) = irregular_plural(long_form) {
        return Some(p.to_string());
    }
    if long_form.ends_with('s')
        || long_form.ends_with('x')
        || long_form.ends_with('z')
        || long_form.ends_with("ch")
        || long_form.ends_with("sh")
    {
        return Some(format!("{}es", long_form));
    }
    let chars: Vec<char> = long_form.chars().collect();
    if chars.last() == Some(&'y') && chars.len() >= 2 && !"aeiou".contains(chars[chars.len() - 2]) {
        let stem: String = chars[..chars.len() - 1].iter().collect();
        return Some(format!("{}ies", stem));
    }
    Some(format!("{}s", long_form))
}

/// `converters.auto._format_quantity`
fn format_quantity(q: &Quantity, expand: bool) -> String {
    let Some(unit) = q.unit.as_deref() else {
        return format_number(&q.value);
    };
    if expand {
        let label = pluralize(q.unit_long.as_deref(), &q.value);
        // "{} {}".format(x, None) renders the literal "None".
        return format!(
            "{} {}",
            format_number(&q.value),
            label.unwrap_or_else(|| "None".to_string())
        );
    }
    // Short form: tight glue for percent and bare degree.
    if unit == "%" || unit == "\u{b0}" {
        return format!("{}{}", format_number(&q.value), unit);
    }
    format!("{} {}", format_number(&q.value), unit)
}

/// Port of `converters.auto.auto_parse_sentence`.
///
/// Walks free text and rewrites every quantity expression in place. A match
/// that `auto_parse` rejects with `Words2NumError` is left verbatim. The
/// language and the separator overrides are checked once, up front, so an
/// unsupported `lang` or `thousands_sep == decimal_sep` raises instead of
/// leaving every span untouched (#188, #204).
pub fn auto_parse_sentence(
    text: &str,
    lang: &str,
    prefer: &HashMap<String, String>,
    thousands_sep: Option<&str>,
    decimal_sep: Option<&str>,
    expand: bool,
) -> Result<String, W2nError> {
    resolve_lang(lang)?;
    crate::w2n_formats::check_separators(thousands_sep, decimal_sep)
        .map_err(|e| W2nError::Words2Num(e.0))?;
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        match match_quantity_at(&chars, i) {
            // The pattern can never match empty (both alternatives need at
            // least one character), so no zero-width-match handling is needed.
            Some(end) if end > i => {
                let raw: String = chars[i..end].iter().collect();
                match auto_parse(&raw, lang, prefer, thousands_sep, decimal_sep) {
                    Ok(q) => out.push_str(&format_quantity(&q, expand)),
                    Err(W2nError::Words2Num(_)) => out.push_str(&raw),
                    Err(e) => return Err(e),
                }
                i = end;
            }
            _ => {
                out.push(chars[i]);
                i += 1;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(n: i64) -> W2nValue {
        W2nValue::Int(BigInt::from(n))
    }

    /// #192: codes are case-insensitive, with `-` or `_`, and unknown
    /// region subtags fall back to the base language.
    #[test]
    fn resolve_lang_ignores_case_issue_192() {
        for (code, key) in [
            ("EN", "en"),
            ("FR", "fr"),
            ("En", "en"),
            ("EN-US", "en"),
            ("en_us", "en"),
            ("en-us", "en"),
            ("PT-BR", "pt_BR"),
            ("pt_br", "pt_BR"),
            ("SR_LATN", "sr_Latn"),
            ("sr-Latn-RS", "sr_Latn"),
            ("ZH-tw", "zh_TW"),
            ("en_Aero_FAA", "en_Aero_FAA"),
            ("en_aero_icao", "en_aero_icao"),
        ] {
            assert_eq!(resolve_lang(code).unwrap(), key, "{code}");
        }
        assert_eq!(
            words2num("one", "EN", "cardinal").unwrap(),
            crate::w2n_lang_en::W2nValue::Int(1.into())
        );
    }

    /// #197: no "first two characters" guess. Language names and unknown
    /// three-letter codes are rejected, not read as a different language.
    #[test]
    fn resolve_lang_rejects_names_and_prefixes_issue_197() {
        for code in [
            "swedish", "estonian", "kazakh", "kannada", "slovak", "latvian", "enx", "english",
            "e", "", "_en", "en__US",
        ] {
            assert!(
                matches!(resolve_lang(code), Err(W2nError::NotImplemented(_))),
                "{code} must be rejected, got {:?}",
                resolve_lang(code)
            );
        }
        // Real ISO 639-2 codes still resolve, terminological and bibliographic.
        for (code, key) in [("eng", "en"), ("fra", "fr"), ("fre", "fr"), ("deu", "de"),
                            ("ger", "de"), ("swe", "sv"), ("est", "et"), ("ENG-US", "en")] {
            assert_eq!(resolve_lang(code).unwrap(), key, "{code}");
        }
        // Every alias names a supported key and never shadows one.
        for (alias, key) in ISO_639_2_ALIASES {
            assert!(is_known_lang(key), "{alias} -> {key} is not a supported key");
            assert!(!is_known_lang(alias), "{alias} is itself a key");
        }
    }

    /// #201: an unknown `to` raises in sentence mode like in `words2num`.
    #[test]
    fn sentence_rejects_unknown_to_issue_201() {
        for (text, lang, to) in [("twenty five", "en", "bogus"), ("vingt cinq", "fr", "cardnal")] {
            assert!(
                matches!(
                    words2num_sentence(text, lang, to, false),
                    Err(W2nError::NotImplemented(ref m)) if m.contains("unsupported")
                ),
                "{to}"
            );
        }
        for to in CONVERTER_TYPES {
            assert!(words2num_sentence("twenty five", "en", to, false).is_ok(), "{to}");
        }
    }

    #[test]
    fn resolve_lang_rules() {
        assert_eq!(resolve_lang("en").unwrap(), "en");
        assert_eq!(resolve_lang("en-US").unwrap(), "en"); // dash + prefix
        assert_eq!(resolve_lang("zh").unwrap(), "zh");
        assert!(resolve_lang("xx").is_err());
    }

    /// The accepted set is exactly the advertised set.
    ///
    /// These were two hand-maintained lists that had drifted by 52 entries.
    #[test]
    fn accepted_set_equals_advertised_set() {
        let advertised = num2words2_core::supported_lang_keys();
        for key in &advertised {
            assert!(
                resolve_lang(key).is_ok(),
                "{key} is advertised by supported_langs() but resolve_lang rejects it"
            );
        }
        assert_eq!(converter_langs().len(), advertised.len());
    }

    /// A known code must resolve to itself, never to a prefix match.
    ///
    /// `resolve_lang` falls back to the first two characters, so before the
    /// set was derived these eight were answered by an unrelated language —
    /// "ban" (Balinese) by "ba" (Bashkir), "pli" (Pali) by "pl" (Polish).
    #[test]
    fn known_codes_are_not_prefix_matched_to_another_language() {
        for code in [
            "ban", "ceb", "cnh", "fil", "kok", "miz", "pap", "pli", "sr_Latn", "uz_Cyrl",
        ] {
            assert_eq!(
                resolve_lang(code).unwrap(),
                code,
                "{code} must resolve to itself"
            );
        }
        // The fallback still works for codes that genuinely are not known.
        assert_eq!(resolve_lang("en_GB").unwrap(), "en");
        assert!(resolve_lang("zz").is_err());
    }

    #[test]
    fn converter_en_cardinal_ordinal_year() {
        let c = converter_for("en");
        assert_eq!(c.to_cardinal("forty-two").unwrap(), int(42));
        assert_eq!(c.to_ordinal("twenty-first").unwrap(), int(21));
        assert_eq!(c.to_year("nineteen ninety nine").unwrap(), int(1999));
        assert!(c.to_cardinal("minus").is_err()); // not a run head
    }

    #[test]
    fn walker_matches_python() {
        assert_eq!(
            words2num_sentence("I bought twenty-three apples.", "en", "cardinal", false).unwrap(),
            "I bought 23 apples."
        );
        assert_eq!(
            words2num_sentence(
                "In nineteen ninety nine, two thousand people came.",
                "en",
                "year",
                false
            )
            .unwrap(),
            "In 1999, 2000 people came."
        );
        // A run may not start with "minus".
        assert_eq!(
            words2num_sentence("minus forty two", "en", "cardinal", false).unwrap(),
            "minus 42"
        );
        // "point" is a valid head.
        assert_eq!(
            words2num_sentence("point five", "en", "cardinal", false).unwrap(),
            "0.5"
        );
    }

    #[test]
    fn auto_parse_currency_and_units() {
        let prefer = HashMap::new();
        let q = auto_parse("$12.50", "en", &prefer, None, None).unwrap();
        assert_eq!(q.value, W2nValue::Dec(BigDecimal::from_str("12.50").unwrap()));
        assert_eq!(q.unit.as_deref(), Some("USD"));
        assert_eq!(q.kind.as_deref(), Some("currency"));

        let q = auto_parse("forty-two kg", "en", &prefer, None, None).unwrap();
        assert_eq!(q.value, int(42));
        assert_eq!(q.unit.as_deref(), Some("kg"));

        let q = auto_parse("$5bn", "en", &prefer, None, None).unwrap();
        assert_eq!(q.value, int(5_000_000_000));

        // An unknown scale suffix is a Words2NumError, not a KeyError (#194).
        assert!(matches!(
            auto_parse("$5kn", "en", &prefer, None, None),
            Err(W2nError::Words2Num(_))
        ));
    }

    #[test]
    fn auto_parse_sentence_rewrites() {
        let prefer = HashMap::new();
        let out = auto_parse_sentence(
            "The package weighs 5kg and costs $12.50.",
            "en",
            &prefer,
            None,
            None,
            false,
        )
        .unwrap();
        assert!(out.contains("5 kg"));
        assert!(out.contains("12.5 USD"));
    }

    #[test]
    fn pluralize_rules() {
        assert_eq!(pluralize(Some("dollar"), &int(5)).as_deref(), Some("dollars"));
        assert_eq!(pluralize(Some("dollar"), &int(1)).as_deref(), Some("dollar"));
        assert_eq!(pluralize(Some("foot"), &int(5)).as_deref(), Some("feet"));
        assert_eq!(pluralize(Some("yen"), &int(5)).as_deref(), Some("yen"));
    }
}

#[cfg(test)]
mod digits_tests {
    use super::words2num_sentence as s;
    #[test]
    fn digit_sequences_concatenate_not_sum() {
        // Spoken digit-by-digit (house numbers, zips, phone-style) -> concat.
        assert_eq!(s("Two seven five", "en", "digits", false).unwrap(), "275");
        assert_eq!(
            s("Seven Eight Three One Eight Avenue", "en", "digits", false).unwrap(),
            "78318 Avenue"
        );
        assert_eq!(
            s("one eight nine zero zero one one", "en", "digits", false).unwrap(),
            "1890011"
        );
        // Multilingual via the reverse tables.
        assert_eq!(s("sieben acht drei", "de", "digits", false).unwrap(), "783");
        // Cardinals are NOT digit runs -> declined, left verbatim in digits mode
        // (handled by the cardinal converter instead), and never mis-summed.
        assert_eq!(
            s("sixty-one hundred Main Street", "en", "digits", false).unwrap(),
            "sixty-one hundred Main Street"
        );
        // The old cardinal behaviour that motivated this was "two seven five"
        // -> 14; the cardinal walk now reads the digit string itself.
        assert_eq!(s("Two seven five", "en", "cardinal", false).unwrap(), "275");
    }

    #[test]
    fn cardinal_walk_reads_years_and_digit_strings() {
        // Spoken years outgrow the cardinal reading.
        assert_eq!(s("twenty twenty five", "en", "cardinal", false).unwrap(), "2025");
        assert_eq!(s("nineteen ninety-nine", "en", "cardinal", false).unwrap(), "1999");
        assert_eq!(s("twenty oh five", "en", "cardinal", false).unwrap(), "2005");
        assert_eq!(
            s("december one twenty twenty five.", "en", "cardinal", false).unwrap(),
            "december 1 2025."
        );
        // Digit strings concatenate and keep a leading zero.
        assert_eq!(
            s("account ending in four five six seven", "en", "cardinal", false).unwrap(),
            "account ending in 4567"
        );
        assert_eq!(s("zero six one two", "en", "cardinal", false).unwrap(), "0612");
        // Valid cardinals and lone digits are untouched.
        assert_eq!(s("twenty five people", "en", "cardinal", false).unwrap(), "25 people");
        assert_eq!(s("press one", "en", "cardinal", false).unwrap(), "press 1");
        assert_eq!(s("five twenty", "en", "cardinal", false).unwrap(), "5 20");
        assert_eq!(s("two thousand twenty five", "en", "cardinal", false).unwrap(), "2025");
        // `to="year"` keeps its own pair reading.
        assert_eq!(s("twenty twenty five", "en", "year", false).unwrap(), "2025");
        // es: the bare hundred prefix opens the run.
        assert_eq!(
            s("ciento cincuenta y cuatro dólares", "es", "cardinal", false).unwrap(),
            "154 dólares"
        );
        // Tokens that already hold digits are literals: never summed or merged.
        assert_eq!(
            s("calling from 850 820 9095.", "en", "cardinal", false).unwrap(),
            "calling from 850 820 9095."
        );
        assert_eq!(s("850-820-9095", "en", "cardinal", false).unwrap(), "850-820-9095");
        assert_eq!(
            s("zip 94504 amount 426 and 93 cents", "en", "cardinal", false).unwrap(),
            "zip 94504 amount 426 and 93 cents"
        );
        assert_eq!(s("press 1 or two", "en", "cardinal", false).unwrap(), "press 1 or 2");
        assert_eq!(s("el 2 y cinco", "es", "cardinal", false).unwrap(), "el 2 y 5");
        // es: apocope "un" inside a number, never on its own.
        assert_eq!(
            s("cincuenta y un centavos", "es", "cardinal", false).unwrap(),
            "51 centavos"
        );
        assert_eq!(
            s("un mil treinta y cuatro dólares", "es", "cardinal", false).unwrap(),
            "1034 dólares"
        );
        assert_eq!(s("veintiún dólares", "es", "cardinal", false).unwrap(), "21 dólares");
        assert_eq!(
            s("un momento por favor", "es", "cardinal", false).unwrap(),
            "un momento por favor"
        );
        assert_eq!(s("una llamada", "es", "cardinal", false).unwrap(), "una llamada");
        assert_eq!(s("ciento uno", "es", "cardinal", false).unwrap(), "101");
    }

}

#[cfg(test)]
mod ordinal_tests {
    use super::{words2num, words2num_sentence};

    fn ordnum(text: &str, lang: &str) -> Option<i64> {
        match words2num(text, lang, "ordinal_num") {
            Ok(crate::w2n_lang_en::W2nValue::Int(n)) => n.to_string().parse().ok(),
            _ => None,
        }
    }

    fn ord(text: &str, lang: &str) -> Option<i64> {
        match words2num(text, lang, "ordinal") {
            Ok(crate::w2n_lang_en::W2nValue::Int(n)) => n.to_string().parse().ok(),
            _ => None,
        }
    }

    /// Inflected ordinal forms (ordA) meet the gender forms (forms): a cardinal
    /// run whose last word is also an ordinal form stays a cardinal, and a
    /// number before a plural ordinal is a fraction kept in words.
    #[test]
    fn cardinal_runs_and_fractions_next_to_ordinal_forms() {
        let s = |t: &str, l: &str| words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(s("tři sta korun", "cs"), "300 korun"); // "sta" is also "stá" (100th)
        assert_eq!(s("dvě stě korun", "cs"), "200 korun");
        assert_eq!(s("две стотни", "bg"), "две стотни"); // two hundredths
        assert_eq!(s("две хиляди", "bg"), "2000");
    }

    #[test]
    fn ordinal_table_has_no_minus_first_issue_62() {
        assert_eq!(ord("üheksas", "et"), Some(9));
        assert_eq!(ord("kesepuluh", "ms"), Some(10));
    }

    #[test]
    fn ordinal_num_prefix_hyphen_is_not_a_sign_issue_63() {
        assert_eq!(ordnum("ika-1", "fil"), Some(1));
        assert_eq!(ordnum("ika-21", "tl"), Some(21));
        assert_eq!(ordnum("we-100", "zu"), Some(100));
        assert_eq!(ordnum("ka-21", "ban"), Some(21));
        assert_eq!(ordnum("ke-21", "ms"), Some(21));
        assert_eq!(ordnum("-5th", "en"), Some(-5));
    }

    #[test]
    fn ordinal_num_reads_grouped_figures_issue_64() {
        assert_eq!(ordnum("1,000th", "en"), Some(1000));
        assert_eq!(ordnum("1.000.", "de"), Some(1000));
        assert_eq!(ordnum("1 000e", "fr"), Some(1000));
        assert_eq!(ordnum("1.000.000th", "en"), Some(1000000));
        assert_eq!(ordnum("2.5th", "en"), None);
        assert_eq!(ordnum("covid19", "en"), None);
        assert_eq!(ordnum("we21", "xh"), Some(21)); // xh glues its marker
        assert_eq!(ordnum("第21", "ja"), Some(21));
        assert_eq!(ordnum("21.", "sr_Latn"), Some(21));
    }

    #[test]
    fn ordinal_num_reads_latin_roman_numerals_issue_127() {
        assert_eq!(ordnum("XXI", "la"), Some(21));
        assert_eq!(ordnum("MMXXIV", "la"), Some(2024));
        assert_eq!(ordnum("IIII", "la"), None);
        assert_eq!(ordnum("XXI", "en"), None);
    }

    #[test]
    fn ordinal_num_reads_the_word_forms_ar_bn_render_issue_246() {
        assert_eq!(ordnum("الأول", "ar"), Some(1));
        assert_eq!(ordnum("الأول والعشرون", "ar"), Some(21));
        assert_eq!(ordnum("একুশতম", "bn"), Some(21));
        assert_eq!(ordnum("প্রথম", "bn"), Some(1));
        assert_eq!(ordnum("twenty-first", "en"), None); // en renders figures
    }

    #[test]
    fn ordinals_above_the_table_issue_125() {
        assert_eq!(ord("dix mille unième", "fr"), Some(10001));
        assert_eq!(ord("douze mille trois cent quarante-cinquième", "fr"), Some(12345));
        assert_eq!(ord("zwanzigtausendste", "de"), Some(20000));
        assert_eq!(ord("millonésimo", "es"), Some(1000000));
    }

    #[test]
    fn compound_ordinal_spoken_in_parts_issue_70() {
        let s = |t: &str, l: &str| words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(s("vigésimo cuarto piso", "es"), "24º piso");
        assert_eq!(s("el vigésimo segundo piso", "es"), "el 22º piso");
        assert_eq!(s("kırk ikinci", "tr"), "42'nci");
        assert_eq!(s("venti quattresimo", "it"), "24º");
        assert_eq!(s("al douăzeci și unulea", "ro"), "al 21-lea");
        assert_eq!(s("dwadzieścia pierwszy", "pl"), "dwadzieścia pierwszy");
        assert_eq!(s("двадцать первый день", "ru"), "двадцать первый день");
        // Not a compound where the language does not build one that way.
        assert_eq!(s("les vingt premiers", "fr"), "les 20 premiers");
        assert_eq!(s("los veinte primeros", "es"), "los 20 primeros");
    }

    #[test]
    fn agreed_ordinal_keeps_its_ending_in_figures_issue_153() {
        let s = |t: &str, l: &str| words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(s("पहली बार", "hi"), "१ली बार");
        assert_eq!(s("पहला दिन", "hi"), "१ला दिन");
        assert_eq!(s("પહેલી વાર", "gu"), "૧લી વાર");
        assert_eq!(s("ikinciye geldi", "tr"), "2'nciye geldi");
        assert_eq!(s("нэгдүгээр анги", "mn"), "1 дүгээр анги");
        // Nothing is appended to figures that end in punctuation.
        let one = num_bigint::BigInt::from(1);
        assert_eq!(super::agree_figures("hu", &one, "elsőt", "1.".to_string()), "1.");
    }
}

#[cfg(test)]
mod digit_mode_tests {
    use super::words2num_sentence as s;
    use crate::w2n_lang_en::W2nValue;
    use num_bigint::BigInt;

    fn digits(text: &str, lang: &str) -> Option<BigInt> {
        match super::words2num(text, lang, "digits") {
            Ok(W2nValue::Int(n)) => Some(n),
            _ => None,
        }
    }

    #[test]
    fn hyphenated_digit_words() {
        // #50: Wolof 6-9 are one hyphenated word each.
        assert_eq!(digits("ñaar juróom-ñaar juróom", "wo"), Some(BigInt::from(275)));
        assert_eq!(digits("juróom-ñeent juróom-ñett", "wo"), Some(BigInt::from(98)));
        assert_eq!(digits("one-two-three", "en"), Some(BigInt::from(123)));
    }

    #[test]
    fn multi_token_digit_words() {
        // #106: lus/miz zero is "a awmlo".
        assert_eq!(digits("a awmlo", "lus"), Some(BigInt::from(0)));
        assert_eq!(digits("pakhat a awmlo a awmlo", "miz"), Some(BigInt::from(100)));
    }

    #[test]
    fn double_triple_and_leading_zeros() {
        // #190: "double"/"triple" in to="digits"; #195: the int drops leading
        // zeros, the sentence keeps them.
        assert_eq!(digits("double zero seven", "en"), Some(BigInt::from(7)));
        assert_eq!(digits("triple five o one", "en"), Some(BigInt::from(55501)));
        assert_eq!(digits("double", "en"), None);
        assert_eq!(s("call double zero seven", "en", "digits", false).unwrap(), "call 007");
        assert_eq!(s("zero zero seven", "en", "digits", false).unwrap(), "007");
        assert_eq!(s("zéro six un deux", "fr", "digits", false).unwrap(), "0612");
    }

    #[test]
    fn long_runs_do_not_overflow() {
        // #215: a run longer than i64 is still read.
        let nines = vec!["neuf"; 25].join(" ");
        assert_eq!(digits(&nines, "fr"), Some("9".repeat(25).parse().unwrap()));
    }

    #[test]
    fn regional_english_sentences() {
        // #193
        assert_eq!(s("born in nineteen ninety nine", "en_IN", "cardinal", false).unwrap(), "born in 1999");
        assert_eq!(s("a hundred dollars", "en_IN", "cardinal", false).unwrap(), "100 dollars");
        assert_eq!(s("two point five million", "en_NG", "cardinal", false).unwrap(), "2.5 million");
        assert_eq!(s("point five", "en_IN", "cardinal", false).unwrap(), "0.5");
        assert_eq!(s("double zero seven", "en_NP", "cardinal", false).unwrap(), "007");
        assert_eq!(s("one lakh people", "en_IN", "cardinal", false).unwrap(), "100000 people");
    }

    #[test]
    fn stacked_and_juxtaposed_in_sentences() {
        // #57, #58
        assert_eq!(s("a thousand million people", "en", "cardinal", false).unwrap(), "1000000000 people");
        assert_ne!(s("rooms one hundred one hundred and two", "en", "cardinal", false).unwrap(), "rooms 10102");
    }
}

/// The October 2026 audit fixes to `auto_parse` / `auto_parse_sentence`.
#[cfg(test)]
mod auto_parse_audit_tests {
    use super::*;

    fn q(text: &str, lang: &str) -> Result<Quantity, W2nError> {
        auto_parse(text, lang, &HashMap::new(), None, None)
    }
    fn sent(text: &str) -> String {
        auto_parse_sentence(text, "en", &HashMap::new(), None, None, false).unwrap()
    }
    fn dec(s: &str) -> W2nValue {
        W2nValue::Dec(BigDecimal::from_str(s).unwrap())
    }
    fn int(n: i64) -> W2nValue {
        W2nValue::Int(BigInt::from(n))
    }

    /// #71: digit decimals are exact and scale suffixes shift the point.
    #[test]
    fn digit_decimals_are_exact() {
        assert_eq!(q("$4.1bn", "en").unwrap().value, int(4_100_000_000));
        assert_eq!(q("$1.005k", "en").unwrap().value, int(1005));
        assert_eq!(q("$1.2345k", "en").unwrap().value, dec("1234.5"));
        assert_eq!(sent("cost $4.1bn total"), "cost 4100000000 USD total");
        assert_eq!(sent("$12345678901234567.89"), "12345678901234567.89 USD");
        assert_eq!(sent("Pay $12.50 now"), "Pay 12.5 USD now");
    }

    /// #194: "n" spellings of the large scales, and no KeyError.
    #[test]
    fn scale_suffix_spellings() {
        assert_eq!(q("$5Mn", "en").unwrap().value, int(5_000_000));
        assert_eq!(q("$5TN", "en").unwrap().value, int(5_000_000_000_000));
        assert_eq!(q("$5BN", "en").unwrap().value, int(5_000_000_000));
        assert!(matches!(q("$5kn", "en"), Err(W2nError::Words2Num(_))));
        assert_eq!(sent("it cost $5kn today"), "it cost $5kn today");
    }

    /// #53: a dot decimal survives a comma-decimal locale.
    #[test]
    fn dot_decimal_in_comma_locale() {
        assert_eq!(q("3.14 m", "fr").unwrap().value, dec("3.14"));
        assert_eq!(q("2.5%", "fr").unwrap().value, dec("2.5"));
    }

    /// #54: a hyphenated locale uses its own separators.
    #[test]
    fn hyphenated_locale() {
        let v = q("1.234,56 \u{20ac}", "pt-BR").unwrap();
        assert_eq!(v.value, dec("1234.56"));
        assert_eq!(v.unit.as_deref(), Some("EUR"));
    }

    /// #188: identical separator overrides raise, even in sentence mode.
    #[test]
    fn identical_separators_raise() {
        let p = HashMap::new();
        assert!(matches!(
            auto_parse("$1,5", "en", &p, Some(","), Some(",")),
            Err(W2nError::Words2Num(_))
        ));
        assert!(matches!(
            auto_parse_sentence("pay $1,5 now", "en", &p, Some(","), Some(","), false),
            Err(W2nError::Words2Num(_))
        ));
    }

    /// #204: an unsupported language raises NotImplementedError up front.
    #[test]
    fn unknown_lang_raises() {
        let p = HashMap::new();
        assert!(matches!(q("$5", "xx"), Err(W2nError::NotImplemented(_))));
        assert!(matches!(q("forty", "xx"), Err(W2nError::NotImplemented(_))));
        assert!(matches!(
            auto_parse_sentence("forty two kg", "xx", &p, None, None, false),
            Err(W2nError::NotImplemented(_))
        ));
    }

    /// #203: `prefer` swaps the whole unit, and only for a real reading.
    #[test]
    fn prefer_swaps_the_unit() {
        let mut p = HashMap::new();
        p.insert("m".to_string(), "minute".to_string());
        let v = auto_parse("5m", "en", &p, None, None).unwrap();
        assert_eq!(v.unit.as_deref(), Some("min"));
        assert_eq!(v.unit_long.as_deref(), Some("minute"));
        assert_eq!(v.kind.as_deref(), Some("time"));
        assert_eq!(
            auto_parse_sentence("ran 5m", "en", &p, None, None, false).unwrap(),
            "ran 5 min"
        );
        p.insert("m".to_string(), "mile".to_string());
        assert_eq!(auto_parse("5m", "en", &p, None, None).unwrap().unit.as_deref(), Some("mi"));
        // Not a reading of "m": ignored, the metre stands.
        for target in ["kg", "bogus"] {
            p.insert("m".to_string(), target.to_string());
            let v = auto_parse("5m", "en", &p, None, None).unwrap();
            assert_eq!(v.unit.as_deref(), Some("m"));
            assert_eq!(v.unit_long.as_deref(), Some("meter"));
            assert_eq!(v.kind.as_deref(), Some("length"));
        }
    }
}

/// Sentence walker fixes from the October 2026 audit, one test per issue.
#[cfg(test)]
mod audit_tests {
    use super::words2num_sentence as s;

    fn c(text: &str, lang: &str) -> String {
        s(text, lang, "cardinal", false).unwrap()
    }

    #[test]
    fn issue_51_two_digit_words_are_a_range() {
        assert_eq!(c("two three minutes", "en"), "2 3 minutes");
        assert_eq!(c("deux trois minutes", "fr"), "2 3 minutes");
        assert_eq!(c("zero six", "en"), "06");
        assert_eq!(c("four five six", "en"), "456");
    }

    #[test]
    fn issue_68_digit_tokens_never_join_a_run() {
        assert_eq!(c("nineteen 90", "en"), "19 90");
        assert_eq!(c("born twenty 1990", "en"), "born 20 1990");
        assert_eq!(c("cinq 5", "fr"), "5 5");
    }

    #[test]
    fn issue_79_oh_alone_is_a_word() {
        assert_eq!(c("oh no", "en"), "oh no");
        assert_eq!(c("I said oh", "en"), "I said oh");
        assert_eq!(c("room oh seven", "en"), "room 07");
    }

    /// 2000 number tokens that never merge: each start used to re-parse a run
    /// growing to the end of the text, O(n^3).
    #[test]
    fn issue_186_long_runs_are_linear() {
        for (lang, word) in [("de", "zweitausend"), ("it", "duemila"), ("es", "mil"), ("en", "nineteen ninety")] {
            let text = vec![word; 2000].join(" ");
            let start = std::time::Instant::now();
            let out = c(&text, lang);
            assert!(!out.is_empty());
            assert!(
                start.elapsed().as_secs_f64() < 20.0,
                "{lang}: 2000 tokens took {:?}",
                start.elapsed()
            );
        }
    }

    #[test]
    fn issue_74_77_78_179_180_aliases() {
        assert_eq!(c("Около трёх тысяч человек.", "ru"), "Около 3000 человек.");
        assert_eq!(c("około trzech tysięcy osób", "pl"), "około 3000 osób");
        assert_eq!(c("دفعت ثلاثة وعشرين يورو", "ar"), "دفعت 23 يورو");
        assert_eq!(c("ba mươi tư", "vi"), "34");
        assert_eq!(c("hai nghìn không trăm hai mươi", "vi"), "2020");
        assert_eq!(c("δεκαοχτώ", "el"), "18");
        assert_eq!(c("tres kroner", "da"), "60 kroner");
        assert_eq!(c("pětadvacet korun", "cs"), "25 korun");
        assert_eq!(c("șaișpe", "ro"), "16");
        // The cardinal API reads them too; the renderer's own spelling wins.
        assert!(matches!(super::words2num("трёх", "ru", "cardinal"), Ok(crate::w2n_lang_en::W2nValue::Int(v)) if v == 3.into()));
    }

    #[test]
    fn issue_75_tr_spaced_hundreds() {
        assert_eq!(c("üç yüz bin beş yüz", "tr"), "300500");
        assert_eq!(c("Ev üç yüz bin lira.", "tr"), "Ev 300000 lira.");
    }

    #[test]
    fn issue_73_76_184_affixes() {
        assert_eq!(c("on ikide", "tr"), "12'de");
        assert_eq!(c("yüzde elli", "tr"), "yüzde 50");
        assert_eq!(c("Háromkor találkozunk.", "hu"), "3-kor találkozunk.");
        assert_eq!(c("Tulen viidellä bussilla.", "fi"), "Tulen 5:llä bussilla.");
        assert_eq!(c("נולד באלף תשע מאות תשעים ותשע", "he"), "נולד ב-1999");
        assert_eq!(c("البيت بثلاثمائة ألف دولار", "ar"), "البيت ب300000 دولار");
        assert_eq!(c("le train d'onze heures", "fr"), "le train d'11 heures");
        assert_eq!(c("l'un d'entre eux", "fr"), "l'un d'entre eux");
    }

    #[test]
    fn issue_177_unspaced_scripts() {
        assert_eq!(c("我买了三个苹果。", "zh"), "我买了3个苹果。");
        assert_eq!(c("家は三百万円です。", "ja"), "家は3000000円です。");
        assert_eq!(c("ฉันจ่ายยี่สิบสามบาท", "th"), "ฉันจ่าย23บาท");
        assert_eq!(c("我们一起去", "zh"), "我们一起去");
        assert_eq!(c("สามารถ", "th"), "สามารถ");
    }

    #[test]
    fn issue_178_ko_native_numerals() {
        assert_eq!(c("사과 세 개를 샀어요.", "ko"), "사과 3 개를 샀어요.");
        assert_eq!(c("그녀는 서른다섯 살이에요.", "ko"), "그녀는 35 살이에요.");
        assert_eq!(c("하나 둘 셋", "ko"), "1 2 3");
        assert_eq!(c("네, 알겠습니다", "ko"), "네, 알겠습니다");
        assert_eq!(c("집은 삼만원이에요", "ko"), "집은 30000원이에요");
    }

    #[test]
    fn issue_202_no_exponent_in_text() {
        assert_eq!(c("zero point zero zero zero zero zero zero one", "en"), "0.0000001");
        let prefer = std::collections::HashMap::new();
        assert_eq!(
            super::auto_parse_sentence("0.0000001 kg", "en", &prefer, None, None, false).unwrap(),
            "0.0000001 kg"
        );
    }
}
