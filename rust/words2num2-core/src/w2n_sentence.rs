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

/// [`lexicon::alias_rewrite`] for the rest of the crate: declined forms and
/// spelling variants rewritten to the renderer's (ar "خمسين" -> "خمسون").
pub(crate) fn alias_rewrite(lang: &str, norm: &str) -> Option<String> {
    lexicon::alias_rewrite(lang, norm)
}

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
    // The fullwidth marks too, which end a run as their ASCII forms do (#1026).
    let mark = |c: char| {
        matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'' | '\u{2019}')
            || matches!(c, '\u{FF0C}' | '\u{FF01}' | '\u{FF1F}' | '\u{FF1B}' | '\u{FF1A}')
            || more_terminal(c)
            || folded_mark(c).is_some()
            || matches!(c, '\u{FF02}' | '\u{FF07}' | '\u{FF63}' | '\u{FE5A}' | '\u{FE5C}' | '\u{FE5E}' | '\u{FF08}') // ＂ ＇ ｣ ﹚ ﹜ ﹞ （
            || crate::is_clause_punctuation(c)
    };
    // Format characters ride with a mark before them ("twenty,\u{200b}"),
    // and only then: a bare ZWSP is a word-break hint, as inside unspaced
    // th/zh/ja numbers (#870, #998).
    let bare = s.trim_end_matches(is_format_char);
    if bare.len() < s.len() && bare.ends_with(mark) {
        return bare.trim_end_matches(|c: char| mark(c) || is_format_char(c));
    }
    s.trim_end_matches(mark)
}

/// The trailing punctuation of `s` without its format characters.
fn trailing_mark(s: &str) -> String {
    trailing_punct(s).chars().filter(|c| !is_format_char(*c)).collect()
}

/// The word before a run is left as written because a mark opens it
/// (")two"), but the run would continue its number ("thousand twenty four"
/// after ")two"): the run stays as written too, no split value (#1112,
/// #1114). Walks back over at most 4 words to the mark-led one; walking the
/// words without the mark decides whether they all read as one number.
fn continues_punct_led(
    lang: &str,
    to: &str,
    has_kwargs: bool,
    parts: &[String],
    i: usize,
    run: &str,
    after: &[String],
    out: &str,
) -> bool {
    // A part of format characters alone is skipped like a space ("two
    // hundred \u{200b} five", #1130).
    let blank = |t: &str| py_str_isspace(t) || t.chars().all(is_format_char);
    // A pause comma cut off as its own part ("dollars，"): passed over as
    // the ASCII comma glued to the word is; the joined walk decides.
    const PAUSE: [char; 10] = ['\u{FF0C}', '\u{3001}', '\u{FF64}', '\u{FE50}', '\u{FE51}', '\u{FE10}', '\u{FE11}', '\u{060C}', '\u{104A}', '\u{055D}'];
    let pause = |t: &str| !t.is_empty() && t.chars().all(|c| PAUSE.contains(&c) || is_format_char(c)) && t.chars().any(|c| PAUSE.contains(&c));
    let mut k = i;
    let mut found = None;
    for _ in 0..5 {
        let Some(p) = (0..k).rev().find(|&q| !blank(&parts[q]) && !pause(&parts[q])) else { break };
        let raw = &parts[p];
        // Format characters alone in front are no mark ("\u{200b}a").
        if raw.trim_start_matches(is_format_char).starts_with(|c: char| !c.is_alphanumeric()) {
            found = Some(p);
            break;
        }
        if raw.chars().any(|c| c.is_ascii_digit()) || has_trailing_mark(raw.trim_end_matches(is_format_char).trim_end_matches(',').trim_end_matches(PAUSE)) {
            break;
        }
        k = p;
    }
    found.is_some_and(|p| {
        let raw = &parts[p];
        let core = raw.trim_start_matches(|c: char| !c.is_alphanumeric());
        let joined = format!("{}{}{}", core, parts[p + 1..i].concat(), run);
        // Without the mark the walker reads it all as one number, not two;
        // the walk, not the converter, so spoken years and digit strings
        // ("nineteen eighty four", "oh five") count too (#1119).
        // The words between were left as written too ("-one 2 three" is
        // not joined across the 2).
        // The number opens at the mark-led word, or inside it (ar "وخمسة" ->
        // "و55", ceb "ika-usa libo" -> "ika-1000"): zh "，第" is the prefix
        // 第 alone, no number word, and "第21" stays as the walk reads it
        // (#1122, #1143).
        let is_one = |w: &str| {
            let w = w.trim();
            let lead: String = w.chars().take_while(|c| !c.is_ascii_digit()).collect();
            !w.contains(char::is_whitespace)
                && w.len() > lead.len()
                && (!lead.chars().any(char::is_alphabetic) || {
                    // Letters only: the walk may write a hyphen after the
                    // prefix (id "ke-21", he "ו-25", #1151).
                    let letters = |t: &str| t.chars().filter(|c| c.is_alphabetic()).flat_map(char::to_lowercase).collect::<String>();
                    let (lead, core) = (letters(&lead), letters(core));
                    core.len() > lead.len() && core.starts_with(&lead)
                })
        };
        if core.is_empty() || has_trailing_mark(raw) || !out.ends_with(&parts[p..i].concat()) {
            return false;
        }
        let Ok(walked) = words2num_sentence_walk(&joined, lang, to, has_kwargs) else { return false };
        if is_one(&walked) {
            return true;
        }
        // currency=True: one amount once folded ("5 dollars and 50" ->
        // "$5.50", fr "20 euros 50" -> "20,50 €", #1123).
        // The word after the run is read too, as the fold reads it: "fifty
        // cents" is a subunit, "fifty people" is not (#181).
        CURRENCY_FOLD.with(|c| {
            c.borrow().as_deref().is_some_and(|cur| {
                let tail: String = after.iter().take(2).map(String::as_str).collect();
                // Through the currency pieces as the pass walks them, so the
                // article-one before a unit is its count ("a dollar and
                // fifty cents").
                // With a mark the walk reads past ("-", "(\u{200b}") in
                // front, so the pieces rewrite a word only as the pass itself
                // does ("-a dollar" keeps its "a", #1162); the walk then reads
                // the words without it.
                // A mark the walk keeps the word behind (")a", "#a") needs
                // none: that word stays as written either way.
                // Probed with the run's own first word: "-fifty" reads as -50,
                // "#fifty" and "\u{200b})fifty" stay as written (#1163).
                let lead_raw = &raw[..raw.len() - core.len()];
                let probe = format!("{}{}", lead_raw, run.split_whitespace().next().unwrap_or(""));
                let read_past = !lead_raw.is_empty()
                    && words2num_sentence_walk(&probe, lang, to, has_kwargs).is_ok_and(|w| w != probe);
                let lead = if read_past { lead_raw } else { "" };
                let text = format!("{}{}{}", lead, joined, tail);
                let mut walked = String::new();
                for (piece, walk) in crate::w2n_currency::pieces_for_walk(&text, cur, matches!(to, "cardinal" | "year" | "currency")) {
                    let piece = if walked.is_empty() { piece.strip_prefix(lead).map(str::to_string).unwrap_or(piece) } else { piece };
                    if !walk {
                        walked.push_str(&piece);
                        continue;
                    }
                    let Ok(w) = words2num_sentence_walk(&piece, lang, to, has_kwargs) else { return false };
                    walked.push_str(&w);
                }
                let folded = crate::w2n_currency::fold_currency(&walked, cur);
                // The walked words open with the figure: no word before it
                // ("we paid 50 dollars", "it's 50 dollars" are no amount
                // opened by the mark-led word, #1157).
                let lead_ok = walked
                    .split(|c: char| c.is_ascii_digit())
                    .next()
                    .is_some_and(|pre| !pre.chars().any(char::is_alphabetic));
                folded != walked && lead_ok && folded.split_whitespace().filter(|t| t.chars().any(|c| c.is_ascii_digit())).count() == 1
            })
        })
    })
}

/// Does the token end in a real mark, not only invisible format characters?
/// A bare ZWSP is a word-break hint inside unspaced th/zh/ja numbers
/// ("สองร้อย\u{200b}ห้าสิบ" is 250), no end of the run (#998).
fn has_trailing_mark(s: &str) -> bool {
    !trailing_mark(s).is_empty()
}

/// An invisible format character (ZWSP, ZWNJ, ZWJ, word joiner, BOM, soft
/// hyphen): it rides with the punctuation around it, so "twenty,\u{200b} one"
/// still ends its number at the comma and keeps the ZWSP (#870).
/// The bidi marks LRM, RLM and ALM, common after punctuation in ar / fa /
/// he text, count as such too (#1000).
pub(crate) fn is_format_char(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{2060}' | '\u{FEFF}' | '\u{00AD}' | '\u{061C}')
}

/// The piece opens with a bracket, quote or other mark `normalize` drops
/// ("(five)", "«six»", "‘seven’"). Re-emitting the value would lose it, so the
/// walker leaves such a piece as written.
fn opens_with_punct(s: &str) -> bool {
    // Format characters in front do not hide the mark: "\u{200e})five" is
    // left as written like ")five" (#1147, #1148).
    s.trim_start_matches(is_format_char).chars().next().is_some_and(|c| {
        crate::is_clause_punctuation(c)
            || matches!(c, '\u{2018}' | '\u{201B}' | '`' | '_')
            // A compat closer or connector, like its ASCII form: "x ）fifty y"
            // stays as written, as "x )fifty y" does (#1103).
            || matches!(folded_mark(c), Some(Mark::Closing | Mark::Connector))
    })
}

/// `re.search(r"[\.,;:!\?\"']+$", s)` → `m.group()`, or `""`.
///
/// The leftmost match of `[...]+$` is exactly the maximal trailing run, plus
/// the bare format characters the parse ignores: "five\u{200b}" is re-emitted
/// as "5\u{200b}", not "5" (#1005).
///
/// Not when a date or phone pass reads the walked text after: those match
/// bare figures, so there the characters go as before (#1020).
fn trailing_punct(s: &str) -> &str {
    let kept = rstrip_punct(s);
    let kept = if BARE_FORMAT_KEPT.with(|c| c.get()) { kept.trim_end_matches(is_format_char) } else { kept };
    &s[kept.len()..]
}

/// A clause mark that a spaced language cuts off the front of a word
/// (",deux", "!five"): the word after it reads as a number (#1041).
pub(crate) fn is_clause_lead(c: char) -> bool {
    tokens::is_leading_clause_mark(c)
}

/// A glued sign with format characters after it ("-\u{200e}twenty five"):
/// they go back after the sign of the figure, "-\u{200e}25", not lost
/// (#1126).
fn sign_format(first: &str, text: &str) -> String {
    // Format characters before the sign went out with lead_format (#1139).
    let first = first.trim_start_matches(is_format_char);
    let mut chars = first.chars();
    let (Some(sign), Some(c)) = (chars.next(), chars.next()) else { return text.to_string() };
    // The other forms read as a glued minus too: fullwidth, small,
    // superscript, subscript, the hyphens and the figure dash (#1083 audit,
    // #1158 audit). A glued plus is never absorbed into the figure.
    if !matches!(sign, '-' | '+' | '\u{2010}'..='\u{2012}' | '\u{2212}' | '\u{207B}' | '\u{208B}' | '\u{FE58}' | '\u{FE63}' | '\u{FF0D}') || !is_format_char(c) || !text.starts_with(['-', '+', '\u{2212}']) {
        return text.to_string();
    }
    let fmt: String = first[sign.len_utf8()..].chars().take_while(|c| is_format_char(*c)).collect();
    let k = text.chars().next().map_or(0, char::len_utf8);
    format!("{}{}{}", &text[..k], fmt, &text[k..])
}

/// The format characters before a run's first word, written back before its
/// value: "x \u{200b}five y" is "x \u{200b}5 y" (#1000). Not before a date or
/// phone pass, as for those after a value (#1020).
fn lead_format(first: &str) -> String {
    if BARE_FORMAT_KEPT.with(|c| c.get()) {
        first.chars().take_while(|c| is_format_char(*c)).collect()
    } else {
        String::new()
    }
}

thread_local! {
    /// The aero (spelling-alphabet) variants: a format-only token between two
    /// digit words ends a dictation there, as their digit words concatenate
    /// (#1089, #1092). Elsewhere a phone number reads across it (#1083 audit).
    static AERO_GAPS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn aero_gaps() -> bool {
    AERO_GAPS.with(|c| c.get())
}

thread_local! {
    /// Set to the resolved language while the `currency=True` pass walks its
    /// pieces: the closer-led join reads the joined words as the currency
    /// fold will, so "five dollars and fifty" counts as one amount (#1123).
    static CURRENCY_FOLD: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

thread_local! {
    static BARE_FORMAT_KEPT: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Run `f` with bare format characters after a value re-emitted, or not;
/// restored on return, panic included.
fn with_bare_format<R>(on: bool, f: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            BARE_FORMAT_KEPT.with(|c| c.set(self.0));
        }
    }
    let _restore = Restore(BARE_FORMAT_KEPT.with(|c| c.replace(on)));
    f()
}

/// `re.search(r"[\.,;:!\?]$", tok)` — note this class has **no** quote
/// characters, unlike the strip above: `forty-two"` does not close a run.
///
/// Also the script-native sentence and clause marks, which end a run as "."
/// and "," do: my "တစ်ရာ။ ငါး" is 100, then 5, never 105 (#1023).
fn ends_with_terminal_punct(s: &str) -> bool {
    matches!(
        s.trim_end_matches(is_format_char).chars().next_back(),
        Some(
            '.' | ',' | ';' | ':' | '!' | '?'
                | '\u{104B}' | '\u{104A}' // my ။ ၊
                | '\u{0964}' | '\u{0965}' // । ॥
                | '\u{060C}' | '\u{061B}' | '\u{061F}' | '\u{06D4}' // ، ؛ ؟ ۔
                | '\u{3002}' | '\u{3001}' | '\u{FF0C}' | '\u{FF01}' | '\u{FF1F}' | '\u{FF1B}' | '\u{FF1A}' // 。、，！？；：
                | '\u{1362}' | '\u{1363}' | '\u{1364}' // ። ፣ ፤
                | '\u{17D4}' | '\u{17D5}' // ។ ៕
        )
    ) || s.trim_end_matches(is_format_char).chars().next_back().is_some_and(more_terminal)
}

/// The word ends a run: a terminal mark or a closing straight quote. The
/// look-ahead tests use it too, so a run neither reads nor joins across
/// the quote ("x \"iyirmi\" birinci y" is "x \"20\" 1-ci y", #1133).
fn ends_run_mark(s: &str) -> bool {
    ends_with_terminal_punct(s) || ends_with_close_quote(s)
}

/// The kind of punctuation a compatibility form (fullwidth, small, vertical,
/// superscript, Greek ";") stands for, read from its NFKC fold: "︕" is a
/// "!", "⁾" a ")", "﹙" a "(" (#1094, #1096, #1099).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Mark {
    /// Folds to . , ; : ! ? 、 。 or a run of dots: ends a run.
    Terminal,
    /// Folds to a closing bracket or quote.
    Closing,
    /// Folds to an opening bracket.
    Opening,
    /// Folds to "_" or "`": cut off either side, like "_" (#1102 F2).
    Connector,
}

pub(crate) fn folded_mark(c: char) -> Option<Mark> {
    use unicode_normalization::UnicodeNormalization;
    // ASCII and letters are handled by the explicit lists; the ellipsis "…"
    // stays a trailing mark that does not end a run.
    if c.is_ascii() || c.is_alphanumeric() || c == '\u{2026}' {
        return None;
    }
    // Only these blocks hold compatibility forms of ASCII or CJK punctuation;
    // skip the NFKC fold for everything else (#1102 F4).
    let cp = c as u32;
    if !matches!(cp, 0x037E | 0x2024..=0x2025 | 0x203C..=0x2049 | 0x207D..=0x208E | 0x2329..=0x232A | 0xFE10..=0xFE6F | 0xFF00..=0xFFEF) {
        return None;
    }
    let f: String = std::iter::once(c).nfkc().collect();
    if f.is_empty() || f == c.to_string() {
        return None;
    }
    let all = |set: &str| f.chars().all(|x| set.contains(x));
    if all(".,;:!?\u{3001}\u{3002}") {
        Some(Mark::Terminal)
    } else if all(")]}\"'\u{300D}\u{300F}\u{3011}\u{300B}\u{3015}\u{3009}\u{3017}\u{3019}\u{301B}") {
        Some(Mark::Closing)
    } else if all("([{\u{300C}\u{300E}\u{3010}\u{300A}\u{3014}\u{3008}\u{3016}\u{3018}\u{301A}") {
        Some(Mark::Opening)
    } else if all("_`") {
        Some(Mark::Connector)
    } else {
        None
    }
}

/// An opening bracket or quote, ASCII, typographic or a compatibility form.
/// Glued before an amount or a date ("(fifty dollars)", "«quinze euros»",
/// "［fifth of may") it goes back before the folded form; glued after a
/// currency word ("dollars(") it goes back after it, not lost (#1104,
/// #1105, #1106).
pub(crate) fn is_opener(c: char) -> bool {
    "([{\"'¿¡«‹“‘„‚「『【《〔〈".contains(c) || folded_mark(c) == Some(Mark::Opening)
}

/// Does `c` fold to dots only ("．", "﹒", "︙")? Such a mark before a figure
/// reads as a decimal point.
pub(crate) fn folds_to_dots(c: char) -> bool {
    use unicode_normalization::UnicodeNormalization;
    let f: String = std::iter::once(c).nfkc().collect();
    !f.is_empty() && f.chars().all(|x| x == '.')
}

/// The word closes a straight quote ("\"twenty\"", "twenty'"): the run
/// ends there, as at a closing bracket, so the quote is not lost inside a
/// longer number ("x \"twenty\" three y" is "x \"20\" 3 y", #1120). A
/// format character before the quote ("twenty\u{200b}\"") does not hide it.
fn ends_with_close_quote(s: &str) -> bool {
    let t = s.trim_end_matches(is_format_char);
    t.ends_with(['"', '\''])
        && t.trim_end_matches(['"', '\''])
            .trim_end_matches(is_format_char)
            .chars()
            .next_back()
            // A combining mark or the tsheg ends a word too: ml "ഇരുപത്\""
            // (virama), bo "གཅིག་\"" (#1150).
            .is_some_and(|c| c.is_alphanumeric() || unicode_normalization::char::is_combining_mark(c) || matches!(c, '\u{0F0B}' | '\u{0F0C}'))
}

