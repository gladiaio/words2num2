//! Port of `words2num2/formats.py` — per-locale number-format defaults and the
//! configurable numeric-string parser.
//!
//! # Fidelity notes
//!
//! Three things in the Python original do not have a free Rust equivalent and
//! are reimplemented here on purpose:
//!
//! 1. **`\s` / `str.strip()` is *not* `char::is_whitespace()`.** Python's `\s`
//!    (and `str.isspace()`, which drives `str.strip()`) matches 29 codepoints,
//!    including `U+001C..U+001F` (the FILE/GROUP/RECORD/UNIT separators). Rust's
//!    `char::is_whitespace()` follows the Unicode `White_Space` property, which
//!    excludes those four. `parse_number_string("\x1c42")` returns `42` in
//!    Python, so the exact set is reproduced in [`is_py_space`].
//!
//! 2. **`\d` is *not* `is_ascii_digit()`.** In a `str` pattern Python's `\d`
//!    matches Unicode general category `Nd` — all 760 of them (Unicode 16) — and `int()` /
//!    `float()` then accept those digits, mixing scripts freely:
//!    `_to_number("١2")` is `12` and `_to_number("٢٣.٥")` is `23.5`. [`ND_RUNS`]
//!    carries the 61 contiguous `Nd` decades so [`to_number`] can transliterate
//!    to ASCII before parsing, exactly as CPython does internally.
//!
//! 3. **`"not a parseable number: %r"` interpolates Python's `repr()`.** That is
//!    reproduced in [`py_repr`], quote selection and `\xNN`/`\uNNNN`/`\UNNNNNNNN`
//!    escapes included, so `parse_number_string("1\u{a0}234", Some("\t"), ..)`
//!    reports `not a parseable number: '1\xa0234'` byte for byte.
//!
//! The space-like separators are distinct codepoints and stay distinct:
//! NBSP `U+00A0`, NARROW NBSP `U+202F`, THIN SPACE `U+2009`. Note that the
//! locale table itself only ever uses a plain ASCII space `U+0020` — the other
//! three appear solely in the `thousands_sep in (" ", NBSP, NNBSP, THIN_SPACE)`
//! membership test and in the two regex character classes.

use bigdecimal::BigDecimal;
use num_bigint::BigInt;
use std::cmp::Ordering;
use std::fmt;

// ---------------------------------------------------------------------------
// Errors and values
// ---------------------------------------------------------------------------

/// Port of `words2num2.base.Words2NumError` (a `ValueError` subclass).
///
/// Carries the exact message Python produces; the pyo3 boundary is responsible
/// for turning it back into the Python exception class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Words2NumError(pub String);

impl Words2NumError {
    fn new(msg: impl Into<String>) -> Self {
        Words2NumError(msg.into())
    }
}

impl fmt::Display for Words2NumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Words2NumError {}

/// `words2num2` hands back an `int`, a `float` or a `Decimal` depending on the
/// input.
///
/// The parser returns an `Int` for a whole number and an exact `Dec` for one
/// with a decimal part (the Python `formats.py` returned a binary `float`
/// there, #71). `Float` is kept so this enum lines up with the sibling modules.
#[derive(Debug, Clone, PartialEq)]
pub enum W2nValue {
    Int(BigInt),
    Float(f64),
    Dec(BigDecimal),
}

/// `Result` alias matching the module's single error type.
pub type Result<T> = std::result::Result<T, Words2NumError>;

// ---------------------------------------------------------------------------
// Space-like separators
// ---------------------------------------------------------------------------

/// `NBSP = "\xa0"` — NO-BREAK SPACE.
pub const NBSP: char = '\u{00A0}';
/// `NNBSP = " "` — NARROW NO-BREAK SPACE, the French standard.
pub const NNBSP: char = '\u{202F}';
/// `THIN_SPACE = " "` — THIN SPACE.
pub const THIN_SPACE: char = '\u{2009}';
/// `SPACE_LIKE = " \t" + NBSP + NNBSP + THIN_SPACE`.
///
/// Dead code in the Python module too: nothing reads it, there or here. Kept
/// because it is part of the module's public surface.
pub const SPACE_LIKE: &str = " \t\u{00A0}\u{202F}\u{2009}";

/// The exact set Python's `\s` matches — equivalently `str.isspace()`, which is
/// what `str.strip()` / `str.lstrip()` use.
///
/// 29 codepoints. **Not** `char::is_whitespace()`: that omits `U+001C..U+001F`.
fn is_py_space(c: char) -> bool {
    matches!(c,
        '\u{0009}'..='\u{000D}'   // TAB LF VT FF CR
        | '\u{001C}'..='\u{001F}' // FS GS RS US — Rust's is_whitespace() misses these
        | '\u{0020}'
        | '\u{0085}'
        | '\u{00A0}'              // NBSP
        | '\u{1680}'
        | '\u{2000}'..='\u{200A}' // includes THIN SPACE U+2009
        | '\u{2028}'
        | '\u{2029}'
        | '\u{202F}'              // NNBSP
        | '\u{205F}'
        | '\u{3000}'
    )
}

/// `str.strip()`
fn py_strip(s: &str) -> &str {
    s.trim_matches(is_py_space)
}

/// `str.lstrip()`
fn py_lstrip(s: &str) -> &str {
    s.trim_start_matches(is_py_space)
}

// ---------------------------------------------------------------------------
// NUMBER_FORMAT_DEFAULTS
// ---------------------------------------------------------------------------

/// One row of [`NUMBER_FORMAT_DEFAULTS`] — Python's `{"thousands": …,
/// "decimal": …}` dict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberFormat {
    pub thousands: &'static str,
    pub decimal: &'static str,
}

const fn nf(thousands: &'static str, decimal: &'static str) -> NumberFormat {
    NumberFormat { thousands, decimal }
}