/// More sentence and clause marks that end a run and stay in the output:
/// ‼ ‥ ⁇ ⁈ ⁉, halfwidth ｡ ､, small-form ﹐ ﹒ ﹔ ﹖ ﹗, ․, hy ։ ՝, am ፧ (#1080).
fn more_terminal(c: char) -> bool {
    folded_mark(c) == Some(Mark::Terminal) || matches!(
        c,
        '\u{203C}' | '\u{2025}' | '\u{2047}' | '\u{2048}' | '\u{2049}' | '\u{FF61}' | '\u{FF64}'
            | '\u{FE50}' | '\u{FE52}' | '\u{FE54}' | '\u{FE56}' | '\u{FE57}' | '\u{2024}'
            | '\u{0589}' | '\u{055D}' | '\u{1367}'
            // NFKC-folded forms of . ; ! ? , : (#1088): fullwidth ．, Greek ;,
            // vertical ︐ ︑ ︒ ︓ ︔ ︕ ︖, small ﹕.
            | '\u{FF0E}' | '\u{037E}' | '\u{FE10}'..='\u{FE16}' | '\u{FE55}' | '\u{FE51}' | '\u{FE30}'
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
                crate::w2n_forms::is_number_part(lang, &norm)
                    || crate::w2n_forms::is_bound_form(lang, &norm)
                    || crate::w2n_forms::is_glued_morpheme(lang, &norm)
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
            // "-oh", "minus zero": a signed zero is no digit, its sign would
            // be dropped (#956).
            let norm = crate::normalize(word);
            if word.trim_start().starts_with(['-', '+', '\u{2212}']) || crate::strip_sign(self.n2w_key(), &norm).is_some() {
                return None;
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
            // "seven-two": a hyphen between two digit words. A leading or
            // trailing one is a sign or a dash, never dropped (#697).
            if read.is_none() && tok.contains('-') && tok.split('-').all(|t| !t.is_empty()) {
                let subs: Vec<&str> = tok.split('-').collect();
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
    // pt "vinte é cinco" is two numbers: "é" (is) is no connector "e" (#411).
    let conns = crate::w2n_forms::connectors(lang);
    if text.split_whitespace().any(|w| {
        let n = crate::normalize(w);
        conns.contains(&n.as_str()) && folded_letter(w, &n)
    }) {
        return Err(W2nError::Words2Num(unparseable_msg(text)));
    }
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
            // One sign: "moins -5" is no -(-5) (#698).
            if rest.trim_start().starts_with(['-', '+', '\u{2212}']) {
                return Err(W2nError::Words2Num(unparseable_msg(text)));
            }
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

/// Is `norm`, or what is left of it after its first words (an ordinal
/// prefix: ha "na miliyan goma"), a spelling the renderer gives two values?
fn ambiguous_stem(key: &str, norm: &str) -> bool {
    let words: Vec<&str> = norm.split_whitespace().collect();
    (0..words.len().saturating_sub(1)).any(|k| crate::is_ambiguous_number(key, &words[k..].join(" ")))
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
    // th "หมื่นห้า": 15 000, the digit one place below the bare scale word (#875).
    if let Some((false, v)) = crate::elided_place(lang, norm).filter(|_| !ordinal) {
        return Ok(W2nValue::Int(BigInt::from(v)));
    }
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
/// May the spaced words of `norm` be read glued? Only when every one is
/// itself a number word or part: fr "d eux" is no "deux", vi "ba y" no "bảy",
/// nl "twee de" no "tweede" (#285).
fn despace_ok(lang: &str, norm: &str) -> bool {
    let conns = crate::w2n_forms::connectors(lang);
    norm.split_whitespace().all(|p| {
        conns.contains(&p)
            || crate::token_may_be_number(lang, p)
            || crate::w2n_forms::is_number_part(lang, p)
            || crate::w2n_forms::is_bound_form(lang, p)
            || crate::w2n_forms::is_glued_morpheme(lang, p)
            || apocope_word(lang, p).is_some()
    })
}

/// `norm` with each tens morpheme glued to the word before it, every other
/// space kept: et "kaks kümmend kolm" -> "kakskümmend kolm", es "veinti
/// tres" -> unchanged pairs glued forward. `None` when no piece is one.
fn morpheme_glued(lang: &str, norm: &str) -> Option<String> {
    let toks: Vec<&str> = norm.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    let mut any = false;
    let mut k = 0;
    while k < toks.len() {
        let t = toks[k];
        if crate::w2n_forms::is_glued_morpheme(lang, t) {
            any = true;
            match out.last_mut() {
                // A suffix morpheme (fi "kymmentä") joins the unit before it;
                // a prefix one (es "veinti") the unit after it.
                Some(prev)
                    if !crate::w2n_forms::is_glued_morpheme(lang, prev)
                        && k > 0
                        && crate::w2n_forms::morpheme_follows_unit(lang, t) == Some(true) =>
                {
                    prev.push_str(t)
                }
                _ if k + 1 < toks.len() => {
                    out.push(format!("{t}{}", toks[k + 1]));
                    k += 1;
                }
                _ => out.push(t.to_string()),
            }
        } else {
            out.push(t.to_string());
        }
        k += 1;
    }
    any.then(|| out.join(" "))
}

/// Is the token at `j` (cleaned `clean`) a word the next token, a morpheme
/// that follows its unit (fi "kymmentä"), glues onto?
fn unit_of_next_morpheme(key: &str, parts: &[String], j: usize, clean: &str) -> bool {
    let norm = crate::normalize(clean);
    if norm.contains(' ') || crate::w2n_forms::is_glued_morpheme(key, &norm) || has_trailing_mark(&parts[j]) {
        return false;
    }
    let Some(next) = parts[j + 1..].iter().find(|p| !py_str_isspace(p)) else { return false };
    let next = crate::normalize(&rstrip_punct(next).to_lowercase());
    // Only when the two glue into a number: cs "pět desát" is no 50
    // ("padesát"), fi "sata kymmentä" no number at all (#993).
    crate::w2n_forms::morpheme_follows_unit(key, &next) == Some(true)
        && crate::lookup_norm(key, &format!("{norm}{next}"), false, &[]).ok().flatten().is_some()
}

/// my counters, currencies and units: a "နှစ်" right before one is the
/// number two, never the noun "year" ("ငါးရာ နှစ် ကျပ်" is 502 kyat, #1015).
const MY_COUNTERS: &[&str] = &[
    "ယောက်", "ဦး", "ပါး", "ကောင်", "ခု", "လုံး", "စင်း", "စီး", "ချောင်း", "ပြား", "ထည်", "အုပ်",
    "ခွက်", "ပုလင်း", "ဘူး", "အိမ်", "ဆောင်", "ပင်", "ပွင့်", "ရွက်", "မျိုး", "ကြိမ်", "ခါ", "ချပ်",
    "စု", "ဖွဲ့", "ရက်", "လ", "ပတ်", "နာရီ", "မိနစ်", "စက္ကန့်", "ကျပ်", "ကျပ်သား", "ဒေါ်လာ",
    "ယူရို", "ပိဿာ", "ကီလို", "မီတာ", "ပေ", "လက်မ", "မိုင်",
];

/// Is `next` (the word after a my "နှစ်") a counter, currency or unit?
fn my_counter(next: Option<&str>) -> bool {
    next.is_some_and(|w| {
        let w = crate::normalize(w);
        MY_COUNTERS.iter().any(|c| crate::normalize(c) == w)
    })
}

/// my: is `clean` the standalone "နှစ်" (year) after a number that is not
/// whole ten-thousands, with no counter after it and not among the digits
/// after the point "ဒသမ" ("ငါး ဒသမ နှစ်" is 5.2, #1018) (see the walker, #876)?
fn my_year_counter(resolved: &str, clean: &str, so_far: Option<&W2nValue>, next: Option<&str>, run: &[String]) -> bool {
    if !resolved.starts_with("my") || crate::normalize(clean) != crate::normalize("နှစ်") || my_counter(next) {
        return false;
    }
    let point = crate::normalize("ဒသမ");
    if run.iter().any(|t| crate::normalize(rstrip_punct(t)) == point) {
        return false;
    }
    match so_far {
        Some(W2nValue::Int(v)) => v % BigInt::from(10_000) != BigInt::from(0),
        Some(_) => true,
        None => false,
    }
}

fn base_convert_as_written(lang: &str, norm: &str, ordinal: bool) -> Result<W2nValue, W2nError> {
    let norm = norm.to_string();
    let supported = crate::is_supported_lang(lang);
    if !ordinal && supported {
        if let Some(canon) = canonical_apocopes_norm(lang, &norm) {
            // The sign is already off: es "menos menos un" is no -(-1) (#298).
            if let Ok(v) = base_convert_unsigned(lang, &canon, false) {
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
            if norm.contains(' ') && despace_ok(lang, &norm) {
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
            if norm.contains(' ') && despace_ok(lang, &norm) {
                let despaced: String = norm.split_whitespace().collect();
                if let Ok(Some(v)) =
                    crate::lookup_norm(lang, &despaced, false, &[])
                {
                    return Ok(W2nValue::Int(BigInt::from(v)));
                }
                // Only the morpheme glued, the renderer's own spaces kept:
                // et "kaks kümmend kolm" is "kakskümmend kolm" (#983).
                if let Some(v) = morpheme_glued(lang, &norm).and_then(|g| crate::lookup_norm(lang, &g, false, &[]).ok().flatten()) {
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
/// The separators and the region-aware format key `auto_parse` reads its
/// figures with, for the paths that fall back to the word reader.
struct FigureFormat<'a> {
    key: &'a str,
    thousands_sep: Option<&'a str>,
    decimal_sep: Option<&'a str>,
}

fn call_words2num(text: &str, lang: &str, fmt: &FigureFormat) -> Result<W2nValue, W2nError> {
    let resolved = resolve_lang(lang)?;
    // A sign before a figure, and the ar "،" decimal, read as words2num reads
    // them, with the caller's region and separators: fr "moins 1.234" is
    // -1234, ar "-، 50" is -0.50 (#896).
    let fmt_key = fmt.key;
    let to_value = |v: crate::w2n_formats::W2nValue| match v {
        crate::w2n_formats::W2nValue::Int(i) => W2nValue::Int(i),
        crate::w2n_formats::W2nValue::Dec(d) => W2nValue::Dec(d),
        crate::w2n_formats::W2nValue::Float(f) => W2nValue::Float(f),
    };
    if let Some(signed) = crate::signed_figure_in(text, lang) {
        return crate::w2n_formats::parse_number_string(&signed, fmt.thousands_sep, fmt.decimal_sep, Some(fmt_key))
            .map(to_value)
            .map_err(|e| W2nError::Words2Num(e.0));
    }
    // A figure reads as a bare one does, with the caller's separators or not
    // at all: "1.5 km" with decimal_sep="," is no float 1.5 (#952), de
    // "−1.234 kg" is -1234 kg as "−1.234" is. Any script's digits and any
    // minus the library reads (#976).
    let figure = text.trim();
    let caller_sep = |c: char| [fmt.thousands_sep, fmt.decimal_sep].iter().flatten().any(|s| s.contains(c));
    let figure_char =
        |c: char| {
            is_unicode_digit(c)
                || ".,'_ +-\u{2019}\u{00A0}\u{202F}\u{2009}".contains(c)
                || crate::w2n_formats::is_minus(c)
                || caller_sep(c)
        };
    if figure.chars().any(is_unicode_digit) && figure.chars().all(figure_char) {
        let parsed = crate::w2n_formats::parse_number_string(figure, fmt.thousands_sep, fmt.decimal_sep, Some(fmt_key))
            .map(to_value)
            .map_err(|e| W2nError::Words2Num(e.0));
        // With no separators of the caller's, a shape the figure parser
        // declines ("2024.", ".5") reads as words2num reads it (#979).
        let explicit = fmt.thousands_sep.is_some_and(|t| !t.is_empty()) || fmt.decimal_sep.is_some_and(|d| !d.is_empty());
        if parsed.is_ok() || explicit {
            return parsed;
        }
    }
    let converter = converter_for(&resolved);
    if crate::w2n_decimal::has_written_mark(converter.n2w_key(), text) {
        return table_decimal(&converter, &resolved, fmt_key, text)
            .and_then(|(int, _, frac)| BigDecimal::from_str(&format!("{}.{}", int, frac)).ok())
            .map(W2nValue::Dec)
            .ok_or_else(|| W2nError::Words2Num(unparseable_msg(text)));
    }
    converter.to_cardinal(text)
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
    // The caller's region, for the decimal mark a spoken decimal writes.
    let fmt_key = crate::w2n_formats::format_key(lang, &resolved);

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
                // ar "مائة ، خمسون": the mark normalizing drops makes it a
                // decimal, never the cardinal 150 — in every mode (#371, #620).
                "cardinal" | "currency" if crate::w2n_decimal::has_written_mark(converter.n2w_key(), text) => {
                    table_decimal(&converter, &resolved, &fmt_key, text)
                        .and_then(|(int, _, frac)| BigDecimal::from_str(&format!("{}.{}", int, frac)).ok())
                        .map(W2nValue::Dec)
                        .ok_or_else(|| W2nError::Words2Num(unparseable_msg(text)))
                }
                // A dictated "خمسة ، ستة ، سبعة" (to="digits") keeps its commas
                // as separators; only the number modes refuse the mark.
                "year" | "ordinal" if crate::w2n_decimal::has_written_mark(converter.n2w_key(), text) => {
                    Err(W2nError::Words2Num(unparseable_msg(text)))
                }
                "cardinal" => base_convert(lang, text, false).or_else(|e| {
                    table_decimal(&converter, &resolved, &fmt_key, text)
                        .and_then(|(int, _, frac)| BigDecimal::from_str(&format!("{}.{}", int, frac)).ok())
                        .map(W2nValue::Dec)
                        .ok_or(e)
                }),
                // A currency amount was read above; what is left is the cardinal.
                "currency" => base_convert(lang, text, false),
                "year" => table_year(lang, text),
                "ordinal" if crate::has_postfix_scales(lang) && ambiguous_stem(lang, &crate::normalize(text)) => {
                    Err(W2nError::Words2Num(unparseable_msg(text)))
                }
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
    // Any script's digits: "twenty ２" is no 22, "one hundred ٥" no 105 (#288);
    // nor "⑤ million" 5 000 000: a numerical symbol stays as written unless
    // the caller asks for them (#949).
    token.chars().any(|c| is_unicode_digit(c) || (crate::is_compat_digit(c) && !crate::numerical_symbols()))
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
    if token.is_empty() || holds_digit(token) || tokens::has_interior_punct(token) {
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
    if token.is_empty() || holds_digit(token) || tokens::has_interior_punct(token) {
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
        includable.contains(&norm.as_str()) && !folded_letter(token, &norm)
    }
}

/// pt "é", it "è" (is) fold to the connector "e" (and) but are another word
/// (#411). Other accented connectors are the language's own (ku "û").
fn folded_letter(token: &str, norm: &str) -> bool {
    norm == "e" && matches!(token.to_lowercase().as_str(), "é" | "è" | "e\u{301}" | "e\u{300}")
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

/// The words the renderer writes a comma after inside one number (en
/// "thousand" and every larger scale, it "miliardo" and the scales above
/// it), normalized and lowercased. Elsewhere a
/// comma there separates a list: ru "две тысячи, триста" is 2000 and 300
/// (#657). Read from the renderer's own output, once per language.
fn scale_comma_words(key: &str) -> std::sync::Arc<Vec<String>> {
    static CACHE: std::sync::OnceLock<std::sync::RwLock<HashMap<String, std::sync::Arc<Vec<String>>>>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(w) = cache.read().ok().and_then(|m| m.get(key).cloned()) {
        return w;
    }
    let mut words: Vec<String> = Vec::new();
    if let Some(r) = num2words2_core::get_lang_by_key(key) {
        // Every power, singular and plural multipliers, a remainder after it.
        let samples = (3..=18u32).flat_map(|e| {
            let p = 10i64.pow(e);
            [1i64, 2, 23, 123].into_iter().filter_map(move |k| {
                k.checked_mul(p)?.checked_add(if e >= 6 { 456 * 10i64.pow(e - 3) } else { 0 } + 101)
            })
        });
        for n in samples {
            let Ok(text) = r.to_cardinal(&BigInt::from(n)) else { continue };
            for tok in text.split_whitespace().filter(|t| t.ends_with(',')) {
                let clean = rstrip_punct(tok).to_lowercase().replace('-', " ");
                // Normalized: unaccented or NFD text matches too, pt "milhoes," (#948).
                if let Some(last) = py_split_whitespace(&clean).last().map(|l| crate::normalize(l)) {
                    if !words.contains(&last) {
                        words.push(last);
                    }
                }
            }
        }
    }
    let words = std::sync::Arc::new(words);
    if let Ok(mut m) = cache.write() {
        m.insert(key.to_string(), words.clone());
    }
    words
}

/// The scale a token closes with a comma: "thousand," -> 1000, "million," ->
/// 1000000, pt "mil," -> 1000. `None` for any other token or punctuation.
fn comma_after_scale(converter: &Converter, tok: &str) -> Option<BigInt> {
    if trailing_mark(tok) != "," {
        return None;
    }
    // "\u{200b}thousand,": a format character is no part of the word (#1121).
    let clean: String = rstrip_punct(tok).to_lowercase().replace('-', " ").chars().filter(|c| !is_format_char(*c)).collect();
    let last = *py_split_whitespace(&clean).last()?;
    let words = scale_comma_words(converter.n2w_key());
    if words.is_empty() {
        return None; // the renderer writes no scale comma (ru, pl, es, ...)
    }
    let norm = crate::normalize(last);
    if !words.iter().any(|w| *w == norm) {
        return None;
    }
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
        // A token of format characters alone is no digit word: "fife \u{200b}
        // seven" is 5, then 7 (#1089, #1092).
        if aero_gaps() && tok.chars().all(is_format_char) {
            break;
        }
        // A dash at either end is a sign or punctuation, no hyphen between two
        // words: "-four five six" is no 456. A trailing one ends the run and
        // stays after the figure: "five five five- one" is "555- 1" (#956).
        let core = rstrip_punct(tok);
        if core.starts_with('-') {
            break;
        }
        let cut = core.trim_end_matches(crate::w2n_formats::is_minus);
        let ends_dash = cut.len() < core.len();
        let clean = cut.to_lowercase().replace('-', " ");
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
        if ends_with_terminal_punct(tok) || ends_dash || ends_with_close_quote(tok) {
            break;
        }
        j += 1;
    }
    words
}

/// What follows the words of a dictation run's last part: its dash, then its
/// punctuation ("three-," gives "-,").
fn run_tail(s: &str) -> &str {
    let core = rstrip_punct(s).trim_end_matches(crate::w2n_formats::is_minus);
    &s[core.len()..]
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
    // my: years and another count is no dictation; the walker reads it, nor
    // does a run open on its year (#1016).
    if my_years_shape(converter, resolved, parts, i)
        || (crate::normalize(rstrip_punct(&parts[i])) == crate::normalize("နှစ်")
            && prev_word_index(parts, i).is_some_and(|k| my_years_shape(converter, resolved, parts, k)))
    {
        return None;
    }
    while j < parts.len() && digits.len() < MAX_DIGIT_RUN_WORDS {
        let tok = &parts[j];
        if py_str_isspace(tok) {
            j += 1;
            continue;
        }
        if aero_gaps() && tok.chars().all(is_format_char) && j > 0 && py_str_isspace(&parts[j - 1]) {
            break; // a spaced format-only token ends a dictation (#1089)
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
        if ends_with_terminal_punct(tok) || ends_with_close_quote(tok) {
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

/// my: a figure before နှစ်: digits, with "," or "." between them, and no
/// leading zero ("01" is a code): "2024", "2,024", "1.5", "၁.၅" (#1039).
fn my_figure(p: &str) -> bool {
    // Digits, then ","-groups of three, then one "." and its digits.
    let (int, frac) = match p.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (p, None),
    };
    let digits = |t: &str| !t.is_empty() && t.chars().all(is_unicode_digit);
    let mut groups = int.split(',');
    let head = groups.next().unwrap_or("");
    let grouped = groups.all(|g| g.chars().count() == 3 && digits(g));
    let lead_zero = head.chars().count() > 1 && head.chars().next().and_then(digit_value) == Some(0);
    digits(head) && grouped && !lead_zero && frac.is_none_or(digits)
}

/// The index of the word before part `i`, if any.
fn prev_word_index(parts: &[String], i: usize) -> Option<usize> {
    (0..i).rev().find(|&k| !py_str_isspace(&parts[k]))
}

/// my: does part `k` open "<digit> နှစ် <digit> <counter>" or "<digit> နှစ်
/// <digit> နှစ်" (years, then a count), unmarked and with no digit word
/// before or after? "သုံး နှစ် လေး လ" is 3 years 4 months, no dictation
/// (#1016); a longer run ("ရှစ် လေး နှစ် တစ်", a phone number) stays one.
fn my_years_shape(converter: &Converter, resolved: &str, parts: &[String], k: usize) -> bool {
    if !resolved.starts_with("my") {
        return false;
    }
    let digit = |t: &str| matches!(converter.to_cardinal(&rstrip_punct(t).to_lowercase()), Ok(W2nValue::Int(v)) if v >= BigInt::from(0) && v <= BigInt::from(9));
    let year = |t: &str| crate::normalize(rstrip_punct(t)) == crate::normalize("နှစ်");
    let words: Vec<usize> = (k..parts.len()).filter(|&j| !py_str_isspace(&parts[j])).take(5).collect();
    let w = |n: usize| words.get(n).map(|&j| parts[j].as_str());
    // No digit word before; and a နှစ် right after any number word or figure
    // is that number's year, which the walker's guard claims (#1016).
    if prev_word_index(parts, k).is_some_and(|p| {
        let raw = &parts[p];
        let clean = rstrip_punct(raw).to_lowercase();
        // A symbol ("²") is no number (#958).
        !has_trailing_mark(raw)
            && !raw.chars().any(crate::is_compat_digit)
            && (digit(raw)
                || (year(&parts[k])
                    && (converter.to_cardinal(&clean).is_ok() || my_figure(&clean))))
    }) {
        return false;
    }
    let unmarked = |n: usize| words[..n].iter().all(|&j| !has_trailing_mark(&parts[j]));
    let (Some(w0), Some(w1), Some(w2)) = (w(0), w(1), w(2)) else { return false };
    // A leading zero is a code, no count of years (#1037).
    let zero = |t: &str| matches!(converter.to_cardinal(&rstrip_punct(t).to_lowercase()), Ok(W2nValue::Int(v)) if v == BigInt::from(0));
    // w0 is a digit word: no figure ("09"), no symbol ("²") (#1035 audit).
    if w0.chars().any(char::is_numeric) || !(digit(w0) && !zero(w0) && year(w1) && digit(w2)) {
        return false;
    }
    // A counter, not a currency or a unit of measure: "သုံး နှစ် လေး ကျပ်" is
    // 324 kyat (#1038).
    let unit = |t: Option<&str>| {
        t.is_some_and(|t| {
            let t = crate::normalize(rstrip_punct(t));
            ["ကျပ်", "ပြား", "ကျပ်သား", "ဒေါ်လာ", "ယူရို", "ပိဿာ", "ကီလို", "မီတာ", "ပေ", "လက်မ", "မိုင်"]
                .iter()
                .any(|u| crate::normalize(u) == t)
        })
    };
    let three = unmarked(2) && !unit(w(3)) && my_counter(w(3).map(|t| rstrip_punct(t).to_lowercase()).as_deref());
    // Not "<d> နှစ် သုည နှစ်": the walker reads no "သုည နှစ်" as years. Nor
    // when a number goes on after the last နှစ် ("… နှစ် ဒသမ ငါး", "… နှစ်
    // ထောင်"): it is a digit there (#1042).
    let number_after = w(4).is_some_and(|t| {
        converter.to_cardinal(&rstrip_punct(t).to_lowercase()).is_ok() || crate::normalize(rstrip_punct(t)) == crate::normalize("ဒသမ")
    });
    let four = !year(w2)
        && !zero(w2)
        && w(3).is_some_and(year)
        && unmarked(3)
        && (has_trailing_mark(&parts[words[3]])
            || !(number_after || my_counter(w(4).map(|t| rstrip_punct(t).to_lowercase()).as_deref())));
    three || four
}

/// zh / ja / th: written without spaces, cut into pieces by the segmenter.
fn unspaced_script(resolved: &str) -> bool {
    matches!(resolved.split(&['_', '-'][..]).next().unwrap_or(resolved), "zh" | "cn" | "ja" | "jp" | "th")
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
    fmt_key: &str,
    text: &str,
) -> Option<(String, char, String)> {
    let region_sep = crate::w2n_formats::locale_format(fmt_key).and_then(|f| f.decimal.chars().next());
    let default_sep = region_sep.unwrap_or('.');
    let cardinal = |words: &str| match converter.to_cardinal(words).ok()? {
        W2nValue::Int(i) => Some(i),
        _ => None,
    };
    crate::w2n_decimal::read(
        converter.n2w_key(),
        text,
        // The decimal word writes the region's mark, so the output reads back
        // as written: es_MX "tres coma uno cuatro uno" is "3.141" and es "tres
        // punto uno cuatro uno" "3,141", never 3141 (#951). A language with no
        // format row (af) keeps the word's own mark.
        &|w| decimal_word(resolved, w).map(|c| region_sep.unwrap_or(c)),
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
        || has_trailing_mark(raw)
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
    if ends_run_mark(&parts[end]) || *c <= BigInt::from(0) {
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
        // семью: 7 and "family", сорока: 40 and "magpie" (#433).
        "ru" => "семью сорока",
        "uk" => "сорока",
        _ => "",
    }
}

/// Does num2words2 write `v` (its ordinal, if `ordinal`) exactly as `text`,
/// hyphens read as spaces? The #285 glue readings never are: 7 is "bảy",
/// not "ba y".
fn renderer_spells(key: &str, v: &W2nValue, ordinal: bool, text: &str) -> bool {
    let W2nValue::Int(n) = v else { return false };
    let Some(r) = num2words2_core::get_lang_by_key(key) else { return false };
    let canon = |s: &str| crate::normalize(s).replace('-', " ").split_whitespace().collect::<Vec<_>>().join(" ");
    let t = canon(text);
    r.to_cardinal(n).is_ok_and(|w| canon(&w) == t) || (ordinal && r.to_ordinal(n).is_ok_and(|w| canon(&w) == t))
}

/// The run `stripped` with its last word, an ordinal (`clean`), swapped for
/// the renderer's cardinal of that ordinal's value: bm "ba tan ni sabanan"
/// -> "ba tan ni saba", normalized.
fn cardinal_stem(converter: &Converter, stripped: &str, clean: &str) -> Option<String> {
    if !converter.is_ordinal_word(clean) {
        return None;
    }
    let W2nValue::Int(v) = converter.to_ordinal(clean).ok()? else { return None };
    let r = num2words2_core::get_lang_by_key(converter.n2w_key())?;
    let card = r.to_cardinal(&v).ok()?;
    let words: Vec<&str> = stripped.split_whitespace().collect();
    let (_, head) = words.split_last()?;
    Some(crate::normalize(&format!("{} {card}", head.join(" "))))
}

/// Is `norm` a word the renderer writes in some k·10^e (ha "tiriliyan", even
/// where the reader cannot read it alone)?
fn renderer_vocab(key: &str, norm: &str) -> bool {
    static CACHE: std::sync::OnceLock<std::sync::RwLock<HashMap<String, std::sync::Arc<std::collections::HashSet<String>>>>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let words = match cache.read().ok().and_then(|m| m.get(key).cloned()) {
        Some(w) => w,
        None => {
            let mut set = std::collections::HashSet::new();
            if let Some(r) = num2words2_core::get_lang_by_key(key) {
                for e in 0..=18u32 {
                    for k in 1..=9i64 {
                        let Some(v) = k.checked_mul(10i64.pow(e)) else { continue };
                        if let Ok(t) = r.to_cardinal(&BigInt::from(v)) {
                            // bm, ff write large values in figures: no word.
                            set.extend(
                                crate::normalize(&t)
                                    .split_whitespace()
                                    .filter(|w| !w.chars().all(|c| c.is_ascii_digit()))
                                    .map(str::to_string),
                            );
                        }
                    }
                }
            }
            let set = std::sync::Arc::new(set);
            if let Ok(mut m) = cache.write() {
                m.insert(key.to_string(), set.clone());
            }
            set
        }
    };
    words.contains(norm)
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
    // ru "с семью друзьями": after "с"/"со" семью is the instrumental of 7;
    // "family" there is "семьёй".
    let prev = prev_word(parts, i).map(|p| p.to_lowercase());
    if base == "ru" && norm == crate::normalize("семью") && prev.as_deref().is_some_and(|p| p == "с" || p == "со") {
        return false;
    }
    // "около сорока человек": after a quantity word or preposition сорока is
    // the oblique case of 40, not the magpie.
    if matches!(base, "ru" | "uk") && norm == crate::normalize("сорока") {
        const RU: &[&str] = &[
            "около", "более", "менее", "свыше", "до", "из", "от", "в", "на", "о", "по", "при", "к", "с", "со",
            "порядка", "больше", "меньше", "после", "для", "среди", "между", "у", "за", "без", "спустя", "старше",
            "моложе", "младше", "выше", "ниже", "позднее", "раньше", "примерно", "почти", "приблизительно", "лет",
        ];
        const UK: &[&str] = &[
            "близько", "понад", "до", "з", "із", "від", "у", "в", "на", "по", "при", "після", "для", "серед", "між",
            "за", "без", "старше", "більше", "менше", "вище", "нижче", "майже", "приблизно", "років",
        ];
        let words = if base == "ru" { RU } else { UK };
        if prev.as_deref().is_some_and(|p| words.contains(&p)) {
            return false;
        }
    }
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

/// Is the next non-space part after `i` exactly `text` (a spaced ":")?
fn next_part_is(parts: &[String], i: usize, text: &str) -> bool {
    parts[i + 1..].iter().find(|t| !py_str_isspace(t)).is_some_and(|t| t == text)
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
    let punct = trailing_mark(&parts[i]);
    let base = resolved.split(&['_', '-'][..]).next().unwrap_or(resolved);
    // "Speaker one:", fr "Locuteur un :": a speaker label, only before its
    // colon ("a guest, one of hundreds" keeps the pronoun, #912, #1175).
    let colon = punct.starts_with(':') || next_part_is(parts, i, ":");
    let speaker = colon
        && prev.as_deref().is_some_and(|p| {
            matches!(
                p,
                "speaker" | "participant" | "interviewer" | "respondent" | "panelist" | "caller" | "guest"
                    | "voice" | "person" | "locuteur" | "intervenant" | "hablante" | "locutor"
                    | "interlocutor" | "participante" | "falante" | "parlante" | "oratore" | "relatore"
                    | "partecipante" | "interlocutore"
            )
        });
    // "three, two, one", "press one one": right after a cardinal number,
    // with no sentence mark between, it counts on when it ends its clause
    // ("trois, deux, un, partez", "three, two, one!"); "deux, un instant",
    // "two, one of them" keep the article (#911, #1176, #1177).
    let prev_raw = parts[..i].iter().rev().find(|t| !py_str_isspace(t));
    let next_raw = parts[i + 1..].iter().find(|t| !py_str_isspace(t));
    let item = !punct.is_empty() || next.is_none() || next_raw.is_some_and(|t| !t.chars().any(char::is_alphanumeric));
    let after_number = item
        && prev_raw.is_some_and(|r| matches!(trailing_mark(r).as_str(), "" | ","))
        && prev.as_deref().is_some_and(|p| !p.is_empty() && matches!(crate::lookup_norm(resolved, p, false, &[]), Ok(Some(_))))
        && !(base == "en" && next.as_deref() == Some("of"))
        // de "ein" is also a verb particle ("er schläft um zwei ein"), and
        // counts as "eins"; ro "o" is the article (#1174 audit).
        && !(base == "de" && crate::normalize(&parts[i]).trim().starts_with("ein") && crate::normalize(&parts[i]).trim() != "eins")
        && !(base == "ro" && crate::normalize(&parts[i]).trim() == "o");
    if after_number || speaker {
        return true;
    }
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

/// Words before a clock time: "at nine fifteen", "until ten thirty".
const EN_TIME_CUES: &[&str] = &[
    "at", "around", "about", "by", "until", "till", "til", "from", "before", "after", "since",
    "between",
];

/// en "nine fifteen" -> "9:15": an hour (1..=12) then its minutes (10..=59,
/// or "oh" + a digit), read as a clock time only after a cue word ("at",
/// "until"...) or before am/pm. Without one it stays two numbers ("five
/// twenty", "nine fifteen year olds"), and an hour over twelve is no time
/// ("at nineteen fifty" is the year). Returns the time and its last part.
fn en_time_run(parts: &[String], i: usize) -> Option<(String, usize)> {
    use crate::w2n_lang_en::{tens_value, unit_value};
    let words = en_number_words(parts, i);
    let word = |k: usize| words.get(k).map(|w| w.word.as_str());
    let hour = unit_value(word(0)?).filter(|h| (1..=12).contains(h))?;
    if !words[0].last {
        return None; // "nine-fifteen" is one part: not split here
    }
    let digit = |k: usize| word(k).and_then(unit_value).filter(|v| (1..=9).contains(v));
    let mut readings: Vec<(usize, i64)> = Vec::new();
    match word(1)? {
        "oh" | "o" => readings.extend(digit(2).map(|d| (3, d))),
        m => {
            if let Some(v) = unit_value(m).filter(|v| (10..=19).contains(v)) {
                readings.push((2, v));
            } else if let Some(v) = tens_value(m).filter(|v| (20..=50).contains(v)) {
                readings.push((2, v));
                readings.extend(digit(2).map(|d| (3, v + d)));
            }
        }
    }
    let (n, minutes) = readings.into_iter().rev().find(|&(n, _)| words[n - 1].last)?;
    if words.len() > n {
        return None; // more number words follow: not a time ("nine fifteen twenty")
    }
    let end = words[n - 1].part;
    let cue = prev_word(parts, i).is_some_and(|p| EN_TIME_CUES.contains(&p.as_str()));
    let next = next_word(parts, end);
    let meridiem = next.as_deref().is_some_and(|w| matches!(w, "am" | "pm" | "a.m" | "p.m"));
    // After a cue only, not when the next word goes on with the number
    // ("nine fifteen thousand", "point five", "twenty first") or counts
    // something ("nine fifty each", "five twenty five percent", "twelve
    // fifteen times") (#928, #904).
    let ends_cleanly = trailing_mark(&parts[end]).is_empty().then_some(()).map_or(true, |_| {
        // "each" / "per" count only when they end the clause ("nine fifty
        // each"); "each morning", "per the schedule" keep the time (#1192).
        let then = next_word_index(parts, end).and_then(|k| next_word(parts, k));
        let ends_after = next_word_index(parts, end).is_none_or(|k| !trailing_mark(&parts[k]).is_empty() || then.is_none());
        next.as_deref().is_none_or(|w| {
            !(crate::w2n_lang_en::is_scale_word(w)
                || (matches!(w, "each" | "per") && ends_after)
                || matches!(
                    w,
                    "hundred" | "thousand" | "point" | "percent" | "%" | "times" | "year" | "years" | "people"
                        | "dollars" | "dollar" | "cents" | "euros" | "pounds" | "pence" | "yen" | "minutes"
                        | "seconds" | "hours" | "days" | "weeks" | "months" | "items" | "units" | "grams"
                        | "kilograms" | "kg" | "miles" | "kilometers" | "kilometres" | "km" | "meters" | "metres"
                        | "feet" | "inches" | "pounds-force" | "litres" | "liters" | "children" | "men"
                        | "women" | "students" | "pieces" | "kilos"
                )
                // Any currency name ("rupees", "francs") (#1194), not a
                // homograph or name ("pound the table", "Penny") or "p" of a
                // spaced "p m" (#1197 audit).
                || (crate::w2n_currency::is_currency_word("en", w)
                    && !crate::w2n_currency::time_homograph(w)
                    && !(w == "p" && then.as_deref().is_some_and(|t| t == "m" || t == "m.")))
                // "twenty first": the minutes and the ordinal are one ordinal;
                // "fifteen first" is not (#1190 audit).
                || (minutes % 10 == 0
                    && minutes >= 20
                    && crate::w2n_lang_en::ordinal_cardinal(w)
                        .and_then(crate::w2n_lang_en::unit_value)
                        .is_some_and(|u| (1..=9).contains(&u))))
        })
    });
    (meridiem || (cue && ends_cleanly)).then(|| (format!("{hour}:{minutes:02}"), end))
}

/// en "nineties" -> 9: the decade a decade word names, for `to="year"`.
fn en_decade_value(word: &str) -> Option<i64> {
    "twenties thirties forties fifties sixties seventies eighties nineties"
        .split_whitespace()
        .position(|d| d == word)
        .map(|p| p as i64 + 2)
}

/// The index of the next part that is not blank.
fn next_word_index(parts: &[String], i: usize) -> Option<usize> {
    (i + 1..parts.len()).find(|&k| !py_str_isspace(&parts[k]))
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
    words2num_sentence_full(sentence, lang, to, has_kwargs, currency, false, None, "written", false)
}

/// [`words2num_sentence_opts`] with the date pass: day + month (+ year) spans
/// written the way the language writes a date ("March 12, 2026", "12 March
/// 2026", "am 12. März 2026"). See [`crate::w2n_date`].
///
/// `region` ("US", "GB"...) overrides the language code's region for the
/// date order and the phone numbers; `date_format` is `"written"` (default)
/// or `"iso"` ("2026-03-12" for every date with a year). `phone` writes each
/// phone number the way its country does ("06 12 34 56 78", "(555)
/// 234-5678", "+33 6 12 34 56 78"; see [`crate::w2n_phone`]).
#[allow(clippy::too_many_arguments)]
pub fn words2num_sentence_full(
    sentence: &str,
    lang: &str,
    to: &str,
    has_kwargs: bool,
    currency: bool,
    date: bool,
    region: Option<&str>,
    date_format: &str,
    phone: bool,
) -> Result<String, W2nError> {
    let iso = match date_format {
        "written" => false,
        "iso" => true,
        other => {
            return Err(W2nError::NotImplemented(format!(
                "date_format must be 'written' or 'iso', not {}",
                py_repr_str(other)
            )))
        }
    };
    // The currency pass reads a figure with its format character (#1021).
    let folds = date || phone;
    let mut out = with_bare_format(!folds, || words2num_sentence_folded(sentence, lang, to, has_kwargs, currency))?;
    // The range mark of `share_scale_words` (#865); never in the input's own
    // text unless the caller wrote it, and then only where it is glued to a
    // connector the rewrite produced.
    if out.contains(SHARE_MARK) && !sentence.contains(SHARE_MARK) {
        out = out.replace(SHARE_MARK, "");
    }
    if !currency {
        out = fold_scale_half(&out, lang, sentence);
    }
    // The phone pass leaves a day-month-year run of figures to the date pass
    // ("né le 23 10 2026", #940), and reads a longer number whole first
    // ("06 12 2024 56", #1181).
    if phone {
        out = crate::w2n_phone::fold_phones(&out, lang, region);
    }
    Ok(if date { crate::w2n_date::fold_dates_with(&out, lang, region, iso) } else { out })
}

/// Does "<conn> <half>" end the quantity, given the half word's glued
/// `trail` and the tokens from `from` on? The end, a mark, an (elided)
/// partitive, the language's conjunction or a currency name; not a noun of
/// its own ("y medio kilo", "y medio millón", #1209) nor a set phrase ("medio
/// de comunicación"). Whitespace and format characters are no token and no
/// mark ("y medio  ambiente", "medio\t ambiente", #1207 audit).
fn half_ends(toks: &[&str], from: usize, trail: &str, lang: &str, currency_ends: bool) -> bool {
    let core = |t: &str| crate::normalize(t.trim_end_matches(|c: char| !c.is_alphanumeric()));
    let blank = |t: &str| t.chars().all(|c| c.is_whitespace() || is_format_char(c));
    let real = |from: usize| (from..toks.len()).find(|&k| !blank(toks[k]));
    if !blank(trail) {
        return true;
    }
    let Some(next_k) = real(from) else { return true };
    let n = toks[next_k];
    let lower = n.to_lowercase();
    if lower.starts_with("d'") || lower.starts_with("d\u{2019}") || lower.starts_with("d\u{2018}") || lower.starts_with("d\u{2bc}") || !n.chars().any(char::is_alphanumeric) {
        return true;
    }
    let w = core(n);
    let set_phrase = real(next_k + 1).map(|k| toks[k]).is_some_and(|x| {
        matches!(
            core(x).as_str(),
            "comunicacion" | "comunicacao" | "transporte" | "transportes" | "vida" | "pago" | "pagos" | "pagamento" | "prueba" | "prova"
                | "cultivo" | "trasporto" | "comunicazione" | "transport" | "communication"
        )
    });
    (matches!(w.as_str(), "de" | "del" | "di" | "do" | "da" | "dos" | "das" | "des" | "du" | "d") && !set_phrase)
        || matches!(w.as_str(), "y" | "e" | "et" | "i" | "o" | "ou" | "que")
        // A currency name ends a scale quantity ("dos millones y medio euros")
        // but is the next noun after an amount ("cinco euros y media libra",
        // #1212 audit).
        || (currency_ends && crate::w2n_currency::is_currency_word(lang, &w))
}

/// "cinco pesos y medio" -> "$5.50", not "$5 y medio" (#860): a spelled-out
/// whole amount, its currency word and "and a half" read as the amount and
/// a half. The figure gets ".5" before the currency pass renders it; only a
/// main unit the pass renders with two decimals (not "centavos y medio",
/// "yenes", gl, #1212 audit).
fn fold_currency_half(text: &str, lang: &str, input: &str) -> String {
    let base = lang.split(['_', '-']).next().unwrap_or(lang).to_lowercase();
    let (conn, halves): (&str, &[&str]) = match base.as_str() {
        "es" => ("y", &["medio", "media"]),
        "pt" => ("e", &["meio", "meia"]),
        "fr" => ("et", &["demi", "demie"]),
        "it" => ("e", &["mezzo", "mezza"]),
        "ca" => ("i", &["mig", "mitja"]),
        _ => return text.to_string(),
    };
    let core = |t: &str| crate::normalize(t.trim_end_matches(|c: char| !c.is_alphanumeric()));
    let currency = |t: &str| !t.is_empty() && crate::w2n_currency::is_currency_word(lang, &core(t));
    let half_at = |toks: &[&str], k: usize| {
        toks.get(k).is_some_and(|c| c.eq_ignore_ascii_case(conn)) && toks.get(k + 1).is_some_and(|h| halves.contains(&core(h).as_str()))
    };
    // Each "<currency> <conn> <half>" of the input, in order: was the amount
    // before it spelled out? A figure as written is never added to (#1208).
    let words: Vec<&str> = input.split_whitespace().collect();
    let spelled: Vec<bool> = (2..words.len())
        .filter(|&k| currency(words[k - 1]) && half_at(&words, k))
        .map(|k| !words[k - 2].chars().any(|c| c.is_numeric()))
        .collect();
    let toks: Vec<&str> = text.split(' ').collect();
    let pairs = (2..toks.len()).filter(|&k| currency(toks[k - 1]) && half_at(&toks, k)).count();
    if pairs != spelled.len() || pairs == 0 {
        return text.to_string();
    }
    let mut seen = 0;
    let mut out: Vec<String> = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i];
        if i + 2 < toks.len() && currency(toks[i + 1]) && half_at(&toks, i + 2) {
            seen += 1;
            let h = toks[i + 3];
            let trail = &h[h.trim_end_matches(|c: char| !c.is_alphanumeric()).len()..];
            // An opener the currency pass renders ("¿5", "(5") rides along;
            // any other lead ("-5", "—5") or a trail it does not render
            // ("medio€", "medio\u{200b}") would leave a raw "5.5" (#1212
            // audit).
            let figure = t.trim_start_matches(|c: char| is_opener(c));
            let trail_ok = trail.chars().all(|c| ",.;:!?)]}\"'»…”’".contains(c) || (c.is_whitespace() && !is_format_char(c)));
            let whole = !figure.is_empty() && figure.len() <= 15 && figure.bytes().all(|b| b.is_ascii_digit());
            let unit = toks[i + 1];
            let plain_unit = unit.trim_end_matches(|c: char| !c.is_alphanumeric()).len() == unit.len();
            if spelled[seen - 1]
                && trail_ok
                && whole
                && plain_unit
                && crate::w2n_currency::half_renders(lang, &core(unit))
                && half_ends(&toks, i + 4, trail, lang, false)
            {
                // The mark after "medio." goes after the amount.
                out.push(format!("{t}.5"));
                out.push(format!("{unit}{trail}"));
                i += 4;
                continue;
            }
        }
        out.push(t.to_string());
        i += 1;
    }
    out.join(" ")
}

/// "un millón y medio" -> 1500000, not "1000000 y medio" (#859): a figure
/// that is a whole number of millions (or more) followed by "and a half" in
/// the language takes half of its largest scale.
fn fold_scale_half(text: &str, lang: &str, input: &str) -> String {
    let base = lang.split(['_', '-']).next().unwrap_or(lang).to_lowercase();
    let (conn, halves): (&str, &[&str]) = match base.as_str() {
        "es" => ("y", &["medio"]),
        "gl" => ("e", &["medio"]),
        "pt" => ("e", &["meio"]),
        "fr" => ("et", &["demi"]),
        "it" => ("e", &["mezzo"]),
        "ca" => ("i", &["mig"]),
        _ => return text.to_string(),
    };
    let core = |t: &str| crate::normalize(t.trim_end_matches(|c: char| !c.is_alphanumeric()));
    // Only after a scale word the input spelled out: "1000000 y medio" as
    // written stays (#1208). Each "<conn> <half>" of the input, in order:
    // was a scale word before it?
    const SCALES: &[&str] = &[
        "millon", "millones", "billon", "billones", "milhao", "milhoes", "bilhao", "bilhoes", "biliao", "bilioes", "million", "millions",
        "milliard", "milliards", "billion", "billions", "milione", "milioni", "miliardo", "miliardi", "bilione", "bilioni", "milio",
        "milions", "bilio", "bilions", "trilhao", "trilhoes", "billiard", "billiards", "biliardo", "biliardi",
    ];
    let words: Vec<&str> = input.split_whitespace().collect();
    let spelled: Vec<bool> = (1..words.len().saturating_sub(1))
        .filter(|&k| core(words[k]) == conn && halves.contains(&core(words[k + 1]).as_str()))
        .map(|k| SCALES.contains(&core(words[k - 1]).as_str()) && words[k - 1].trim_end_matches(|c: char| c.is_alphanumeric()).is_empty())
        .collect();
    let toks: Vec<&str> = text.split(' ').collect();
    // The same pairs in the output; if the counts disagree, fold nothing.
    let pairs = (1..toks.len().saturating_sub(1)).filter(|&k| toks[k].eq_ignore_ascii_case(conn) && halves.contains(&core(toks[k + 1]).as_str())).count();
    if pairs != spelled.len() {
        return text.to_string();
    }
    let mut seen = 0;
    let mut out: Vec<String> = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i];
        if toks.get(i + 1).is_some_and(|c| c.eq_ignore_ascii_case(conn)) && toks.get(i + 2).is_some_and(|h| halves.contains(&core(h).as_str())) {
            seen += 1;
        }
        let ok = seen > 0 && spelled[seen - 1];
        let value = (!t.is_empty() && t.len() <= 18 && t.bytes().all(|b| b.is_ascii_digit())).then(|| t.parse::<u64>().ok()).flatten();
        let half = toks.get(i + 2).map(|h| {
            let core = h.trim_end_matches(|c: char| !c.is_alphanumeric());
            (core, &h[core.len()..])
        });
        // "y medio" ends the quantity: the end, a mark, or "de" (not "y medio
        // kilo", "y medio millón", where it is the next noun's, #1209).
        // A conjunction or a currency name may follow ("y medio y …", "y
        // medio euros").
        let ends = |trail: &str| half_ends(&toks, i + 3, trail, lang, true);
        match (value, toks.get(i + 1), half) {
            (Some(v), Some(&c), Some((h, trail))) if ok && ends(trail) && v > 0 && v % 1_000_000 == 0 && c.eq_ignore_ascii_case(conn) && halves.contains(&h.to_lowercase().as_str()) => {
                let mut scale = 1_000_000u64;
                while v % (scale * 1000) == 0 {
                    scale *= 1000;
                }
                out.push(format!("{}{trail}", v + scale / 2));
                i += 3;
            }
            _ => {
                out.push(t.to_string());
                i += 1;
            }
        }
    }
    out.join(" ")
}

/// "cinco o seis mil millones" -> "cinco mil millones o seis mil millones"
/// (#865): in a spelled-out alternative or range whose last bound carries the
/// scale words, every earlier bound takes the same scale words, so the walk
/// reads each bound whole. A bound is one number word below 1000 with no
/// mark ("5" as written is never multiplied, #1217); the whole run of scale
/// words is copied ("mil millones", #1218); a chain "five or six or seven
/// thousand" shares it right to left (#1219); a comma list ("cinco, seis o
/// siete mil") is left alone. Rising pairs only ("veinte o dos mil" stays).
/// Glued to a range's "and" by [`share_scale_words`], removed after the walk.
pub(crate) const SHARE_MARK: char = '\u{E000}';

pub(crate) fn share_scale_words(text: &str, lang: &str) -> Option<String> {
    let base = lang.split(['_', '-']).next().unwrap_or(lang).to_lowercase();
    let (or_words, and_words, range_cues): (&[&str], &[&str], &[&str]) = match base.as_str() {
        "en" => (&["or"], &["and"], &["between"]),
        "es" => (&["o", "u"], &["y", "e"], &["entre"]),
        "gl" => (&["ou", "o"], &["e"], &["entre"]),
        "pt" => (&["ou"], &["e"], &["entre"]),
        "fr" => (&["ou"], &["et"], &["entre"]),
        "it" => (&["o", "oppure"], &["e"], &["tra", "fra"]),
        "ca" => (&["o"], &["i"], &["entre"]),
        "de" => (&["oder"], &["und"], &["zwischen"]),
        "nl" => (&["of"], &["en"], &["tussen"]),
        _ => return None,
    };
    let core = |t: &str| crate::normalize(t);
    let scale = |t: &str| {
        t.chars().all(char::is_alphabetic)
            && matches!(
                core(t).as_str(),
                "thousand" | "mil" | "mille" | "mila" | "tausend" | "duizend" | "million" | "millions" | "millon" | "millones" | "milhao"
                    | "milhoes" | "milione" | "milioni" | "milio" | "milions" | "millionen" | "miljoen" | "miljoenen" | "billion"
                    | "billions" | "milliard" | "milliards" | "bilhao" | "bilhoes" | "miliardo" | "miliardi" | "milliarde"
                    | "milliarden" | "miljard" | "miljarden" | "millons" | "millos"
            )
    };
    let value = |t: &str| -> Option<i64> {
        if !t.chars().all(char::is_alphabetic) {
            return None;
        }
        let w = core(t);
        // "one", "un", "ein": the article form the table may leave out.
        if matches!(w.as_str(), "one" | "un" | "uno" | "una" | "um" | "uma" | "ein" | "eine" | "een" | "een" | "unha") {
            return Some(1);
        }
        match crate::lookup_norm(lang, &w, false, &[]) {
            Ok(Some(v)) if (1..1000).contains(&v) => Some(v),
            _ => None,
        }
    };
    let mut toks: Vec<String> = text.split(' ').map(str::to_string).collect();
    let mut changed = false;
    // Right to left: the scale words of a later bound reach every earlier one.
    let mut k = toks.len();
    while k > 1 {
        k -= 1;
        let conn = core(&toks[k]);
        let is_or = or_words.contains(&conn.as_str());
        let is_and = and_words.contains(&conn.as_str()) && k >= 2 && range_cues.contains(&core(&toks[k - 2]).as_str());
        if !(is_or || is_and) || !toks[k].chars().all(char::is_alphabetic) {
            continue;
        }
        // An opener may stand before the first bound ("(cinco o seis mil)").
        let (Some(a), Some(b)) = (value(toks[k - 1].trim_start_matches(is_opener)), toks.get(k + 1).and_then(|t| value(t))) else { continue };
        // The scale words right after the second bound.
        let mut j = k + 2;
        while j < toks.len() && scale(toks[j].trim_end_matches(|c: char| !c.is_alphanumeric())) {
            let last = toks[j].trim_end_matches(|c: char| !c.is_alphanumeric()).len() != toks[j].len();
            j += 1;
            if last {
                break;
            }
        }
        if j == k + 2 || a >= b {
            continue;
        }
        // A comma list ("cinco, seis o siete mil") is no pair.
        if k >= 2 && toks[k - 2].ends_with(',') && value(toks[k - 2].trim_end_matches(',')).is_some() {
            continue;
        }
        let words: Vec<String> = toks[k + 2..j].iter().map(|t| t.trim_end_matches(|c: char| !c.is_alphanumeric()).to_string()).collect();
        // "and" would join the bounds into one number ("five thousand and
        // six" is 5006): a private-use mark glued to it keeps them apart, and
        // `SHARE_MARK` is stripped after the walk.
        if is_and {
            toks[k] = format!("{SHARE_MARK}{}", toks[k]);
        }
        let at = k;
        for (n, w) in words.into_iter().enumerate() {
            toks.insert(at + n, w);
        }
        changed = true;
    }
    changed.then(|| toks.join(" "))
}

/// es "diez y seis" -> "dieciséis" (16..19, #866), or None when the text
/// holds none. Not in a list after "<number>," ("cinco, diez y seis" is two
/// numbers), not before "minutos" (a clock time), and only with a mark after
/// the unit the walk splits off; an opener before "diez" rides along (#1213
/// audit).
pub(crate) fn es_split_teens(text: &str, lang: &str) -> Option<String> {
    if lang.split(['_', '-']).next() != Some("es") {
        return None;
    }
    const CLAUSE: &str = ",.;:!?»)…%\"'”’";
    let toks: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(toks.len());
    let mut changed = false;
    let mut last_teen = false;
    let mut i = 0;
    while i < toks.len() {
        let head = toks[i];
        // Only an opener rides along ("¿diez"); "#diez", "$diez" stay (#1213
        // audit).
        let lead_len = head.len() - head.trim_start_matches(is_opener).len();
        let (lead, head_core) = head.split_at(lead_len);
        let unit = toks.get(i + 2).and_then(|u| {
            let core = u.trim_end_matches(|c: char| !c.is_alphanumeric());
            let one = match crate::normalize(core).as_str() {
                "seis" => "dieciséis",
                "siete" => "diecisiete",
                "ocho" => "dieciocho",
                "nueve" => "diecinueve",
                _ => return None,
            };
            let trail = &u[core.len()..];
            trail.chars().all(|c| CLAUSE.contains(c)).then_some((one, trail))
        });
        // A teen just read is no list head ("diez y seis, diez y siete").
        // After "<number>," it is a list ("cinco, diez y seis"); "Sí, diez y
        // seis" is the teen (#1213 audit).
        let listed = i > 0 && !last_teen && {
            let prev = toks[i - 1].trim_end_matches(',');
            toks[i - 1].ends_with(',')
                && !prev.is_empty()
                && (prev.chars().all(|c| c.is_ascii_digit())
                    || matches!(crate::normalize(prev).as_str(), "una" | "un" | "uno")
                    || matches!(crate::lookup_norm(lang, &crate::normalize(prev), false, &[]), Ok(Some(_))))
        };
        // After "las" it is a clock time: "las diez y seis minutos", and with
        // to="currency" (no clock pass) "a las diez y seis" (#1213 audit).
        let after_las = i > 0 && crate::normalize(toks[i - 1]) == "las";
        let unit_word = |n: &str| crate::normalize(n.trim_end_matches(|c: char| !c.is_alphanumeric()));
        // A mark on the unit ends the time too ("a las diez y seis. Luego").
        let unit_marked = toks.get(i + 2).is_some_and(|u| u.ends_with(|c: char| !c.is_alphanumeric()));
        let minutes = after_las
            && (unit_marked || toks.get(i + 3).is_none_or(|n| matches!(unit_word(n).as_str(), "minutos" | "minuto" | "min" | "y" | "de" | "en" | "punto")));
        match unit {
            Some((one, trail))
                // The bare word: a mark on "diez" ("diez, y seis", "…diez") is a
                // boundary or would be lost (#1213 audit).
                if head_core.chars().all(char::is_alphabetic)
                    && crate::normalize(head_core) == "diez"
                    && toks.get(i + 1).is_some_and(|y| y.eq_ignore_ascii_case("y"))
                    && !listed
                    && !minutes =>
            {
                out.push(format!("{lead}{one}{trail}"));
                changed = true;
                last_teen = true;
                i += 3;
            }
            _ => {
                out.push(head.to_string());
                last_teen = false;
                i += 1;
            }
        }
    }
    changed.then(|| out.join(" "))
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
    let resolved_lang = resolve_lang(lang)?;
    check_converter_type(to)?;
    // fr / de / es clock times, read on the words before the walk ("à neuf
    // heures quinze" -> "à 9h15"); the walk leaves the figures alone.
    let timed;
    let sentence = if matches!(to, "cardinal" | "year") && !has_kwargs {
        timed = crate::w2n_time::fold_times(sentence, lang);
        timed.as_str()
    } else {
        sentence
    };
    // es "diez y seis" .. "diez y nueve": the split teens the RAE accepts are
    // 16..19, not "10 y 6" (#866); after the clock pass ("a las diez y seis"
    // is 10:06), and only where the teen is then read (not in ordinal mode,
    // #1213 audit).
    let teens;
    let sentence = match es_split_teens(sentence, &resolved_lang).filter(|_| matches!(to, "cardinal" | "year" | "currency")) {
        Some(t) => {
            teens = t;
            teens.as_str()
        }
        None => sentence,
    };
    // "cinco o seis mil" -> "cinco mil o seis mil": the first bound of an
    // alternative or range takes the scale words of the second (#865).
    let shared;
    let sentence = match share_scale_words(sentence, &resolved_lang).filter(|_| matches!(to, "cardinal" | "currency")) {
        Some(t) => {
            shared = t;
            shared.as_str()
        }
        None => sentence,
    };
    if currency {
        let resolved = resolved_lang.clone();
        // Walked piece by piece: fr "trente-quatre cents" after "dollars et"
        // is 34 US cents, not 3400 (see `pieces_for_walk`).
        let mut out = String::new();
        // Cleared on every exit, a panic unwinding included.
        struct ClearFold;
        impl Drop for ClearFold {
            fn drop(&mut self) {
                CURRENCY_FOLD.with(|c| *c.borrow_mut() = None);
            }
        }
        CURRENCY_FOLD.with(|c| *c.borrow_mut() = Some(resolved.clone()));
        let _clear = ClearFold;
        let walked: Result<(), W2nError> = (|| {
            for (piece, walk) in crate::w2n_currency::pieces_for_walk(sentence, &resolved, matches!(to, "cardinal" | "year" | "currency")) {
                if walk {
                    out.push_str(&words2num_sentence_walk(&piece, lang, to, has_kwargs)?);
                } else {
                    out.push_str(&piece);
                }
            }
            Ok(())
        })();
        drop(_clear);
        walked?;
        // Scale-half first: the currency half consumes no pair it counts
        // (#1212 audit).
        let halved = fold_currency_half(&fold_scale_half(&out, &resolved, sentence), &resolved, sentence);
        return Ok(crate::w2n_currency::fold_currency(&halved, &resolved));
    }
    // The plain walk cuts the same subunit off: fr "douze dollars et
    // cinquante cents" is "12 dollars et 50 cents", not "… 5000" (#868).
    // The subunit is swapped for a placeholder word, not cut off, so the
    // words before it keep their right-hand context: "un euro et un centime"
    // stays "1 euro et 1 centime" (#1001).
    let resolved = resolved_lang;
    let pieces = crate::w2n_currency::subunit_pieces(sentence, &resolved, matches!(to, "cardinal" | "year" | "currency"));
    if pieces.len() == 1 {
        return words2num_sentence_walk(sentence, lang, to, has_kwargs);
    }
    // Private-use characters, unlikely in a transcript; a text that holds
    // the mark anyway takes the unmasked walk below.
    const MARK: &str = "\u{F8FF}wnsub\u{F8FF}";
    let mut masked = String::new();
    let mut kept: Vec<String> = Vec::new();
    for (piece, walk) in pieces {
        if walk {
            masked.push_str(&piece);
        } else {
            masked.push_str(MARK);
            kept.push(piece);
        }
    }
    if sentence.contains(MARK) {
        return words2num_sentence_walk(sentence, lang, to, has_kwargs);
    }
    let walked = words2num_sentence_walk(&masked, lang, to, has_kwargs)?;
    let mut out = String::new();
    let mut rest = walked.as_str();
    for word in kept {
        let Some(at) = rest.find(MARK) else { return words2num_sentence_walk(sentence, lang, to, has_kwargs) };
        out.push_str(&rest[..at]);
        out.push_str(&word);
        rest = &rest[at + MARK.len()..];
    }
    out.push_str(rest);
    Ok(out)
}

fn words2num_sentence_walk(
    sentence: &str,
    lang: &str,
    to: &str,
    has_kwargs: bool,
) -> Result<String, W2nError> {
    let resolved = resolve_lang(lang)?;
    AERO_GAPS.with(|c| c.set(resolved.to_lowercase().contains("aero")));
    let fmt_key = crate::w2n_formats::format_key(lang, &resolved);
    let converter = converter_for(&resolved);
    let postfix_scales = matches!(converter, Converter::Table(l) if crate::has_postfix_scales(l));
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
    // `to="year"` on a sentence: decades ("the nineteen nineties" -> "the
    // 1990s") and no year out of a time ("at nine fifteen" stays "at 9 15").
    let en_year = en_grammar && to == "year" && !has_kwargs;
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
    // (run start, part index): a run that met a span the renderer writes for two
    // values is regrown to stop before that span, so the number before it still
    // reads (ha "biyar miliyan goma" is "5 miliyan goma"; #796).
    let mut run_limit: Option<(usize, usize)> = None;
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
        // Nor, in a script written without spaces, does a piece opening on a
        // glued mark ("，五" of "二十，五"): the mark would be lost (#1028).
        let starts = starts_run(&converter, &head, decimal_heads)
            && !(unspaced_script(&resolved) && piece.trim_start_matches(is_format_char).starts_with(tokens::is_trailing_punct))
            && !(head_is_oh && !(en_cardinal && en_digit_run(&parts, i).is_some()));
        // kn "ನೂರ", he "שני", mn "дөчин": a number only with what follows, so it
        // opens a run only in front of another number word, and alone stays a word.
        let part_head = !starts
            && !ends_run_mark(piece)
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
        let next_starts_run = !ends_run_mark(piece)
            && next.as_deref().is_some_and(|t| starts_run(&converter, t, decimal_heads));
        // A run must START with a real number word, or with an apocope
        // ("un" in es "un mil") followed by one, or — in English — with
        // "a"/"an" in front of a scale word ("a hundred dollars").
        let apocope_head = head_is_apocope && next_starts_run;
        let article_head = (head_is_article
            && !ends_run_mark(piece)
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
                && !ends_run_mark(piece)
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
            if continues_punct_led(lang, to, has_kwargs, &parts, i, rstrip_punct(py_strip(&run)), &parts[end + 1..], &out) {
                out.push_str(&run);
                i = end + 1;
                continue;
            }
            out.push_str(sep_before(i));
            out.push_str(&lead_format(&parts[i]));
            out.push_str(&digits);
            out.push_str(if en_digits { run_tail(&run) } else { trailing_punct(&run) });
            out.push_str(sep_after(end));
            i = end + 1;
            continue;
        }

        // In a language that puts the multiplier after its scale, the words
        // after a scale word the reader cannot read ha "tiriliyan" belong to
        // that number: no run opens there, so it stays words whole (#882).
        if postfix_scales
            && prev_word(&parts, i).is_some_and(|p| {
                !is_candidate(&converter, &p, includable) && renderer_vocab(converter.n2w_key(), &crate::normalize(&p))
            })
            && is_candidate(&converter, &rstrip_punct(piece).to_lowercase(), includable)
        {
            let mut k = i;
            while k < n && (py_str_isspace(&parts[k]) || is_candidate(&converter, &rstrip_punct(&parts[k]).to_lowercase(), includable)) {
                k += 1;
                if ends_run_mark(&parts[k - 1]) {
                    break;
                }
            }
            out.push_str(&parts[i..k].concat());
            i = k;
            continue;
        }
        let limit = run_limit.filter(|&(start, _)| start == i).map_or(n, |(_, k)| k);
        let regrown = limit < n;
        // Grow a number run starting at i.
        let mut best_value: Option<W2nValue> = None;
        let mut best_end = i;
        let mut best_ordinal = false;
        let mut gap_ended = false;
        // The spoken decimal separator of a table-language decimal run.
        let mut best_sep: Option<char> = None;
        let mut j = i;
        let mut words = 0usize;
        let mut failed = 0usize;
        // The commas the run crossed after a scale word, with that scale (#67).
        let mut commas: Vec<(usize, BigInt)> = Vec::new();
        // The end of a span the renderer writes for two values (ha "miliyan
        // dubu ɗari": 1 001 100 and 1 100 000): no one number, so its words
        // stay rather than read as two (#348).
        let mut ambiguous_end: Option<usize> = None;
        let mut ambiguous_from: Option<usize> = None;
        while j < limit {
            let tok = &parts[j];
            if py_str_isspace(tok) {
                j += 1;
                continue;
            }
            let clean = rstrip_punct(tok).to_lowercase();
            // A format-only token between two single-digit words ends a
            // dictation: "fife \u{200b} seven" is 5, then 7 (#1089, #1092).
            let digit_word = |w: Option<String>| {
                w.is_some_and(|w| matches!(converter.to_cardinal(&w), Ok(W2nValue::Int(v)) if v >= BigInt::from(0) && v <= BigInt::from(9)))
            };
            // Only inside a pure dictation: every word so far a single digit,
            // so a compound or a decimal ("atus ida \u{200b} lima", "three
            // point one \u{200b} four") is never split (#1100).
            let dictation_gap = j > i
                && aero_gaps()
                && tok.chars().all(is_format_char)
                && py_str_isspace(&parts[j - 1])
                && parts.get(j + 1).is_some_and(|t| py_str_isspace(t))
                && parts[i..j]
                    .iter()
                    .filter(|t| !py_str_isspace(t))
                    .all(|t| digit_word(Some(rstrip_punct(t).to_lowercase())))
                && digit_word(next_word(&parts, j));
            if dictation_gap {
                // The digit before the gap is a digit, no article: "one
                // \u{200b} two" is "1 \u{200b} 2" (#1101).
                gap_ended = true;
            }
            if clean.is_empty()
                || dictation_gap
                || opens_with_punct(tok)
                // A piece the zh/ja/th segmenter left opening on a glued mark
                // ("，五" of "二十，五"): the run never crosses the mark (#1028).
                || (j > i && unspaced_script(&resolved) && tok.trim_start_matches(is_format_char).starts_with(tokens::is_trailing_punct))
                || tokens::is_punct_part(&clean)
                // The raw part too: "\u{200b})" cleans to a bare format
                // character, still a mark part (#1006).
                || tokens::is_punct_part(tok)
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
            // my "နှစ်" is "two" and the counter "year": after a number it is
            // the year ("အသက် နှစ်ဆယ့်တစ် နှစ်" is 21 years), unless the
            // number is whole ten-thousands, where the renderer itself spaces
            // the two ("တစ်သောင်း နှစ်" is 10002) (#876), or a counter follows
            // ("ငါးရာ နှစ် ကျပ်" is 502 kyat, #1015).
            let after = if has_trailing_mark(tok) { None } else { next_word(&parts, j) };
            if j > i && my_year_counter(&resolved, &clean, best_value.as_ref(), after.as_deref(), &parts[i..j]) {
                break;
            }
            // A morpheme that leads its unit (es "veinti", it "trent") opens a
            // new number once this run has one: "veinti tres veinti tres" is 23
            // 23, so the run stops rather than re-parse ever longer spans (#990).
            // Likewise a unit the next suffix morpheme glues onto (fi "kolme
            // kymmentä kolme kymmentä" is 30 30).
            // Only right after a complete group: a unit before a leading
            // morpheme, a trailing morpheme before a unit. kn writes its tens
            // spaced inside a larger number ("ಸಾವಿರದ ಇಪ್ಪತ್ತ್ ಐದು"), no break.
            if best_value.is_some() && j > i {
                let key = converter.n2w_key();
                let prev = parts[i..j]
                    .iter()
                    .rev()
                    .find(|p| !py_str_isspace(p))
                    .map(|p| crate::normalize(&rstrip_punct(p).to_lowercase()))
                    .unwrap_or_default();
                let prev_trailing = crate::w2n_forms::morpheme_follows_unit(key, &prev) == Some(true);
                let leads = crate::w2n_forms::morpheme_follows_unit(key, &crate::normalize(&clean)) == Some(false);
                let next_glues = !leads && unit_of_next_morpheme(key, &parts, j, &clean);
                // The parse only when a morpheme is in play (cheap checks first).
                let prev_unit = (leads || next_glues)
                    && matches!(converter.to_cardinal(&prev), Ok(W2nValue::Int(v)) if v >= BigInt::from(1) && v <= BigInt::from(9));
                // A unit then "unit + trailing morpheme" is two numbers too: fi
                // "kolme kymmentä viisi | kolme kymmentä viisi" (#999).
                if (leads && prev_unit) || ((prev_trailing || prev_unit) && next_glues) {
                    break;
                }
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
                    // Format characters are no part of the word: "\u{200b}third" is
                    // an ordinal word (#1090, #1091).
                    let bare: String = clean.chars().filter(|c| !is_format_char(*c)).collect();
                    let ordinal = plain
                        && converter.is_ordinal_word(&bare)
                        && (matches!(converter, Converter::En)
                            || converter.to_ordinal(&stripped).is_ok_and(|o| o == v));
                    Some((v, ordinal))
                }
                Err(_) if plain && !en_cardinal => {
                    // "trois virgule cinq": a decimal spoken in a table language.
                    // In running text a comma glued to a word, or any ASCII
                    // comma, separates a list: only the renderer's own spaced
                    // "،" makes a decimal here (#668).
                    let list_comma = converter.n2w_key().starts_with("ar")
                        && (stripped.contains(',')
                            || stripped.split_whitespace().any(|w| w != "\u{060C}" && w.contains('\u{060C}')));
                    let dec = (!list_comma).then(|| table_decimal(&converter, &resolved, &fmt_key, &stripped)).flatten().and_then(
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
            // A word that is no number nor number part completes a run only as
            // an ordinal ("vigésimo cuarto", it "venti quattresimo" glued) or a
            // decimal's denominator ("три целых четырнадцать сотых"). A short
            // fragment never glues onto a number: nl "twee de" is no "tweede",
            // de "zwei te" no "zweite", vi "ba y" no "bảy" (#285).
            // The renderer's own spelling always reads: eu "hiru milioi", gu
            // "વીસ છમો" (its scale and ordinal pieces are no candidates).
            let hit = hit.filter(|(v, ordinal)| {
                candidate
                    || matches!(v, W2nValue::Dec(_))
                    || converter.is_ordinal_word(&clean)
                    || (*ordinal && rstrip_punct(&clean).chars().count() >= 4)
                    || renderer_spells(converter.n2w_key(), v, *ordinal, &stripped)
            });
            // ha "na miliyan goma": the ordinal of 10^7 and of 1 000 010 alike
            // (#348). Declined, and the span stays words.
            // bm "ba tan ni sabanan": the span ends in its own ordinal word; its
            // stem is the cardinal "… ni saba" (#884 M2).
            let ambiguous = postfix_scales
                && (ambiguous_stem(converter.n2w_key(), &crate::normalize(&stripped))
                    || cardinal_stem(&converter, &stripped, &clean)
                        .is_some_and(|c| ambiguous_stem(converter.n2w_key(), &c)));
            let hit = hit.filter(|(_, ordinal)| !(*ordinal && ambiguous));
            match hit {
                // A run never ends on a connector: "one and a half" is "1 and
                // a half", not "1 half"; "five point" keeps its "point".
                // fi "viisi kymmentä kuusi kymmentä": the "kuusi" a suffix
                // morpheme follows is its unit, no end of this run (#988).
                // Nor on a part of format characters alone glued to the next
                // word (a zh/ja/th segmenter piece, "二十 \u{200e}二十"): the
                // run would swallow the space before it (#1083 audit). Nor on
                // one after a word with a mark ("\"five\" \u{200b}"): the mark
                // would be lost (#1115, #1117).
                Some((v, ordinal))
                    if candidate_is_number(&converter, &clean, includable)
                        && !(tok.chars().all(is_format_char)
                            && (parts.get(j + 1).is_some_and(|t| !py_str_isspace(t))
                                || (j > i
                                    && (!trailing_punct(parts[i..j].concat().trim_end()).is_empty()
                                        || parts[i..j]
                                            .iter()
                                            .rev()
                                            .find(|t| !py_str_isspace(t) && !t.chars().all(is_format_char))
                                            .is_some_and(|t| has_trailing_mark(t))))))
                        && !unit_of_next_morpheme(converter.n2w_key(), &parts, j, &clean) =>
                {
                    best_value = Some(v);
                    best_end = j;
                    best_ordinal = ordinal;
                    failed = 0;
                }
                Some(_) => failed += 1,
                None if !candidate => {
                    // bm "ba tan ni sabanan": the span ends in its ordinal word,
                    // which is no candidate; it starts where its cardinal stem
                    // is ambiguous ("x saba ba tan ni naaninan" reads the 3).
                    if ambiguous {
                        ambiguous_end = Some(j);
                        ambiguous_from.get_or_insert_with(|| {
                            (i..=j)
                                .filter(|&k| !py_str_isspace(&parts[k]))
                                .find(|&k| {
                                    let tail = rstrip_punct(py_strip(&parts[k..=j].concat())).to_string();
                                    cardinal_stem(&converter, &tail, &clean).is_some_and(|c| {
                                        crate::is_ambiguous_number(converter.n2w_key(), &c)
                                            && converter.convert(to, &c, has_kwargs).is_err()
                                    })
                                })
                                .unwrap_or(i)
                        });
                    }
                    break;
                }
                None => {
                    if ambiguous {
                        ambiguous_end = Some(j);
                        ambiguous_from.get_or_insert_with(|| {
                            (i..=j)
                                .filter(|&k| !py_str_isspace(&parts[k]))
                                .find(|&k| {
                                    let tail = rstrip_punct(py_strip(&parts[k..=j].concat())).to_string();
                                    // Only a span nothing reads: "dubu tamanin" (1 080 or
                                    // 80 000) reads, so it is no split point (#884 M1).
                                    crate::is_ambiguous_number(converter.n2w_key(), &crate::normalize(&tail))
                                        && converter.convert(to, &tail, has_kwargs).is_err()
                                })
                                .unwrap_or(i)
                        });
                    }
                    failed += 1
                }
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
            if ends_with_close_quote(tok) {
                break;
            }
            j += 1;
        }

        // "at nine fifteen", "nine thirty pm": a clock time, "9:15".
        if en_cardinal || en_year {
            if let Some((time, end)) = en_time_run(&parts, i) {
                out.push_str(sep_before(i));
                out.push_str(&lead_format(&parts[i]));
                out.push_str(&time);
                out.push_str(trailing_punct(&parts[end]));
                out.push_str(sep_after(end));
                i = end + 1;
                continue;
            }
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

        // The regrown prefix must read right up to the ambiguous span, or the
        // words before it stay words too: no stray "9" out of "… tara biliyan".
        if regrown {
            let last_before = (i..limit).rev().find(|&k| !py_str_isspace(&parts[k]));
            if best_value.is_none() || last_before != Some(best_end) {
                out.push_str(&parts[i..limit].concat());
                i = limit;
                continue;
            }
        }
        if let Some(mut end) = ambiguous_end.filter(|&e| e > best_end) {
            // A number before the ambiguous span still reads: regrow the run
            // to stop where the span starts (#796).
            // Not where the run follows a number word left in words: in ha the
            // multiplier comes after its scale, so "tiriliyan tara biliyan …"
            // has no "tara" of its own.
            // A scale word right before it (a comma there ends that number).
            let after_number_word = parts[..i]
                .iter()
                .rev()
                .find(|t| !py_str_isspace(t))
                .is_some_and(|raw| {
                    let p = rstrip_punct(raw).to_lowercase();
                    !has_trailing_mark(raw)
                        && (is_scale_word(&converter, &p, 1_000)
                            || (!is_candidate(&converter, &p, includable)
                                && renderer_vocab(converter.n2w_key(), &crate::normalize(&p))))
                });
            if let Some(from) = ambiguous_from.filter(|&f| f > i && !after_number_word) {
                run_limit = Some((i, from));
                continue;
            }
            // The rest of the same number stays words too: bm "ba tan ni kelen
            // ni mugan ni wolonfila" is no "ba tan ni kelen ni 27", nor its
            // ordinal word ("… ni kelennan", #798). A comma ends the number:
            // ha and bm write none inside one (#797).
            // Two unit words side by side with no connector are two numbers:
            // "ba tan ni kelen saba" ends before "saba" (#884 L1). Anything
            // else may still belong to the number, so the words stay.
            let unit = |w: &str| matches!(converter.to_cardinal(w), Ok(W2nValue::Int(v)) if v >= BigInt::from(1) && v <= BigInt::from(9));
            let mut prev_unit = unit(&rstrip_punct(&parts[end]).to_lowercase());
            let ends_ordinal = converter.is_ordinal_word(&rstrip_punct(&parts[end]).to_lowercase());
            let mut k = end + 1;
            while k < n && !has_trailing_mark(&parts[end]) && !ends_ordinal {
                let part = &parts[k];
                if py_str_isspace(part) {
                    k += 1;
                    continue;
                }
                let clean = rstrip_punct(part).to_lowercase();
                let number_word = is_candidate(&converter, &clean, includable)
                    || converter.is_ordinal_word(&clean)
                    || converter.to_ordinal(&clean).is_ok();
                if tokens::is_punct_part(part) || !number_word || (prev_unit && unit(&clean)) {
                    break;
                }
                prev_unit = unit(&clean);
                if !is_includable(&clean, includable) {
                    end = k;
                }
                // An ordinal word ends its number (bm "… ni kelennan saba").
                if converter.is_ordinal_word(&clean) {
                    break;
                }
                if has_trailing_mark(part) {
                    break;
                }
                k += 1;
            }
            out.push_str(&parts[i..=end].concat());
            i = end + 1;
            continue;
        }

        if en_year {
            // "the nineteen nineties", "the twenty twenties": the decade of that
            // century, one token ("1990s"); the cardinal walk keeps it in words.
            // A spoken century, 16 to 20 ("eighteen eighties", "twenty
            // twenties"), no ordinal and no mark before the decade word:
            // "forty twenties" are banknotes, "nineteen, nineties" two words
            // (#905, #927).
            let century = match &best_value {
                Some(W2nValue::Int(c))
                    if !best_ordinal
                        && trailing_punct(&parts[best_end]).trim_matches(is_format_char).is_empty()
                        && crate::w2n_lang_en::ordinal_cardinal(&rstrip_punct(&parts[best_end]).to_lowercase()).is_none() =>
                {
                    // 10..=21 after "the", "early", "mid", "late" ("the fourteen
                    // nineties"); 16..=20 always (#1172).
                    // Past format characters and an opening mark ("the (fifteen
                    // nineties)", "the\u{200e} fifteen nineties").
                    let cue = parts[..i]
                        .iter()
                        .rev()
                        .map(|t| t.trim_matches(is_format_char))
                        .find(|t| !t.is_empty() && !py_str_isspace(t) && t.chars().any(char::is_alphanumeric))
                        .map(|t| rstrip_punct(t.trim_start_matches(|c: char| !c.is_alphanumeric())).to_lowercase());
                    let cued = cue.is_some_and(|p| matches!(p.as_str(), "the" | "early" | "mid" | "late" | "mid-"));
                    i64::try_from(c).ok().filter(|c| (16..=20).contains(c) || (cued && (10..=21).contains(c)))
                }
                _ => None,
            };
            let decade = next_word_index(&parts, best_end)
                .and_then(|k| en_decade_value(&rstrip_punct(&parts[k]).to_lowercase()).map(|d| (k, d)));
            if let (Some(c), Some((k, d))) = (century, decade) {
                out.push_str(sep_before(i));
                out.push_str(&lead_format(&parts[i]));
                out.push_str(&format!("{c}{d}0s"));
                out.push_str(trailing_punct(&parts[k]));
                out.push_str(sep_after(k));
                i = k + 1;
                continue;
            }
            // "at nine fifteen" is a time: a year read in running text needs a
            // two-digit first half ("nineteen oh five", "twenty twenty six").
            // The single-token `words2num(..., to="year")` still reads "nine
            // eleven" as 911, the way num2words2 spells 100..999 (#111).
            let three_digit = matches!(&best_value, Some(W2nValue::Int(v)) if (BigInt::from(100)..BigInt::from(1000)).contains(v));
            // Not "nine hundred", "seven hundred fifty", "one oh one": the
            // scale word or the "oh" digit makes it a year, no time (#903,
            // #1173).
            // "oh" only in an isolated "<digit> oh <digit>": inside a longer
            // dictated string ("five five five oh one two") it groups nothing
            // (#1171 audit).
            let digit = |w: Option<String>| {
                w.is_some_and(|w| crate::w2n_lang_en::unit_value(w.trim_matches(is_format_char)).is_some_and(|u| u <= 9) || w == "zero" || w == "oh")
            };
            let oh_alone = next_word(&parts, i).is_some_and(|w| {
                let w = w.trim_matches(is_format_char);
                w == "oh" || w.starts_with("oh-")
            }) && !digit(prev_word(&parts, i))
                && !digit(next_word(&parts, best_end));
            let hundred = oh_alone
                || next_word(&parts, i).is_some_and(|w| w.trim_matches(is_format_char) == "hundred");
            if three_digit && best_end > i && !hundred && crate::w2n_lang_en::unit_value(&head).is_some_and(|u| (1..=9).contains(&u)) {
                if let Ok(v) = converter.to_cardinal(&head) {
                    best_value = Some(v);
                    best_end = i;
                }
            }
        }

        // es "ochenta y un poco más": "un" after the tens' "y" opens the
        // quantifier "a little", not 81 (#861).
        if plain && !best_ordinal && best_end > i {
            let base = resolved.split(['_', '-']).next().unwrap_or(&resolved);
            let w = |k: usize| crate::normalize(rstrip_punct(&parts[k]).trim());
            let prev = (i..best_end).rev().find(|&k| !py_str_isspace(&parts[k]));
            // es only, masculine "un": the standalone number is "uno", so "un
            // poco" is the quantifier. pt/gl/ca "um"/"un" is also the number
            // and "pouco"/"poc" also the adverb ("vinte e um pouco mudou"),
            // and "una poco" is never the quantifier (#1210 audit).
            let article = base == "es"
                && w(best_end) == "un"
                && trailing_mark(&parts[best_end]).is_empty()
                && prev.is_some_and(|k| w(k) == "y");
            // Only on positive evidence: "un poco" / "um pouco" is the
            // quantifier, never a count of it. A noun, a verb or a pronoun
            // after it is as often the compound's ("trinta e um passaram",
            // "vinte e um anos", "treinta y un km"), and pt "um" is also the
            // pronoun (#1210 audit).
            let singular = next_word(&parts, best_end).is_some_and(|n| crate::normalize(&n) == "poco");
            if article && singular {
                if let Some(k) = prev.and_then(|c| (i..c).rev().find(|&k| !py_str_isspace(&parts[k]))) {
                    if let Ok(v) = converter.to_cardinal(parts[i..=k].concat().trim()) {
                        best_value = Some(v);
                        best_end = k;
                    }
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

        // th "สองพันห้า": 2005 as written, 2500 as spoken; no one number, so
        // the words stay (#875). A year too: unspaced text gives no sure year
        // label ("ต่อปี" is "per year"), so none is guessed (#967, #968).
        if best_value.is_some() && !best_ordinal {
            let run = parts[i..=best_end].concat();
            let norm = crate::normalize(rstrip_punct(py_strip(&run)));
            if matches!(crate::elided_place(converter.n2w_key(), &norm), Some((true, _))) {
                out.push_str(&run);
                i = best_end + 1;
                continue;
            }
        }

        if let Some(v) = best_value {
            // Preserve trailing punctuation that was stripped during parse.
            let run = parts[i..=best_end].concat();
            let trailing = trailing_punct(&run);
            // The word before is left as written because a mark opens it
            // (")two"), but this run would continue its number ("thousand
            // twenty four" after ")two"): the run stays as written too, no
            // split value (#1112, #1114).
            let stripped_run = rstrip_punct(py_strip(&run)).to_string();
            let punct_led_join = continues_punct_led(lang, to, has_kwargs, &parts, i, &stripped_run, &parts[best_end + 1..], &out);
            let rendered = if punct_led_join {
                None
            } else if best_ordinal {
                ordinal_rendering(&converter, &resolved, &parts, i, best_end, &v)
            } else if plain
                && !best_ordinal
                // Not across a quote that ends the run: "сто\" четвърти" (#1149).
                && !ends_run_mark(&parts[best_end])
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
                && !gap_ended
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
                && !gap_ended
                && is_homograph(&resolved, &parts, i, piece)
                && !next_is_number
                // "sei e trenta": a connector, then a number.
                && !(next.as_deref().is_some_and(|t| is_includable(t, includable))
                    && next_word2(&parts, i).is_some_and(|t| starts_run(&converter, &t, decimal_heads)))
                && !prev_word(&parts, i).is_some_and(|p| starts_run(&converter, &p, decimal_heads))
            {
                // "tu sei bravo", "quoi de neuf": a lone homograph is the word (#80).
                None
            } else if best_end == i
                && resolved.starts_with("my")
                && crate::normalize(rstrip_punct(piece)) == crate::normalize("နှစ်")
                // A number word or a figure ("5 နှစ်", "၅ နှစ်", #1016).
                && prev_word(&parts, i).is_some_and(|p| starts_run(&converter, &p, decimal_heads) || my_figure(&p))
                // Not after "<figure> နှစ်", whose နှစ် was the year: "5 နှစ်
                // နှစ်" is 5 years, then 2 (#1050).
                && !prev_word_index(&parts, i).is_some_and(|k| {
                    crate::normalize(rstrip_punct(&parts[k])) == crate::normalize("နှစ်")
                        && !has_trailing_mark(&parts[k])
                        && prev_word_index(&parts, k).is_some_and(|f| {
                            !has_trailing_mark(&parts[f]) && my_figure(rstrip_punct(&parts[f]))
                        })
                })
                // Not across a mark: "ငါး။ နှစ် ယောက်" opens a new sentence;
                // nor after a word with trailing punctuation ("ငါး-").
                && parts[..i].iter().rev().find(|t| !py_str_isspace(t)).is_some_and(|raw| {
                    !has_trailing_mark(raw) && !tokens::is_punct_part(raw) && rstrip_punct(raw) == raw.as_str()
                })
                // Not before a counter: "ငါးရာ နှစ် ကျပ်" (#1015).
                && !(!has_trailing_mark(piece) && my_counter(next_word(&parts, i).as_deref()))
            {
                // my "ငါး နှစ်": the year after a number, no 2 (#876).
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
                    after_decimal = matches!(v, W2nValue::Dec(_)) && trailing.chars().all(is_format_char);
                    out.push_str(sep_before(i));
                    out.push_str(&lead_format(&parts[i]));
                    out.push_str(&sign_format(&parts[i], &text));
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
    // Area and volume (#896): "m2", "m^2" are read as "m²".
    ("mm\u{b2}", ui("mm\u{b2}", "square millimeter", "area", 1.0)),
    ("cm\u{b2}", ui("cm\u{b2}", "square centimeter", "area", 1.0)),
    ("m\u{b2}", ui("m\u{b2}", "square meter", "area", 1.0)),
    ("km\u{b2}", ui("km\u{b2}", "square kilometer", "area", 1.0)),
    ("ft\u{b2}", ui("ft\u{b2}", "square foot", "area", 1.0)),
    ("in\u{b2}", ui("in\u{b2}", "square inch", "area", 1.0)),
    ("mm\u{b3}", ui("mm\u{b3}", "cubic millimeter", "volume", 1.0)),
    ("cm\u{b3}", ui("cm\u{b3}", "cubic centimeter", "volume", 1.0)),
    ("m\u{b3}", ui("m\u{b3}", "cubic meter", "volume", 1.0)),
    ("km\u{b3}", ui("km\u{b3}", "cubic kilometer", "volume", 1.0)),
    ("ft\u{b3}", ui("ft\u{b3}", "cubic foot", "volume", 1.0)),
    ("in\u{b3}", ui("in\u{b3}", "cubic inch", "volume", 1.0)),
];

/// A unit's power suffix after its letters: "²"/"³", "^2"/"^3", or, when
/// `glued` allows it, a "2"/"3" at the end ("m2"), as the superscript. `None`
/// when there is none, or when a letter or digit runs on ("m²a").
fn eat_unit_power(cur: &mut Cursor, glued: bool) -> Option<char> {
    let sup = |c: char| match c {
        '\u{b2}' | '2' => Some('\u{b2}'),
        '\u{b3}' | '3' => Some('\u{b3}'),
        _ => None,
    };
    match cur.peek() {
        Some(c @ ('\u{b2}' | '\u{b3}')) if !cur.at(1).is_some_and(|c| c.is_alphanumeric()) => {
            cur.i += 1;
            sup(c)
        }
        Some('^') if cur.at(1).is_some_and(|c| matches!(c, '2' | '3')) && !cur.at(2).is_some_and(|c| c.is_alphanumeric()) => {
            let c = cur.at(1);
            cur.i += 2;
            c.and_then(sup)
        }
        Some(c @ ('2' | '3')) if glued && !cur.at(1).is_some_and(|c| c.is_alphanumeric()) => {
            cur.i += 1;
            sup(c)
        }
        _ => None,
    }
}

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
    // SI symbols are case-sensitive: "Mg" (megagram) folded to "mg" is 10^9
    // off, so a mega-or-above prefix on a base symbol ("Mg", "Ms", "Mm",
    // "Ml") and "NM"/"Nm" (nautical mile, newton-metre; not nm) are not
    // folded. A folded hit is never certain (#393).
    let (info, folded) = match units_get(unit_str) {
        Some(info) => (info, false),
        None => {
            let lower = unit_str.to_lowercase();
            // "Mm²" is no mm²: the prefix rule looks at the base symbol.
            let c: Vec<char> = unit_str.trim_end_matches(['\u{b2}', '\u{b3}']).chars().collect();
            let si_prefixed = c.len() == 2 && "MGTPEZY".contains(c[0]) && "gmslt".contains(c[1]);
            let info = units_get(&lower)?;
            // The caller may still pick the folded reading: OCR or caps-locked
            // input loses case (prefer={"Mg": "milligram"}).
            let chosen = prefer
                .get(unit_str)
                .is_some_and(|t| *t == info.long || *t == info.short || t.to_lowercase() == lower);
            if (si_prefixed || lower == "nm") && !chosen {
                return None;
            }
            (info, true)
        }
    };
    let cap = |c: f64| if folded { c.min(0.8) } else { c };

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
    Some((info, cap(info.confidence)))
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
        let mut letters = cur.take_while(|c| c.is_ascii_alphabetic());
        let save = cur.i;
        // "in2" is no square inch: a bare "in" is ambiguous (#803), so it
        // takes only a written power ("in²", "in^2"). "M²" is folded below.
        let glued = !letters.eq_ignore_ascii_case("in");
        if let Some(p) = eat_unit_power(&mut cur, glued) {
            if units_get(&format!("{}{p}", letters.to_lowercase())).is_some() {
                letters.push(p);
            } else {
                cur.i = save;
            }
        }
        letters
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
    fmt: &FigureFormat,
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
    let value = match call_words2num(head, lang, fmt) {
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
    // "10²" is no 102 (#869).
    if crate::holds_compat_digit(text) {
        return Err(W2nError::Words2Num(format!("could not auto-parse {}", py_repr_str(text))));
    }
    // es "1º", "1.000º": the figure, as words2num reads it (#896).
    if let Some(figure) = crate::ordinal_indicator_figure(text) {
        let mut q = auto_parse(figure, lang, prefer, thousands_sep, decimal_sep)?;
        if !matches!(q.value, W2nValue::Int(_)) {
            return Err(W2nError::Words2Num(format!("could not auto-parse {}", py_repr_str(text))));
        }
        q.raw = text.to_string();
        return Ok(q);
    }
    // Figures read with the caller's region: es_MX groups with a comma (#423).
    let fmt_key = crate::w2n_formats::format_key(lang, &resolved);
    let fmt_lang = fmt_key.as_str();
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
            let mut value = parse_number_string(&num_str, thousands_sep, decimal_sep, Some(fmt_lang))?;
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
        match parse_number_string(&num_str, thousands_sep, decimal_sep, Some(fmt_lang)) {
            Ok(value) => return Ok(quantity_currency(value, &sym, raw)),
            Err(W2nError::Words2Num(_)) => {}
            Err(e) => return Err(e),
        }
    }

    // 3. Number + unit suffix (digit form): "5cm", "20°C", "42%", "3.5 km"
    if let Some(mut res) = try_digit_unit(&chars, prefer, thousands_sep, decimal_sep, fmt_lang)? {
        res.raw = raw.to_string();
        return Ok(res);
    }

    // 4. Word-form number + unit: "forty-two kg", "twenty-three percent"
    let fmt = FigureFormat { key: fmt_lang, thousands_sep, decimal_sep };
    if let Some(mut res) = try_word_unit(text, lang, prefer, &fmt)? {
        res.raw = raw.to_string();
        return Ok(res);
    }

    // 5. Pure number — digit form, then word form.
    match parse_number_string(text, thousands_sep, decimal_sep, Some(fmt_lang)) {
        Ok(value) => return Ok(Quantity::bare(value, raw)),
        // Figures the caller gave separators for are no words: "1.5" with
        // decimal_sep="," raises, it is no float 1.5. A dash and a comma
        // before the figure are its sign ("-, 5"), read with those separators.
        Err(e @ W2nError::Words2Num(_))
            if (thousands_sep.is_some_and(|t| !t.is_empty()) || decimal_sep.is_some_and(|d| !d.is_empty()))
                && text.chars().all(|c| c.is_ascii_digit() || ".,'_ +-".contains(c))
                && crate::signed_figure_in(text, lang).is_none() =>
        {
            return Err(W2nError::Words2Num(format!("could not auto-parse {}: {}", py_repr_str(raw), e.message())));
        }
        Err(W2nError::Words2Num(_)) => {}
        Err(e) => return Err(e),
    }

    match call_words2num(text, lang, &fmt) {
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
                let letters = cur.take_while(|c| c.is_ascii_alphabetic() || c == '\u{b5}');
                // "5 m²", "5 m^2": the power belongs to the unit (#896). Not
                // a glued digit in running text: "5 m2 slots" is a product code.
                let save = cur.i;
                // "5 KM²" folds as "5 KM" does (#953).
                match eat_unit_power(&mut cur, false) {
                    Some(p) if units_get(&format!("{}{p}", letters.to_lowercase())).is_some() => {}
                    _ => cur.i = save,
                }
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
        "square foot" => "square feet",
        "cubic foot" => "cubic feet",
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
fn format_quantity(q: &Quantity, expand: bool, mark: char) -> String {
    // The locale's decimal mark, so the text reads back as written: de
    // "3,125 kg" stays "3,125 kg", never "3.125 kg" (3125 in de) (#867).
    let format_number = |v: &W2nValue| {
        let s = format_number(v);
        if mark == '.' { s } else { s.replace('.', &mark.to_string()) }
    };
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

/// Is the digit at `i` glued to a Latin word ("MP3", "B12", "COVID19")? An
/// "x" between digits is a multiplier ("2x500 g").
fn glued_to_word(chars: &[char], i: usize) -> bool {
    if i == 0 || !chars[i].is_ascii_digit() || !chars[i - 1].is_ascii_alphanumeric() {
        return false;
    }
    let x_mult = matches!(chars[i - 1], 'x' | 'X') && (i < 2 || !chars[i - 2].is_ascii_alphabetic());
    !x_mult
}

/// The unit letters at the end of a quantity match ("5 kg" -> "kg").
fn unit_token(raw: &str) -> &str {
    let start = raw
        .char_indices()
        .rfind(|(_, c)| !c.is_ascii_alphabetic())
        .map_or(0, |(k, c)| k + c.len_utf8());
    &raw[start..]
}

/// A unit token whose shape says it is something else in running text: a
/// single capital letter other than L or a degree scale ("5G", "4K", "3D",
/// "Room 5 C"), "in" ("1 in 3"), an
/// "s" glued to a decade ("1990s"), or the verb "won" before a number
/// ("Score 2 won 3"; "5000 won." stays the currency) (#392).
fn not_a_unit(chars: &[char], raw: &str, end: usize, currency: bool) -> bool {
    let unit = unit_token(raw);
    let glued = !raw.contains(char::is_whitespace);
    let number_follows = || {
        chars[end..].iter().skip_while(|c| c.is_whitespace()).next().is_some_and(|c| c.is_ascii_digit())
    };
    // A currency's letters are its scale ("$5M", "€3K"): only "won" there.
    if currency {
        return unit.eq_ignore_ascii_case("won") && number_follows();
    }
    let degree = raw[..raw.len() - unit.len()].ends_with('\u{b0}');
    // A decade is a whole number: "1.0s" and "0.50s" are seconds.
    let num = raw.trim_end_matches('s');
    // "1,000s" groups thousands; "2,50s" has a decimal comma.
    let decimal = num.contains('.') || num.rsplit_once(',').is_some_and(|(_, tail)| tail.len() != 3);
    let decade = unit == "s" && glued && num.ends_with('0') && !decimal;
    // "L" is the litre's own symbol.
    (unit.len() == 1 && unit.chars().all(|c| c.is_ascii_uppercase()) && unit != "L" && !degree)
        || unit.eq_ignore_ascii_case("in")
        || decade
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
    let resolved = resolve_lang(lang)?;
    crate::w2n_formats::check_separators(thousands_sep, decimal_sep)
        .map_err(|e| W2nError::Words2Num(e.0))?;
    // The mark this same call reads as the decimal: the caller's, else the
    // point when only a thousands separator other than it is given (as
    // `parse_with_explicit` reads), else the locale's.
    let ts = thousands_sep.filter(|t| !t.is_empty());
    let mark = decimal_sep
        .and_then(|d| d.chars().next())
        .or_else(|| ts.filter(|t| *t != ".").map(|_| '.'))
        .or_else(|| {
            crate::w2n_formats::locale_format(&crate::w2n_formats::format_key(lang, &resolved))
                .and_then(|f| f.decimal.chars().next())
        })
        .unwrap_or('.');
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        match match_quantity_at(&chars, i) {
            // The pattern can never match empty (both alternatives need at
            // least one character), so no zero-width-match handling is needed.
            // A number starts on a word boundary: "MP3s", "B12" hold none
            // (#392). Only Latin letters count: zh "约5kg", ar "و5%" glue a
            // number to a word, and "2x500 g" is a multiplier.
            Some(end) if end > i && !glued_to_word(&chars, i) => {
                let raw: String = chars[i..end].iter().collect();
                match auto_parse(&raw, lang, prefer, thousands_sep, decimal_sep) {
                    // In running text a unit shaped like something else is left
                    // as written: "the 1990s", "a 5G network", "1 in 3", "Room
                    // 5 C", "Score 2 won 3" (#392).
                    Ok(q) if q.unit.is_some()
                        && !prefer.contains_key(unit_token(&raw))
                        && not_a_unit(&chars, &raw, end, q.kind.as_deref() == Some("currency")) =>
                    {
                        out.push_str(&raw)
                    }
                    Ok(q) => out.push_str(&format_quantity(&q, expand, mark)),
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
        assert_eq!(pluralize(Some("square foot"), &int(5)).as_deref(), Some("square feet"));
        assert_eq!(pluralize(Some("cubic foot"), &int(2)).as_deref(), Some("cubic feet"));
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
    fn year_mode_reads_decades_and_leaves_times() {
        let y = |t: &str| s(t, "en", "year", false).unwrap();
        // A century and a decade word: one decade token.
        assert_eq!(y("the nineteen nineties"), "the 1990s");
        assert_eq!(y("back in the nineteen eighties, we"), "back in the 1980s, we");
        assert_eq!(y("the twenty twenties."), "the 2020s.");
        assert_eq!(y("nineteen sixties music"), "1960s music");
        // The cardinal walk still keeps it in words.
        assert_eq!(s("the nineteen nineties", "en", "cardinal", false).unwrap(), "the nineteen nineties");
        // A bare decade word is no number.
        assert_eq!(y("in the eighties"), "in the eighties");
        // A time is a clock time, not a three-digit year.
        assert_eq!(y("call me at nine fifteen"), "call me at 9:15");
        // Without a cue, two numbers.
        assert_eq!(y("open five thirty to six"), "open 5 30 to 6");
        // Years with a two-digit first half are untouched.
        assert_eq!(y("born in nineteen eighty four"), "born in 1984");
        assert_eq!(y("in nineteen oh five"), "in 1905");
        assert_eq!(y("twenty twenty six"), "2026");
        assert_eq!(y("in two thousand and five"), "in 2005");
        assert_eq!(y("open twenty four seven"), "open 24 7");
        // The single-token reading keeps num2words2's 100..999 spelling (#111).
        assert!(matches!(crate::en_to_year("nine eleven"), Ok(crate::w2n_lang_en::W2nValue::Int(v)) if v == num_bigint::BigInt::from(911)));
    }

    #[test]
    fn clock_times() {
        let c = |t: &str| s(t, "en", "cardinal", false).unwrap();
        assert_eq!(c("call me at nine fifteen"), "call me at 9:15");
        assert_eq!(c("at nine fifteen pm"), "at 9:15 pm");
        assert_eq!(c("nine thirty am"), "9:30 am");
        assert_eq!(c("at ten oh five"), "at 10:05");
        assert_eq!(c("meet at twelve thirty."), "meet at 12:30.");
        assert_eq!(c("around five forty five"), "around 5:45");
        assert_eq!(c("by eleven forty five p.m."), "by 11:45 p.m.");
        assert_eq!(c("from nine fifteen until ten thirty"), "from 9:15 until 10:30");
        assert_eq!(c("at nine fifteen tomorrow"), "at 9:15 tomorrow");
        // No cue, no time.
        assert_eq!(c("five twenty"), "5 20");
        assert_eq!(c("I have nine fifteen year olds"), "I have 9 15 year olds");
        // No minutes, an hour over twelve, or more numbers after: no time.
        assert_eq!(c("from nine to five"), "from 9 to 5");
        assert_eq!(c("at seven o'clock"), "at 7 o'clock");
        assert_eq!(c("at nineteen fifty"), "at 1950");
        assert!(!c("at nine fifteen twenty").contains(':'));
        // Year mode reads them the same way.
        assert_eq!(s("at nine fifteen pm", "en", "year", false).unwrap(), "at 9:15 pm");
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

#[cfg(test)]
mod double_sign_tests {
    /// #298: an apocope does not let a second sign through.
    #[test]
    fn double_negative_raises_issue_298() {
        for (t, l) in [("menos menos un", "es"), ("menos menos un millón", "es"), ("menos menos veintiún", "es"), ("meno meno un", "it")] {
            assert!(crate::words2num(t, l, "cardinal").is_err(), "{l} {t}");
        }
        assert_eq!(
            format!("{:?}", crate::words2num("menos un millón", "es", "cardinal").unwrap()),
            format!("{:?}", crate::words2num("-1000000", "es", "cardinal").unwrap())
        );
    }
}

#[cfg(test)]
mod false_number_tests {
    use super::words2num_sentence;

    fn s(t: &str, l: &str) -> String {
        words2num_sentence(t, l, "cardinal", false).unwrap()
    }

    /// #283: clause punctuation inside a token joins no two words.
    #[test]
    fn interior_punctuation_issue_283() {
        assert_eq!(s("twenty,two", "en"), "twenty,two");
        assert_eq!(s("He was twenty;two came", "en"), "He was twenty;two came");
        assert_eq!(s("_twenty_", "en"), "_twenty_");
        assert_eq!(s("zwanzig!drei", "de"), "zwanzig!drei");
        assert_eq!(s("twenty-two, three", "en"), "22, 3");
    }

    /// #285: a word glued onto a number reads no other number.
    #[test]
    fn no_glue_onto_the_next_word_issue_285() {
        assert_eq!(s("Chacun d'eux est venu.", "fr"), "Chacun d'eux est venu.");
        assert_eq!(s("Tôi có ba y tá", "vi"), "Tôi có 3 y tá");
        assert_eq!(s("Ik zag er twee de straat oversteken.", "nl"), "Ik zag er 2 de straat oversteken.");
        assert_eq!(s("Es waren zwei te Leute", "de"), "Es waren 2 te Leute");
        assert_eq!(s("Tri je ura.", "sl"), "3 je ura.");
        assert_eq!(s("eight y", "en_IN"), "8 y");
        assert_eq!(s("X šest Y.", "cs"), "X 6 Y.");
        assert_eq!(s("X jedenásť Y.", "sk"), "X 11 Y.");
        assert_eq!(s("X pięćdziesiąt Y.", "pl"), "X 50 Y.");
        // Still read: a compound ordinal glued by the renderer, and the
        // renderer's own spellings whose pieces are no candidates.
        assert_eq!(s("venti quattresimo", "it"), "24º");
        assert_eq!(s("hiru milioi", "eu"), "3000000");
        assert_eq!(s("tati miliyon", "ff"), "3000000");
        assert_eq!(s("bhiriyoni mbiri", "sn"), "2000000000");
        assert_eq!(s("વીસ છમો", "gu"), "26મો");
    }

    /// #288: digits of any script are literals, never part of a word run.
    #[test]
    fn unicode_digits_are_literals_issue_288() {
        assert_eq!(s("twenty ２", "en"), "20 ２");
        assert_eq!(s("one hundred ٥", "en"), "100 ٥");
        assert_eq!(s("room ５０１", "en"), "room ５０１");
    }

    /// #411: pt "é", it "è" (is) are no connector "e" (and).
    #[test]
    fn copula_is_no_connector_issue_411() {
        assert_eq!(s("metade de cem é cinquenta", "pt"), "metade de 100 é 50");
        assert_eq!(s("la metà di cento è cinquanta", "it"), "la metà di 100 è 50");
        assert_eq!(s("cento e cinquanta", "it"), "150");
        assert!(crate::words2num("vinte é cinco", "pt", "cardinal").is_err());
    }

    /// #433: ru/uk homographs of 7 and 40.
    #[test]
    fn russian_homographs_issue_433() {
        assert_eq!(s("я люблю свою семью", "ru"), "я люблю свою семью");
        assert_eq!(s("Сорока украла кольцо", "ru"), "Сорока украла кольцо");
        assert_eq!(s("Сорока украла кольцо", "uk"), "Сорока украла кольцо");
        assert_eq!(s("с семью друзьями", "ru"), "с 7 друзьями");
        assert_eq!(s("около сорока человек", "ru"), "около 40 человек");
        assert_eq!(s("в сорока километрах", "ru"), "в 40 километрах");
        assert_eq!(s("близько сорока людей", "uk"), "близько 40 людей");
        assert_eq!(s("после сорока лет", "ru"), "после 40 лет");
        assert_eq!(s("между сорока и пятьюдесятью", "ru"), "между 40 и 50");
        assert_eq!(s("після сорока років", "uk"), "після 40 років");
        assert_eq!(s("О сорока!", "uk"), "О сорока!");
    }
}

#[cfg(test)]
mod scale_comma_tests {
    use super::words2num_sentence;

    /// #657: a comma after a scale word is crossed only where the renderer
    /// writes one inside a number.
    #[test]
    fn comma_after_scale_only_where_rendered_issue_657() {
        let s = |t: &str, l: &str| words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(s("в две тысячи, сто человек", "ru"), "в 2000, 100 человек");
        assert_eq!(s("Ceny: dwa tysiące, trzysta i pięćset złotych.", "pl"), "Ceny: 2000, 300 i 500 złotych.");
        assert_eq!(s("Precios: dos mil, trescientos y quinientos euros.", "es"), "Precios: 2000, 300 y 500 euros.");
        assert_eq!(s("две хиляди, триста", "bg"), "2000, 300");
        // The renderers that write the comma keep reading one number (#67),
        // after the scales they write it after: it only after "miliardo".
        assert_eq!(s("twelve thousand, three hundred and forty-five", "en"), "12345");
        assert_eq!(
            s("x un miliardo, duecentotrentaquattro milioni e cinquecentosessantasettemilaottocentonovanta y", "it"),
            "x 1234567890 y"
        );
        assert_eq!(s("q un milione, duecento z", "it"), "q 1000000, 200 z");
    }
}

#[cfg(test)]
mod ambiguous_span_tests {
    use super::words2num_sentence;

    /// #668: a glued or ASCII comma in running text is no decimal mark.
    #[test]
    fn arabic_list_commas_stay_lists() {
        let s = |t: &str| words2num_sentence(t, "ar", "cardinal", false).unwrap();
        assert!(!s("عندي ثلاثة وعشرون,أربعون شخصا").contains("23.40"));
        assert!(!s("عندي مائة وخمسون,عشرون كتابا").contains("150.20"));
        assert!(!s("عندي واحد وعشرون ,خمسون").contains("21.50"));
        assert!(!s("عندي واحد وعشرون،عشرون").contains("21.20"));
    }

    /// #348: a span the renderer writes for two values stays words, with the
    /// rest of the same number, and its ordinal is declined too.
    #[test]
    fn ambiguous_spans_stay_words() {
        let s = |t: &str, l: &str| words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(s("na biya miliyan biyu dubu goma jiya", "ha"), "na biya miliyan biyu dubu goma jiya");
        assert_eq!(s("x ba tan ni kelen ni mugan ni wolonfila y", "bm"), "x ba tan ni kelen ni mugan ni wolonfila y");
        // Unambiguous numbers still read.
        assert_eq!(s("ina da miliyan dubu naira", "ha"), "ina da 1001000 naira");
        assert_eq!(s("Na biya naira miliyan talatin", "ha"), "Na biya naira 30000000");
        // #796: a number before the ambiguous span still reads.
        assert_eq!(s("mutum biyar miliyan dubu ɗari", "ha"), "mutum 5 miliyan dubu ɗari");
        assert_eq!(s("x dubu biyu miliyan dubu ɗari y", "ha"), "x 2000 miliyan dubu ɗari y");
        assert_eq!(s("x mugan ba tan ni kelen y", "bm"), "x 20 ba tan ni kelen y");
        // #884 M1: a span that reads on its own is no split point.
        assert_eq!(s("x uku dubu tamanin y", "ha"), "x uku dubu tamanin y");
        assert_eq!(s("dubu tamanin da dubu tamanin", "ha"), "dubu tamanin da dubu tamanin");
        // A multiplier after a scale word left in words is not split off.
        assert_eq!(s("x tiriliyan tara biliyan ɗari tara y", "ha"), "x tiriliyan tara biliyan ɗari tara y");
        // #797: a comma ends the words-span.
        assert_eq!(s("x miliyan dubu ɗari, dubu biyar y", "ha"), "x miliyan dubu ɗari, 5000 y");
        assert_eq!(s("x ba tan ni kelen, ni mugan y", "bm"), "x ba tan ni kelen, ni 20 y");
        // #798 and #884 M2: the number's ordinal word stays words with it.
        assert_eq!(s("x ba tan ni kelen ni kelennan y", "bm"), "x ba tan ni kelen ni kelennan y");
        assert_eq!(s("x ba tan ni sabanan y", "bm"), "x ba tan ni sabanan y");
        assert_eq!(s("x saba ba tan ni naaninan y", "bm"), "x 3 ba tan ni naaninan y");
        assert_eq!(s("x ba tan ni kelennan saba y", "bm"), "x ba tan ni kelennan 3 y");
        // #882 pins: no fragments after an unreadable scale word.
        assert_eq!(s("x tiriliyan biliyan goma y", "ha"), "x tiriliyan biliyan goma y");
        assert_eq!(s("x tiriliyan biyar biliyan goma y", "ha"), "x tiriliyan biyar biliyan goma y");
        // #884 L1: a separate number after the span reads.
        assert_eq!(s("x ba tan ni kelen saba y", "bm"), "x ba tan ni kelen 3 y");
        // #884 L2: a comma before the run lets the split happen.
        assert_eq!(s("x mutum biyar, uku miliyan dubu ɗari y", "ha"), "x mutum 5, 3 miliyan dubu ɗari y");
    }
}

#[cfg(test)]
mod unit_misread_tests {
    use super::{auto_parse, auto_parse_sentence};
    use std::collections::HashMap;

    fn sent(t: &str, expand: bool) -> String {
        auto_parse_sentence(t, "en", &HashMap::new(), None, None, expand).unwrap()
    }

    /// #392: uncertain units and verbs stay as written in running text.
    #[test]
    fn uncertain_units_stay_in_sentences_issue_392() {
        for t in ["in the 1990s", "a 5G network", "1 in 3 adults", "Born in 1990 in Paris", "a 4K TV", "a 3D movie", "Room 5 C is free", "Score 2 won 3", "my MP3s"] {
            assert_eq!(sent(t, true), t, "{t}");
        }
        for t in ["Score 2 Won 3", "Score 2 WON 3"] {
            assert_eq!(sent(t, false), t, "{t}");
        }
        // Certain units still convert, the common one-letter ones too.
        assert_eq!(sent("it weighs 5 kg", false), "it weighs 5 kg");
        assert_eq!(sent("it weighs 5kg", true), "it weighs 5 kilograms");
        assert_eq!(sent("sprint 100 m", true), "sprint 100 meters");
        assert_eq!(sent("200 g flour", true), "200 grams flour");
        assert_eq!(sent("wait 30 s", true), "wait 30 seconds");
        assert_eq!(sent("I paid 5000 won.", false), "I paid 5000 KRW.");
        assert_eq!(sent("2x500 g", true), "2x500 grams");
        // Glued to a word of another script.
        assert_eq!(sent("约5kg", false), "约5 kg");
        assert_eq!(sent("و5%", true), "و5 percent");
        // Still no number inside a Latin word.
        assert_eq!(sent("vitamin B12 5 mg", true), "vitamin B12 5 milligrams");
        assert_eq!(sent("it is 20°C", false), "it is 20 °C");
        assert_eq!(sent("a 2L bottle", true), "a 2 liters bottle");
        // A currency's capital letter is its scale.
        assert_eq!(sent("raised $5M today", false), "raised 5000000 USD today");
        assert_eq!(sent("€3K", false), "3000 EUR");
        assert_eq!(sent("£10K bonus", false), "10000 GBP bonus");
        // Decimals are seconds, never a decade.
        assert_eq!(sent("1.0s", false), "1 s");
        assert_eq!(sent("1,000s of people", false), "1,000s of people");
        assert_eq!(sent("2,50s", false), "2.5 s");
        assert_eq!(sent("it is 20 °F", true), "it is 20 degrees fahrenheit");
    }

    /// #393: SI prefixes are case-sensitive.
    #[test]
    fn si_prefix_case_issue_393() {
        let p = HashMap::new();
        for t in ["5 Mg", "5 Mm", "5 Ms", "5 Nm", "5 NM"] {
            assert!(auto_parse(t, "en", &p, None, None).map_or(true, |q| q.unit.is_none()), "{t}");
        }
        let q = auto_parse("5 KG", "en", &p, None, None).unwrap();
        assert_eq!(q.unit.as_deref(), Some("kg"));
        assert!(q.confidence < 1.0);
        assert_eq!(sent("Take 5 Mg daily", false), "Take 5 Mg daily");
        // The caller can still choose the folded reading.
        let mg: HashMap<String, String> = [("Mg".to_string(), "milligram".to_string())].into();
        assert_eq!(auto_parse("5 Mg", "en", &mg, None, None).unwrap().unit.as_deref(), Some("mg"));
        let kg: HashMap<String, String> = [("KG".to_string(), "kilogram".to_string())].into();
        assert!(auto_parse("5 KG", "en", &kg, None, None).unwrap().confidence >= 0.9);
    }
}

#[cfg(test)]
mod sign_comma_tests {
    use crate::w2n_lang_en::W2nValue;

    /// #896: a comma after a sign word before a figure is punctuation.
    #[test]
    fn comma_after_sign_word_issue_896() {
        assert_eq!(crate::words2num("moins, 1.234", "fr", "cardinal").unwrap(), W2nValue::Int((-1234).into()));
        assert_eq!(crate::words2num("minus, 1.234", "de", "cardinal").unwrap(), W2nValue::Int((-1234).into()));
        let dec = |t: &str, l: &str| match crate::words2num(t, l, "cardinal").unwrap() {
            W2nValue::Dec(d) => d.to_string(),
            other => format!("{other:?}"),
        };
        // A dash takes the comma too, and so does "،" where it is no decimal mark.
        assert_eq!(crate::words2num("-, 1.234", "de", "cardinal").unwrap(), W2nValue::Int((-1234).into()));
        assert_eq!(crate::words2num("-, 5", "en", "cardinal").unwrap(), W2nValue::Int((-5).into()));
        assert_eq!(dec("منفی، 1,234.5", "fa"), "-1234.5");
        // ar: "،" after the sign is the decimal mark.
        assert_eq!(dec("-، 50", "ar"), "-0.50");
        assert_eq!(dec("-، 05", "ar"), "-0.05");
        assert!(crate::words2num("سالب، 50", "ar", "cardinal").is_err());
    }
}

#[cfg(test)]
mod sign_and_script_tests {
    use super::words2num;

    /// #698: a sign word before a signed figure is no double negative.
    #[test]
    fn one_sign_before_a_figure_issue_698() {
        for (t, l) in [("moins -5", "fr"), ("минус -5", "ru"), ("- -5", "de"), ("negative -5", "fr"), ("minus -5", "en_IN"), ("moins +5", "fr")] {
            assert!(crate::words2num(t, l, "cardinal").is_err(), "{l} {t}");
        }
        assert!(crate::words2num("moins -5", "fr", "year").is_err());
        // A double negative in figures is refused too.
        assert!(crate::words2num("-(-5)", "en", "cardinal").is_err());
    }

    /// #697: digits mode never drops a leading minus.
    #[test]
    fn digits_keep_their_sign_issue_697() {
        for (t, l) in [("-five", "en"), ("-5", "en"), ("-cinq", "fr"), ("-one two three", "en")] {
            assert!(words2num(t, l, "digits").is_err(), "{l} {t}");
        }
        assert!(words2num("seven-two", "en", "digits").is_ok());
        // Nor a signed zero (#956).
        for t in ["-oh three", "-zero three", "minus oh three", "-oh"] {
            assert!(words2num(t, "en", "digits").is_err(), "{t}");
        }
    }

    /// #956: an English dictation run stops at a dash on either edge of a
    /// part; only a hyphen between two words joins them.
    #[test]
    fn dictation_run_keeps_edge_dashes_issue_956() {
        let s = |t: &str, to: &str| super::words2num_sentence(t, "en", to, false).unwrap();
        assert_eq!(s("it was -four five six degrees", "cardinal"), "it was -4 5 6 degrees");
        assert_eq!(s("call one two three -four", "cardinal"), "call 123 -4");
        assert_eq!(s("call one two three-", "cardinal"), "call 123-");
        assert_eq!(s("my number is five five five- one two one two", "cardinal"), "my number is 555- 1212");
        assert_eq!(s("zip nine four five oh four-", "digits"), "zip 94504-");
        assert_eq!(s("call one two three-, four", "cardinal"), "call 123-, 4");
        assert_eq!(s("call one two three-. Then", "cardinal"), "call 123-. Then");
        // Any glued dash the library reads as a minus.
        assert_eq!(s("five five five\u{2212} one two one two", "cardinal"), "555\u{2212} 1212");
        assert_eq!(s("call one two three\u{FF0D}", "digits"), "call 123\u{FF0D}");
        assert_eq!(s("code -one two three", "digits"), "code -one 2 3");
        assert_eq!(s("x -oh three y", "digits"), "x -oh 3 y");
        // An inner hyphen still joins: "one two-three four" is 1234.
        assert_eq!(s("call one two-three four", "cardinal"), "call 1234");
        assert_eq!(s("call four five six seven", "cardinal"), "call 4567");
    }

    /// #869: a superscript or subscript digit is no part of the figure.
    #[test]
    fn script_digits_are_no_digits_issue_869() {
        for (t, l) in [("10²", "en"), ("2³", "en"), ("10⁶", "en"), ("12²", "de"), ("10₂", "en"), ("５²", "ja"), ("²5", "en"), ("10.²", "en")] {
            assert!(crate::words2num(t, l, "cardinal").is_err(), "{l} {t}");
        }
        // Nor alone, nor any other compatibility digit (#955).
        for t in ["¹⁰", "²", "①", "⑩", "1①", "1⑩", "1⒈", "19⑨⑨", "½", "1½", "2⑤"] {
            assert!(crate::words2num(t, "en", "cardinal").is_err(), "{t}");
            assert!(super::auto_parse(t, "en", &std::collections::HashMap::new(), None, None).is_err(), "{t}");
        }
        assert!(crate::words2num("1①", "en", "year").is_err());
        assert!(crate::words2num("1①", "en", "ordinal").is_err());
        // In running text they stay as written (#949).
        for t in ["the ⑤ million y", "see note ³ below", "E = mc ²", "three ⁵", "x ½ y", "x 𝟓 hundred y"] {
            assert_eq!(super::words2num_sentence(t, "en", "cardinal", false).unwrap().replace("three", "3"), t.replace("three", "3"), "{t}");
        }
        // The caller may ask for them: a run alone reads, a glued one never.
        crate::with_numerical_symbols(true, || {
            assert!(crate::words2num("①", "en", "cardinal").is_ok());
            assert!(crate::words2num("¹⁰", "en", "cardinal").is_ok());
            for t in ["1①", "10²", "½", "1½"] {
                assert!(crate::words2num(t, "en", "cardinal").is_err(), "{t}");
            }
            assert_eq!(super::words2num_sentence("the ⑤ million y", "en", "cardinal", false).unwrap(), "the 5000000 y");
        });
        // CJK numerical symbols too (#965).
        for (t, l) in [("㊀", "zh"), ("㈩", "zh"), ("㆒", "ja"), ("〸", "zh"), ("㊄十", "zh")] {
            assert!(crate::words2num(t, l, "cardinal").is_err(), "{l} {t}");
        }
        for l in ["zh", "ja"] {
            assert_eq!(super::words2num_sentence("x ㈢ y", l, "cardinal", false).unwrap(), "x ㈢ y");
        }
        assert_eq!(crate::words2num("五十", "zh", "cardinal").unwrap(), crate::w2n_lang_en::W2nValue::Int(50.into()));
        // The public normalize keeps them, the lookup one folds (#972).
        assert_eq!(crate::normalize_public("x ① y", false), "x ① y");
        assert_eq!(crate::normalize_public("Forty-Two ½", false), "forty two ½");
        assert_eq!(crate::normalize_public("x ① y", true), "x 1 y");
        assert_eq!(crate::normalize("x ① y"), "x 1 y");
        // A compatibility ideograph is `Lo`, its ideograph, no symbol.
        for c in ['\u{F9B2}', '\u{F9D1}', '\u{F973}'] {
            assert!(!crate::is_compat_digit(c), "{c}");
        }
        // And the default is back after the call.
        assert!(crate::words2num("①", "en", "cardinal").is_err());
        assert!(crate::words2num("10", "en", "cardinal").is_ok());
        let p = std::collections::HashMap::new();
        assert!(super::auto_parse("10²", "en", &p, None, None).is_err());
    }
}

#[cfg(test)]
mod elided_place_tests {
    /// #875: th / lo / km elide the place of a digit after a bare scale
    /// word; after a multiplier the spelling is also the renderer's units
    /// reading, so it is no one number in running text.
    #[test]
    fn digit_after_scale_word_issue_875() {
        let card = |t: &str, l: &str| match crate::words2num(t, l, "cardinal") {
            Ok(crate::w2n_lang_en::W2nValue::Int(v)) => Some(v.to_string()),
            _ => None,
        };
        for (t, l, v) in [
            ("หมื่นห้า", "th", "15000"),
            ("ล้านสอง", "th", "1200000"),
            ("พันห้า", "th", "1500"),
            ("ติดลบหมื่นห้า", "th", "-15000"),
            ("ໝື່ນຫ້າ", "lo", "15000"),
            ("ម៉ឺនប្រាំ", "km", "15000"),
            ("ពាន់ប្រាំពីរ", "km", "1700"),
            // The renderer's own spellings keep their reading.
            ("สองพันห้า", "th", "2005"),
            ("หนึ่งหมื่นห้า", "th", "10005"),
            ("ສອງພັນ ຫ້າ", "lo", "2005"),
            ("ពីរពាន់ ប្រាំ", "km", "2005"),
            ("สองพันห้าร้อย", "th", "2500"),
        ] {
            assert_eq!(card(t, l).as_deref(), Some(v), "{l} {t}");
        }
        let sent = |t: &str, l: &str| super::words2num_sentence(t, l, "cardinal", false).unwrap();
        for (t, l, want) in [
            ("ราคาสองพันห้า", "th", "ราคาสองพันห้า"),
            ("รถคันนี้ราคาแปดแสนห้า", "th", "รถคันนี้ราคาแปดแสนห้า"),
            ("เงินเดือนประมาณหมื่นห้า", "th", "เงินเดือนประมาณ15000"),
            ("บ้านหลังนี้ราคาล้านสอง", "th", "บ้านหลังนี้ราคา1200000"),
            ("ราคาสองพันห้าร้อยบาท", "th", "ราคา2500บาท"),
            ("ລາຄາ ສອງພັນ ຫ້າ ກີບ", "lo", "ລາຄາ ສອງພັນ ຫ້າ ກີບ"),
            ("ម៉ឺនប្រាំ រៀល", "km", "15000 រៀល"),
            ("តម្លៃ ពីរពាន់ ប្រាំ រៀល", "km", "តម្លៃ ពីរពាន់ ប្រាំ រៀល"),
        ] {
            assert_eq!(sent(t, l), want, "{l} {t}");
        }
        // No year label is guessed: "per year" amounts and year ranges stay
        // words alike (#967, #968).
        for (t, l) in [
            ("ปีสองพันห้า", "th"),
            ("ค่าธรรมเนียมต่อปีสองพันห้าบาท", "th"),
            ("ปีสองพันห้าถึงสองพันหก", "th"),
            ("ຄ່າທຳນຽມຕໍ່ປີ ສອງພັນ ຫ້າ ກີບ", "lo"),
            ("ថ្លៃប្រចាំឆ្នាំ ពីរពាន់ ប្រាំ រៀល", "km"),
        ] {
            assert_eq!(sent(t, l), t, "{l} {t}");
        }
        // A decimal's whole part takes the same reading (a bare scale word is
        // never the renderer's).
        assert_eq!(format!("{:?}", crate::words2num("หมื่นห้าจุดห้า", "th", "cardinal").unwrap()), format!("{:?}", crate::words2num("15000.5", "en", "cardinal").unwrap()));
        // Not my: "နှစ်ထောင့်ငါး" is 2005.
        assert!(crate::elided_place("my", "နှစ်ထောင့်ငါး").is_none());
    }
}

#[cfg(test)]
mod area_and_indicator_tests {
    use super::{auto_parse, auto_parse_sentence};
    use std::collections::HashMap;

    /// #896: area and volume units, and ordinal indicators.
    #[test]
    fn area_units_and_ordinal_indicators_issue_896() {
        let p = HashMap::new();
        for (t, u) in [("5 m²", "m²"), ("5m²", "m²"), ("5 km²", "km²"), ("10 m³", "m³"), ("5 m2", "m²"), ("5 m^2", "m²"), ("2 cm³", "cm³")] {
            let q = auto_parse(t, "en", &p, None, None).unwrap();
            assert_eq!(q.unit.as_deref(), Some(u), "{t}");
        }
        assert_eq!(auto_parse_sentence("a 50 m² flat", "en", &p, None, None, true).unwrap(), "a 50 square meters flat");
        assert_eq!(auto_parse_sentence("a 50 m² flat", "en", &p, None, None, false).unwrap(), "a 50 m² flat");
        // "5 m" is still metres, "5 m 2" no area.
        assert_eq!(auto_parse("5 m", "en", &p, None, None).unwrap().unit.as_deref(), Some("m"));
        for (t, l) in [("1º", "es"), ("1ª", "pt"), ("1.º", "es"), ("21º", "es")] {
            let v = crate::words2num(t, l, "ordinal").unwrap();
            let n: i64 = if t.starts_with("21") { 21 } else { 1 };
            assert_eq!(v, crate::w2n_lang_en::W2nValue::Int(n.into()), "{l} {t}");
        }
        // auto_parse reads the indicator too, with the locale's grouping.
        for (t, n) in [("1º", 1), ("1.000º", 1000), ("1ª", 1)] {
            assert_eq!(auto_parse(t, "es", &p, None, None).unwrap().value, super::W2nValue::Int(n.into()), "{t}");
        }
        // A fraction takes no indicator.
        for (t, l) in [("1,5º", "es"), ("0,5º", "es"), ("2.5ª", "pt"), ("1.00º", "es"), ("1.000º", "en"), ("1.000º", "es_MX")] {
            assert!(crate::words2num(t, l, "ordinal").is_err(), "{l} {t}");
            assert!(crate::words2num(t, l, "cardinal").is_err(), "{l} {t}");
            assert!(auto_parse(t, l, &p, None, None).is_err(), "{l} {t}");
        }
        // Expand mode pluralizes the foot forms.
        let ex = |t: &str| auto_parse_sentence(t, "en", &p, None, None, true).unwrap();
        assert_eq!(ex("x 2.5 ft² y"), "x 2.5 square feet y");
        assert_eq!(ex("5 ft^3 box"), "5 cubic feet box");
        // A glued digit in running text is a product code; "in2" no unit at all;
        // a power running into a letter or digit is no power.
        for t in ["5 m2 slots", "1 in2 adults", "5 m²a", "5 mm²²"] {
            assert_eq!(auto_parse_sentence(t, "en", &p, None, None, false).unwrap(), t, "{t}");
        }
        for t in ["5 in2", "5 m²a", "5 m^²", "5 Mm²"] {
            assert!(auto_parse(t, "en", &p, None, None).is_err(), "{t}");
        }
        // Running text folds a powered unit as auto_parse does (#953).
        assert_eq!(ex("5 KM² area"), "5 square kilometers area");
        assert_eq!(ex("5 FT³ box"), "5 cubic feet box");
        assert_eq!(auto_parse_sentence("5 Km^2 area", "en", &p, None, None, false).unwrap(), "5 km² area");
        assert_eq!(ex("5 Mm² area"), "5 Mm² area");
        // A folded powered unit is no certain reading.
        let q = auto_parse("5 KM²", "en", &p, None, None).unwrap();
        assert_eq!((q.unit.as_deref(), q.confidence), (Some("km²"), 0.8));
    }
}

#[cfg(test)]
mod auto_parse_sign_tests {
    use super::{auto_parse, W2nValue};
    use std::collections::HashMap;

    /// #896: auto_parse reads a sign before a figure, and the ar decimal,
    /// as words2num does; a comma split from the sign is punctuation.
    #[test]
    fn auto_parse_sign_word_figure_issue_896() {
        let p = HashMap::new();
        let v = |t: &str, l: &str| auto_parse(t, l, &p, None, None).unwrap().value;
        assert_eq!(v("moins 1.234", "fr"), W2nValue::Int((-1234).into()));
        assert_eq!(v("moins , 1.234", "fr"), W2nValue::Int((-1234).into()));
        assert_eq!(v("moins ,1.234", "fr"), W2nValue::Int((-1234).into()));
        assert_eq!(format!("{:?}", v("-، 50", "ar")), format!("{:?}", W2nValue::Dec("-0.50".parse().unwrap())));
        let q = auto_parse("moins 1.234 kg", "fr", &p, None, None).unwrap();
        assert_eq!(q.value, W2nValue::Int((-1234).into()));
        assert_eq!(crate::words2num("moins , 1.234", "fr", "cardinal").unwrap(), crate::w2n_lang_en::W2nValue::Int((-1234).into()));
        // The caller's separators and region hold on this path too.
        let dec = |v: W2nValue| format!("{v:?}");
        let with = |t: &str, l: &str, ts: Option<&str>, ds: Option<&str>| auto_parse(t, l, &p, ts, ds).unwrap().value;
        assert_eq!(dec(with("moins 1.234", "fr", None, Some("."))), dec(W2nValue::Dec("-1.234".parse().unwrap())));
        assert_eq!(dec(with("moins 1.234 kg", "fr", None, Some("."))), dec(W2nValue::Dec("-1.234".parse().unwrap())));
        assert_eq!(dec(with("minus 1,234", "en", None, Some(","))), dec(W2nValue::Dec("-1.234".parse().unwrap())));
        assert_eq!(dec(v("menos 1.234", "es_MX")), dec(W2nValue::Dec("-1.234".parse().unwrap())));
        assert_eq!(v("menos 1,234", "es_MX"), W2nValue::Int((-1234).into()));
        // A dash and a comma are a sign with separators given too.
        assert_eq!(with("-, 5", "en", None, Some(".")), W2nValue::Int((-5).into()));
        assert_eq!(dec(with("-, 1.234", "fr", None, Some(","))), dec(W2nValue::Int((-1234).into())));
        assert_eq!(dec(with("-, 1.234", "fr", None, Some("."))), dec(W2nValue::Dec("-1.234".parse().unwrap())));
        assert!(auto_parse("1.5", "en", &p, None, Some(",")).is_err());
        // #952: a figure before a unit reads with the caller's separators or
        // not at all, as a bare one does.
        for (t, l, ts, ds) in [("1.5 km", "en", None, Some(",")), ("3.14159 kg", "en", None, Some(",")), ("12.5 km", "de", Some("."), Some(",")), ("-2.5 kg", "en", None, Some(","))] {
            assert!(auto_parse(t, l, &p, ts, ds).is_err(), "{l} {t}");
        }
        assert_eq!(dec(with("1,5 km", "en", None, Some(","))), dec(W2nValue::Dec("1.5".parse().unwrap())));
        assert_eq!(with("12.500 km", "de", Some("."), Some(",")), W2nValue::Int(12500.into()));
        assert_eq!(with("five km", "en", None, Some(",")), W2nValue::Int(5.into()));
        // #976: a typographic minus or fullwidth digits too, with or without separators.
        for (t, l, ts, ds) in [("\u{2212}2.5 kg", "en", None, Some(",")), ("\u{2013}3.75 m", "fr", Some(" "), Some(",")), ("\u{FF11}.\u{FF15} km", "en", None, Some(",")), ("\u{FF11}\u{FF12}.\u{FF15} km", "de", Some("."), Some(","))] {
            assert!(auto_parse(t, l, &p, ts, ds).is_err(), "{l} {t}");
        }
        assert_eq!(with("\u{2212}1.234 km", "de", Some("."), Some(",")), W2nValue::Int((-1234).into()));
        assert_eq!(v("\u{2212}1.234 kg", "de"), v("\u{2212}1.234", "de"));
        // #979: what the figure parser declines still reads without separators.
        assert_eq!(v("2024.", "en"), W2nValue::Int(2024.into()));
        assert_eq!(v("-1.", "en"), W2nValue::Int((-1).into()));
        assert!(matches!(v(".5", "de"), W2nValue::Float(f) if f == 0.5), "as on main");
        assert_eq!(auto_parse("12. km", "en", &p, None, None).unwrap().value, W2nValue::Int(12.into()));
        // The marks the parser takes by default, with no separators given.
        assert_eq!(dec(v("1\u{2019}234.5 kg", "en")), dec(v("1\u{2019}234.5", "en")));
        assert_eq!(dec(v("1\u{202F}234,5 percent", "fr")), dec(v("1\u{202F}234,5", "fr")));
        // The caller's own separator characters (ar "٫", de_CH "’").
        assert_eq!(dec(auto_parse("٣٫٥ km", "ar", &p, None, Some("٫")).unwrap().value), dec(W2nValue::Dec("3.5".parse().unwrap())));
        assert_eq!(auto_parse("1٬234 km", "ar", &p, Some("٬"), Some("٫")).unwrap().value, W2nValue::Int(1234.into()));
        // "AR" is ar: its "،" stays the decimal mark.
        assert_eq!(format!("{:?}", crate::words2num("-، 50", "AR", "cardinal").unwrap()), format!("{:?}", crate::words2num("-، 50", "ar", "cardinal").unwrap()));
    }
}

#[cfg(test)]
mod region_decimal_word_tests {
    /// #951: a spoken decimal is written with the region's mark, so the
    /// figure reads back as the same number in that region.
    #[test]
    fn spoken_decimal_takes_the_region_mark_issue_951() {
        assert_eq!(super::words2num_sentence("x menos tres coma uno cuatro uno y", "es_MX", "cardinal", false).unwrap(), "x menos 3.141 y");
        // fr_CH reads a single separator before three digits the CLDR way.
        let p = std::collections::HashMap::new();
        assert_eq!(super::auto_parse("1.234", "fr_CH", &p, None, None).unwrap().value, super::W2nValue::Int(1234.into()));
        let s = |t: &str, l: &str| super::words2num_sentence(t, l, "cardinal", false).unwrap();
        for (t, l, want) in [
            ("x tres coma uno cuatro uno y", "es_MX", "x 3.141 y"),
            ("x drei Komma eins vier eins y", "de_CH", "x 3.141 y"),
            ("x tre virgola uno quattro uno y", "it_CH", "x 3.141 y"),
            ("x un virgule deux trois quatre y", "fr_CH", "x 1,234 y"),
            ("x tres punto uno cuatro uno y", "es", "x 3,141 y"),
            ("x tres punto uno y", "es_MX", "x 3.1 y"),
            ("x tres coma uno cuatro uno y", "ES-mx", "x 3.141 y"),
            ("x drei Komma eins vier eins y", "de_LI", "x 3.141 y"),
            ("x tres coma uno cuatro uno y", "es_419", "x 3.141 y"),
            ("x três ponto um quatro um y", "pt_BR", "x 3,141 y"),
            ("x tre punto uno quattro uno y", "it", "x 3,141 y"),
            ("x drei Komma fünf y", "de", "x 3,5 y"),
        ] {
            let out = s(t, l);
            assert_eq!(out, want, "{l} {t}");
            let fig = out.split_whitespace().nth(1).unwrap();
            assert_eq!(format!("{:?}", crate::words2num(fig, l, "cardinal").unwrap()), format!("{:?}", crate::words2num(&t[2..t.len() - 2], l, "cardinal").unwrap()), "{l} {fig}");
        }
    }
}

#[cfg(test)]
mod glued_morpheme_tests {
    /// #947: a tens morpheme the renderer writes glued may be spoken apart;
    /// a stray fragment still is no number (#285).
    #[test]
    fn spaced_tens_morpheme_issue_947() {
        let card = |t: &str, l: &str| crate::words2num(t, l, "cardinal").ok().map(|v| format!("{v:?}"));
        let int = |n: i64| Some(format!("{:?}", crate::w2n_lang_en::W2nValue::Int(n.into())));
        for (t, l, n) in [("kolme kymmentä", "fi", 30), ("două zeci", "ro", 20), ("kaks kümmend", "et", 20), ("huszon kettő", "hu", 22), ("veinti tres", "es", 23), ("kaks kümmend kolm", "et", 23)] {
            assert_eq!(card(t, l), int(n), "{l} {t}");
        }
        // Nor an ordinary word beside a number (#982).
        for (t, l) in [("d eux", "fr"), ("ba y", "vi"), ("twee de", "nl"), ("fem ton", "sv"), ("før ti", "nb"), ("sie ben", "de"), ("dici otto", "it"), ("hat van", "hu")] {
            assert_eq!(card(t, l), None, "{l} {t}");
        }
        let s = |t: &str, l: &str| super::words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(s("hän on kolme kymmentä viisi vuotta", "fi"), "hän on 35 vuotta");
        assert_eq!(s("trei zeci de oameni", "ro"), "30 de oameni");
        assert_eq!(s("x d eux y", "fr"), "x d eux y");
        assert_eq!(s("kaks kümmend kolm aastat", "et"), "23 aastat");
        assert_eq!(s("două zeci și trei", "ro"), "23");
        assert_eq!(s("lastbilen väger fem ton", "sv"), "lastbilen väger 5 ton");
        assert_eq!(s("kom hjem før ti", "nb"), "kom hjem før 10");
        assert_eq!(s("perché dici otto volte", "it"), "perché dici 8 volte");
        // #987: a morpheme that is an everyday word stays one.
        assert_eq!(s("ագարակում կա յոթ անասուն", "hy"), "ագարակում կա 7 անասուն");
        assert_eq!(card("յոթ անասուն", "hy"), None);
        // #988: a unit a suffix morpheme follows belongs to the next tens.
        assert_eq!(s("viisi kymmentä kuusi kymmentä vuotta", "fi"), "50 60 vuotta");
        assert_eq!(s("viis kümmend kuus kümmend aastat", "et"), "50 60 aastat");
        assert_eq!(s("два ццаць тры ццаць", "be"), "20 30");
        // #993: only a unit the morpheme glues into a number.
        assert_eq!(s("pět desát šest desát", "cs"), "5 desát 6 desát");
        assert_eq!(s("sata kymmentä", "fi"), "100 kymmentä");
        assert_eq!(s("zece zeci de ani", "ro"), "10 zeci de ani");
        // #990: a new tens group ends the run.
        assert_eq!(s("veinti uno veinti dos veinti tres", "es"), "21 22 23");
        assert_eq!(s("kolme kymmentä kolme kymmentä", "fi"), "30 30");
        assert_eq!(s("kolme kymmentä viisi kolme kymmentä viisi", "fi"), "35 35");
    }
}

#[cfg(test)]
mod scale_comma_normalize_tests {
    /// #948: the comma num2words2 writes after a scale word is crossed
    /// whatever the accents: pt "milhoes," as "milhões,", and NFD text.
    #[test]
    fn scale_comma_ignores_accents_issue_948() {
        let s = |t: &str| super::words2num_sentence(t, "pt_BR", "cardinal", false).unwrap();
        assert_eq!(s("dois milhoes, trezentos mil"), "2300000");
        assert_eq!(s("um milhao, duzentos mil"), "1200000");
        assert_eq!(s("dois bilhoes, trezentos milhoes"), "2300000000");
        assert_eq!(s("dois milho\u{303}es, trezentos mil"), "2300000");
        assert_eq!(s("DOIS MILHOES, TREZENTOS MIL"), "2300000");
        assert_eq!(s("em dois mil, cinco pessoas vieram"), "em 2000, 5 pessoas vieram");
    }
}

#[cfg(test)]
mod glued_sign_figure_tests {
    /// #950: a sign glued to a figure, or of several words, reads as "-figure".
    #[test]
    fn glued_and_multiword_signs_issue_950() {
        let w = |t: &str, l: &str, to: &str| crate::words2num(t, l, to).ok().map(|v| format!("{v:?}"));
        let same = |a: &str, b: &str, l: &str, to: &str| assert_eq!(w(a, l, to), w(b, l, to), "{l} {a}");
        same("负1234", "-1234", "zh", "cardinal");
        same("負1234", "-1234", "zh_TW", "cardinal");
        same("マイナス2.75", "-2.75", "ja", "cardinal");
        same("ติดลบ1234", "-1234", "th", "cardinal");
        same("ติดลบ1999", "-1999", "th", "year");
        same("ngaphansi kwe 1.5", "-1.5", "zu", "cardinal");
        same("ngaphansi kwe 1 234", "-1 234", "zu", "cardinal");
        same("wansi wa 2.75", "-2.75", "lg", "cardinal");
        assert!(w("-1234", "zh", "cardinal").is_some());
        // Unchanged: a word sign glued to a figure where the renderer spaces it.
        assert_eq!(w("minus5", "en", "cardinal"), None);
        assert_eq!(w("moins1.234", "fr", "cardinal"), None);
        same("moins, 1.234", "-1.234", "fr", "cardinal");
        // A sign word with a mark, then a space, then the figure (#1032).
        same("moins: 1.234", "-1.234", "fr", "cardinal");
        same("minus: 1.234", "-1.234", "de", "cardinal");
        same("\u{043C}\u{0438}\u{043D}\u{0443}\u{0441}: 1.234", "-1.234", "ru", "cardinal");
        same("menos: 1.234", "-1.234", "es", "cardinal");
        same("moins: 1 234", "-1 234", "fr", "cardinal");
        same("minus: 1,234", "-1,234", "en", "cardinal");
        same("minus. 1 234", "-1 234", "en", "cardinal");
        same("minus: 1,5", "-1,5", "de", "cardinal");
        same("moins; 2,5", "-2,5", "fr", "cardinal");
        same("-. 5", "-5", "en", "cardinal");
        // Both sides read, not both fail.
        assert!(w("moins: 1.234", "fr", "cardinal").is_some());
        assert!(w("minus: 1,234", "en", "cardinal").is_some());
        assert_eq!(w("-:5", "en", "cardinal"), None);
        assert_eq!(w("-(5", "en", "cardinal"), None);
        // #984: a point before the figure is the figure's, a bare dash glued to
        // a comma no sign word, and other punctuation no part of a sign.
        assert_eq!(w("-.5", "de", "cardinal").as_deref(), Some("Float(-0.5)"), "as on main");
        assert_eq!(w("\u{8D1F} .5", "zh", "cardinal").as_deref(), Some("Float(-0.5)"), "as on main");
        assert_eq!(w("-.1999", "de", "cardinal").as_deref(), Some("Float(-0.1999)"), "as on main");
        for (t, l) in [("-(5", "en"), ("-:5", "en"), ("-,5", "en"), ("\u{8D1F}.5", "zh"), ("-.,5", "en"), ("-(,5", "en"), ("- .,5", "en")] {
            assert_eq!(w(t, l, "cardinal"), None, "{l} {t}");
        }
        let p = std::collections::HashMap::new();
        assert!(super::auto_parse("-,5", "fr", &p, None, Some(",")).is_err());
    }
}

#[cfg(test)]
mod format_char_punct_tests {
    /// #870: an invisible format character after a mark rides with it.
    #[test]
    fn format_char_after_mark_issue_870() {
        let s = |t: &str, l: &str| super::words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(s("one hundred.\u{200B} Five people", "en"), "100.\u{200B} 5 people");
        // "one" ending its clause after a number counts (#911), as in "twenty, one".
        assert_eq!(s("twenty,\u{200B} one", "en"), "20,\u{200B} 1");
        assert_eq!(s("twenty;\u{00AD} one", "en"), "20;\u{00AD} one");
        assert_eq!(s("zwanzig.\u{2060} Drei Leute", "de"), "20.\u{2060} 3 Leute");
        // A bare format character is a word-break hint, no mark (#998).
        assert_eq!(s("\u{4E8C}\u{767E}\u{200B}\u{4E94}\u{5341}", "zh"), "250");
        assert_eq!(s("สองร้อย\u{200B}ห้าสิบ", "th"), "250");
        // It still ends a run that a space ends, and is kept (#1005).
        assert_eq!(s("one\u{200B} twenty-one", "en"), "1\u{200B} 21");
        assert_eq!(s("x five\u{200B} y", "en"), "x 5\u{200B} y");
        assert_eq!(s("x five\u{2060} y", "en"), "x 5\u{2060} y");
        assert_eq!(s("five\u{00AD} apples", "en"), "5\u{00AD} apples");
        assert_eq!(s("x f\u{FC}nf\u{200B} y", "de"), "x 5\u{200B} y");
        assert_eq!(s("x twenty\u{200B} one", "en"), "x 21");
        // A leading format character and the bidi marks are kept (#1000).
        assert_eq!(s("x \u{200B}five y", "en"), "x \u{200B}5 y");
        assert_eq!(s("twenty.\u{200E} One", "en"), "20.\u{200E} One");
        assert_eq!(s("\u{0639}\u{0634}\u{0631}\u{0648}\u{0646}\u{060C}\u{200F} \u{0648}\u{0627}\u{062D}\u{062F}", "ar"), "20\u{060C}\u{200F} 1");
        // currency=True folds the amount, the character after it (#1021); a
        // date or phone pass folds as before, without it (#1020).
        let c = |t: &str, l: &str| super::words2num_sentence_opts(t, l, "cardinal", false, true).unwrap();
        assert_eq!(c("x twenty\u{200B} dollars", "en"), "x $20\u{200B}");
        assert_eq!(c("it costs twelve\u{200B} dollars", "en"), "it costs $12\u{200B}");
        assert_eq!(c("twelve dollars and fifty\u{200B} cents", "en"), "$12.50\u{200B}");
        assert_eq!(c("twelve dollars fifty\u{200B}", "en"), "$12.50\u{200B}");
        assert_eq!(c("ninety nine\u{200B} cents", "en"), "$0.99\u{200B}");
        assert_eq!(c("douze\u{200B} euros", "fr"), "12 \u{20AC}\u{200B}");
        assert_eq!(c("zw\u{F6}lf Euro f\u{FC}nfzig\u{200B}", "de"), "12,50 \u{20AC}\u{200B}");
        assert_eq!(c("x 20\u{200B} dollars", "en"), "x $20\u{200B}");
        let full = |t: &str, date: bool, phone: bool| {
            super::words2num_sentence_full(t, "en", "cardinal", false, false, date, None, "written", phone).unwrap()
        };
        assert_eq!(full("on the fifth\u{200B} of march", true, false), "on March 5");
        assert_eq!(full("the twelfth of march two thousand twenty six\u{200B}", true, false), "March 12, 2026");
        assert_eq!(
            full("call zero six one two three four five six seven eight\u{200B}", false, true),
            full("call zero six one two three four five six seven eight", false, true)
        );
        // Without a fold the character is kept.
        assert_eq!(full("x five\u{200B} y", false, false), "x 5\u{200B} y");
        assert_eq!(s("twenty one", "en"), "21");
        // A closing bracket, quote or script mark with a format character is
        // cut off whole, as without it (#1004).
        assert_eq!(s("twenty)\u{200B} one", "en"), "20)\u{200B} one");
        assert_eq!(s("I counted twenty.)\u{200B} One more", "en"), "I counted 20.)\u{200B} One more");
        assert_eq!(s("one hundred)\u{200B}", "en"), "100)\u{200B}");
        assert_eq!(s("twenty\u{201D}\u{200B} one", "en"), "20\u{201D}\u{200B} one");
        assert_eq!(s("\u{0639}\u{0634}\u{0631}\u{0648}\u{0646}\u{060C}\u{200B} \u{0648}\u{0627}\u{062D}\u{062F}", "ar"), "20\u{060C}\u{200B} 1");
        assert_eq!(s("x zwanzig tausend\u{3002}\u{200B} drei", "de"), "x 20000\u{3002}\u{200B} 3");
        // A format character before the mark reads as without it (#1011),
        // and is kept with the mark (#1005).
        assert_eq!(s("twenty\u{2060}) one", "en"), "20\u{2060}) one");
        assert_eq!(s("vingt\u{2060}\u{BB} trois", "fr"), "20\u{2060}\u{BB} 3");
        assert_eq!(s("x twenty\u{200B}) one y", "en"), "x 20\u{200B}) one y");
        assert_eq!(s("x zwanzig\u{200B}) drei y", "de"), "x 20\u{200B}) 3 y");
    }
}

#[cfg(test)]
mod auto_sentence_mark_tests {
    /// #867: a decimal is written with the locale's mark, so a second pass
    /// reads the same number.
    #[test]
    fn decimal_mark_round_trips_issue_867() {
        let p = std::collections::HashMap::new();
        let a = |t: &str, l: &str| super::auto_parse_sentence(t, l, &p, None, None, false).unwrap();
        for (t, l, want) in [
            ("3,125 kg", "de", "3,125 kg"),
            ("0,125 kg", "de", "0,125 kg"),
            ("3,125 €", "de", "3,125 EUR"),
            ("3,125 %", "fr", "3,125%"),
            ("3.5 kg", "de", "3,5 kg"),
            ("3.5 kg", "en", "3.5 kg"),
        ] {
            let once = a(t, l);
            assert_eq!(once, want, "{l} {t}");
            assert_eq!(a(&once, l), once, "{l} {t} twice");
        }
        // The caller's decimal mark wins.
        assert_eq!(super::auto_parse_sentence("3,5 kg", "en", &p, None, Some(","), false).unwrap(), "3,5 kg");
        // A thousands separator alone makes the point the decimal, as the
        // parser reads it.
        let ts = |t: &str| super::auto_parse_sentence(t, "de", &p, Some(","), None, false).unwrap();
        assert_eq!(ts("3.125 kg"), "3.125 kg");
        assert_eq!(ts(&ts("3.125 kg")), "3.125 kg");
    }
}

#[cfg(test)]
mod plain_walk_subunit_tests {
    /// #868: the plain walk reads a fr US-cent subunit as cents, not hundreds.
    #[test]
    fn subunit_after_unit_issue_868() {
        let s = |t: &str, l: &str, to: &str| super::words2num_sentence(t, l, to, false).unwrap();
        assert_eq!(s("douze dollars et cinquante cents", "fr", "cardinal"), "12 dollars et 50 cents");
        assert_eq!(s("zéro dollar et nonante-neuf cents", "fr_CH", "cardinal"), "0 dollar et 99 cents");
        assert_eq!(s("douze dollars et cinquante cents", "fr_DZ", "currency"), "12 dollars et 50 cents");
        // A real hundreds multiple stays one.
        assert_eq!(s("mille euros et deux cents personnes", "fr", "cardinal"), "1000 euros et 200 personnes");
        // A subunit no multiplier is not cut, so "un" before it reads (#1001).
        assert_eq!(s("un euro et un centime", "fr", "cardinal"), "1 euro et 1 centime");
        assert_eq!(s("un euro amb un cèntim", "ca", "cardinal"), "1 euro amb 1 cèntim");
        // Hundreds stay hundreds after a unit and a connector (#1002, #1003).
        assert_eq!(s("vingt euros et mille deux cent cinquante personnes", "fr", "cardinal"), "20 euros et 1250 personnes");
        assert_eq!(s("le billet coûte cinq dollars et la salle compte quatre cents places", "fr", "cardinal"), "le billet coûte 5 dollars et la salle compte 400 places");
        assert_eq!(s("vingt dollars et douze cents personnes", "fr", "cardinal"), "20 dollars et 1200 personnes");
        assert_eq!(s("dix dollars et cinq. Trois cents personnes", "fr", "cardinal"), "10 dollars et 5. 300 personnes");
        assert_eq!(s("douze dollars et douze cents", "fr", "cardinal"), "12 dollars et 12 cents");
        // The connector after the unit, not the "et" inside the count (#1010).
        assert_eq!(s("douze dollars et vingt et un cents", "fr_BE", "cardinal"), "12 dollars et 21 cents");
        assert_eq!(s("douze dollars et nonante et un cents", "fr_CH", "cardinal"), "12 dollars et 91 cents");
        // A one-word count is the amount, no article or adjective (#1009).
        assert_eq!(s("un dollar et un cent", "fr", "cardinal"), "1 dollar et 1 cent");
        assert_eq!(s("deux dollars et neuf cents", "fr", "cardinal"), "2 dollars et 9 cents");
        assert_eq!(s("deux dollars et neuf cents de plus", "fr", "cardinal"), "2 dollars et 900 de plus");
        assert_eq!(s("douze dollars et (vingt cents)", "fr", "cardinal"), "12 dollars et (20 cents)");
        assert_eq!(s("un dollar et (un cent)", "fr", "cardinal"), "1 dollar et (un cent)");
        assert_eq!(s("douze dollars et neuf\u{2026} cents", "fr", "cardinal"), "12 dollars et 9\u{2026} cents");
        assert_eq!(s("douze dollars et vingt( cents", "fr", "cardinal"), "12 dollars et 20( cents");
        assert_eq!(s("douze dollars et vingt cents", "fr", "ordinal"), "douze dollars et vingt cents");
        assert_eq!(s("douze dollars et vingt cents", "fr", "digits"), "douze dollars et vingt cents");
        let co = |t: &str, to: &str| super::words2num_sentence_opts(t, "fr", to, false, true).unwrap();
        assert_eq!(co("douze dollars et vingt cents", "ordinal"), "douze dollars et vingt cents");
        assert_eq!(co("douze dollars et un cent chacun", "cardinal"), "12,01 $ chacun");
        // currency=True: the count is a plain number naming no currency,
        // not across a sentence or two "et"-joined numbers (#1013, #1017).
        let c = |t: &str, l: &str| super::words2num_sentence_opts(t, l, "cardinal", false, true).unwrap();
        assert_eq!(c("douze dollars et vingt et un cents", "fr_BE"), "12,21 $");
        assert_eq!(c("cinquante dollars et soixante cents et deux cents personnes", "fr_BE"), "50,60 $ et 200 personnes");
        assert_eq!(c("dix euros et cinquante centimes et trois cents personnes", "fr_CH"), "10,50 € et 300 personnes");
        assert_eq!(c("cent dollars et vingt cents et quatre cents invités", "fr_CA"), "100,20 $ et 400 invités");
        assert_eq!(c("dix dollars et cinq et deux cents personnes", "fr"), "10 $ et 5 et 200 personnes");
        assert_eq!(c("mille euros et cinq et trois cents personnes", "fr"), "1 000 € et 5 et 300 personnes");
        assert_eq!(c("dix dollars et deux cents", "fr"), "10,02 $");
        assert_eq!(c("dix dollars et quinze cents chacun", "fr"), "10,15 $ chacun");
        // The renderer's x.00: "et zéro cent" is a subunit amount (#1019).
        assert_eq!(c("douze dollars et zéro cent", "fr"), "12,00 $");
        assert_eq!(c("un dollar et zéro cent", "fr"), "1,00 $");
        assert_eq!(c("douze dollars et zéro cent", "fr_CH"), "12,00 $");
        assert_eq!(c("cent dollars et zéro cent", "fr_BE"), "100,00 $");
        assert_eq!(c("douze dollars et zéro cents", "fr"), "12,00 $");
        // A count in figures is a subunit amount, never hundreds.
        assert_eq!(c("12 dollars et 50 cents", "fr"), "12,50 $");
        assert_eq!(c("douze dollars et 99 cents", "fr"), "12,99 $");
        assert_eq!(c("12 dollars et 2 cents personnes", "fr"), "12,02 $ personnes");
        assert_eq!(c("12 dollars et 21 cents", "fr_BE"), "12,21 $");
        assert_eq!(c("douze dollars et 150 cents", "fr"), "12 $ et 150 cents");
        // A count that is one amount but no integer stays as written (#1022).
        assert_eq!(c("douze dollars et deux virgule cinq cents", "fr"), "12 $ et 2,5 cents");
        assert_eq!(c("douze dollars et deux virgule cinq cent", "fr"), "12 $ et 2,5 cent");
        assert_eq!(c("douze dollars et mille virgule cinq cent", "fr"), "12 $ et 1000,5 cent");
        assert_eq!(c("12 dollars et 2,5 cents", "fr"), "12 $ et 2,5 cents");
        assert_eq!(c("12 dollars et 2.5 cents", "fr_CA"), "12 $ et 2.5 cents");
        assert_eq!(c("12 dollars et \u{665}\u{660} cents", "fr"), "12 $ et \u{665}\u{660} cents");
        assert_eq!(c("12 dollars et 99999999999999999999 cents", "fr"), "12 $ et 99999999999999999999 cents");
        // A count of 100 or more keeps "cents" as written; "mille deux
        // cents personnes" is still 1200 (#1025).
        assert_eq!(c("douze dollars et cent cinquante cents", "fr"), "12 $ et 150 cents");
        assert_eq!(c("douze dollars et quatre cents cinquante cents", "fr"), "12 $ et 450 cents");
        assert_eq!(c("douze dollars et mille cents", "fr"), "12 $ et 1000 cents");
        assert_eq!(c("douze dollars et cent un cents", "fr_BE"), "12 $ et 101 cents");
        assert_eq!(c("douze dollars et cent cinquante cents personnes", "fr"), "12 $ et 150 cents personnes");
        assert_eq!(c("douze dollars et \u{216B} cents", "fr"), "12 $ et \u{216B} cents");
        assert_eq!(c("dix dollars et mille deux cents personnes", "fr"), "10 $ et 1200 personnes");
        // Unless "cent" extends the count into one number (#1029).
        assert_eq!(c("dix euros et deux mille cent personnes", "fr_BE"), "10 \u{20AC} et 2100 personnes");
        assert_eq!(c("vingt euros et mille cent cinquante personnes", "fr"), "20 \u{20AC} et 1150 personnes");
        assert_eq!(c("vingt euros et mille cent un personnes", "fr"), "20 \u{20AC} et 1101 personnes");
        assert_eq!(c("vingt euros et dix mille cent personnes", "fr"), "20 \u{20AC} et 10100 personnes");
        assert_eq!(c("quinze euro et mille cent", "fr_CA"), "15 \u{20AC} et 1100");
        // A symbol is opaque only as the whole count.
        assert_eq!(c("douze dollars et \u{216B} deux cents personnes", "fr"), "12 $ et \u{216B} 200 personnes");
        // Words that are no count are no amount.
        assert_eq!(c("le billet coûte cinq dollars et la salle compte quatre cents places", "fr"), "le billet coûte 5 $ et la salle compte 400 places");
        // A mid-clause "un cent" is one cent (#1009).
        assert_eq!(s("douze dollars et un cent chacun", "fr", "cardinal"), "12 dollars et 1 cent chacun");
    }
}

#[cfg(test)]
mod morpheme_perf_tests {
    /// #990, #994: spaced morphemes cost about what the glued words cost.
    /// A ratio against the glued text, not wall-clock, so a slow runner does
    /// not matter; before the fix the spaced text took ~8-10x.
    #[test]
    fn spaced_morphemes_cost_like_glued_issue_990() {
        let time = |t: &str, l: &str| {
            let mut best = f64::MAX;
            for _ in 0..3 {
                let start = std::time::Instant::now();
                let _ = super::words2num_sentence(t, l, "cardinal", false);
                best = best.min(start.elapsed().as_secs_f64());
            }
            best
        };
        for (spaced, glued, l) in [
            ("veinti tres", "veintitrés", "es"),
            ("kolme kymmentä viisi", "kolmekymmentäviisi", "fi"),
            ("kaks kümmend kolm", "kakskümmend kolm", "et"),
        ] {
            let a = time(&[spaced; 600].join(" "), l);
            let b = time(&[glued; 600].join(" "), l);
            assert!(a / b.max(1e-3) < 4.0, "{l}: spaced {a:.3}s vs glued {b:.3}s");
        }
    }
}

#[cfg(test)]
mod my_year_counter_tests {
    /// #876: my "နှစ်" after a number is the year, no 2.
    #[test]
    fn year_counter_after_number_issue_876() {
        let s = |t: &str| super::words2num_sentence(t, "my", "cardinal", false).unwrap();
        assert_eq!(s("အသက် နှစ်ဆယ့်တစ် နှစ်"), "အသက် 21 နှစ်");
        assert_eq!(s("ငါး နှစ် ကြာ"), "5 နှစ် ကြာ");
        assert_eq!(s("နှစ်ထောင့်နှစ်ဆယ့်လေး နှစ်"), "2024 နှစ်");
        // The renderer's own spaced two after ten-thousands, and a lone two.
        assert_eq!(s("တစ်သောင်း နှစ်"), "10002");
        assert_eq!(s("နှစ် ယောက်"), "2 ယောက်");
        assert!(crate::words2num("နှစ်ထောင့်နှစ်ဆယ့်လေး နှစ်", "my", "cardinal").is_err());
        assert!(crate::words2num("နှစ်ထောင့်နှစ်ဆယ့်လေး ငါး", "my", "cardinal").is_err());
        // Not across a mark: a new sentence's "two people".
        assert_eq!(s("ငါး။ နှစ် ယောက် လာတယ်"), "5။ 2 ယောက် လာတယ်");
        assert_eq!(s("ငါး, နှစ် ယောက်"), "5, 2 ယောက်");
        assert_eq!(s("ငါး. နှစ် ယောက်"), "5. 2 ယောက်");
        assert_eq!(s("အဆင့် သုံး၊ နှစ် နဲ့ တစ်"), "အဆင့် 3၊ 2 နဲ့ 1");
        assert_eq!(s("ငါး- နှစ် ယောက်"), "ငါး- 2 ယောက်");
        // Before a counter or currency it is the two (#1015).
        assert_eq!(s("ငါးရာ နှစ် ကျပ်"), "502 ကျပ်");
        assert_eq!(s("တစ်ထောင် နှစ် ကျပ်"), "1002 ကျပ်");
        assert_eq!(s("ဈေး ငါးထောင် နှစ် ကျပ်"), "ဈေး 5002 ကျပ်");
        assert_eq!(s("တစ်ရာ နှစ် ယောက်"), "102 ယောက်");
        // Not across a mark: "နှစ်။ ကျပ်" ends on the year.
        assert_eq!(s("ငါးရာ နှစ်။ ကျပ်"), "500 နှစ်။ ကျပ်");
        // Years then another count, not a dictation; a figure before (#1016).
        assert_eq!(s("သုံး နှစ် လေး လ"), "3 နှစ် 4 လ");
        assert_eq!(s("ငါး နှစ် ငါး နှစ်"), "5 နှစ် 5 နှစ်");
        assert_eq!(s("ကိုး နှစ် နှစ် ယောက်"), "9 နှစ် 2 ယောက်");
        assert_eq!(s("5 နှစ်"), "5 နှစ်");
        assert_eq!(s("\u{1045} နှစ်"), "\u{1045} နှစ်");
        // A longer dictation stays one digit string.
        assert_eq!(s("ရှစ် လေး နှစ် တစ် ကြာ"), "8421 ကြာ");
        assert_eq!(s("ကိုး ရှစ် နှစ် သုံး လေး ငါး ခြောက်"), "9823456");
        // A dictation after a number word or a figure keeps its first 2.
        assert_eq!(s("ငါးရာ နှစ် သုံး လေး ငါး ခြောက် ခုနစ်"), "500 234567");
        assert_eq!(s("09 နှစ် ငါး သုံး လေး ငါး ခြောက် ခုနစ် ရှစ်"), "09 25345678");
        assert_eq!(s("တစ် နှစ် နှစ် နှစ်"), "1222");
        assert_eq!(s("5 နှစ် နှစ် နှစ်"), "5 222");
        assert_eq!(s("တစ်ဆယ် နှစ် နှစ် လေး ယောက်"), "10 224 ယောက်");
        assert_eq!(s("ငါးရာ နှစ် နှစ် ငါး နှစ်"), "500 2252");
        assert_eq!(s("ငါး နှစ် သုည နှစ်"), "5202");
        // A leading zero, a currency or unit, a number after the last နှစ်:
        // no years (#1037, #1038, #1042); separators in a figure (#1039).
        assert_eq!(s("သုည နှစ် သုံး လ"), "023 လ");
        assert_eq!(s("01 နှစ် လေး လ"), "01 2 4 လ");
        assert_eq!(s("သုံး နှစ် လေး ကျပ်"), "324 ကျပ်");
        assert_eq!(s("လေး နှစ် လေး နှစ် ဒသမ ငါး"), "4242 ဒသမ 5");
        assert_eq!(s("လေး နှစ် လေး နှစ် ထောင်"), "4242 1000");
        assert_eq!(s("2,024 နှစ်"), "2,024 နှစ်");
        assert_eq!(s("\u{1041}.\u{1045} နှစ်"), "\u{1041}.\u{1045} နှစ်");
        assert_eq!(s("\u{2460} နှစ်"), "\u{2460} 2");
        assert_eq!(s("ငါးရာ နှစ် နှစ် သုံး လ"), "500 223 လ");
        assert_eq!(s("ငါး နှစ် နှစ် နှစ်"), "5222");
        assert_eq!(s("2,024 နှစ် နှစ် လေး ယောက်"), "2,024 224 ယောက်");
        assert_eq!(s("09 နှစ် ငါး နှစ်"), "09 252");
        assert_eq!(s("\u{B2} နှစ် ငါး နှစ်"), "\u{B2} 252");
        assert_eq!(s("1,2,3 နှစ်"), "1,2,3 2");
        assert_eq!(s("\u{B2} သုံး နှစ် လေး လ"), "\u{B2} 3 နှစ် 4 လ");
        assert_eq!(s("သုံး နှစ် လေး ပြား"), "324 ပြား");
        assert_eq!(s("5 နှစ် နှစ်"), "5 နှစ် 2");
        assert_eq!(s("5\u{104B} နှစ် နှစ်"), "5\u{104B} 2 နှစ်");
        assert_eq!(s("2024, နှစ် နှစ်"), "2024, 2 နှစ်");
        // A compatibility digit is no figure.
        assert_eq!(s("\u{B2} နှစ်"), "\u{B2} 2");
        // A digit after the point is no year (#1018).
        assert_eq!(s("ငါး ဒသမ နှစ်"), "5.2");
        assert_eq!(s("တစ် ဒသမ နှစ် ငါး"), "1.25");
        assert_eq!(s("သုည ဒသမ ငါး နှစ်"), "0.52");
        assert_eq!(s("သုည ဒသမ နှစ် နှစ်"), "0.22");
        assert_eq!(s("ငါး ဒသမ နှစ် ကျပ်"), "5.2 ကျပ်");
    }

    /// #876: spaced groups add only below the lowest place filled, in every
    /// language with additive groups: ko and hu as en "2024 2".
    #[test]
    fn spaced_groups_below_lowest_place_issue_876() {
        let s = |t: &str, l: &str| super::words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(s("칠만 이천팔백삼십팔 삼", "ko"), "72838 3");
        assert_eq!(s("천삼십육 이", "ko"), "1036 2");
        assert_eq!(s("ezerháromszáznegyvenhárom kettő", "hu"), "1343 2");
        assert!(crate::words2num("ezerötszáz száz", "hu", "cardinal").is_err());
    }
}

#[cfg(test)]
mod script_mark_tests {
    /// #1023: a script's sentence or clause mark ends the run, as "." does.
    #[test]
    fn script_mark_ends_run_issue_1023() {
        let s = |t: &str, l: &str| super::words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(s("တစ်ရာ။ ငါး ယောက်", "my"), "100။ 5 ယောက်");
        assert_eq!(s("တစ်ရာ၊ ငါး ယောက်", "my"), "100၊ 5 ယောက်");
        assert_eq!(s("တစ်ထောင်။ ငါး", "my"), "1000။ 5");
        assert_eq!(s("ငါး။ ငါး", "my"), "5။ 5");
        assert_eq!(s("x ایک سو۔ پانچ y", "ur"), "x 100۔ 5 y");
        assert_eq!(s("x មួយរយ។ ប្រាំ y", "km"), "x 100។ 5 y");
        assert_eq!(s("x መቶ። አምስት y", "am"), "x 100። 5 y");
        // A glued fullwidth mark is kept between the two (#1026).
        assert_eq!(s("\u{4E94}\u{FF0C}\u{5341}\u{4E8C}", "zh"), "5\u{FF0C}12");
        assert_eq!(s("x \u{4E94}\u{FF01}\u{5341}\u{4E8C} y", "zh"), "x 5\u{FF01}12 y");
        assert_eq!(s("\u{767E}\u{FF0C}\u{5341}\u{4E8C}", "ja"), "100\u{FF0C}12");
        assert_eq!(s("\u{5343}\u{FF1A}\u{5341}\u{4E8C}", "ja"), "1000\u{FF1A}12");
        assert_eq!(s("ร้อย\u{FF0C}สิบสอง", "th"), "100\u{FF0C}12");
        // A run never crosses a glued mark into a lone numeral, nor opens on
        // one: the mark stays, the numeral stays as written (#1028).
        assert_eq!(s("二十，五", "zh"), "20，五");
        assert_eq!(s("一千，五", "zh"), "1000，五");
        assert_eq!(s("二千，五", "ja"), "2000，五");
        assert_eq!(s("ยี่สิบ，ห้า", "th"), "20，ห้า");
        assert_eq!(s("x 二十,五 y", "zh"), "x 20,五 y");
        assert_eq!(s("二十，五，六", "zh"), "20，五，六");
        assert_eq!(s("十二，五，三", "zh"), "12，五，三");
        // Unchanged: a multi-numeral run after the mark, a counted one.
        assert_eq!(s("二十，十二", "zh"), "20，12");
        assert_eq!(s("二十，五个", "zh"), "20，5个");
        assert_eq!(s("好，一样", "zh"), "好，一样");
        assert_eq!(s("每星期一、三、五", "zh"), "每星期一、三、五");
        assert_eq!(s("一）二", "zh"), "一）二");
        assert_eq!(s("五…五", "zh"), "五…五");
        assert_eq!(s("万一、五", "ja"), "万一、五");
        assert_eq!(s("比分三：二", "zh"), "比分三：二");
        assert_eq!(s("บ้าน,ห้า", "th"), "บ้าน,ห้า");
        assert_eq!(s("一、二、三", "zh"), "一、二、三");
        // Spaced languages read a mark-led number as before.
        assert!(s("x ,deux mille y", "fr").contains("2000"));
        assert!(s("x ,\u{0434}\u{0432}\u{0430}\u{0434}\u{0446}\u{0430}\u{0442}\u{044C} \u{043E}\u{0434}\u{0438}\u{043D} y", "ru").contains("21"));
        // ‼ ‥ ⁇ ⁈ ⁉ ｡ ､ ﹗ ․ ։ ՝ ፧ end the run and stay (#1080 audit).
        assert_eq!(s("x fifty\u{203C} two y", "en"), "x 50\u{203C} 2 y");
        assert_eq!(s("x fifty\u{FF61} y", "en"), "x 50\u{FF61} y");
        // NFKC-folded forms (#1088) and a mark before the word (#1087).
        assert_eq!(s("x fifty\u{FF0E} two y", "en"), "x 50\u{FF0E} 2 y");
        assert_eq!(s("x \u{03C0}\u{03B5}\u{03BD}\u{03AE}\u{03BD}\u{03C4}\u{03B1}\u{037E} \u{03B4}\u{03CD}\u{03BF} y", "el"), "x 50\u{037E} 2 y");
        assert_eq!(s("x \u{203C}fifty y", "en"), "x \u{203C}50 y");
        assert_eq!(s("he said \u{FF02}fifty\u{FF02} apples", "en"), "he said \u{FF02}50\u{FF02} apples");
        let c = |t: &str| super::words2num_sentence_opts(t, "en", "cardinal", false, true).unwrap();
        assert_eq!(c("x five dollars fifty\u{203C} y"), "x $5.50\u{203C} y");
        assert_eq!(s("\u{4E8C}\u{5341}\u{FF61}\u{4E94}", "zh"), "20\u{FF61}\u{4E94}");
        // Unchanged without the mark.
        assert_eq!(s("တစ်ရာ ငါး", "my"), "105");
    }
}

#[cfg(test)]
mod leading_mark_tests {
    /// #1114: a closer glued before a number word keeps that word as
    /// written, and the rest of its number stays as written with it: no
    /// smaller value read from the tail.
    #[test]
    fn closer_led_number_stays_whole_issue_1114() {
        let s = |t: &str, l: &str| super::words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(s("x )two thousand twenty four y", "en"), "x )two thousand twenty four y");
        assert_eq!(s("x ]one thousand, two hundred and thirty-four y", "en"), "x ]one thousand, two hundred and thirty-four y");
        assert_eq!(s("x )deux cents y", "fr"), "x )deux cents y");
        assert_eq!(s("x }zwei tausend y", "de"), "x }zwei tausend y");
        assert_eq!(s("x )\u{0434}\u{0432}\u{0435} \u{0442}\u{044B}\u{0441}\u{044F}\u{0447}\u{0438} y", "ru"), "x )\u{0434}\u{0432}\u{0435} \u{0442}\u{044B}\u{0441}\u{044F}\u{0447}\u{0438} y");
        assert_eq!(s("x )wun zero fife y", "en_AERO"), "x )wun zero fife y");
        // Two numbers after the mark stay two; other words end the number.
        assert_eq!(s(")twenty one twenty one", "en"), ")twenty one 21");
        assert_eq!(s("x )fifty apples two y", "en"), "x )fifty apples 2 y");
        assert_eq!(s("x )two 3 y", "en"), "x )two 3 y");
        assert_eq!(s("x (two thousand twenty four) y", "en"), "x (2024) y");
        let c = |t: &str, l: &str| super::words2num_sentence_opts(t, l, "cardinal", false, true).unwrap();
        assert_eq!(c("he paid )twenty five dollars", "en"), "he paid )twenty five dollars");
        let d = |t: &str, l: &str| {
            super::words2num_sentence_full(t, l, "cardinal", false, false, true, None, "written", false).unwrap()
        };
        assert_eq!(d("x )twenty first of may y", "en"), "x )twenty first of may y");
    }

    /// #1041, #1044: a clause mark glued before a number is cut off and kept;
    /// the number reads whole after it.
    #[test]
    fn leading_clause_mark_issue_1041() {
        let s = |t: &str, l: &str| super::words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(s(",deux cents", "fr"), ",200");
        assert_eq!(s("il reste ,deux cents euros", "fr"), "il reste ,200 euros");
        assert_eq!(s("j ai dit ...deux cents", "fr"), "j ai dit ...200");
        assert_eq!(s("x ,one thousand two hundred y", "en"), "x ,1200 y");
        assert_eq!(s(",two point five", "en"), ",2.5");
        assert_eq!(s("x \u{0964}\u{092A}\u{093E}\u{0901}\u{091A} \u{0938}\u{094C} y", "hi"), "x \u{0964}500 y");
        assert_eq!(s("twenty ,five", "en"), "20 ,5");
        // More marks (#1049); not in zh / ja / th (#1048).
        assert_eq!(s("x \u{06D4}\u{062F}\u{0648} \u{0633}\u{0648} y", "ur"), "x \u{06D4}200 y");
        assert_eq!(s("twenty \u{FF0C}five", "en"), "20 \u{FF0C}5");
        assert_eq!(s("x \u{3002}two hundred y", "en"), "x \u{3002}200 y");
        assert_eq!(s("x \u{3001}five y", "en"), "x \u{3001}5 y");
        assert_eq!(s("二十 ,五", "zh"), "20 ,五");
        assert_eq!(s("每星期一 ,三 ,五", "zh"), "每星期一 ,三 ,五");
        // currency=True folds after the mark (#1047).
        let c = |t: &str, l: &str| super::words2num_sentence_opts(t, l, "cardinal", false, true).unwrap();
        assert_eq!(c("x ,five dollars y", "en"), "x ,$5 y");
        assert_eq!(c("il reste ,deux cents euros", "fr"), "il reste ,200 \u{20AC}");
        assert_eq!(c("x .5 dollars", "en"), "x .5 dollars");
        assert_eq!(c("x \u{3002}five dollars y", "en"), "x \u{3002}$5 y");
        assert_eq!(c("x \u{3001}deux cents euros y", "fr"), "x \u{3001}200 \u{20AC} y");
        // Figures and plain marks are untouched.
        assert_eq!(s("x .5 y", "en"), "x .5 y");
        assert_eq!(s("x 3.5 y", "en"), "x 3.5 y");
        assert_eq!(s("x ...and y", "en"), "x ...and y");
    }
}