/// CLDR-inspired per-locale defaults, in Python source order. `"_default"` is
/// the fallback.
///
/// A slice rather than a map: 55 rows, looked up by linear scan, and the order
/// stays visibly identical to the Python dict.
///
/// Every `thousands` that reads as a space here is a plain ASCII `U+0020` — the
/// Python table uses no NBSP/NNBSP/THIN SPACE anywhere (verified against the
/// live module, codepoint by codepoint).
pub static NUMBER_FORMAT_DEFAULTS: &[(&str, NumberFormat)] = &[
    ("_default", nf(",", ".")),
    // English + CJK group (comma thousands, dot decimal)
    ("en", nf(",", ".")),
    ("en_GB", nf(",", ".")),
    ("en_IN", nf(",", ".")),
    ("en_NG", nf(",", ".")),
    ("zh", nf(",", ".")),
    ("zh_CN", nf(",", ".")),
    ("zh_HK", nf(",", ".")),
    ("zh_TW", nf(",", ".")),
    ("ja", nf(",", ".")),
    ("ko", nf(",", ".")),
    ("th", nf(",", ".")),
    ("vi", nf(".", ",")),
    // French (space thousands, comma decimal)
    ("fr", nf(" ", ",")),
    ("fr_BE", nf(" ", ",")),
    ("fr_DZ", nf(" ", ",")),
    // Swiss French uses apostrophe
    ("fr_CH", nf("'", ".")),
    // Continental European (dot thousands, comma decimal)
    ("de", nf(".", ",")),
    ("es", nf(".", ",")),
    ("es_CO", nf(".", ",")),
    ("es_CR", nf(".", ",")),
    ("es_GT", nf(".", ",")),
    ("es_NI", nf(".", ",")),
    ("es_VE", nf(".", ",")),
    ("it", nf(".", ",")),
    ("pt", nf(".", ",")),
    ("pt_BR", nf(".", ",")),
    ("nl", nf(".", ",")),
    ("ro", nf(".", ",")),
    ("hr", nf(".", ",")),
    ("sl", nf(".", ",")),
    ("sr", nf(".", ",")),
    ("tr", nf(".", ",")),
    ("el", nf(".", ",")),
    // Slavic / Scandinavian / Baltic (space thousands, comma decimal)
    ("ru", nf(" ", ",")),
    ("uk", nf(" ", ",")),
    ("be", nf(" ", ",")),
    ("bg", nf(" ", ",")),
    ("pl", nf(" ", ",")),
    ("cs", nf(" ", ",")),
    ("sk", nf(" ", ",")),
    ("hu", nf(" ", ",")),
    ("sv", nf(" ", ",")),
    ("no", nf(" ", ",")),
    ("nn", nf(" ", ",")),
    ("da", nf(".", ",")),
    ("fi", nf(" ", ",")),
    ("et", nf(" ", ",")),
    ("lt", nf(" ", ",")),
    ("lv", nf(" ", ",")),
    ("is", nf(".", ",")),
    ("fo", nf(".", ",")),
    // Arabic + Persian use Western digits but local separators in some
    // contexts; default to Western style.
    ("ar", nf(",", ".")),
    ("fa", nf(",", ".")),
    ("he", nf(",", ".")),
];

/// `lang in NUMBER_FORMAT_DEFAULTS` → `NUMBER_FORMAT_DEFAULTS[lang]`
fn lookup_format(key: &str) -> Option<NumberFormat> {
    NUMBER_FORMAT_DEFAULTS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}

/// The `{thousands, decimal}` row for `lang`, if the table knows it: the exact
/// key, else its base language. Both `_` and `-` separate the region, so
/// `pt-BR` finds `pt_BR` and `es-MX` finds `es` (#54).
pub fn locale_format(lang: &str) -> Option<NumberFormat> {
    if lang.is_empty() || lang == "_default" {
        return None;
    }
    if let Some(f) = lookup_format(lang) {
        return Some(f);
    }
    let normalized = lang.replace('-', "_");
    if let Some(f) = lookup_format(&normalized) {
        return Some(f);
    }
    let base = normalized.split('_').next().unwrap_or("");
    lookup_format(base)
}

/// Port of `get_format`. Returns the `{thousands, decimal}` row for `lang`,
/// falling back to `_default` for a language the table does not know.
///
/// The parser itself no longer applies `_default` to an unknown language: it
/// auto-detects instead (see [`parse_number_string`]). This stays the public
/// "which separators would a document in this locale use" answer.
pub fn get_format(lang: &str) -> NumberFormat {
    locale_format(lang).unwrap_or_else(|| lookup_format("_default").expect("_default is always present"))
}

// ---------------------------------------------------------------------------
// parse_number_string
// ---------------------------------------------------------------------------

/// The typographic apostrophes Swiss typography and word processors put where
/// an ASCII `'` groups thousands: `1’234.5` (U+2019), `1ʼ234` (U+02BC) (#109).
fn ascii_apostrophes(s: &str) -> String {
    s.replace(['\u{2019}', '\u{02BC}'], "'")
}

/// Reject separator overrides that cannot be told apart from the number:
/// `thousands_sep == decimal_sep` would delete the decimal point
/// (`parse_number_string("1.5", ".", ".")` used to return 15), and a digit as a
/// separator rewrites the digits themselves (#188). `None` and `""` both mean
/// "not given".
pub fn check_separators(thousands_sep: Option<&str>, decimal_sep: Option<&str>) -> Result<()> {
    let t = thousands_sep.filter(|t| !t.is_empty());
    let d = decimal_sep.filter(|d| !d.is_empty());
    for sep in [t, d].into_iter().flatten() {
        if sep.chars().any(|c| decimal_digit(c).is_some()) {
            return Err(Words2NumError::new(format!(
                "a digit cannot be a separator: {}",
                py_repr(sep)
            )));
        }
    }
    if let (Some(t), Some(d)) = (t, d) {
        if ascii_apostrophes(t) == ascii_apostrophes(d) {
            return Err(Words2NumError::new(format!(
                "thousands_sep and decimal_sep must differ (both {})",
                py_repr(t)
            )));
        }
    }
    Ok(())
}

/// Parses a numeric string with configurable separators.
///
/// Resolution order:
///   1. If `thousands_sep` and/or `decimal_sep` are given, use them (they must
///      differ, see [`check_separators`]).
///   2. Otherwise, if `lang` has a row in [`NUMBER_FORMAT_DEFAULTS`], read the
///      string strictly in that locale: its thousands separator only between
///      groups of exactly three digits, its decimal separator at most once.
///   3. Otherwise — or when the locale reading does not fit — auto-detect from
///      the string itself, with the locale's decimal separator as the tie-break
///      for the one genuinely ambiguous shape (`1.234`).
///
/// A dot or comma followed by 1, 2 or 4+ digits is a decimal in any locale:
/// `"3.14"` is 3.14 in French too, `"1,5"` is 1.5 in English (#53). `"1.234"`
/// stays a thousands group where the locale writes decimals with a comma.
///
/// Decimals come back exact, as [`W2nValue::Dec`] (Python `Decimal`), never as
/// a binary float: `"12345678901234567.89"` keeps every digit and a scale
/// suffix multiplies without rounding error (#71). Whole numbers stay `Int`.
///
/// `Option::None` is Python's `None` and `Some("")` is Python's `""`; both mean
/// "not given" for a separator.
pub fn parse_number_string(
    s: &str,
    thousands_sep: Option<&str>,
    decimal_sep: Option<&str>,
    lang: Option<&str>,
) -> Result<W2nValue> {
    check_separators(thousands_sep, decimal_sep)?;
    let s = py_strip(s);
    if s.is_empty() {
        return Err(Words2NumError::new("empty numeric string"));
    }

    // if s[0] in "+-": ... ; s = s[1:].lstrip()
    // Exactly one sign character is consumed — "+-42" keeps the "-" and later
    // fails with `not a parseable number: '-42'`.
    let mut sign = 1i32;
    let first = s.chars().next().expect("non-empty");
    let s = if first == '+' || first == '-' {
        if first == '-' {
            sign = -1;
        }
        py_lstrip(&s[first.len_utf8()..])
    } else {
        s
    };
    if s.is_empty() {
        return Err(Words2NumError::new("empty numeric string after sign"));
    }
    let s = ascii_apostrophes(s);

    if thousands_sep.is_some() || decimal_sep.is_some() {
        let t = thousands_sep.map(ascii_apostrophes);
        let d = decimal_sep.map(ascii_apostrophes);
        return Ok(apply_sign(sign, parse_with_explicit(&s, t.as_deref(), d.as_deref())?));
    }

    let fmt = lang.and_then(locale_format);
    if let Some(fmt) = fmt {
        if let Some(v) = parse_locale_strict(&s, fmt) {
            return Ok(apply_sign(sign, v?));
        }
    }

    let hint = fmt.and_then(|f| f.decimal.chars().next());
    Ok(apply_sign(sign, auto_detect_parse(&s, hint)?))
}

/// Python's `sign * value`, where `sign` is the int `1` or `-1`.
///
/// `1 * x` is `x`. `-1 * 0` is `0` (ints have no negative zero) — negation
/// reproduces it. A `Decimal` zero keeps no sign either (BigDecimal has none).
fn apply_sign(sign: i32, v: W2nValue) -> W2nValue {
    if sign >= 0 {
        return v;
    }
    match v {
        W2nValue::Int(i) => W2nValue::Int(-i),
        W2nValue::Float(f) => W2nValue::Float(-f),
        W2nValue::Dec(d) => W2nValue::Dec(-d),
    }
}

fn is_space_like_sep(t: &str) -> bool {
    t == " " || t == "\u{00A0}" || t == "\u{202F}" || t == "\u{2009}"
}

/// Explicit separators.
///
/// ```python
/// if thousands_sep:
///     if thousands_sep in (" ", NBSP, NNBSP, THIN_SPACE):
///         s = re.sub(r"[\s\xa0  ]", "", s)
///     else:
///         s = s.replace(thousands_sep, "")
/// if decimal_sep:
///     if decimal_sep != ".":
///         if "." in s:
///             s = s.replace(".", "")
///         s = s.replace(decimal_sep, ".")
/// return _to_number(s)
/// ```
///
/// The membership test is exact string equality against those four separators.
/// A tab is *not* in the tuple, so `thousands_sep="\t"` takes the plain
/// `.replace` branch and leaves an NBSP in place — hence
/// `parse_number_string("1\u{a0}234", Some("\t"), None, None)` failing while
/// `Some(" ")` succeeds.
fn parse_with_explicit(
    s: &str,
    thousands_sep: Option<&str>,
    decimal_sep: Option<&str>,
) -> Result<W2nValue> {
    let mut cur = s.to_string();

    // `if thousands_sep:` — Python truthiness: None and "" are both falsy.
    if let Some(t) = thousands_sep.filter(|t| !t.is_empty()) {
        if is_space_like_sep(t) {
            // treat any space-like as thousands when one is configured
            cur = cur.chars().filter(|&c| !is_py_space(c)).collect();
        } else {
            cur = cur.replace(t, "");
        }
    }

    if let Some(d) = decimal_sep.filter(|d| !d.is_empty()) {
        if d != "." {
            if cur.contains('.') {
                // The caller named a non-dot decimal, so a dot left in the
                // string can only be a grouping mark ("1.234,5" with ",").
                cur = cur.replace('.', "");
            }
            cur = cur.replace(d, ".");
        }
    }

    to_number(&cur)
}

/// Are these the groups of a thousands-grouped integer: 1-3 digits, then
/// groups of exactly 3? `["1", "234", "567"]` yes; `["1", "5", "2026"]`,
/// `["1234", "567"]`, `["1", "", "234"]` no.
fn valid_grouping(groups: &[&str]) -> bool {
    let all_digits = |g: &str| g.chars().all(|c| decimal_digit(c).is_some());
    let len = |g: &str| g.chars().count();
    match groups.split_first() {
        Some((head, rest)) => {
            (1..=3).contains(&len(head))
                && all_digits(head)
                && rest.iter().all(|g| len(g) == 3 && all_digits(g))
        }
        None => false,
    }
}

/// The locale reading, strict: digits, the locale's thousands separator only
/// between valid groups, its decimal separator at most once with digits after.
/// `None` when the string does not fit — the caller then auto-detects, so
/// `"3.14"` in French (whose separators are space and comma) is read by the
/// auto-detect rules rather than having its dot dropped (#53).
fn parse_locale_strict(s: &str, fmt: NumberFormat) -> Option<Result<W2nValue>> {
    let t = fmt.thousands;
    let space_t = is_space_like_sep(t);
    let d = fmt.decimal;
    let (int_part, frac) = match s.split_once(d) {
        Some((i, f)) => (i, Some(f)),
        None => (s, None),
    };
    if let Some(f) = frac {
        if f.is_empty() || !f.chars().all(|c| decimal_digit(c).is_some()) {
            return None;
        }
    }
    let groups: Vec<&str> = if space_t {
        int_part.split(is_py_space).collect()
    } else if t.is_empty() {
        vec![int_part]
    } else {
        int_part.split(t).collect()
    };
    let ok = if groups.len() == 1 {
        !groups[0].is_empty() && groups[0].chars().all(|c| decimal_digit(c).is_some())
    } else {
        valid_grouping(&groups)
    };
    if !ok {
        return None;
    }
    let mut ascii = groups.concat();
    if let Some(f) = frac {
        ascii.push('.');
        ascii.push_str(f);
    }
    Some(to_number(&ascii))
}

/// Auto-detection.
///
/// Spaces, apostrophes and underscores only ever group thousands and are
/// dropped. Then, for dots and commas:
/// * both present: the rightmost kind is the decimal (once), the other groups
///   thousands — in valid groups of three;
/// * one kind, several times: thousands, in valid groups of three, else not a
///   number (`"1.5.2026"` is a date, not 152026) (#55);
/// * one occurrence: a decimal, except `1-3 digits` + sep + `3 digits`, which
///   is genuinely ambiguous (`1.234`): there `hint`, the locale's decimal
///   separator, decides — the other separator is thousands. With no locale it
///   stays a decimal ("preserves precision"). A leading group of 4+ digits
///   proves the string is not grouped, so `"1234.567"` is 1234.567 (#55).
fn auto_detect_parse(s: &str, hint: Option<char>) -> Result<W2nValue> {
    // Strip clearly-thousands-only separators first:
    //   re.sub(r"[\s\xa0  '_]", "", s)
    let s2: String = s
        .chars()
        .filter(|&c| !(is_py_space(c) || c == '\'' || c == '_'))
        .collect();
    let not_a_number = || Words2NumError::new(format!("not a parseable number: {}", py_repr(&s2)));

    let has_comma = s2.contains(',');
    let has_dot = s2.contains('.');

    if has_comma && has_dot {
        // Both present — the rightmost is decimal.
        let last_comma = last_char_index(&s2, ',').expect("has_comma");
        let last_dot = last_char_index(&s2, '.').expect("has_dot");
        let (dec, grp) = if last_comma > last_dot { (',', '.') } else { ('.', ',') };
        let (int_part, frac) = s2.rsplit_once(dec).expect("has dec");
        if int_part.contains(dec) {
            return Err(not_a_number());
        }
        let groups: Vec<&str> = int_part.split(grp).collect();
        if !valid_grouping(&groups) {
            return Err(not_a_number());
        }
        return to_number(&format!("{}.{}", groups.concat(), frac));
    }

    for sep in [',', '.'] {
        if !s2.contains(sep) {
            continue;
        }
        let parts: Vec<&str> = s2.split(sep).collect();
        if parts.len() > 2 {
            // Multiple occurrences → thousands, if they group in threes.
            if !valid_grouping(&parts) {
                return Err(not_a_number());
            }
            return to_number(&parts.concat());
        }
        let (before, after) = (parts[0], parts[1]);
        let before_len = before.chars().count();
        let after_len = after.chars().count();
        let decimal = format!("{}.{}", before, after);
        if before.is_empty() && after_len == 3 {
            // ",123": no leading group to be a decimal of; the separator is
            // a stray thousands mark.
            return to_number(after);
        }
        if after_len == 3 && (1..=3).contains(&before_len) {
            return match hint {
                Some(h) if h != sep => to_number(&format!("{}{}", before, after)),
                _ => to_number(&decimal),
            };
        }
        return to_number(&decimal);
    }
    to_number(&s2)
}

/// `str.rfind(ch)` — the **character** index of the last `ch`.
fn last_char_index(s: &str, target: char) -> Option<usize> {
    s.chars()
        .enumerate()
        .filter(|(_, c)| *c == target)
        .map(|(i, _)| i)
        .last()
}

/// Port of `_to_number`, exact.
///
/// ```python
/// if not re.fullmatch(r"\d+(?:\.\d+)?", s):
///     raise Words2NumError("not a parseable number: %r" % s)
/// if "." in s:
///     return Decimal(s)        # was float(s): binary rounding (#71)
/// return int(s)
/// ```
///
/// `\d` is Unicode `Nd`, and CPython's `int()`/`Decimal()` map those digits to
/// ASCII before parsing — so [`fullmatch_number`] returns the transliterated
/// ASCII form and the parse runs on that.
fn to_number(s: &str) -> Result<W2nValue> {
    let Some(ascii) = fullmatch_number(s) else {
        return Err(Words2NumError::new(format!(
            "not a parseable number: {}",
            py_repr(s)
        )));
    };
    if ascii.contains('.') {
        // The scale is the number of digits written: "12.50" keeps its two.
        Ok(W2nValue::Dec(
            ascii.parse::<BigDecimal>().expect("gated by fullmatch_number"),
        ))
    } else {
        // int() is arbitrary precision — BigInt, never a fixed-width cast.
        Ok(W2nValue::Int(
            ascii.parse::<BigInt>().expect("gated by fullmatch_number"),
        ))
    }
}

/// `re.fullmatch(r"\d+(?:\.\d+)?", s)`, returning the ASCII transliteration of
/// the match (or `None` for no match).
///
/// No backtracking is needed: `\d` and `.` are disjoint, so the tokenization
/// — maximal digit run, optional dot, maximal digit run, end — is
/// deterministic and the greedy scan below is equivalent to the regex.
/// (`"12."` fails both ways: the optional group needs `\d+` after the dot, and
/// backing `\d+` off cannot put a dot at the end.)
fn fullmatch_number(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();

    // \d+
    let mut n = 0usize;
    while let Some(d) = it.peek().copied().and_then(decimal_digit) {
        out.push((b'0' + d) as char);
        it.next();
        n += 1;
    }
    if n == 0 {
        return None;
    }

    // (?:\.\d+)?
    if let Some(&c) = it.peek() {
        if c != '.' {
            return None;
        }
        it.next();
        out.push('.');

        let mut m = 0usize;
        while let Some(d) = it.peek().copied().and_then(decimal_digit) {
            out.push((b'0' + d) as char);
            it.next();
            m += 1;
        }
        if m == 0 {
            return None;
        }
        if it.next().is_some() {
            return None; // trailing junk — fullmatch requires end-of-string
        }
    }

    Some(out)
}

/// Every Unicode general-category `Nd` codepoint, as 71 contiguous runs,
/// pinned to **Unicode 16.0.0** (see the module report on version sensitivity).
///
/// Generated from `python3.14` (the newest interpreter in the package's
/// supported 3.10–3.15 range) and verified: `re.fullmatch(r"\d", c)` matches
/// exactly `unicodedata.category(c) == "Nd"` (760 codepoints), every run's
/// length is a multiple of 10, and `unicodedata.decimal(c)` always equals
/// `(cp - run_start) % 10`.
const ND_RUNS: &[(u32, u32)] = &[
    (0x0030, 0x0039),
    (0x0660, 0x0669),
    (0x06F0, 0x06F9),
    (0x07C0, 0x07C9),
    (0x0966, 0x096F),
    (0x09E6, 0x09EF),
    (0x0A66, 0x0A6F),
    (0x0AE6, 0x0AEF),
    (0x0B66, 0x0B6F),
    (0x0BE6, 0x0BEF),
    (0x0C66, 0x0C6F),
    (0x0CE6, 0x0CEF),
    (0x0D66, 0x0D6F),
    (0x0DE6, 0x0DEF),
    (0x0E50, 0x0E59),
    (0x0ED0, 0x0ED9),
    (0x0F20, 0x0F29),
    (0x1040, 0x1049),
    (0x1090, 0x1099),
    (0x17E0, 0x17E9),
    (0x1810, 0x1819),
    (0x1946, 0x194F),
    (0x19D0, 0x19D9),
    (0x1A80, 0x1A89),
    (0x1A90, 0x1A99),
    (0x1B50, 0x1B59),
    (0x1BB0, 0x1BB9),
    (0x1C40, 0x1C49),
    (0x1C50, 0x1C59),
    (0xA620, 0xA629),
    (0xA8D0, 0xA8D9),
    (0xA900, 0xA909),
    (0xA9D0, 0xA9D9),
    (0xA9F0, 0xA9F9),
    (0xAA50, 0xAA59),
    (0xABF0, 0xABF9),
    (0xFF10, 0xFF19),
    (0x104A0, 0x104A9),
    (0x10D30, 0x10D39),
    (0x10D40, 0x10D49), // + Unicode 16 (Garay)
    (0x11066, 0x1106F),
    (0x110F0, 0x110F9),
    (0x11136, 0x1113F),
    (0x111D0, 0x111D9),
    (0x112F0, 0x112F9),
    (0x11450, 0x11459),
    (0x114D0, 0x114D9),
    (0x11650, 0x11659),
    (0x116C0, 0x116C9),
    (0x116D0, 0x116E3), // + Unicode 16 (Kirat Rai); 2 decades, 20 codepoints
    (0x11730, 0x11739),
    (0x118E0, 0x118E9),
    (0x11950, 0x11959),
    (0x11BF0, 0x11BF9), // + Unicode 16 (Sunuwar)
    (0x11C50, 0x11C59),
    (0x11D50, 0x11D59),
    (0x11DA0, 0x11DA9),
    (0x11F50, 0x11F59), // + Unicode 15.1 (Kawi)
    (0x16130, 0x16139), // + Unicode 16 (Gurung Khema)
    (0x16A60, 0x16A69),
    (0x16AC0, 0x16AC9), // + Unicode 15 (Tangsa)
    (0x16B50, 0x16B59),
    (0x16D70, 0x16D79), // + Unicode 16 (Ol Onal)
    (0x1CCF0, 0x1CCF9), // + Unicode 16 (Symbols for legacy computing)
    (0x1D7CE, 0x1D7FF),
    (0x1E140, 0x1E149),
    (0x1E2F0, 0x1E2F9),
    (0x1E4F0, 0x1E4F9), // + Unicode 15.1 (Nag Mundari)
    (0x1E5F1, 0x1E5FA), // + Unicode 16 (Ol Onal digits); start offset != 0
    (0x1E950, 0x1E959),
    (0x1FBF0, 0x1FBF9),
];

/// `\d` + `Py_UNICODE_TODECIMAL`: the digit value 0-9 of any `Nd` codepoint.
fn decimal_digit(c: char) -> Option<u8> {
    let cp = c as u32;
    if c.is_ascii_digit() {
        return Some((cp - 0x30) as u8);
    }
    let idx = ND_RUNS
        .binary_search_by(|&(lo, hi)| {
            if hi < cp {
                Ordering::Less
            } else if lo > cp {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        })
        .ok()?;
    Some(((cp - ND_RUNS[idx].0) % 10) as u8)
}

// ---------------------------------------------------------------------------
// repr()
// ---------------------------------------------------------------------------

/// Codepoints for which `str.isprintable()` is false, **excluding** category
/// `Cn` (unassigned). Pinned to **Unicode 16.0.0** (`python3.14`).
///
/// 28 runs covering `Cc`, `Cf`, `Cs`, `Co`, `Zl`, `Zp` and `Zs` (ASCII space
/// `U+0020` is printable and correctly falls between the first two runs). The
/// `Cs` run is fused with a `Co` block at `0xD800..=0xF8FF`; surrogates cannot
/// exist as a Rust `char`, so only the private-use tail of it is reachable.
///
/// `Cn` is omitted deliberately — it is ~680 further runs, it churns with every
/// Unicode release, and it only ever affects the text of an error message for
/// input containing *unassigned* codepoints. See the module report for the
/// resulting bound on repr fidelity.
const NONPRINTABLE_RUNS: &[(u32, u32)] = &[
    (0x0000, 0x001F),     // Cc
    (0x007F, 0x00A0),     // Cc + NBSP
    (0x00AD, 0x00AD),     // Cf
    (0x0600, 0x0605),     // Cf
    (0x061C, 0x061C),     // Cf
    (0x06DD, 0x06DD),     // Cf
    (0x070F, 0x070F),     // Cf
    (0x0890, 0x0891),     // Cf (+ Unicode 14)
    (0x08E2, 0x08E2),     // Cf
    (0x1680, 0x1680),     // Zs
    (0x180E, 0x180E),     // Cf
    (0x2000, 0x200F),     // Zs (incl. THIN SPACE) + Cf
    (0x2028, 0x202F),     // Zl + Zp + Cf + NNBSP
    (0x205F, 0x2064),     // Zs + Cf
    (0x2066, 0x206F),     // Cf
    (0x3000, 0x3000),     // Zs
    (0xD800, 0xF8FF),     // Cs + Co
    (0xFEFF, 0xFEFF),     // Cf
    (0xFFF9, 0xFFFB),     // Cf
    (0x110BD, 0x110BD),   // Cf
    (0x110CD, 0x110CD),   // Cf
    (0x13430, 0x1343F),   // Cf (extended in Unicode 15/16)
    (0x1BCA0, 0x1BCA3),   // Cf
    (0x1D173, 0x1D17A),   // Cf
    (0xE0001, 0xE0001),   // Cf
    (0xE0020, 0xE007F),   // Cf
    (0xF0000, 0xFFFFD),   // Co
    (0x100000, 0x10FFFD), // Co
];

/// `str.isprintable()`, modulo the `Cn` gap documented on [`NONPRINTABLE_RUNS`].
fn is_py_printable(c: char) -> bool {
    let cp = c as u32;
    NONPRINTABLE_RUNS
        .binary_search_by(|&(lo, hi)| {
            if hi < cp {
                Ordering::Less
            } else if lo > cp {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        })
        .is_err()
}

/// Python's `repr()` for a `str`, as used by `"… %r" % s`.
///
/// * Quote is `'` unless the string holds a `'` and no `"`, then `"`.
/// * `\\`, the active quote, `\n`, `\r`, `\t` get short escapes; every other
///   non-printable becomes `\xNN` (< 0x100), `\uNNNN` (< 0x10000) or
///   `\UNNNNNNNN`. Note `\v`, `\f`, `\a`, `\b` have **no** short form in
///   Python's repr — they come out as `\x0b`, `\x0c`, `\x07`, `\x08`.
fn py_repr(s: &str) -> String {
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
            c if is_py_printable(c) => out.push(c),
            c => {
                let cp = c as u32;
                if cp < 0x100 {
                    out.push_str(&format!("\\x{:02x}", cp));
                } else if cp < 0x10000 {
                    out.push_str(&format!("\\u{:04x}", cp));
                } else {
                    out.push_str(&format!("\\U{:08x}", cp));
                }
            }
        }
    }
    out.push(quote);
    out
}

// ---------------------------------------------------------------------------
// Tests — every expectation below was taken from the live Python module.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn int(n: i64) -> W2nValue {
        W2nValue::Int(BigInt::from(n))
    }
    fn dec(s: &str) -> W2nValue {
        W2nValue::Dec(s.parse::<BigDecimal>().unwrap())
    }
    fn p(s: &str) -> Result<W2nValue> {
        parse_number_string(s, None, None, None)
    }
    fn pl(s: &str, lang: &str) -> Result<W2nValue> {
        parse_number_string(s, None, None, Some(lang))
    }
    fn err(s: &str) -> String {
        p(s).unwrap_err().0
    }

    // -- get_format ------------------------------------------------------

    #[test]
    fn get_format_exact_base_and_default() {
        assert_eq!(get_format("fr").thousands, " ");
        assert_eq!(get_format("fr").decimal, ",");
        assert_eq!(get_format("fr_CH").thousands, "'"); // Swiss French
        assert_eq!(get_format("fr_CH").decimal, ".");
        assert_eq!(get_format("en_XX").thousands, ","); // base fallback: en
        assert_eq!(get_format("zz"), get_format("_default"));
        assert_eq!(get_format(""), get_format("_default")); // == get_format(None)
        assert_eq!(get_format("de").thousands, ".");
        assert_eq!(get_format("vi").thousands, ".");
        assert_eq!(get_format("da").thousands, "."); // dot, unlike its neighbours
        assert_eq!(NUMBER_FORMAT_DEFAULTS.len(), 55);
    }

    #[test]
    fn locale_table_uses_ascii_space_only() {
        // The Python table has no NBSP/NNBSP/THIN SPACE anywhere.
        for (k, f) in NUMBER_FORMAT_DEFAULTS {
            for sep in [f.thousands, f.decimal] {
                assert!(sep.is_ascii(), "{k} has a non-ASCII separator");
            }
        }
    }

    // -- traced cases: auto-detect ---------------------------------------

    #[test]
    fn traced_auto_detect() {
        assert_eq!(p("1,234").unwrap(), dec("1.234")); // ambiguous → decimal
        assert_eq!(p("1.234").unwrap(), dec("1.234"));
        assert_eq!(p("12,345").unwrap(), dec("12.345"));
        // A leading group of 4+ digits proves the string is not grouped (#55).
        assert_eq!(p("1234.567").unwrap(), dec("1234.567"));
        assert_eq!(p("1234,567").unwrap(), dec("1234.567"));
        assert_eq!(p(",123").unwrap(), int(123)); // empty lead → thousands
        assert_eq!(p("1,234.56").unwrap(), dec("1234.56")); // rightmost is decimal
        assert_eq!(p("1.234,56").unwrap(), dec("1234.56"));
        assert_eq!(p("1 234 567").unwrap(), int(1234567));
        assert_eq!(p("1'234.5").unwrap(), dec("1234.5")); // apostrophe stripped
        assert_eq!(p("1_000").unwrap(), int(1000)); // underscore stripped
        // Several separators group thousands only in groups of three (#55).
        assert_eq!(p("1,234,567").unwrap(), int(1234567));
        assert_eq!(err("1,,234"), "not a parseable number: '1,,234'");
        assert_eq!(err("1.5.2026"), "not a parseable number: '1.5.2026'");
        assert_eq!(err("1,23.4"), "not a parseable number: '1,23.4'");
        assert_eq!(p("12,345.67").unwrap(), dec("12345.67"));
        assert_eq!(p("1,234,567.89").unwrap(), dec("1234567.89"));
    }

    /// The three space-like separators must stay distinct codepoints and all
    /// three must be stripped by auto-detection.
    #[test]
    fn traced_space_like_separators_are_distinct() {
        assert_ne!(NBSP, NNBSP);
        assert_ne!(NNBSP, THIN_SPACE);
        assert_eq!(NBSP as u32, 0xA0);
        assert_eq!(NNBSP as u32, 0x202F);
        assert_eq!(THIN_SPACE as u32, 0x2009);
        assert_eq!(SPACE_LIKE.chars().count(), 5);

        assert_eq!(p("1\u{202f}234,5").unwrap(), dec("1234.5")); // NNBSP
        assert_eq!(p("1\u{a0}234,5").unwrap(), dec("1234.5")); // NBSP
        assert_eq!(p("1\u{2009}234,5").unwrap(), dec("1234.5")); // THIN SPACE
    }

    // -- traced cases: sign / strip --------------------------------------

    #[test]
    fn traced_sign_and_strip() {
        assert_eq!(p("+42").unwrap(), int(42));
        assert_eq!(p("  42  ").unwrap(), int(42));
        assert_eq!(p("- 42").unwrap(), int(-42)); // lstrip after the sign
        assert_eq!(p("-1 234,5").unwrap(), dec("-1234.5"));
        assert_eq!(p("-0").unwrap(), int(0)); // ints have no -0
        assert_eq!(p("\u{a0}\u{a0}42\u{a0}").unwrap(), int(42));
        assert_eq!(p("\u{1c}42").unwrap(), int(42)); // \s covers U+001C
        // A decimal zero is exact; BigDecimal carries no sign on zero.
        assert_eq!(p("-0.0").unwrap(), dec("0.0"));
        // Only ONE sign char is consumed.
        assert_eq!(err("+-42"), "not a parseable number: '-42'");
    }

    // -- traced cases: lang defaults + fallback ---------------------------

    #[test]
    fn traced_lang_path() {
        assert_eq!(pl("1.234", "de").unwrap(), int(1234)); // dot = thousands
        assert_eq!(pl("1.234", "en").unwrap(), dec("1.234")); // dot = decimal
        assert_eq!(pl("1 234,5", "fr").unwrap(), dec("1234.5"));
        assert_eq!(pl("1,234.5", "en").unwrap(), dec("1234.5"));
        assert_eq!(pl("1.234,5", "de").unwrap(), dec("1234.5"));
        assert_eq!(pl("1.234.567", "de").unwrap(), int(1234567));
        assert_eq!(pl("12.345,67", "de").unwrap(), dec("12345.67"));
        assert_eq!(pl("1.234.567,89", "de").unwrap(), dec("1234567.89"));
        assert_eq!(pl("1 234,56", "fr").unwrap(), dec("1234.56"));
        // A dot followed by 1, 2 or 4+ digits is a decimal in any locale (#53).
        assert_eq!(pl("3.14", "fr").unwrap(), dec("3.14"));
        assert_eq!(pl("0.5", "es").unwrap(), dec("0.5"));
        assert_eq!(pl("1.5", "de").unwrap(), dec("1.5"));
        assert_eq!(pl("12.5", "ru").unwrap(), dec("12.5"));
        assert_eq!(pl("1,5", "en").unwrap(), dec("1.5"));
        assert_eq!(pl("1.2345", "de").unwrap(), dec("1.2345"));
        // ...but "1.234" stays a thousands group where decimals take a comma.
        assert_eq!(pl("1.234", "fr").unwrap(), int(1234));
        assert_eq!(pl("1,234", "en").unwrap(), int(1234));
        assert_eq!(pl("1,234", "de").unwrap(), dec("1.234"));
        // Not a grouped number in any locale.
        assert!(pl("1.2.3", "fr").is_err());
        // Unknown lang: no locale row, auto-detect (#54).
        assert_eq!(pl("1,5", "zz").unwrap(), dec("1.5"));
        assert_eq!(pl("1.234,5", "id").unwrap(), dec("1234.5"));
        assert_eq!(pl("1,5", "id").unwrap(), dec("1.5"));
        // A hyphenated region finds its row (#54).
        assert_eq!(pl("1.234,56", "pt-BR").unwrap(), dec("1234.56"));
        assert_eq!(pl("1.234,56", "es-MX").unwrap(), dec("1234.56"));
    }

    /// The lang branch swallows Words2NumError and falls through to
    /// auto-detection, which strips the apostrophe the fr format left behind.
    #[test]
    fn traced_lang_failure_falls_through_to_auto_detect() {
        assert_eq!(pl("1'234", "fr").unwrap(), int(1234));
        // Both paths fail → the auto-detect error surfaces.
        assert_eq!(
            pl("1-2", "en").unwrap_err().0,
            "not a parseable number: '1-2'"
        );
    }

    // -- traced cases: explicit separators --------------------------------

    #[test]
    fn traced_explicit_separators() {
        let ps = |s, t, d| parse_number_string(s, t, d, None);
        assert_eq!(ps("1,234", Some(","), None).unwrap(), int(1234));
        assert_eq!(ps("1,234", None, Some(",")).unwrap(), dec("1.234"));
        assert_eq!(ps("1x234y5", Some("x"), Some("y")).unwrap(), dec("1234.5"));
        assert_eq!(ps("1.234", None, Some(".")).unwrap(), dec("1.234"));
        assert_eq!(ps("12'345.67", Some("'"), Some(".")).unwrap(), dec("12345.67"));
        assert_eq!(ps("1_234.56", Some("_"), None).unwrap(), dec("1234.56"));
        // Some("") is Python's "" — falsy, so no rewrite happens at all.
        assert_eq!(ps("1.234", Some(""), None).unwrap(), dec("1.234"));
        assert_eq!(ps("1.234", Some(""), Some("")).unwrap(), dec("1.234"));
        // A configured space-like strips ALL space-likes...
        assert_eq!(ps("1 234", Some(" "), None).unwrap(), int(1234));
        assert_eq!(ps("1\u{a0}234", Some(" "), None).unwrap(), int(1234));
        assert_eq!(ps("1\u{202f}234", Some("\u{202f}"), None).unwrap(), int(1234));
        assert_eq!(ps("1\u{2009}234", Some("\u{2009}"), None).unwrap(), int(1234));
        // ...but "\t" is NOT in (" ", NBSP, NNBSP, THIN_SPACE), so it only
        // does a literal replace and the NBSP survives into the error.
        assert_eq!(
            ps("1\u{a0}234", Some("\t"), None).unwrap_err().0,
            "not a parseable number: '1\\xa0234'"
        );
        assert_eq!(
            ps("1,234", Some("x"), Some("y")).unwrap_err().0,
            "not a parseable number: '1,234'"
        );
    }

    // -- traced cases: _to_number -----------------------------------------

    #[test]
    fn traced_to_number_bigint_and_float() {
        assert_eq!(p("0.1").unwrap(), dec("0.1"));
        // Exact: no binary rounding (#71).
        assert_eq!(p("1.0000000000000000001").unwrap(), dec("1.0000000000000000001"));
        assert_eq!(p("12345678901234567.89").unwrap(), dec("12345678901234567.89"));
        match p("12.50").unwrap() {
            W2nValue::Dec(d) => assert_eq!(d.to_string(), "12.50"), // scale kept
            v => panic!("expected Decimal, got {v:?}"),
        }
        assert_eq!(
            p(&"9".repeat(100)).unwrap(),
            W2nValue::Int("9".repeat(100).parse::<BigInt>().unwrap())
        );
        // Decimal() keeps every digit where float() saturated to inf.
        let big = format!("{}.5", "9".repeat(400));
        assert_eq!(p(&big).unwrap(), dec(&big));
    }

    /// Python's `\d` is Unicode Nd, and int()/float() accept it — mixed scripts
    /// included.
    #[test]
    fn traced_unicode_digits() {
        assert_eq!(p("\u{0662}\u{0663}").unwrap(), int(23)); // ٢٣ Arabic-Indic
        assert_eq!(p("\u{0661}2").unwrap(), int(12)); // ١2 mixed scripts
        assert_eq!(p("1\u{0662}3").unwrap(), int(123)); // 1٢3
        assert_eq!(p("\u{FF11}\u{FF12}\u{FF13}").unwrap(), int(123)); // １２３
        assert_eq!(p("\u{0967}\u{0968}\u{0969}").unwrap(), int(123)); // १२३ Devanagari
        assert_eq!(p("\u{0662}\u{0663}.\u{0665}").unwrap(), dec("23.5")); // ٢٣.٥

        assert_eq!(decimal_digit('7'), Some(7));
        assert_eq!(decimal_digit('\u{0669}'), Some(9)); // ٩
        assert_eq!(decimal_digit('\u{1FBF4}'), Some(4)); // last run
        assert_eq!(decimal_digit('\u{1D7CE}'), Some(0)); // 4-decade run
        assert_eq!(decimal_digit('\u{1D7FF}'), Some(9)); // ...its 4th decade
        assert_eq!(decimal_digit('x'), None);
        assert_eq!(decimal_digit('\u{065F}'), None); // just below the ٠ run
        assert_eq!(decimal_digit('\u{066A}'), None); // just above it
    }

    #[test]
    fn fullmatch_matches_the_regex() {
        assert_eq!(fullmatch_number("123").as_deref(), Some("123"));
        assert_eq!(fullmatch_number("1.23").as_deref(), Some("1.23"));
        assert_eq!(fullmatch_number("12."), None); // needs \d+ after the dot
        assert_eq!(fullmatch_number(".5"), None); // needs \d+ before it
        assert_eq!(fullmatch_number("1.2.3"), None);
        assert_eq!(fullmatch_number(""), None);
        assert_eq!(fullmatch_number("12a"), None);
        assert_eq!(fullmatch_number("1 2"), None);
    }

    // -- traced cases: errors ---------------------------------------------

    #[test]
    fn traced_errors() {
        assert_eq!(err(""), "empty numeric string");
        assert_eq!(err("   "), "empty numeric string");
        assert_eq!(err("-"), "empty numeric string after sign");
        assert_eq!(err("+ "), "empty numeric string after sign");
        assert_eq!(err("abc"), "not a parseable number: 'abc'");
        assert_eq!(err("not a number"), "not a parseable number: 'notanumber'");
        assert_eq!(err("12a"), "not a parseable number: '12a'");
    }

    /// The Swiss typographic apostrophe groups thousands like the ASCII one (#109).
    #[test]
    fn swiss_typographic_apostrophe() {
        assert_eq!(pl("1\u{2019}234.5", "fr_CH").unwrap(), dec("1234.5"));
        assert_eq!(pl("1\u{2019}234\u{2019}567", "fr_CH").unwrap(), int(1234567));
        assert_eq!(p("1\u{2019}234.5").unwrap(), dec("1234.5"));
        assert_eq!(p("1\u{02bc}234").unwrap(), int(1234));
        let ps = |s, t, d| parse_number_string(s, t, d, None);
        assert_eq!(ps("1\u{2019}234.5", Some("'"), Some(".")).unwrap(), dec("1234.5"));
    }

    /// Identical or digit separators are refused, not silently applied (#188).
    #[test]
    fn ambiguous_separator_overrides_are_rejected() {
        let ps = |s, t, d| parse_number_string(s, t, d, None);
        assert_eq!(
            ps("1.5", Some("."), Some(".")).unwrap_err().0,
            "thousands_sep and decimal_sep must differ (both '.')"
        );
        assert!(ps("1,5", Some(","), Some(",")).is_err());
        assert!(ps("1,234.5", Some(","), Some(",")).is_err());
        assert!(ps("1x5", Some("1"), None).is_err());
        // An empty override is "not given", not a clash.
        assert_eq!(ps("1.5", Some(""), Some("")).unwrap(), dec("1.5"));
    }

    #[test]
    fn py_repr_matches_cpython() {
        assert_eq!(py_repr("1 234"), "'1 234'");
        assert_eq!(py_repr("1\u{a0}234"), "'1\\xa0234'");
        assert_eq!(py_repr("ab'c"), "\"ab'c\"");
        assert_eq!(py_repr("ab\"c"), "'ab\"c'");
        assert_eq!(py_repr("a'b\"c"), "'a\\'b\"c'");
        assert_eq!(py_repr("x\ty"), "'x\\ty'");
        assert_eq!(py_repr("x\ny"), "'x\\ny'");
        assert_eq!(py_repr("x\ry"), "'x\\ry'");
        assert_eq!(py_repr("a\\b"), "'a\\\\b'");
        assert_eq!(py_repr("\u{7f}"), "'\\x7f'");
        assert_eq!(py_repr("\u{b}"), "'\\x0b'"); // no \v short form
        assert_eq!(py_repr("\u{c}"), "'\\x0c'"); // no \f short form
        assert_eq!(py_repr("\u{202f}"), "'\\u202f'");
        assert_eq!(py_repr("\u{2009}"), "'\\u2009'");
        assert_eq!(py_repr("\u{1c}"), "'\\x1c'");
        assert_eq!(py_repr("\u{ad}"), "'\\xad'"); // soft hyphen, Cf
        assert_eq!(py_repr("\u{e0001}"), "'\\U000e0001'");
        assert_eq!(py_repr("\u{f0000}"), "'\\U000f0000'");
        assert_eq!(py_repr("\u{1F600}"), "'\u{1F600}'"); // printable, raw
        assert_eq!(py_repr("é"), "'é'");
        assert_eq!(py_repr("\u{301}"), "'\u{301}'"); // combining Mn, printable
        assert_eq!(py_repr(""), "''");
        assert_eq!(py_repr(" "), "' '"); // ASCII space is printable
    }

    #[test]
    fn is_py_space_is_pythons_29_not_rusts_25() {
        let n = (0u32..0x110000)
            .filter_map(char::from_u32)
            .filter(|&c| is_py_space(c))
            .count();
        assert_eq!(n, 29);
        // The four Rust's char::is_whitespace() disagrees about.
        for c in ['\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}'] {
            assert!(is_py_space(c) && !c.is_whitespace());
        }
    }

    #[test]
    fn nd_runs_are_well_formed() {
        let mut total = 0;
        let mut prev_hi = 0u32;
        for (i, &(lo, hi)) in ND_RUNS.iter().enumerate() {
            assert!(lo <= hi);
            assert!(i == 0 || lo > prev_hi, "runs must be sorted and disjoint");
            assert_eq!((hi - lo + 1) % 10, 0, "every run is whole decades");
            prev_hi = hi;
            total += hi - lo + 1;
        }
        assert_eq!(total, 760); // Unicode 16.0.0
    }
}
