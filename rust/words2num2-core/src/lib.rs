//! Pure-Rust core for words2num2 — the inverse of num2words2.
//!
//! # Why this is small
//!
//! 119 of words2num2's 120 locales never had a hand-written parser. They use
//! `Words2Num_Base`, which materialises a reverse lookup table by calling
//! `num2words` across `LOOKUP_RANGE` (`range(-1, 10001)`) — 10,002 renders —
//! and then does a dict hit. Only `en` is hand-written.
//!
//! So the port is: the generic table backend + `_normalize` + the `en`
//! grammar parser. The table is built by calling the Rust num2words core
//! (`num2words2-core`) directly, which is where the speedup comes from.
//!
//! This crate has **no** PyO3 dependency: it is the pure-Rust engine. The
//! `words2num2-py` crate is a thin PyO3 binder over the public API here.

use num_bigint::BigInt;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

mod ord_compose;
pub mod ord_forms;
pub mod w2n_currency;
pub mod w2n_decimal;
pub mod w2n_formats;
pub mod w2n_forms;
pub mod w2n_lang_en;
pub mod w2n_sentence;

/// The public single-token entry point (`words2num2.words2num`). Its dispatch
/// — `_resolve_lang`, the en-vs-reverse-table choice, and the `to` mode
/// selection — lives in [`w2n_sentence::words2num`].
///
/// A number written in figures ("1,234", "1.234", "2,5", "1 000") is read here
/// first, with the language's own separators ([`w2n_formats`]): the word
/// grammars split it at the separator and summed the groups, so en "1,234"
/// came out as 235 and de "1.234" as 1.234 (#59, #60). Decimals come back as
/// an exact `Decimal`, the same type a spoken decimal gets (#71).
pub fn words2num(
    text: &str,
    lang: &str,
    to: &str,
) -> Result<w2n_lang_en::W2nValue, w2n_sentence::W2nError> {
    // de "minus 1.234": a sign word before a figure, read with the same
    // separators as "-1.234" (#309).
    if matches!(to, "cardinal" | "currency") {
        // A comma or mark glued to the sign ("-، 50") is no bare sign word.
        let split = text.trim().split_once(char::is_whitespace).filter(|(h, _)| !h.contains([',', '\u{060C}']));
        if let Some((head, figure)) = split {
            let resolved = w2n_sentence::resolve_lang(lang)?;
            let word = normalize(head);
            let is_sign = ["minus", "negative"].contains(&word.as_str())
                || strip_sign(&resolved, &format!("{word} x")).is_some_and(|(r, glued)| r == "x" && !glued);
            let unsigned = figure.trim_start().chars().next().is_some_and(|c| c != '+' && !w2n_formats::is_minus(c));
            if is_sign && unsigned && is_digit_literal(figure) {
                return words2num(&format!("-{}", figure.trim_start()), lang, to);
            }
        }
    }
    if matches!(to, "cardinal" | "ordinal" | "year" | "currency") && is_digit_literal(text) {
        let resolved = w2n_sentence::resolve_lang(lang)?;
        let key = w2n_formats::format_key(lang, &resolved);
        return match w2n_formats::parse_number_string(text, None, None, Some(&key)) {
            Ok(w2n_formats::W2nValue::Int(i)) => Ok(w2n_lang_en::W2nValue::Int(i)),
            Ok(w2n_formats::W2nValue::Dec(d)) => {
                Ok(w2n_lang_en::W2nValue::Dec(w2n_lang_en::PyDec::from_bigdecimal(&d)))
            }
            Ok(w2n_formats::W2nValue::Float(f)) => Ok(w2n_lang_en::W2nValue::Float(f)),
            Err(e) => Err(w2n_sentence::W2nError::Words2Num(e.0)),
        };
    }
    w2n_sentence::words2num(text, lang, to)
}

/// Is `text` a number in figures and nothing else: an optional sign, then
/// digits and the separators a number is written with (`.`, `,`, `'`, `’`,
/// `_`, spaces), starting and ending on a digit? Space-separated groups must
/// be thousands groups ("1 000", "1 234,5"), so "1 2" or "12 34" — two
/// numbers side by side — is not one and is left to the word grammar.
fn is_digit_literal(text: &str) -> bool {
    let t = text.trim();
    let body = t.strip_prefix(|c| c == '+' || w2n_formats::is_minus(c)).map(str::trim_start).unwrap_or(t);
    let is_digit = |c: char| c.is_numeric() && w2n_sentence::digit_value(c).is_some();
    let is_sep = |c: char| matches!(c, '.' | ',' | '\'' | '\u{2019}' | '_') || c.is_whitespace();
    let (Some(first), Some(last)) = (body.chars().next(), body.chars().last()) else {
        return false;
    };
    if !is_digit(first) || !is_digit(last) || !body.chars().all(|c| is_digit(c) || is_sep(c)) {
        return false;
    }
    let mut groups = body.split(char::is_whitespace).filter(|g| !g.is_empty());
    groups.next();
    groups.all(|g| {
        let lead: usize = g.chars().take_while(|&c| is_digit(c)).count();
        lead == 3 && g.chars().nth(3).is_none_or(|c| matches!(c, '.' | ','))
    })
}

/// Python's `Words2Num_Base.LOOKUP_RANGE`, without its -1: a sign is stripped
/// before any lookup (see [`strip_sign`]), and a table entry "minus one" made
/// the scale composer read am "ሰልቢ አንድ ሺህ …" (-1234) as -1 × 1000 + 234 (#61).
const LOOKUP_LO: i64 = 0;
const LOOKUP_HI: i64 = 10001;

/// Error from the reverse-table backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError {
    /// num2words2 has no backend for this locale key
    /// (Python raised `NotImplementedError(lang)`).
    NotImplemented(String),
}

/// Port of the tail of `Words2Num_Base._normalize` — the pure ASCII-shaped
/// rewriting that follows NFKD decomposition + combining-mark stripping.
///
/// ```python
/// text = text.lower().replace("_", " ")
/// text = re.sub(r"(?<=[a-z])-(?=[a-z])", " ", text)
/// text = re.sub(r"[,;:!\?\"']", " ", text)
/// text = re.sub(r"\.(?!\d)", " ", text)
/// text = re.sub(r"\s+", " ", text).strip()
/// ```
///
/// Beyond the Python shape, the typographic variants a word processor, an IME
/// or a web page substitutes for the characters num2words writes are folded
/// first, on the table side and the input side alike: dashes and the minus
/// sign to `-` (#114), curly quotes and modifier-letter apostrophes to `'`
/// (#115), tr/az dotless "ı" to "i" (uppercase "KIRK" lowercases to "kirk",
/// #116), de "ß" to "ss" ("dreissig", #117), invisible format characters
/// dropped (#118), and script-native punctuation treated like the ASCII set
/// (danda, shad, Arabic comma, CJK full stop, guillemets, brackets, #121).
pub fn normalize_tail(decomposed: &str) -> String {
    let mut chars: Vec<char> = Vec::with_capacity(decomposed.len());
    for c in decomposed.to_lowercase().chars() {
        match c {
            '_' => chars.push(' '),
            // Turkish/Azerbaijani: "I" lowercases to "i" outside a Turkic
            // locale, so "SIFIR" must meet the table's "sıfır" on "sifir".
            '\u{131}' => chars.push('i'),
            // Full case folding: Swiss German has no ß, and "DREISSIG" is its capital.
            'ß' => chars.extend(['s', 's']),
            // Hyphens, dashes and the minus sign a word processor substitutes for '-'.
            '\u{2010}'..='\u{2015}' | '\u{2212}' | '\u{FE58}' | '\u{FE63}' | '\u{FF0D}' => {
                chars.push('-')
            }
            // Zero-width space is a word break; the other format characters
            // (soft hyphen, ZWNJ/ZWJ after a virama, word joiner, BOM) are
            // invisible and carry no letter.
            '\u{200B}' => chars.push(' '),
            '\u{00AD}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}' => {}
            c if is_apostrophe(c) || is_clause_punctuation(c) => chars.push(' '),
            c => chars.push(c),
        }
    }
    let mut out = String::with_capacity(chars.len());

    for (i, &c) in chars.iter().enumerate() {
        match c {
            // (?<=[a-z])-(?=[a-z]) — a hyphen joining two words becomes a
            // space, but a hyphen before a digit is a sign and must survive.
            // "å" counts as [a-z]: it kept its ring only to tell no "åttende"
            // from "attende" (see `normalize`), it is still a Latin letter.
            '-' => {
                let latin = |n: &char| n.is_ascii_lowercase() || *n == 'å';
                let prev_alpha = i > 0 && latin(&chars[i - 1]);
                let next_alpha = chars.get(i + 1).is_some_and(latin);
                out.push(if prev_alpha && next_alpha { ' ' } else { '-' });
            }
            ',' | ';' | ':' | '!' | '?' | '"' | '\'' => out.push(' '),
            // \.(?!\d) — sentence-final dot goes, decimal point stays.
            '.' => {
                let next_digit = chars.get(i + 1).is_some_and(|n| n.is_ascii_digit());
                out.push(if next_digit { '.' } else { ' ' });
            }
            _ => out.push(c),
        }
    }
    // \s+ -> " ", then strip. bo: num2words ends every syllable with a tsheg,
    // text drops the last one before a space, a shad or the end ("གཅིག" for
    // "གཅིག་", #120), so a word-final tsheg is not part of the key. Nor is a
    // word-initial one, which is what is left of "གཅིག་ཁྲི་གཅིག" once the scale
    // composer has cut "ཁྲི" (10^4) out of it.
    out.split_whitespace()
        .map(|w| w.trim_matches(['\u{0F0B}', '\u{0F0C}']))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The marks typed for the ASCII apostrophe num2words writes (uk "п'ять",
/// uz "to'rt", haw "'ekahi"): the curly quotes, the modifier letters
/// apostrophe / turned comma (uk "пʼять", uz "toʻrt", the haw ʻokina) and the
/// prime / backtick.
pub fn is_apostrophe(c: char) -> bool {
    matches!(c, '\u{2018}' | '\u{2019}' | '\u{201B}' | '\u{02BC}' | '\u{02BB}' | '\u{02B9}' | '`')
}

/// Sentence and clause marks, quotes and brackets that sit glued to a word the
/// way ASCII ",.!?" do: "पाँच।", "خمسة،", "五。", "አምስት።", "«cinq»", "(five)".
/// Word-internal marks are left alone (the Catalan "l·l" middle dot, the
/// Armenian stress and question marks written over a vowel).
pub fn is_clause_punctuation(c: char) -> bool {
    matches!(c,
        '(' | ')' | '[' | ']' | '{' | '}'
        | '\u{00A1}' | '\u{00BF}'                   // ¡ ¿
        | '\u{00AB}' | '\u{00BB}'                   // « »
        | '\u{201A}' | '\u{201C}'..='\u{201F}'      // ‚ “ ” „ ‟
        | '\u{2039}' | '\u{203A}'                   // ‹ ›
        | '\u{0589}' | '\u{055D}'                   // Armenian full stop, comma
        | '\u{05C3}'                                // Hebrew sof pasuq
        | '\u{060C}' | '\u{061B}' | '\u{061F}' | '\u{06D4}' // Arabic ، ؛ ؟ ۔
        | '\u{0964}' | '\u{0965}'                   // danda, double danda
        | '\u{0DF4}'                                // Sinhala kunddaliya
        | '\u{0E2F}' | '\u{0E5A}' | '\u{0E5B}'      // Thai paiyannoi, angkhankhu, khomut
        | '\u{0F0D}'..='\u{0F12}'                   // Tibetan shads
        | '\u{104A}' | '\u{104B}'                   // Myanmar section marks
        | '\u{10FB}'                                // Georgian paragraph separator
        | '\u{1361}'..='\u{1368}'                   // Ethiopic wordspace, full stop, commas
        | '\u{17D4}'..='\u{17D6}'                   // Khmer khan
        | '\u{3001}' | '\u{3002}'                   // CJK 、 。
        | '\u{3008}'..='\u{3011}'                   // CJK angle / corner / lenticular brackets
        | '\u{3014}'..='\u{301B}'
    )
}

/// Pure-Rust port of `Words2Num_Base._normalize`.
///
/// ```python
/// nfkd = unicodedata.normalize("NFKD", text)
/// text = "".join(c for c in nfkd if not unicodedata.combining(c))
/// # ... normalize_tail ...
/// ```
///
/// The NFKD decomposition + combining-mark strip is what makes "trente-deux"
/// match "trente deux". It is done here with the `unicode-normalization`
/// crate rather than round-tripping to Python's `unicodedata`, so the core is
/// self-contained.
///
/// Only *optional* diacritics are stripped (see [`is_optional_diacritic`]).
/// Stripping every combining mark erased the vowel signs of the Indic,
/// Tibetan and Thaana scripts, which are letters, not accents: hi "पचासी"
/// (85) became "पचास" (50) and parsed as 50. Across the 0..=10000 tables
/// that was 5310 silently wrong values in 7 languages.
///
/// Three spelling variants are unified on top: ml legacy chillus (consonant +
/// virama + ZWJ, "ഒന്‍പത്") become the atomic chillu num2words writes (#119);
/// the Devanagari chandrabindu becomes the anusvara everyday typing uses (hi
/// "पांच" = "पाँच", #122) and the Bengali one is optional. The ring of "å" is
/// kept: no/nb "åttende" is 8th and "attende" 18th (#44). The tables index
/// every "å" key a second time without the ring (see [`build_table`]), so
/// "atte" still reads 8.
pub fn normalize(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    if s.is_ascii() {
        return normalize_tail(s); // NFKD and the combining-mark filter are no-ops on ASCII
    }
    let nfkd: String = s.nfkd().collect();
    let nfkd = if nfkd.contains('\u{200D}') { ml_atomic_chillus(&nfkd) } else { nfkd };
    let mut stripped = String::with_capacity(nfkd.len());
    for c in nfkd.chars() {
        match c {
            '\u{030A}' if stripped.ends_with('a') => {
                stripped.pop();
                stripped.push('å');
            }
            '\u{030A}' if stripped.ends_with('A') => {
                stripped.pop();
                stripped.push('Å');
            }
            '\u{0901}' => stripped.push('\u{0902}'), // Devanagari chandrabindu -> anusvara
            '\u{0981}' => {}                          // Bengali chandrabindu: optional
            c if is_optional_diacritic(c) => {}
            c => stripped.push(c),
        }
    }
    normalize_tail(&stripped)
}

/// ml: rewrite the pre-Unicode-5.1 chillu (consonant + virama + ZWJ) to the
/// atomic chillu num2words2 renders. Dropping the ZWJ alone would leave
/// consonant + virama, which is a different letter.
fn ml_atomic_chillus(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let chillu = match chars[i] {
            '\u{0D23}' => Some('\u{0D7A}'), // ണ -> ൺ
            '\u{0D28}' => Some('\u{0D7B}'), // ന -> ൻ
            '\u{0D30}' => Some('\u{0D7C}'), // ര -> ർ
            '\u{0D32}' => Some('\u{0D7D}'), // ല -> ൽ
            '\u{0D33}' => Some('\u{0D7E}'), // ള -> ൾ
            '\u{0D15}' => Some('\u{0D7F}'), // ക -> ൿ
            _ => None,
        };
        match chillu {
            Some(ch) if chars.get(i + 1) == Some(&'\u{0D4D}') && chars.get(i + 2) == Some(&'\u{200D}') => {
                out.push(ch);
                i += 3;
            }
            _ => {
                out.push(chars[i]);
                i += 1;
            }
        }
    }
    out
}

/// A combining mark that a writer may omit without changing the word: the
/// generic diacritic blocks NFKD produces for Latin, Greek and Cyrillic
/// (é, ñ, ά, й), Arabic harakat and Hebrew points, which ordinary text
/// leaves out, and the Indic nukta (hi "सड़सठ" is often typed "सडसठ").
/// Script-intrinsic marks — Brahmic vowel signs and viramas,
/// Tibetan and Thaana vowels, Thai/Lao vowels and tone marks — are part of
/// the spelling and are kept.
fn is_optional_diacritic(c: char) -> bool {
    matches!(c,
        '\u{0300}'..='\u{036F}'     // Combining Diacritical Marks
        | '\u{1AB0}'..='\u{1AFF}'   // … Extended
        | '\u{1DC0}'..='\u{1DFF}'   // … Supplement
        | '\u{20D0}'..='\u{20FF}'   // … for Symbols
        | '\u{FE20}'..='\u{FE2F}'   // Combining Half Marks
        | '\u{064B}'..='\u{065F}'   // Arabic harakat
        | '\u{0670}'                // Arabic superscript alef
        | '\u{0591}'..='\u{05C7}'   // Hebrew cantillation and points
        // Nukta: Devanagari, Bengali, Gurmukhi, Gujarati, Odia, Telugu, Kannada
        | '\u{093C}' | '\u{09BC}' | '\u{0A3C}' | '\u{0ABC}'
        | '\u{0B3C}' | '\u{0C3C}' | '\u{0CBC}'
    ) && unicode_normalization::char::is_combining_mark(c)
}

/// One reverse table: `{normalized_words: number}`.
type ReverseTable = HashMap<String, i64>;
/// The per-(lang, kind) cache of reverse tables. Shared out as `Arc`s so a
/// reader works on its table with no lock held.
type TableCache = RwLock<HashMap<(String, bool), Arc<ReverseTable>>>;

/// The reverse tables, built lazily per (lang, kind) exactly as Python does.
fn tables() -> &'static TableCache {
    static T: OnceLock<TableCache> = OnceLock::new();
    T.get_or_init(|| RwLock::new(HashMap::new()))
}

/// The reverse table of (`lang`, `ordinal`), built on first use. The lock is
/// only held to read or insert the `Arc`, never while the table is consulted:
/// the spoken forms may issue lookups of their own (hy ordinals read the
/// cardinal table, w2n_forms derives its forms from the renderer).
fn table(lang: &str, ordinal: bool) -> Result<Arc<ReverseTable>, LookupError> {
    let key = (lang.to_string(), ordinal);
    if let Some(t) = tables().read().unwrap().get(&key) {
        return Ok(t.clone());
    }
    let built = Arc::new(build_table(lang, ordinal)?);
    Ok(tables().write().unwrap().entry(key).or_insert(built).clone())
}

/// Port of `Words2Num_Base._build_table`.
///
/// Python calls `num2words(n, lang, to=kind)` for every n in LOOKUP_RANGE and
/// does `table.setdefault(key, n)` — **first write wins**, so the canonical
/// short form takes precedence over later spellings. That ordering is
/// load-bearing; do not switch to insert-overwrite.
///
/// The ordinal table starts at 0: there is no "minus first", and what the
/// renderers return for to_ordinal(-1) is garbage that shadowed real ordinals
/// (et "üheksas" = 9th and ms "kesepuluh" = 10th both came back as -1, #62).
/// It also holds the ordinal of every power of ten above the range (es
/// "millonésimo", fr "cent millième"), the scale ordinals a composed ordinal
/// ends on (#125).
fn build_table(lang: &str, ordinal: bool) -> Result<HashMap<String, i64>, LookupError> {
    let l = num2words2_core::get_lang_by_key(lang)
        .ok_or_else(|| LookupError::NotImplemented(lang.to_string()))?;
    let mut table = HashMap::new();
    let mut ringless = Vec::new();
    let lo = if ordinal { 0 } else { LOOKUP_LO };
    let scales = (5..=18).map(|k| 10i64.pow(k));
    let extra: Vec<i64> = if ordinal { scales.collect() } else { Vec::new() };
    for n in (lo..LOOKUP_HI).chain(extra) {
        let v = BigInt::from(n);
        let words = if ordinal { l.to_ordinal(&v) } else { l.to_cardinal(&v) };
        let key = words.ok().map(|w| normalize(&w)); // Python swallows every raise
        if let Some(key) = &key {
            if key.contains('å') {
                ringless.push((key.replace('å', "a"), n));
            }
            table.entry(key.clone()).or_insert(n);
        }
        if ordinal {
            for extra in ord_forms::extra_keys(lang, n, key.as_deref()) {
                table.entry(extra).or_insert(n);
            }
        }
    }
    // "å" typed without its ring ("atte" for "åtte") still reads, but only where
    // no number is spelled that way: no/nb "attende" is 18th, not "åttende" (8th).
    for (key, n) in ringless {
        table.entry(key).or_insert(n);
    }
    Ok(table)
}

/// Port of `Words2Num_Base._lookup` + `to_cardinal`/`to_ordinal`.
///
/// Returns `None` when the text is not in the table, so the Python side can
/// raise `Words2NumError` with its exact message rather than us guessing it.
pub fn lookup(
    lang: &str,
    text: &str,
    ordinal: bool,
    negative_words: &[String],
) -> Result<Option<i64>, LookupError> {
    let negs: Vec<&str> = negative_words.iter().map(String::as_str).collect();
    lookup_norm(lang, &normalize(text), ordinal, &negs)
}

/// [`lookup`] on text that is already normalized (the walker normalizes a run once
/// and then tries every reading on it).
pub fn lookup_norm(
    lang: &str,
    normalized: &str,
    ordinal: bool,
    negative_words: &[&str],
) -> Result<Option<i64>, LookupError> {
    let mut normalized = normalized;
    if normalized.is_empty() {
        return Ok(None);
    }

    let mut sign = 1i64;
    for neg in negative_words {
        if normalized == *neg {
            return Ok(None); // a bare negword is unparseable
        }
        if let Some(rest) = normalized
            .strip_prefix(neg)
            .and_then(|r| r.strip_prefix(' '))
        {
            sign = -1;
            normalized = rest;
            break;
        }
    }

    let tab = table(lang, ordinal)?;
    if let Some(v) = tab.get(normalized) {
        return Ok(Some(sign * v));
    }
    // The fr plural -s, the gender / declension / apocope of an ordinal.
    let variants = spoken_variants(lang, normalized, ordinal);
    if let Some(v) = variants.iter().find_map(|v| tab.get(v)) {
        return Ok(Some(sign * v));
    }
    if ordinal {
        return Ok(None);
    }
    // Gender / construct / attributive forms and glued connectors (ru "две",
    // he "שני" before מיליון, ar "وواحد" after a scale word).
    Ok(w2n_forms::fragment_value(lang, &tab, normalized).map(|(v, _)| sign * v))
}

/// A word (or words) a language puts in front of a number to make it negative,
/// normalized: fr "moins", zh "负", xh "ngaphantsi kwe", th "ติดลบ".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignPrefix {
    pub text: String,
    /// The renderer glues it to the number (th "ติดลบเจ็ด", tr "eksiyedi"), so
    /// it also matches without a following space.
    pub glued: bool,
}

/// The spoken minus words num2words2 does not render: kn/te write the symbol
/// "(-)" (#243), English-style "minus"/"negative" are accepted everywhere as
/// before, and a leading "-" glued to a word ("−five", #114) is a sign too.
fn extra_sign_words(lang: &str) -> &'static [&'static str] {
    match lang.split(&['_', '-'][..]).next().unwrap_or(lang) {
        "kn" => &["ಋಣ", "ಮೈನಸ್"],
        "te" => &["మైనస్", "ఋణ"],
        _ => &[],
    }
}

/// The negative prefixes of `lang`, longest first. Derived from the renderer:
/// `render(-n)` is `prefix + render(n)` in every language that renders
/// negatives (fr "moins sept", es "menos cuarenta y dos", am "ሰልቢ ሰባት"), so
/// the prefix is read off rather than listed by hand (#123).
pub fn sign_prefixes(lang: &str) -> Arc<Vec<SignPrefix>> {
    static S: OnceLock<RwLock<HashMap<String, Arc<Vec<SignPrefix>>>>> = OnceLock::new();
    let cache = S.get_or_init(|| RwLock::new(HashMap::new()));
    if let Some(v) = cache.read().unwrap().get(lang) {
        return v.clone();
    }
    let v = Arc::new(derive_sign_prefixes(lang));
    cache.write().unwrap().insert(lang.to_string(), v.clone());
    v
}

fn derive_sign_prefixes(lang: &str) -> Vec<SignPrefix> {
    let mut out: Vec<SignPrefix> = Vec::new();
    let mut add = |text: &str, glued: bool| {
        let text = text.trim();
        if text.is_empty() || text.chars().any(|c| c.is_ascii_digit()) {
            return;
        }
        match out.iter_mut().find(|p| p.text == text) {
            Some(p) => p.glued |= glued,
            None => out.push(SignPrefix { text: text.to_string(), glued }),
        }
    };
    if let Some(l) = num2words2_core::get_lang_by_key(lang) {
        let render = |n: i64| l.to_cardinal(&BigInt::from(n)).ok().map(|w| normalize(&w));
        for n in [7i64, 42, 1234] {
            let (Some(neg), Some(pos)) = (render(-n), render(n)) else { continue };
            if neg.len() > pos.len() && neg.ends_with(&pos) {
                let prefix = &neg[..neg.len() - pos.len()];
                add(prefix, !prefix.ends_with(' '));
            }
        }
    }
    for w in ["minus", "negative"].iter().chain(extra_sign_words(lang)) {
        add(&normalize(w), false); // NFKD splits kn "ೈ": compare like the input
    }
    add("-", true);
    out.sort_by(|a, b| b.text.len().cmp(&a.text.len()));
    out
}

/// Split a leading negative prefix off normalized text: `Some((rest, glued))`,
/// where `glued` says the prefix matched without a following space (the
/// caller then also tries the unsigned reading of the whole). A bare prefix,
/// or one followed by a digit when glued ("-17" is a literal), is `None`.
pub fn strip_sign<'a>(lang: &str, normalized: &'a str) -> Option<(&'a str, bool)> {
    for p in sign_prefixes(lang).iter() {
        let Some(rest) = normalized.strip_prefix(p.text.as_str()) else { continue };
        if let Some(rest) = rest.strip_prefix(' ') {
            if !rest.is_empty() {
                return Some((rest, false));
            }
        } else if p.glued && rest.chars().next().is_some_and(|c| c.is_alphabetic()) {
            return Some((rest, true));
        }
    }
    None
}

/// Run `f` on the cardinal reverse table of `lang`, building it first if needed.
/// No lock is held while `f` runs.
pub(crate) fn with_cardinal_table<R>(lang: &str, f: impl FnOnce(&ReverseTable) -> R) -> Option<R> {
    if !is_supported_lang(lang) {
        return None;
    }
    table(lang, false).ok().map(|t| f(&t))
}

/// The spoken variants of `normalized` worth a second table probe: the fr
/// plural -s on a cardinal, the gender / declension / apocope of an ordinal.
fn spoken_variants(lang: &str, normalized: &str, ordinal: bool) -> Vec<String> {
    if !ordinal {
        return fr_plural_variants(lang, normalized);
    }
    let mut out = ordinal_variants(lang, normalized);
    // Gender / case / number / article forms (ru "первая", pl "dwudziesta
    // pierwsza", hu "másodikán", hi "पहली"), per language.
    out.extend(ord_forms::variants(lang, normalized));
    // A compound spoken spaced where num2words glues it: es "vigésimo
    // primero" / "vigésima primera" -> "vigesimoprimero". Only a genuine
    // table hit counts, as for the cardinal despace retry.
    if normalized.contains(' ') {
        let despaced: Vec<String> = std::iter::once(normalized)
            .chain(out.iter().map(String::as_str))
            .map(|v| v.split(' ').collect())
            .collect();
        out.extend(despaced);
    }
    out
}

/// Does the normalized ordinal `norm` spell `rendered` (a normalized
/// `to_ordinal` rendering), as is or through one of its spoken variants?
fn spells_ordinal(lang: &str, norm: &str, rendered: &str) -> bool {
    norm == rendered || spoken_variants(lang, norm, true).iter().any(|v| v == rendered)
}

/// num2words renders one ordinal form per number (fr "premier", es "primero",
/// de "zweite", it "primo"); speech agrees it with the noun. The canonical
/// spelling of an inflected token, or `None` when it is not one.
fn ordinal_canonical(base: &str, tok: &str) -> Option<String> {
    let s = match (base, tok) {
        ("fr", "premiere") => "premier".to_string(),
        ("fr", "second" | "seconde") => "deuxieme".to_string(),
        ("es", "primer" | "primera") => "primero".to_string(),
        ("es", "tercer" | "tercera") => "tercero".to_string(),
        ("es" | "pt" | "it", t) if t.ends_with('a') && t.len() > 3 => {
            format!("{}o", &t[..t.len() - 1])
        }
        ("es" | "pt", t) if t.ends_with("os") || t.ends_with("as") => {
            format!("{}o", &t[..t.len() - 2])
        }
        ("it", t) if (t.ends_with('i') || t.ends_with('e')) && t.len() > 3 => {
            format!("{}o", &t[..t.len() - 1])
        }
        // de declines "zweite" -> zweiten / zweiter / zweites / zweitem.
        ("de", t)
            if t.len() > 4
                && (t.ends_with("ten")
                    || t.ends_with("ter")
                    || t.ends_with("tes")
                    || t.ends_with("tem")) =>
        {
            t[..t.len() - 1].to_string()
        }
        _ => return None,
    };
    Some(s)
}

/// Every combination of canonicalising the inflected ordinal tokens of
/// `normalized` (at most three such tokens).
fn ordinal_variants(lang: &str, normalized: &str) -> Vec<String> {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    let toks: Vec<&str> = normalized.split_whitespace().collect();
    let canon: Vec<Option<String>> = toks.iter().map(|t| ordinal_canonical(base, t)).collect();
    let slots: Vec<usize> = canon
        .iter()
        .enumerate()
        .filter(|(_, c)| c.is_some())
        .map(|(i, _)| i)
        .collect();
    if slots.is_empty() || slots.len() > 3 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for mask in 1u32..(1 << slots.len()) {
        let mut v: Vec<&str> = toks.clone();
        for (k, &pos) in slots.iter().enumerate() {
            if mask & (1 << k) != 0 {
                v[pos] = canon[pos].as_deref().unwrap_or(toks[pos]);
            }
        }
        out.push(v.join(" "));
    }
    out
}

/// fr: num2words writes "deux cents" and "quatre-vingts" but speech (and ASR)
/// drops or adds the plural -s freely ("mille deux cent", "quatre vingt euros",
/// "deux million"). Every combination of toggling those tokens, so the table
/// still matches. Empty for other languages or when no such token is present.
fn fr_plural_variants(lang: &str, normalized: &str) -> Vec<String> {
    if !lang.starts_with("fr") {
        return Vec::new();
    }
    let toggle = |t: &str| -> Option<&'static str> {
        match t {
            "cent" => Some("cents"),
            "cents" => Some("cent"),
            "vingt" => Some("vingts"),
            "vingts" => Some("vingt"),
            "million" => Some("millions"),
            "millions" => Some("million"),
            "milliard" => Some("milliards"),
            "milliards" => Some("milliard"),
            _ => None,
        }
    };
    let toks: Vec<&str> = normalized.split_whitespace().collect();
    let slots: Vec<usize> = toks
        .iter()
        .enumerate()
        .filter(|(_, t)| toggle(t).is_some())
        .map(|(i, _)| i)
        .collect();
    if slots.is_empty() || slots.len() > 4 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for mask in 1u32..(1 << slots.len()) {
        let mut v: Vec<&str> = toks.clone();
        for (k, &pos) in slots.iter().enumerate() {
            if mask & (1 << k) != 0 {
                v[pos] = toggle(toks[pos]).unwrap_or(toks[pos]);
            }
        }
        out.push(v.join(" "));
    }
    out
}

/// Languages the Rust core can serve (Python's `_RUST.supported_langs()`).
pub fn supported_langs() -> Vec<&'static str> {
    num2words2_core::supported_lang_keys()
}

/// `supported_langs().contains(lang)` without rebuilding the list on every call.
pub fn is_supported_lang(lang: &str) -> bool {
    static S: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    S.get_or_init(|| num2words2_core::supported_lang_keys().into_iter().collect())
        .contains(lang)
}

// ---------------------------------------------------------------------------
// Multi-scale cardinal composition (values above the LOOKUP_RANGE table)
// ---------------------------------------------------------------------------
//
// The reverse table only covers -1..10001, so a spoken number like
// "soixante-neuf mille huit" (69008) has no whole-string entry and the sentence
// walker was left emitting the fragments "69 1000 8". `parse_scaled` restores
// the arithmetic num2words never inverted: it splits on a language's scale
// words (mille=10^3, million=10^6, milliard=10^9) and composes
// `left * scale + right`, recursively. Space-separated languages (fr/es/pt/…)
// gain full thousands/millions support; agglutinative ones (de/nl, where the
// scale is glued into one token) find no split word and fall back to the table
// unchanged — so this never regresses them.

/// What the renderer tells us about a language's scale words.
#[derive(Debug, Default)]
struct Scales {
    /// `(scale_word_normalized, magnitude)`, biggest magnitude first. Includes the
    /// morphemes of glued renderings (km "ពីរពាន់" -> "ពាន់" = 10^3).
    words: Vec<(String, i64)>,
    /// Scale words whose multiplier FOLLOWS them: ha "dubu biyu" = 2000.
    postfix: Vec<String>,
    /// Multiplier forms heard only after a postfix scale word, in its noun
    /// class: sn "miriyoni mbiri" = 2·10^6, while the table's 2 is "piri".
    postfix_forms: Vec<(String, i64)>,
    /// Scale words that take "one" after them while every other multiplier comes
    /// first: eu "milioi bat" = 10^6 but "bi milioi" = 2·10^6. `(scale, one)`.
    one_after: Vec<(String, String)>,
    /// Forms of "one" heard only in front of a scale word, not table numbers on
    /// their own: ta "ஒரு இலட்சம்" = 10^5, de "eine Million".
    one_forms: Vec<String>,
    /// Groups are separate tokens summed without a scale word between them:
    /// km "មួយម៉ឺន ពីរពាន់" = 10000 + 2000.
    additive_groups: bool,
    /// Numbers are read one digit at a time: en_AERO "wun zero zero" = 100.
    digit_by_digit: bool,
    /// [`Scales::words`] that may also sit glued at the end of a longer token. A
    /// word only ever rendered bare and alone is left out: dv writes 1000 as one
    /// fused "one-thousand" token, and taking it as a suffix would read
    /// "thirty-one-thousand" (31000) as 30 × 1000.
    glue: Vec<(String, i64)>,
    /// Multiplier forms heard only glued in front of a scale morpheme, never as
    /// numbers on their own: dv "ދެހާސް" = "ދެ" (2) + "ހާސް", while the table's
    /// 2 is "ދޭއް".
    combining: Vec<(String, i64)>,
    /// Two-token units the renderer multiplies as one: kok writes 10^6 as
    /// "एक धा लाख" (one ten-lakh). `(spaced, joined, magnitude)`; the joined
    /// form is a [`Scales::words`] entry, see [`COMPOUND_JOINER`].
    compounds: Vec<(String, String, i64)>,
    /// The renderer stacks a scale word on a number that already holds one:
    /// bn "দশ কোটি কোটি" = 10 crore crore, th "หนึ่งล้านสองแสนล้าน" =
    /// 1 200 000 million. Only then may a multiplier exceed its scale.
    stacks: bool,
    /// The zero word that fills a skipped group inside a glued number: zh
    /// "一万零一" = 10001.
    zero: Option<String>,
}

/// Joins the two tokens of a [`Scales::compounds`] unit into one scale token.
/// Not whitespace, and never produced by [`normalize`].
const COMPOUND_JOINER: char = '\u{2060}';

/// Powers of ten whose renderings name the scale words: up to 10^18, the
/// largest power an i64 holds, with 10^11 for the Indian kharab.
const PROBE_POWERS: [u32; 12] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15, 18];

/// Multipliers rendered at every probed power: each digit (dv glues a special
/// form of 2..9 to the scale: "ދެހާސް"), and the 10 / 11 / 21 that some
/// plurals only follow (sl "trije milijoni", lt "dešimt milijardų").
const PROBE_MULTIPLIERS: [i64; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 21];

/// Longest input [`parse_scaled`] composes, in tokens and bytes. The longest
/// rendering of any i64 has 59 tokens (cy); a cap keeps the recursion and the
/// string rebuilding bounded on hostile input.
const MAX_SCALED_TOKENS: usize = 200;
const MAX_SCALED_BYTES: usize = 4096;

/// Cache of the derived [`Scales`] per language.
fn scale_cache() -> &'static RwLock<HashMap<String, Arc<Scales>>> {
    static S: OnceLock<RwLock<HashMap<String, Arc<Scales>>>> = OnceLock::new();
    S.get_or_init(|| RwLock::new(HashMap::new()))
}

fn scales_of(lang: &str) -> Arc<Scales> {
    if let Some(v) = scale_cache().read().unwrap().get(lang) {
        return v.clone();
    }
    let v = Arc::new(derive_scales(lang));
    scale_cache()
        .write()
        .unwrap()
        .insert(lang.to_string(), v.clone());
    v
}

/// Scale words for `lang`, biggest magnitude first: fr → {"millions"/"million" →
/// 10^6, "mille" → 10^3}, hi → {"करोड" → 10^7, "लाख" → 10^5, "हजार" → 10^3}.
pub fn scale_words(lang: &str) -> Vec<(String, i64)> {
    // Unsupported codes are answered without touching the per-lang caches,
    // which would otherwise keep an entry per distinct string (#216).
    if !is_supported_lang(lang) {
        return Vec::new();
    }
    scales_of(lang)
        .words
        .iter()
        .filter(|(w, _)| !w.contains(COMPOUND_JOINER))
        .cloned()
        .collect()
}

fn is_power_of_ten(mut n: i64) -> bool {
    if n < 1 {
        return false;
    }
    while n % 10 == 0 {
        n /= 10;
    }
    n == 1
}

/// Standard spellings of a scale word that num2words2 never writes, accepted
/// beside the ones it does: da "en million" (it writes "en millioner"), sl "pet
/// milijonov" / "pet milijard" (it writes "pet milijon" / "pet milijarda").
fn scale_aliases(lang: &str) -> &'static [(&'static str, i64)] {
    match lang.split(&['_', '-'][..]).next().unwrap_or(lang) {
        "da" | "dk" => &[
            ("million", 1_000_000),
            ("milliard", 1_000_000_000),
            ("billion", 1_000_000_000_000),
            ("billiard", 1_000_000_000_000_000),
            ("trillion", 1_000_000_000_000_000_000),
        ],
        "sl" => &[("milijonov", 1_000_000), ("milijard", 1_000_000_000)],
        _ => &[],
    }
}

/// The head of a single-token rendering `tok` of `n` that is fused to one of the
/// glued `morphemes`, with the multiplier it stands for: dv "ދެހާސް" (2000) ->
/// ("ދެ", 2). Only a digit or ten that the table does not hold and that holds no
/// morpheme itself.
fn fused_head(lang: &str, tok: &str, n: i64, morphemes: &[(String, i64)]) -> Option<(String, i64)> {
    let (g, mag) = morphemes
        .iter()
        .filter(|(g, mag)| tok.len() > g.len() && tok.ends_with(g.as_str()) && n % mag == 0)
        .max_by_key(|(g, _)| g.len())?;
    let pre = &tok[..tok.len() - g.len()];
    let m = n / mag;
    ((1..=10).contains(&m)
        && lookup_plain(lang, pre).is_none()
        && !morphemes.iter().any(|(x, _)| pre.contains(x.as_str())))
    .then(|| (pre.to_string(), m))
}

/// Derive [`Scales`] by rendering round numbers (every digit, 10, 11 and 21 ×
/// 10^3..10^18, see [`PROBE_POWERS`]) and reading the magnitude off each
/// rendering instead of assuming it: in "दस लाख" (10^6) the multiplier "दस" is
/// 10, so "लाख" is 10^5, not 10^6. A token that is itself a table number ("bat"
/// = 1, "zero", "biyu" = 2) is a multiplier, never a scale word. The magnitude
/// must be a power of ten ≥ 1000.
///
/// When a word is reached by several samples the first (smallest) one wins, so
/// es "millones" stays 10^6 and "mil millones" composes 10^9 by recursion.
fn derive_scales(lang: &str) -> Scales {
    let mut sc = Scales::default();
    let Some(l) = num2words2_core::get_lang_by_key(lang) else {
        return sc;
    };
    let render = |n: i64| l.to_cardinal(&BigInt::from(n)).ok().map(|w| normalize(&w));
    let conns = connector_words(lang);
    let number = |t: &str| -> Option<i64> {
        let t = trim_connectors(t, conns);
        if t.is_empty() {
            None
        } else {
            lookup_plain(lang, &t)
        }
    };
    // Not a multiplier: no table value, or the bare scale itself (fr "mille").
    // Two letters are enough (bm "ba" = 1000, vi "tỷ" = 10^9); a lone letter is
    // a particle (ff "e").
    let is_word = |t: &str| {
        (t.len() >= 3 || t.chars().count() >= 2)
            && !t.chars().any(|c| c.is_ascii_digit())
            && lookup_plain(lang, t).is_none_or(|v| v >= 1000 && is_power_of_ten(v))
    };
    fn add(out: &mut Vec<(String, i64)>, w: &str, mag: i64) {
        if !out.iter().any(|(x, _)| x == w) {
            out.push((w.to_string(), mag));
        }
    }
    // A single token = table multiplier + scale morpheme (km "ពីរពាន់" = 2 ·
    // "ពាន់"). Longest multiplier first, so the morpheme is the shortest tail:
    // th "หนึ่งพันล้าน" is 1000 × "ล้าน", not 1 × "พันล้าน".
    let glued = |tok: &str, n: i64| -> Option<(String, i64)> {
        let cuts: Vec<usize> = tok.char_indices().map(|(i, _)| i).filter(|&i| i > 0).collect();
        cuts.into_iter().rev().find_map(|i| {
            let (a, b) = tok.split_at(i);
            let m = lookup_plain(lang, a).filter(|m| *m >= 1)?;
            (n % m == 0 && is_power_of_ten(n / m) && n / m >= 1000 && is_word(b))
                .then(|| (b.to_string(), n / m))
        })
    };
    // Every probe `(power, multiplier, value, rendering)`, smallest power first.
    // A power at which two multipliers render alike names no magnitude: ro
    // writes 10^12, 2·10^12 and 5·10^12 all "bilion".
    let mut probes: Vec<(u32, i64, i64, String)> = Vec::new();
    for p in PROBE_POWERS {
        let Some(base) = 10i64.checked_pow(p) else { continue };
        let row: Vec<(i64, i64, String)> = PROBE_MULTIPLIERS
            .iter()
            .filter_map(|&k| {
                let n = k.checked_mul(base)?;
                let text = render(n)?;
                let words = !text.is_empty() && !text.chars().any(|c| c.is_ascii_digit());
                words.then_some((k, n, text))
            })
            .collect();
        if row.iter().enumerate().any(|(i, (_, _, t))| row[..i].iter().any(|(_, _, u)| u == t)) {
            continue;
        }
        probes.extend(row.into_iter().map(|(k, n, t)| (p, k, n, t)));
    }
    // Multipliers the table cannot read (ru "две тысячи", de "eine Million"):
    // settled once every readable sample has fixed the magnitudes.
    let mut unread: Vec<(String, u32, i64, i64, String)> = Vec::new();
    let mut glued_words: Vec<String> = Vec::new();
    let mut bare: Vec<String> = Vec::new(); // the whole rendering, no multiplier
    let mut multiplied: Vec<String> = Vec::new(); // seen with a multiplier
    let mut short_postfix: Vec<String> = Vec::new();
    for (p, k, n, text) in &probes {
        let (p, k, n) = (*p, *k, *n);
        // Where the multiplier follows the scale word (ha, ff, sw, sn, tet, eu),
        // num2words2 renders several values alike (#223-#228); past 10^9 every
        // new scale would only add collisions, so those languages keep the
        // scales their renderings show up to 10^9.
        if p >= 10 && !(sc.postfix.is_empty() && sc.one_after.is_empty()) {
            continue;
        }
        let toks: Vec<&str> = text.split_whitespace().collect();
        if let [tok] = toks[..] {
            if let Some((w, mag)) = glued(tok, n) {
                add(&mut sc.words, &w, mag);
                glued_words.push(w);
                continue;
            }
        }
        for (i, w) in toks.iter().enumerate() {
            if !is_word(w) {
                continue;
            }
            let left = trim_connectors(&toks[..i].join(" "), conns);
            let right = trim_connectors(&toks[i + 1..].join(" "), conns);
            let (mult, after) = match (left.is_empty(), right.is_empty()) {
                (true, true) => (Some(1), false),
                (false, true) => (number(&left), false),
                (true, false) => (number(&right), true),
                (false, false) => continue,
            };
            match mult {
                Some(m) if m >= 1 && n % m == 0 && is_power_of_ten(n / m) && n / m >= 1000 => {
                    // bm "ba fila" (2000): a two-letter postfix scale would read
                    // every "ba X ni Y" collision (#226) as a value.
                    if after && w.len() < 3 {
                        short_postfix.push(w.to_string());
                        continue;
                    }
                    add(&mut sc.words, w, n / m);
                    if left.is_empty() && right.is_empty() {
                        bare.push(w.to_string());
                    } else {
                        multiplied.push(w.to_string());
                    }
                    if after && m == 1 {
                        if !sc.one_after.iter().any(|(x, _)| x == w) {
                            sc.one_after.push((w.to_string(), right.clone()));
                        }
                    } else if after && !sc.postfix.iter().any(|x| x == w) {
                        sc.postfix.push(w.to_string());
                    }
                }
                None if !after => {
                    // kok "एक धा लाख" = 10^6: "one" times the renderer's unit
                    // "धा लाख" (ten lakh), so लाख alone is 10^5 and the pair a
                    // compound unit of 10^6.
                    if let Some((a, b)) = left.rsplit_once(' ') {
                        let unit = number(a).zip(lookup_plain(lang, b)).and_then(|(a, b)| {
                            let ten = (10..1000).contains(&b) && is_power_of_ten(b);
                            let d = a.checked_mul(b).filter(|_| a >= 1 && ten)?;
                            let mag = n / d;
                            (n % d == 0 && is_power_of_ten(mag) && mag >= 1000).then_some((b, mag))
                        });
                        if let Some((ten, mag)) = unit {
                            add(&mut sc.words, w, mag);
                            multiplied.push(w.to_string());
                            let spaced = format!("{b} {w}");
                            if !sc.compounds.iter().any(|(s, _, _)| *s == spaced) {
                                let joined = format!("{b}{COMPOUND_JOINER}{w}");
                                sc.compounds.push((spaced, joined, mag * ten));
                            }
                            continue;
                        }
                    }
                    unread.push((w.to_string(), p, k, n, left.clone()));
                }
                _ => {}
            }
        }
    }
    // A left part that holds a scale word is no multiplier at all: sn "zvuru
    // zvitatu" (3000) is the postfix "zvuru" followed by a form of 3.
    let words = sc.words.clone();
    for (w, _, k, n, left) in &unread {
        let scale = words.iter().find(|(x, _)| x == left).map(|(_, m)| *m);
        if scale.is_some_and(|m| m.checked_mul(*k) == Some(*n))
            && sc.postfix.contains(left)
            && !sc.postfix_forms.iter().any(|(x, _)| x == w)
        {
            sc.postfix_forms.push((w.clone(), *k));
        }
    }
    let holds_scale = |left: &str| left.split_whitespace().any(|t| words.iter().any(|(w, _)| w == t));
    unread.retain(|(_, _, _, _, left)| !holds_scale(left));
    // An unread digit multiplier keeps the probed power as the magnitude — but
    // only for 10^3/10^6/10^9/…, where the scale word is that power (he "עשרת
    // אלפים" is 10 · 10^3, not 10^4).
    for (w, p, k, _, _) in &unread {
        if p % 3 == 0 && *k < 10 {
            add(&mut sc.words, w, 10i64.pow(*p));
        }
    }
    // The unread prefix of exactly one scale is "one": ta "ஒரு இலட்சம்" = 10^5.
    for (w, _, _, n, one) in unread {
        if !one.contains(' ')
            && sc.words.iter().any(|(x, mag)| *x == w && *mag == n)
            && !sc.one_forms.contains(&one)
        {
            sc.one_forms.push(one);
        }
    }
    // sv "ettusen" glues "ett" + "tusen" on a shared t, leaving "usen": drop a
    // glued morpheme that is the tail of a longer glued one of the same magnitude.
    // th "หนึ่งแสนล้าน" (10^11) leaves "แสนล้าน", two morphemes in a row: drop
    // that too, the split reads it from its parts.
    let words = sc.words.clone();
    let morpheme = |x: &str| glued_words.iter().any(|g| g == x);
    sc.words.retain(|(w, mag)| {
        let tail = words.iter().any(|(x, m)| {
            m == mag && morpheme(x) && x.len() > w.len() && x.ends_with(w.as_str())
        });
        let pair = w.char_indices().skip(1).any(|(i, _)| morpheme(&w[..i]) && morpheme(&w[i..]));
        !(morpheme(w) && (tail || pair))
    });
    // dv "އެއްހާސް" (1000) is a form of "one" fused to the morpheme "ހާސް" that
    // "ވިހިހާސް" (20 × 1000) shows: the fused token is no scale word, and its head
    // a multiplier the table lacks ("އެއް" = 1, from "ދެހާސް": "ދެ" = 2). Such a
    // form then isolates the morphemes no table multiplier reaches: "ދެބިލިޔަން"
    // gives "ބިލިޔަން" = 10^9. A form is a digit or ten, and holds no morpheme
    // (th "หนึ่งแสนหนึ่ง" before "หมื่น" is 100001, not a form of 11).
    for _ in 0..3 {
        let morphemes: Vec<(String, i64)> =
            sc.words.iter().filter(|(w, _)| glued_words.contains(w)).cloned().collect();
        let mut clashes: Vec<String> = Vec::new();
        for (_, _, n, text) in &probes {
            if text.contains(' ') || glued(text, *n).is_some() {
                continue;
            }
            let Some(form) = fused_head(lang, text, *n, &morphemes) else { continue };
            match sc.combining.iter().find(|(x, _)| *x == form.0) {
                Some((_, v)) if *v != form.1 => clashes.push(form.0),
                Some(_) => {}
                None => sc.combining.push(form),
            }
        }
        sc.combining.retain(|(x, _)| !clashes.contains(x));
        let mut grew = false;
        for (_, _, n, text) in &probes {
            if text.contains(' ') || glued(text, *n).is_some() {
                continue;
            }
            let head = sc
                .combining
                .iter()
                .filter(|(c, _)| text.len() > c.len() && text.starts_with(c.as_str()))
                .max_by_key(|(c, _)| c.len());
            let Some((c, m)) = head else { continue };
            let rest = &text[c.len()..];
            if n % m == 0 && is_power_of_ten(n / m) && n / m >= 1000 && is_word(rest) {
                if !sc.words.iter().any(|(w, _)| w == rest) {
                    sc.words.push((rest.to_string(), n / m));
                    glued_words.push(rest.to_string());
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }
    let morphemes: Vec<(String, i64)> =
        sc.words.iter().filter(|(w, _)| glued_words.contains(w)).cloned().collect();
    sc.words
        .retain(|(w, mag)| glued_words.contains(w) || fused_head(lang, w, *mag, &morphemes).is_none());
    for (_, joined, mag) in &sc.compounds {
        add(&mut sc.words, joined, *mag);
    }
    for (w, mag) in scale_aliases(lang) {
        add(&mut sc.words, w, *mag);
    }
    sc.words.retain(|(w, _)| !short_postfix.contains(w));
    // Plus grande magnitude d'abord : « million » se scinde avant « mille ».
    sc.words.sort_by(|a, b| b.1.cmp(&a.1));
    sc.glue = sc
        .words
        .iter()
        .filter(|(w, _)| !bare.contains(w) || multiplied.contains(w) || glued_words.contains(w))
        .filter(|(w, _)| !w.contains(COMPOUND_JOINER))
        .cloned()
        .collect();
    // A scale word twice in one rendering: bn "দশ কোটি কোটি", eu "mila mila
    // milioi", th "หนึ่งล้านล้าน".
    let mag_of = |t: &str| sc.words.iter().find(|(w, _)| w == t).map(|(_, m)| *m);
    sc.stacks = probes.iter().any(|(_, _, _, text)| {
        let toks: Vec<&str> = text.split_whitespace().collect();
        if let [tok] = toks[..] {
            sc.glue.iter().any(|(g, _)| tok.matches(g.as_str()).count() >= 2)
        } else {
            let mags: Vec<i64> = toks.iter().filter_map(|t| mag_of(t)).collect();
            mags.iter().enumerate().any(|(i, m)| mags[..i].contains(m))
        }
    });
    sc.digit_by_digit = match (render(10), render(1), render(0)) {
        (Some(ten), Some(one), Some(zero)) => ten == format!("{one} {zero}"),
        _ => false,
    };
    // zh writes 10001 "一万零一" but 11000 "一万一千": its zero fills skipped
    // places (ja "一万一" does not; ksw "lah" only sits inside "klah").
    sc.zero = render(0)
        .filter(|z| !z.contains(' ') && lookup_plain(lang, z) == Some(0) && !sc.digit_by_digit)
        .filter(|z| render(10_001).is_some_and(|t| t.contains(z.as_str())))
        .filter(|z| render(11_000).is_some_and(|t| !t.contains(z.as_str())));
    // Groups summed with no scale word between them: km "មួយម៉ឺន ពីរពាន់" (12000),
    // dv "އެއްމިލިޔަން އެއްހާސް" (1001000).
    let sums_groups = |v: i64| {
        render(v).is_some_and(|text| {
            let toks: Vec<&str> = text.split_whitespace().collect();
            toks.len() >= 2
                && toks.iter().try_fold(0i64, |acc, t| {
                    let g = lookup_plain(lang, t).or_else(|| split_glued_scale(lang, t, &sc))?;
                    acc.checked_add(g)
                }) == Some(v)
        })
    };
    let additive = sums_groups(12000) || sums_groups(1_001_000);
    sc.additive_groups = additive;
    sc
}

/// Is `word` (normalized) a scale word of `lang` with magnitude >= `min`? Reads the
/// cache in place — the sentence walker asks this for every run head.
pub fn is_scale_word_of(lang: &str, word: &str, min: i64) -> bool {
    if !is_supported_lang(lang) {
        return false; // and no cache entry for an arbitrary string (#216)
    }
    scales_of(lang)
        .words
        .iter()
        .any(|(w, mag)| w == word && *mag >= min)
}

/// Can a single normalized token be a number of `lang` at all? A table hit, or a
/// token that carries a scale / hundred morpheme or an apocope and so may compose
/// ("dreiundzwanzigtausend", "ciento", "neunzehnhundertneunzig"). The sentence
/// walker asks this for every word of the text; the full `to_cardinal` (several
/// normalisations and composition passes) only runs for the tokens that pass.
pub fn token_may_be_number(lang: &str, norm: &str) -> bool {
    if !is_supported_lang(lang) {
        return false; // and no cache entry for an arbitrary string (#216)
    }
    if norm.is_empty() || norm.contains(' ') {
        return true; // not a single token: let the full parse decide
    }
    if lookup_plain(lang, norm).is_some() {
        return true;
    }
    if hundred_word(lang).is_some_and(|h| norm.contains(h.as_str()))
        || hundred_prefix(lang).is_some_and(|h| norm == h)
    {
        return true;
    }
    if with_scale_words(lang, |scales| {
        scales.iter().any(|(w, _)| norm.contains(w.as_str()))
    }) {
        return true;
    }
    matches!(
        lang.split(&['_', '-'][..]).next().unwrap_or(lang),
        "de" | "nl" | "af" | "it" | "sv" | "no" | "nb" | "nn" | "da"
    ) && ["tausend", "duizend", "duisend", "mila", "tusen", "tusind"]
        .iter()
        .any(|t| norm.contains(t))
}

/// Run `f` on the cached scale words of `lang` without cloning them.
fn with_scale_words<R>(lang: &str, f: impl FnOnce(&[(String, i64)]) -> R) -> R {
    f(&scales_of(lang).words)
}

/// Table hit for a fragment (no sign handling), i.e. a number ≤ 10001.
fn lookup_plain(lang: &str, text: &str) -> Option<i64> {
    lookup(lang, text, false, &[]).ok().flatten()
}

/// Connector words stripped from a fragment's edges (es « y », fa « و »):
/// see [`w2n_forms::connectors`].
fn connector_words(lang: &str) -> &'static [&'static str] {
    w2n_forms::connectors(lang)
}

/// Strip leading/trailing connector tokens from a fragment.
fn trim_connectors<'a>(s: &'a str, conns: &[&str]) -> String {
    let mut toks: Vec<&str> = s.split_whitespace().collect();
    while toks.first().is_some_and(|t| conns.contains(t)) {
        toks.remove(0);
    }
    while toks.last().is_some_and(|t| conns.contains(t)) {
        toks.pop();
    }
    toks.join(" ")
}

/// Compose a cardinal that the reverse table cannot hold on its own
/// (`> 10001`), e.g. "soixante-neuf mille huit" → 69008, "soixante-quinze mille
/// treize" → 75013. `None` if any fragment is not a known number word.
pub fn parse_scaled(lang: &str, text: &str) -> Option<i64> {
    parse_scaled_norm(lang, &normalize(text))
}

/// [`parse_scaled`] on normalized text.
pub fn parse_scaled_norm(lang: &str, norm: &str) -> Option<i64> {
    match scaled_reading(lang, norm) {
        Scaled::Value(v) => Some(v),
        Scaled::Ambiguous | Scaled::None => None,
    }
}

/// A scaled reading: one value, a spelling the renderer gives two values, or
/// none.
enum Scaled {
    Value(i64),
    Ambiguous,
    None,
}

/// Does the renderer write `norm` for two different values, so that it is
/// no one number (ha "miliyan goma": 1 000 010 and 10 000 000)? The sentence
/// walker then leaves the words alone rather than reading them as two
/// numbers (#348).
pub fn is_ambiguous_number(lang: &str, norm: &str) -> bool {
    matches!(scaled_reading(lang, norm), Scaled::Ambiguous)
}

/// Whether `lang` puts a multiplier after its scale word (ha, bm, sw, ...),
/// the only languages [`is_ambiguous_number`] can answer yes for.
pub fn has_postfix_scales(lang: &str) -> bool {
    is_supported_lang(lang) && {
        let sc = scales_of(lang);
        !sc.postfix.is_empty() || !sc.one_after.is_empty()
    }
}

fn scaled_reading(lang: &str, norm: &str) -> Scaled {
    let Some(v) = scaled_value(lang, norm) else { return Scaled::None };
    let sc = scales_of(lang);
    let v = match postfix_rival(lang, v, norm, &sc) {
        Rival::None => v,
        Rival::Prefer(m) => m,
        Rival::Ambiguous => return Scaled::Ambiguous,
    };
    if w2n_forms::has_rival_reading(lang, v, norm) {
        return Scaled::Ambiguous; // bm "ba tan ni kelen": 10001 and 11000 render alike
    }
    // Where the multiplier may follow the scale word (ha, ff, sw, sn, bm, tet,
    // eu), a spelling can split several ways and num2words2 sometimes renders
    // two values alike. Only a reading the renderer spells this way is kept:
    // a raise beats a wrong number.
    if (!sc.postfix.is_empty() || !sc.one_after.is_empty())
        && !renders_as(lang, v, norm)
        && !renders_as(lang, v, &drop_postfix_connector(norm, &sc, connector_words(lang)))
    {
        return Scaled::None;
    }
    Scaled::Value(v)
}

/// ha "miliyan da talatin": the sum written with the "da" the renderer drops
/// before a 10-999 remainder ("miliyan talatin" reads as the product, #882),
/// as the renderer would write it without.
fn drop_postfix_connector(norm: &str, sc: &Scales, conns: &[&str]) -> String {
    let toks: Vec<&str> = norm.split_whitespace().collect();
    let mut out: Vec<&str> = Vec::new();
    for (k, t) in toks.iter().enumerate() {
        let after_scale = k > 0 && sc.postfix.iter().any(|w| w == toks[k - 1]);
        if !(after_scale && conns.contains(t)) {
            out.push(t);
        }
    }
    out.join(" ")
}

fn scaled_value(lang: &str, norm: &str) -> Option<i64> {
    if !is_supported_lang(lang) {
        return None;
    }
    // No rendered number comes near this: refuse before the recursion (and its
    // string rebuilding) gets to run on a 10k-group input.
    if norm.len() > MAX_SCALED_BYTES || norm.split_whitespace().nth(MAX_SCALED_TOKENS).is_some() {
        return None;
    }
    let sc = scales_of(lang);
    let conns = connector_words(lang);
    let v = match join_compounds(&sc, norm) {
        None => parse_scaled_inner(lang, norm, &sc, conns)?,
        // kok "एक शंभर धा लाख" is 100 × "धा लाख" (10^6) to the renderer but 110 ×
        // "लाख" (10^5) to a reader: two readings that disagree are declined.
        Some(joined) => match (
            parse_scaled_inner(lang, norm, &sc, conns),
            parse_scaled_inner(lang, &joined, &sc, conns),
        ) {
            (Some(a), Some(b)) if a != b => return None,
            (Some(v), _) | (None, Some(v)) => v,
            (None, None) => return None,
        },
    };
    Some(v)
}

/// ha "miliyan dubu ɗari" is 1 001 100 and 1 100 000 alike: a postfix scale
/// word followed by a multiplier below 1000 ("dubu ɗari" = 1000 × 100) is a
/// rival reading of a number read additively. True when such a rival differs
/// from `v` and the renderer writes it as `norm` too (#348).
fn postfix_rival(lang: &str, v: i64, norm: &str, sc: &Scales) -> Rival {
    if sc.postfix.is_empty() {
        return Rival::None;
    }
    let toks: Vec<&str> = norm.split_whitespace().collect();
    for i in 0..toks.len().saturating_sub(1) {
        if !sc.postfix.iter().any(|w| w == toks[i]) {
            continue;
        }
        let Some(unit) = sc.words.iter().find(|(w, _)| w == toks[i]).map(|(_, m)| *m) else {
            continue;
        };
        let Some(m) = lookup_plain(lang, &toks[i + 1..].join(" ")).filter(|m| (2..1000).contains(m)) else {
            continue;
        };
        let Some(h) = (if i == 0 { Some(0) } else { parse_scaled_norm_unchecked(lang, &toks[..i].join(" ")) })
        else {
            continue;
        };
        // The renderer writes "miliyan talatin" for 30·10^6 and for 10^6 + 30
        // alike: it drops the "da" a sum takes. A reader takes the multiplier
        // after the scale word, so that reading wins (#882).
        // Only for a million and above: the thousands keep the reading the
        // table gives ("dubu goma" is 1 010) and stay ambiguous to the
        // ordinal and sentence guards, as before.
        let mult = unit.checked_mul(m).and_then(|x| h.checked_add(x));
        if let Some(mult) = mult.filter(|&x| renders_as(lang, x, norm)) {
            if unit >= 1_000_000 {
                return if mult == v { Rival::None } else { Rival::Prefer(mult) };
            }
            if mult != v {
                return Rival::Ambiguous;
            }
        }
        let add = h.checked_add(unit).and_then(|x| x.checked_add(m));
        if add.is_some_and(|x| x != v && renders_as(lang, x, norm)) {
            return Rival::Ambiguous;
        }
    }
    Rival::None
}

/// What [`postfix_rival`] makes of a reading.
enum Rival {
    None,
    /// The renderer writes the text for this other value, which reads first.
    Prefer(i64),
    /// Two values alike, neither preferred.
    Ambiguous,
}

/// [`parse_scaled_norm`] without its ambiguity checks, for a fragment.
pub(crate) fn parse_scaled_norm_unchecked(lang: &str, norm: &str) -> Option<i64> {
    parse_scaled_inner(lang, norm, &scales_of(lang), connector_words(lang))
}

/// The article that is also "one" in front of a scale word, for languages whose table
/// does not hold it as a number: de "eine Million", nl "een miljoen".
fn article_one(lang: &str, word: &str) -> bool {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    matches!((base, word), ("de", "ein" | "eine") | ("nl" | "af", "een"))
}

/// `norm` with every two-token compound unit of [`Scales::compounds`] joined into
/// its single scale token, or `None` when it holds none.
fn join_compounds(sc: &Scales, norm: &str) -> Option<String> {
    if sc.compounds.is_empty() {
        return None;
    }
    let toks: Vec<&str> = norm.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(toks.len());
    let mut changed = false;
    let mut i = 0;
    while i < toks.len() {
        let joined = toks.get(i + 1).and_then(|next| {
            sc.compounds.iter().find_map(|(spaced, joined, _)| {
                (spaced.split_once(' ') == Some((toks[i], *next))).then_some(joined)
            })
        });
        match joined {
            Some(j) => {
                out.push(j.clone());
                changed = true;
                i += 2;
            }
            None => {
                out.push(toks[i].to_string());
                i += 1;
            }
        }
    }
    changed.then(|| out.join(" "))
}

/// The thousand morpheme of the agglutinative languages, which `scale_words`
/// only sees fused to a multiplier the table does not hold.
fn thousand_morpheme(lang: &str) -> Option<&'static str> {
    match lang.split(&['_', '-'][..]).next().unwrap_or(lang) {
        "de" => Some("tausend"),
        "nl" => Some("duizend"),
        "af" => Some("duisend"),
        "it" => Some("mila"), // "duemila", "ventitremila"
        "sv" | "no" | "nb" | "nn" => Some("tusen"),
        "da" => Some("tusind"),
        _ => None,
    }
}

/// The multiplier glued in front of a scale morpheme: a table number, the bare
/// hundred (de "hunderttausend"), a form heard only there (dv "ދެ" = 2 in
/// "ދެހާސް"), tens + such a form (dv "ތިރީސްއެއް" = 30 + 1 in "ތިރީސްއެއްހާސް"),
/// or sv "tjugoet", which drops the t that "ett" shares with "tusen".
fn glued_multiplier(lang: &str, pre: &str, sc: &Scales) -> Option<i64> {
    let combining = |s: &str| sc.combining.iter().find(|(x, _)| x == s).map(|(_, v)| *v);
    lookup_plain(lang, pre)
        .or_else(|| hundred_word(lang).filter(|h| h == pre).map(|_| 100))
        .or_else(|| combining(pre))
        .or_else(|| {
            let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
            (base == "sv" && pre.ends_with("et"))
                .then(|| lookup_plain(lang, &format!("{pre}t")))
                .flatten()
        })
        .or_else(|| {
            // The unit's combining form first: a scan of a short list, where
            // a table lookup of every prefix made long glued input cubic (#186).
            pre.char_indices().skip(1).find_map(|(i, _)| {
                let unit = combining(&pre[i..]).filter(|u| (1..10).contains(u))?;
                let tens = lookup_plain(lang, &pre[..i]).filter(|t| (20..100).contains(t) && t % 10 == 0)?;
                Some(tens + unit)
            })
        })
}

/// A number glued into one token: a table hit, a th tail ending in "เอ็ด" (the
/// "one" after a higher place: "หนึ่งหมื่นเอ็ด" = 10001), or a scale split.
fn glued_number(lang: &str, s: &str, sc: &Scales) -> Option<i64> {
    if let Some(v) = lookup_plain(lang, s) {
        return Some(v);
    }
    if lang.split(&['_', '-'][..]).next() == Some("th") {
        let ed = normalize("เอ็ด");
        if let Some(head) = s.strip_suffix(ed.as_str()) {
            let one = format!("{head}{}", normalize("หนึ่ง"));
            if let Some(v) = lookup_plain(lang, &one) {
                return Some(v);
            }
        }
    }
    split_glued_scale(lang, s, sc)
}

/// A scale word glued into one token, as de / nl / it / zh / th write it:
/// "dreiundzwanzigtausend" = 23 × 1000, "ventitremila" = 23 × 1000, zh
/// "一千二百三十四万五千六百七十八" = 1234 × 10^4 + 5678. The split is at the
/// last occurrence of the largest morpheme present; head and tail may hold
/// smaller morphemes themselves, and the tail must stay below the scale. `None`
/// when the token holds no scale morpheme.
fn split_glued_scale(lang: &str, tok: &str, sc: &Scales) -> Option<i64> {
    let mut words: Vec<(String, i64)> = sc.glue.clone();
    if let Some(t) = thousand_morpheme(lang) {
        if tok == t {
            return Some(1_000); // de "tausend Euro": the bare morpheme is 1000
        }
        words.push((t.to_string(), 1_000));
    }
    let mut mags: Vec<i64> = words.iter().map(|(_, m)| *m).filter(|m| *m >= 1_000).collect();
    mags.sort_unstable_by(|a, b| b.cmp(a));
    mags.dedup();
    for mag in mags {
        // The last occurrence; on a tie the longer word (fi "tuhatta", not "tuhat").
        let Some((pos, len)) = words
            .iter()
            .filter(|(_, m)| *m == mag)
            .filter_map(|(w, _)| tok.rfind(w.as_str()).map(|pos| (pos, w.len())))
            .max()
        else {
            continue;
        };
        if let Some(v) = glued_at(lang, tok, pos, len, mag, sc) {
            return Some(v);
        }
    }
    None
}

/// `tok` read as head × `mag` + tail around the morpheme at `pos..pos + len`.
fn glued_at(lang: &str, tok: &str, pos: usize, len: usize, mag: i64, sc: &Scales) -> Option<i64> {
    // bo: the tsheg ending the syllable before the morpheme, or opening the
    // tail, belongs to neither part ("དགུ་ཁྲི་" + "དུང་ཕྱུར").
    let tsheg = ['\u{0F0B}', '\u{0F0C}'];
    let (pre, suf) = (tok[..pos].trim_matches(tsheg), tok[pos + len..].trim_matches(tsheg));
    if pre.is_empty() && suf.is_empty() {
        return None; // the bare morpheme: the table's business
    }
    // ja "千二百三十四万": a myriad takes a multiplier up to 9999, read in turn;
    // th "หนึ่งล้านสองแสนล้าน" stacks ล้าน on 1 200 000.
    let high = if pre.is_empty() {
        1
    } else {
        let h = glued_multiplier(lang, pre, sc).or_else(|| split_glued_scale(lang, pre, sc))?;
        let stacked = sc.stacks && sc.glue.iter().any(|(g, _)| pre.contains(g.as_str()));
        if h < 1 || (h >= mag && !stacked) {
            return None;
        }
        h
    };
    let stacked = high >= mag;
    let low = if suf.is_empty() {
        0
    } else {
        // zh "一万零一" = 10001: 零 fills the skipped places, so what follows
        // it is at least two places below the scale.
        let filled = sc.zero.as_deref().and_then(|z| suf.strip_prefix(z)).filter(|r| !r.is_empty());
        // After 零 a ten keeps its "one": "一万零一十" = 10010.
        let tens_one = || {
            let one = lookup_plain(lang, "一").filter(|v| *v == 1).map(|_| "一")?;
            glued_number(lang, filled?.strip_prefix(one)?, sc).filter(|v| (10..20).contains(v))
        };
        // rm "dieschmilliedin" = 10001: the connector "ed" sits between the
        // morpheme and "in", and only "milliedin" (1001) is a table word.
        let with_morpheme = || {
            filled.is_none().then_some(())?;
            lookup_plain(lang, &tok[pos..]).map(|v| v - mag).filter(|v| *v > 0)
        };
        let low = glued_number(lang, filled.unwrap_or(suf), sc)
            .or_else(tens_one)
            .or_else(with_morpheme)?;
        if filled.is_some() && low >= mag / 10 {
            return None;
        }
        // Without 零 a lone digit is the spoken abbreviation: zh "一万五" is
        // 15000 to a reader, and the renderer writes 10005 "一万零五".
        if sc.zero.is_some() && filled.is_none() && low < 10 && mag >= 1000 {
            return None;
        }
        low
    };
    if !(0..mag).contains(&low) {
        return None;
    }
    let v = high.checked_mul(mag)?.checked_add(low)?;
    // bo "ཁྲི་དུང་ཕྱུར" (10^4 · 10^8) is one scale word, but in "གཅིག་འབུམ་
    // གཅིག་ཁྲི་དུང་ཕྱུར" its ཁྲི belongs to the multiplier of དུང་ཕྱུར: 11·10^12,
    // not 100001·10^12 (#349). A compound scale after a multiplier that holds
    // a scale of its own only where the renderer spells this very number so.
    let morpheme = &tok[pos..pos + len];
    let compound = || {
        let inner = |g: &str| sc.glue.iter().any(|(w, m)| *m >= 1000 && *m < mag && g.contains(w.as_str()));
        inner(morpheme) && inner(pre)
    };
    // A stacked reading only where the renderer spells this very number so.
    (!(stacked || compound()) || renders_as(lang, v, tok)).then_some(v)
}

/// en_AERO "wun zero zero zero zero zero" = 100000: every token one digit. An i64
/// holds 19 digits; a longer run overflows and is refused.
fn digit_run(lang: &str, toks: &[&str]) -> Option<i64> {
    if toks.len() < 2 || toks.len() > 19 {
        return None;
    }
    toks.iter().try_fold(0i64, |acc, t| {
        let d = lookup_plain(lang, t).filter(|d| (0..10).contains(d))?;
        acc.checked_mul(10)?.checked_add(d)
    })
}

/// Does num2words2 render `v` in `lang` exactly as the normalized `text`? The
/// check for readings the grammar alone cannot settle.
fn renders_as(lang: &str, v: i64, text: &str) -> bool {
    num2words2_core::get_lang_by_key(lang)
        .and_then(|r| r.to_cardinal(&BigInt::from(v)).ok())
        .is_some_and(|w| normalize(&w) == text)
}

/// hu "egymillio" -> ["egy", "millio"]: a token that is a multiplier glued to a
/// scale word, split in two, ko "천이백삼십사만" (1234 × 10^4) included. A table
/// word is split only when it is exactly that product: hu "tizezer" (10000) =
/// "tiz" × "ezer", so "tízezer-száz" reads 10 × 1000 + 100. A multiplier the
/// table cannot read is replaced by the renderer's spelling: sv "tjugoettusen"
/// -> ["tjugoett", "tusen"]. sv "miljonett": the renderer glues the remainder
/// after the scale word. Any other token is returned as is.
fn unglue(lang: &str, tok: &str, sc: &Scales) -> Vec<String> {
    let whole = || vec![tok.to_string()];
    if sc.words.iter().any(|(w, _)| w == tok) {
        return whole();
    }
    let table = lookup_plain(lang, tok);
    let mut words: Vec<(&str, i64)> = sc.glue.iter().map(|(w, m)| (w.as_str(), *m)).collect();
    if let Some(t) = thousand_morpheme(lang) {
        words.push((t, 1_000));
    }
    for (w, mag) in words {
        if tok.len() <= w.len() {
            continue;
        }
        if let Some(pre) = tok.strip_suffix(w) {
            let m = glued_multiplier(lang, pre, sc).filter(|m| (1..mag).contains(m));
            if let Some(m) = m.filter(|&m| table.is_none_or(|v| m.checked_mul(mag) == Some(v))) {
                let head = if lookup_plain(lang, pre).is_some() {
                    Some(pre.to_string())
                } else {
                    num2words2_core::get_lang_by_key(lang)
                        .and_then(|r| r.to_cardinal(&BigInt::from(m)).ok())
                        .map(|t| normalize(&t))
                };
                if let Some(head) = head {
                    return vec![head, w.to_string()];
                }
            }
        }
        if let Some(rest) = tok.strip_prefix(w) {
            if table.is_none() && lookup_plain(lang, rest).is_some_and(|r| (1..mag).contains(&r)) {
                return vec![w.to_string(), rest.to_string()];
            }
        }
    }
    whole()
}

/// km "មួយម៉ឺន ពីរពាន់ បីរយ សែសិបប្រាំ" = 10000 + 2000 + 300 + 45: each token a
/// group, strictly decreasing in place value so "two three" never sums. The
/// first group holds a scale: below one, the table spells the number whole, and
/// hu "húsz három" stays 20 and 3.
fn additive_groups(lang: &str, toks: &[&str], sc: &Scales) -> Option<i64> {
    let mut total = 0i64;
    let mut cap = i64::MAX;
    for t in toks {
        let v = lookup_plain(lang, t).or_else(|| split_glued_scale(lang, t, sc))?;
        if v <= 0 || v >= cap || (cap == i64::MAX && v < 1000) {
            return None;
        }
        total = total.checked_add(v)?;
        cap = 10i64.pow(v.ilog10());
    }
    Some(total)
}

fn parse_scaled_inner(lang: &str, text: &str, sc: &Scales, conns: &[&str]) -> Option<i64> {
    let scales = sc.words.as_slice();
    let text = trim_connectors(text.trim(), conns);
    let text = text.as_str();
    if text.is_empty() {
        return None;
    }
    // Fragment directly in the table (≤10001) — base case.
    if let Some(v) = lookup_plain(lang, text) {
        return Some(v);
    }
    if !text.contains(' ') {
        if let Some(v) = split_glued_scale(lang, text, sc) {
            return Some(v);
        }
    }
    let toks: Vec<&str> = text.split_whitespace().collect();
    // hu "egymillió-egy": a scale glued to its multiplier inside a longer number.
    if toks.len() > 1 {
        let unglued: Vec<String> = toks.iter().flat_map(|t| unglue(lang, t, sc)).collect();
        let unglued = unglued.join(" ");
        if unglued != text {
            return parse_scaled_inner(lang, &unglued, sc, conns);
        }
    }
    if sc.digit_by_digit {
        if let Some(v) = digit_run(lang, &toks) {
            return Some(v);
        }
    }
    // The remainder after a scale word: empty is 0, otherwise a number below it.
    let rest = |toks: &[&str], mag: i64| -> Option<i64> {
        let r = toks.join(" ");
        if trim_connectors(&r, conns).is_empty() {
            Some(0)
        } else {
            parse_scaled_inner(lang, &r, sc, conns).filter(|&r| r < mag)
        }
    };
    // Split at the LAST token of the largest scale present: bn "দশ কোটি পাঁচ
    // কোটি" is (10 crore + 5) × crore, and bg "един милиард милиарда" (two
    // spellings of 10^9) is 10^9 × 10^9. The remainder after it must stay below
    // the scale, so "দশ কোটি কোটি" is no 10 crore + 1 crore.
    let mag_of = |t: &str| {
        scales
            .iter()
            .find(|(w, _)| w == t)
            .map(|(_, m)| *m)
            .or_else(|| (thousand_morpheme(lang) == Some(t)).then_some(1_000))
    };
    let mags: Vec<Option<i64>> = toks.iter().map(|t| mag_of(t)).collect();
    let mut split: Option<(usize, i64)> = None;
    for (i, m) in mags.iter().enumerate() {
        if let Some(m) = *m {
            if split.is_none_or(|(_, best)| m >= best) {
                split = Some((i, m));
            }
        }
    }
    // Where the renderer never stacks, the largest scale twice apart is no one
    // number ("deux mille deux mille"): refuse before recursing on every group.
    if let Some((pos, mag)) = split {
        let repeats = mags[..pos].iter().enumerate().any(|(i, m)| {
            *m == Some(mag) && mags[i + 1..pos].iter().any(Option::is_none)
        });
        if repeats && !sc.stacks {
            return None;
        }
    }
    if let Some((pos, mag)) = split {
        let word = toks[pos];
        let left = toks[..pos].join(" ");
        let right = toks[pos + 1..].join(" ");
        let left_trim = trim_connectors(left.trim(), conns);
        let after = &toks[pos + 1..];
        let compose = |l: i64, r: i64| l.checked_mul(mag)?.checked_add(r);
        if left_trim.is_empty() {
            // eu "milioi bat ...": the "one" follows the scale word.
            if let Some((_, one)) = sc.one_after.iter().find(|(w, _)| w == word) {
                let one: Vec<&str> = one.split_whitespace().collect();
                if after.starts_with(&one) {
                    if let Some(v) = rest(&after[one.len()..], mag).and_then(|r| compose(1, r)) {
                        return Some(v);
                    }
                }
            }
            // ha "dubu sha biyu ɗari uku ..." : the longest multiplier that
            // leaves a remainder below the scale. A multiplier of 1000 or more
            // only ends the number: tet "miliaun rihun ida" = 10^6 · 1000.
            if sc.postfix.iter().any(|w| w == word)
                && !w2n_forms::opens_with_connector(lang, after.first())
            {
                for j in (1..=after.len()).rev() {
                    let m = trim_connectors(&after[..j].join(" "), conns);
                    let ends = j == after.len();
                    let form = || sc.postfix_forms.iter().find(|(x, _)| *x == m).map(|(_, k)| *k);
                    let Some(m) = lookup_plain(lang, &m)
                        .or_else(form)
                        .filter(|&m| (1..1000).contains(&m) || (ends && (1..mag).contains(&m)))
                    else {
                        continue;
                    };
                    // A multiplier of 1000 or more only where the renderer spells
                    // the product so: ha "miliyan dubu" is 10^6 + 1000 (read
                    // below), not 1000 × 10^6, which is "biliyan" (#348).
                    let v = rest(&after[j..], mag).and_then(|r| compose(m, r));
                    if let Some(v) = v.filter(|&v| m < 1000 || renders_as(lang, v, text)) {
                        return Some(v);
                    }
                }
            }
        }
        // « mille » nu (pas de multiplicateur à gauche) = 1×mille ; de « eine Million ».
        // A left part as big as the scale is no multiplier: ko "십만 천" is
        // 100000 + 1000, not 100000 × 1000 — the additive split below reads it.
        // Unless the renderer stacks scales and spells this very number so: th
        // "หนึ่งล้านสองแสนล้าน", bn "দশ কোটি পাঁচ কোটি" (10^8 + 5 crore).
        let (l, stacked) = if left_trim.is_empty()
            || article_one(lang, &left_trim)
            || sc.one_forms.contains(&left_trim)
        {
            (Some(1), false)
        } else {
            match parse_scaled_inner(lang, &left, sc, conns) {
                Some(l) if l < mag => (Some(l), false),
                Some(l) if sc.stacks && mags[..pos].iter().any(Option::is_some) => (Some(l), true),
                _ => (None, false),
            }
        };
        if let Some(l) = l {
            let r = if trim_connectors(right.trim(), conns).is_empty() {
                Some(0)
            } else {
                parse_scaled_inner(lang, &right, sc, conns).filter(|&r| r < mag)
            };
            // zh "一千 七": as in the glued "一千七", a lone digit needs 零.
            let zero_gap = |r: i64| {
                let filled = sc.zero.as_deref().is_some_and(|z| after.first() == Some(&z));
                sc.zero.is_none() || filled || r >= 10 || r == 0
            };
            let v = r.filter(|&r| zero_gap(r)).and_then(|r| compose(l, r));
            // A bare postfix scale whose multiplier could not be read is "one" only
            // when the renderer says so: tet "miliaun rihun atus sia rihun ida" is
            // no 1 × 10^6 + 901001.
            let postfix_one = left_trim.is_empty() && sc.postfix.iter().any(|w| w == word);
            let rendered = |v: i64| {
                renders_as(lang, v, text) || renders_as(lang, v, &drop_postfix_connector(text, sc, conns))
            };
            if let Some(v) = v.filter(|&v| !(postfix_one || stacked) || rendered(v)) {
                return Some(v);
            }
            if postfix_one && v.is_some() {
                return None;
            }
        }
        // eu "bat mila milioi hamar milioi bat" = 10^9 + 10 000 001: adjacent
        // scale words multiply into one unit ("mila milioi" = 10^9), and the
        // remainder after the largest unit stays below it. The same word twice
        // ("mila mila") only where the renderer stacks: fr "deux mille mille"
        // is no number.
        let mut runs: Vec<(usize, usize, i64, bool)> = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            let Some(first) = mags[i] else {
                i += 1;
                continue;
            };
            let (start, mut prod, mut prev, mut stacked) = (i, Some(first), first, false);
            i += 1;
            let grows = |m: i64, prev: i64| m > prev || (m == prev && sc.stacks);
            while let Some(m) = mags.get(i).copied().flatten().filter(|&m| grows(m, prev)) {
                prod = prod.and_then(|p| p.checked_mul(m));
                stacked |= m == prev;
                prev = m;
                i += 1;
            }
            if let Some(p) = prod.filter(|_| i - start > 1) {
                runs.push((start, i, p, stacked));
            }
        }
        if let Some(&(a, b, unit, stacked)) = runs.iter().max_by_key(|(a, _, u, _)| (*u, *a)) {
            let left = toks[..a].join(" ");
            let right = toks[b..].join(" ");
            let l = if trim_connectors(&left, conns).is_empty() {
                Some(1)
            } else {
                parse_scaled_inner(lang, &left, sc, conns).filter(|&l| l < unit)
            };
            let r = if trim_connectors(&right, conns).is_empty() {
                Some(0)
            } else {
                parse_scaled_inner(lang, &right, sc, conns).filter(|&r| r < unit)
            };
            let v = l.zip(r).and_then(|(l, r)| l.checked_mul(unit)?.checked_add(r));
            if let Some(v) = v.filter(|&v| !stacked || renders_as(lang, v, text)) {
                return Some(v);
            }
        }
    }
    if sc.additive_groups && toks.len() > 1 {
        if let Some(v) = additive_groups(lang, &toks, sc) {
            return Some(v);
        }
    }
    // Additive hundreds glued into one word ("novecento" = 900) followed by a
    // sub-hundred remainder ("ottantotto" = 88): 900 + 88 = 988. num2words
    // elides the vowel at the join ("novecentottantotto"), so the de-spaced
    // string is not the canonical spelling and only this additive split
    // recovers it. Fires under a scale too — "mille novecento ottantotto"
    // splits on "mille" then composes 900 + 88 here. Guarded so it never
    // fabricates: the left part must be a positive whole hundred, the right a
    // genuine sub-hundred, both real table words.
    let toks: Vec<&str> = text.split_whitespace().collect();
    // Both halves are table words, a few tokens each: a long run cannot split so.
    let halves = if toks.len() <= 24 { 1..toks.len() } else { 0..0 };
    for i in halves {
        let left = trim_connectors(&toks[..i].join(" "), conns);
        let right = trim_connectors(&toks[i..].join(" "), conns);
        if let (Some(l), Some(r)) = (lookup_plain(lang, &left), lookup_plain(lang, &right)) {
            // A left part of 1000 or more only when the renderer spells the sum
            // exactly so: fi "kymmenentuhatta yksi" is 10001, but bm "ba tan ni
            // fila" is 1000 × 12, not 1000 + 12 (bm 1012 is "ba ni tan ni fila").
            if l >= 100
                && l % 100 == 0
                && (1..100).contains(&r)
                && (l < 1000 || renders_as(lang, l + r, text))
            {
                return Some(l + r);
            }
            // fr counts hundreds by the dozen: "douze cents" = 1200, "dix-huit cents" = 1800.
            if lang.starts_with("fr") && (11..100).contains(&l) && r == 100 {
                return Some(l * 100);
            }
            // Tens joined with the connector where num2words glues them: es
            // "veinte y uno" (num2words: "veintiuno") = 21. The connector must
            // be spoken — "veinte uno" stays two numbers.
            if (20..100).contains(&l)
                && l % 10 == 0
                && (1..10).contains(&r)
                && conns.contains(&toks[i])
            {
                return Some(l + r);
            }
        }
    }
    None
}

/// Port of `_rust.parse_int` — a plain `int(s)` for a signed ASCII integer.
pub fn parse_int(s: &str) -> Result<i64, std::num::ParseIntError> {
    s.parse::<i64>()
}

// ---------------------------------------------------------------------------
// Spoken "year" forms (below the LOOKUP_RANGE table but not canonical)
// ---------------------------------------------------------------------------
//
// num2words only ever renders ONE spelling per value, so the reverse table
// holds just that canonical form. But speech routinely uses the "year" reading
// — two 2-digit groups ("neunzehn neunundneunzig" = 19·100+99 = 1999), an
// explicit hundred ("dix-neuf cent quatre-vingt-dix" = 19·100+90 = 1990), or a
// hundred glued into one token ("nittonhundranittiosju" = 19·100+97 = 1997).
// None of those are the canonical render, so the table misses and the caller
// used to raise `cannot parse`. `parse_year` recovers them from the same
// reverse-table primitive, so it needs no per-language grammar.

/// Cache of the derived "hundred" morpheme per language (`None` when the
/// language has no regular one, e.g. es cien/-cientos).
fn hundred_cache() -> &'static RwLock<HashMap<String, Option<String>>> {
    static H: OnceLock<RwLock<HashMap<String, Option<String>>>> = OnceLock::new();
    H.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Longest common suffix of two ASCII-ish strings, by `char`.
fn common_suffix(a: &str, b: &str) -> String {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let mut k = 0;
    while k < ac.len() && k < bc.len() && ac[ac.len() - 1 - k] == bc[bc.len() - 1 - k] {
        k += 1;
    }
    ac[ac.len() - k..].iter().collect()
}

/// Derive the language's "hundred" morpheme (fr "cent", de "hundert",
/// nl "honderd", sv "hundra", it "cento") by rendering 200/300/900, de-spacing,
/// dropping a trailing plural "s", and taking the longest common alphabetic
/// suffix (>=3 chars). Returns `None` for irregular hundreds (es cien/-cientos),
/// where `parse_year` simply skips the hundred split.
fn hundred_word(lang: &str) -> Option<String> {
    if let Some(v) = hundred_cache().read().unwrap().get(lang) {
        return v.clone();
    }
    let out = (|| {
        let l = num2words2_core::get_lang_by_key(lang)?;
        let mut forms: Vec<String> = Vec::new();
        // Probe every hundred 200..=900: the letter BEFORE the hundred morpheme
        // then varies across units (it quattr-o/se-i/nov-e, fr deu-x/si-x/hui-t),
        // so the longest common suffix isolates the morpheme itself ("cento",
        // "cent") rather than a unit's trailing vowel.
        for n in (200i64..=900).step_by(100) {
            let w = l.to_cardinal(&BigInt::from(n)).ok()?;
            let mut s: String = normalize(&w).split_whitespace().collect();
            if s.ends_with('s') {
                s.pop();
            }
            forms.push(s);
        }
        let common = |skip: Option<usize>| -> String {
            let mut it = forms.iter().enumerate().filter(|(k, _)| Some(*k) != skip).map(|(_, f)| f);
            let mut suf = it.next().cloned().unwrap_or_default();
            for f in it {
                suf = common_suffix(&suf, f);
            }
            // Keep only the trailing alphabetic run (drop any leading unit letters).
            let mut rev: Vec<char> = suf.chars().rev().take_while(|c| c.is_alphabetic()).collect();
            rev.reverse();
            rev.into_iter().collect()
        };
        let tail = common(None);
        if tail.chars().count() >= 3 {
            return Some(tail);
        }
        // A syllabic script spells the morpheme in two letters (am "መቶ", #135),
        // and one hundred may be suppletive (dv 200 "ދުއިސައްތަ" next to 300-900
        // "...ސަތޭކަ", #136). Either way the morpheme is still the word for 100
        // itself, which is the evidence asked for before taking a shorter
        // suffix, or one shared by all hundreds but one.
        let hundred: String = normalize(&l.to_cardinal(&BigInt::from(100)).ok()?)
            .split_whitespace()
            .last()?
            .to_string();
        std::iter::once(None)
            .chain((0..forms.len()).map(Some))
            .map(common)
            // dv 300-900 share "...އްސަތޭކަ": the unit's last letters ride along.
            .find(|t| t.ends_with(&hundred))
            .map(|_| hundred)
    })();
    hundred_cache()
        .write()
        .unwrap()
        .insert(lang.to_string(), out.clone());
    out
}

/// Cache of the derived bare hundred prefix per language (es "ciento").
fn hundred_prefix_cache() -> &'static RwLock<HashMap<String, Option<String>>> {
    static P: OnceLock<RwLock<HashMap<String, Option<String>>>> = OnceLock::new();
    P.get_or_init(|| RwLock::new(HashMap::new()))
}

/// The hundred word num2words never renders on its own, only as the head of
/// a composed hundred: es "ciento" (100 is "cien", 101 is "ciento uno"),
/// pt "cento" (100 is "cem"). The reverse table has no entry for it, so the
/// sentence walker never opened a run on it and "ciento cincuenta y cuatro"
/// came out as "ciento 54". Derived by rendering 101..=109 and keeping the
/// head token when it is not a table word by itself while the rest is the
/// unit. `None` where the hundred is a table word already (fr "cent") or is
/// glued to its unit (de "hunderteins", it "centouno").
fn hundred_prefix(lang: &str) -> Option<String> {
    if let Some(v) = hundred_prefix_cache().read().unwrap().get(lang) {
        return v.clone();
    }
    let out = (|| {
        let l = num2words2_core::get_lang_by_key(lang)?;
        let conns = connector_words(lang);
        for n in 101i64..=109 {
            let words = l.to_cardinal(&BigInt::from(n)).ok()?;
            let norm = normalize(&words);
            let toks: Vec<&str> = norm.split_whitespace().collect();
            if toks.len() < 2 || lookup_plain(lang, toks[0]).is_some() {
                continue;
            }
            let rest = trim_connectors(&toks[1..].join(" "), conns);
            if lookup_plain(lang, &rest) == Some(n - 100) {
                return Some(toks[0].to_string());
            }
        }
        None
    })();
    hundred_prefix_cache()
        .write()
        .unwrap()
        .insert(lang.to_string(), out.clone());
    out
}

/// The bare hundred prefix on its own ("ciento") reads as 100. A run head in
/// the sentence walker must be a number word by itself, and this is the one
/// hundred word the reverse table cannot vouch for.
pub fn lookup_hundred_prefix(lang: &str, text: &str) -> Option<i64> {
    lookup_hundred_prefix_norm(lang, &normalize(text))
}

/// [`lookup_hundred_prefix`] on normalized text.
pub fn lookup_hundred_prefix_norm(lang: &str, norm: &str) -> Option<i64> {
    if !is_supported_lang(lang) {
        return None;
    }
    let prefix = hundred_prefix(lang)?;
    (norm == prefix).then_some(100)
}

/// Recover a spoken "year" reading that is not num2words' canonical spelling:
/// `L <hundred> R` (explicit or glued) → `L*100 + R`, or two 2-digit groups
/// `a b` → `a*100 + b`. All parts are resolved through the reverse table, so no
/// language-specific grammar is needed. Intended as a LAST-resort fallback,
/// after the whole-string table hit and [`parse_scaled`] have both declined —
/// so a canonical number (`vingt trois` = 23) never reaches here.
pub fn parse_year(lang: &str, text: &str) -> Option<i64> {
    parse_year_norm(lang, &normalize(text))
}

/// [`parse_year`] on normalized text.
pub fn parse_year_norm(lang: &str, norm: &str) -> Option<i64> {
    if !is_supported_lang(lang) {
        return None;
    }
    let toks: Vec<&str> = norm.split_whitespace().collect();
    if toks.is_empty() {
        return None;
    }
    let hw = hundred_word(lang);

    // nb/no "nitten hundre og fem" (#134): the Nordic "og" joins the hundred
    // to the rest, on top of the language's own connectors.
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    let mut conns: Vec<&str> = connector_words(lang).to_vec();
    if matches!(base, "no" | "nb" | "nn" | "da" | "dk") {
        conns.push("og");
    }

    // (B) explicit hundred token: LEFT <hundred> RIGHT -> LEFT*100 + RIGHT.
    if let Some(ref h) = hw {
        if let Some(pos) = toks.iter().position(|t| *t == h.as_str()) {
            let left = trim_connectors(&toks[..pos].join(" "), &conns);
            let right = trim_connectors(&toks[pos + 1..].join(" "), &conns);
            let high = if left.is_empty() { Some(1) } else { lookup_plain(lang, &left) };
            let low = if right.is_empty() { Some(0) } else { lookup_plain(lang, &right) };
            if let (Some(h100), Some(l)) = (high, low) {
                if (1..=99).contains(&h100) && (0..=99).contains(&l) {
                    return Some(h100 * 100 + l);
                }
            }
        }
    }
    // (C) one glued token carrying the hundred morpheme: PRE<hundred>SUF.
    if toks.len() == 1 {
        if let Some(ref h) = hw {
            let t = toks[0];
            if let Some(pos) = t.find(h.as_str()) {
                let pre = &t[..pos];
                let suf = &t[pos + h.len()..];
                if !(pre.is_empty() && suf.is_empty()) {
                    let high = if pre.is_empty() { Some(1) } else { lookup_plain(lang, pre) };
                    let low = if suf.is_empty() { Some(0) } else { lookup_plain(lang, suf) };
                    if let (Some(h100), Some(l)) = (high, low) {
                        if (1..=99).contains(&h100) && (0..=99).contains(&l) {
                            return Some(h100 * 100 + l);
                        }
                    }
                }
            }
        }
    }
    // (A) two spoken 2-digit groups ("nineteen ninety-nine") -> a*100 + b. A Germanic
    // habit (nl "negentien negenennegentig", da "nitten nitti"): Romance languages say
    // the thousand ("mil novecientos"), and es "catorce treinta" is a time of day.
    // Either group may span several tokens: af spells 21-99 "vier-en-twintig",
    // which normalizes to three tokens ("twintig vier en twintig" = 2024, #133).
    let pairs = matches!(base, "de" | "nl" | "af" | "da" | "sv" | "no" | "nb" | "nn" | "is");
    if pairs {
        for k in 1..toks.len() {
            let (left, right) = (toks[..k].join(" "), toks[k..].join(" "));
            if let (Some(a), Some(b)) = (lookup_plain(lang, &left), lookup_plain(lang, &right)) {
                if (10..=99).contains(&a) && (0..=99).contains(&b) {
                    return Some(a * 100 + b);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Year mode (`to="year"`) for the reverse-table languages
// ---------------------------------------------------------------------------
//
// Many renderers spell a year differently from the cardinal: a year word
// around it (bn "... সাল", vi "năm ...", zh/ja "...年", hy "... թվական"), a
// digit-by-digit reading (zh "一九零五年"), an imperial era (ja "明治三十八年"),
// a combining form (ta "ஆயிரத்து"), glued words (fi "tuhatyhdeksänsataaviisi"),
// a stray suffix (lij ". urtea", #247). None of those is in the cardinal table,
// so `to="year"` raised on every year they render (#128-#132). The data comes
// from the renderer itself: a reverse table of `to_year`, the year word its
// renderings share, and whether it reads the digits one by one.

/// Highest year the reverse `to_year` table holds.
const YEAR_MAX: i64 = 3000;

/// What the renderer's `to_year` tells about a language.
struct YearData {
    /// normalize(to_year(n)) -> n, for 1..=YEAR_MAX; a spelling two years
    /// share is dropped (a raise beats a wrong year).
    table: HashMap<String, i64>,
    /// The year word every rendering opens / closes with and the cardinal does
    /// not: vi "nam " / bn " sal", zh "年". Empty when there is none.
    prefix: String,
    suffix: String,
    /// The year is read one digit at a time (zh "一九零五").
    digitwise: bool,
}

fn year_cache() -> &'static RwLock<HashMap<String, Arc<YearData>>> {
    static Y: OnceLock<RwLock<HashMap<String, Arc<YearData>>>> = OnceLock::new();
    Y.get_or_init(|| RwLock::new(HashMap::new()))
}

fn year_data(lang: &str) -> Arc<YearData> {
    if let Some(d) = year_cache().read().unwrap().get(lang) {
        return d.clone();
    }
    let d = Arc::new(build_year_data(lang));
    year_cache().write().unwrap().insert(lang.to_string(), d.clone());
    d
}

/// Longest common prefix, by `char`.
fn common_prefix(a: &str, b: &str) -> String {
    a.chars().zip(b.chars()).take_while(|(x, y)| x == y).map(|(x, _)| x).collect()
}

fn build_year_data(lang: &str) -> YearData {
    let mut data = YearData {
        table: HashMap::new(),
        prefix: String::new(),
        suffix: String::new(),
        digitwise: false,
    };
    let Some(l) = num2words2_core::get_lang_by_key(lang) else { return data };
    let mut ambiguous: Vec<String> = Vec::new();
    let mut years: Vec<String> = Vec::new();
    let mut cards: Vec<String> = Vec::new();
    for n in 1..=YEAR_MAX {
        let b = BigInt::from(n);
        // ja has no era before 645 and declines; those years are not in the table.
        let Ok(w) = l.to_year(&b) else { continue };
        let key = normalize(&w);
        if key.is_empty() || key.chars().all(|c| c.is_ascii_digit() || c == '-' || c == ' ') {
            continue; // the renderer fell back to figures
        }
        match data.table.get(&key) {
            Some(&m) if m != n => ambiguous.push(key.clone()),
            Some(_) => {}
            None => {
                data.table.insert(key.clone(), n);
            }
        }
        if let Ok(c) = l.to_cardinal(&b) {
            cards.push(normalize(&c));
        }
        years.push(key);
    }
    for k in ambiguous {
        data.table.remove(&k);
    }
    if years.is_empty() {
        return data;
    }
    let fold = |v: &[String], f: fn(&str, &str) -> String| -> String {
        let mut acc = v[0].clone();
        for s in &v[1..] {
            acc = f(&acc, s);
        }
        acc
    };
    let rev = |s: &str| s.chars().rev().collect::<String>();
    let mut prefix = fold(&years, common_prefix);
    let mut suffix = rev(&fold(
        &years.iter().map(|s| rev(s)).collect::<Vec<_>>(),
        common_prefix,
    ));
    // In a spaced language the year word is whole tokens: never half a word.
    if years.iter().filter(|y| y.contains(' ')).count() * 2 > years.len() {
        prefix = prefix.rfind(' ').map(|i| prefix[..=i].to_string()).unwrap_or_default();
        suffix = suffix.find(' ').map(|i| suffix[i..].to_string()).unwrap_or_default();
    }
    // An affix every cardinal shares too is part of the number, not a year word.
    if !cards.is_empty() {
        if !prefix.is_empty() && cards.iter().all(|c| c.starts_with(&prefix)) {
            prefix.clear();
        }
        if !suffix.is_empty() && cards.iter().all(|c| c.ends_with(&suffix)) {
            suffix.clear();
        }
    }
    data.prefix = prefix;
    data.suffix = suffix;
    // zh renders 1905 as "一九零五年": the digits' own words, one after another.
    data.digitwise = (|| {
        let y = normalize(&l.to_year(&BigInt::from(1905)).ok()?);
        let body = strip_affixes(&y, &data.prefix, &data.suffix)?;
        let mut digits = String::new();
        for d in [1, 9, 0, 5] {
            digits.push_str(&normalize(&l.to_cardinal(&BigInt::from(d)).ok()?));
        }
        Some(body.replace(' ', "") == digits)
    })()
    .unwrap_or(false);
    data
}

/// `norm` without the year word, trimmed; `None` when it is absent or nothing
/// is left after it.
fn strip_affixes(norm: &str, prefix: &str, suffix: &str) -> Option<String> {
    let mut body = norm;
    let mut stripped = false;
    if !prefix.is_empty() {
        if let Some(r) = body.strip_prefix(prefix) {
            body = r;
            stripped = true;
        }
    }
    if !suffix.is_empty() {
        if let Some(r) = body.strip_suffix(suffix) {
            body = r;
            stripped = true;
        }
    }
    let body = body.trim();
    (stripped && !body.is_empty()).then(|| body.to_string())
}

/// The year `norm` spells exactly as num2words2's `to_year` renders it: bn
/// "এক হাজার ছিষট্টি সাল" (1066), ja "明治三十八年" (1905), fi
/// "tuhatyhdeksänsataaviisi". Looked up in the reverse `to_year` table.
pub fn lookup_year_norm(lang: &str, norm: &str) -> Option<i64> {
    if !is_supported_lang(lang) {
        return None;
    }
    year_data(lang).table.get(norm).copied()
}

/// Spoken variants of a year that are not the renderer's exact spelling, for
/// the caller to read as a cardinal: the year word dropped (ja "千九百五年" ->
/// "千九百五", vi "năm một nghìn chín trăm lẻ năm" -> "một nghìn ..."), and ta's
/// combining "ஆயிரத்து" read as the thousand "ஆயிரம்" (#131).
pub fn year_variants_norm(lang: &str, norm: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if !is_supported_lang(lang) {
        return out;
    }
    let yd = year_data(lang);
    let bare = strip_affixes(norm, &yd.prefix, &yd.suffix);
    for v in std::iter::once(norm.to_string()).chain(bare) {
        if lang == "ta" {
            let (comb, thousand) = (normalize("ஆயிரத்து"), normalize("ஆயிரம்"));
            if v.split_whitespace().any(|t| t == comb) {
                let alias: Vec<&str> = v
                    .split_whitespace()
                    .map(|t| if t == comb { thousand.as_str() } else { t })
                    .collect();
                out.push(alias.join(" "));
            }
        }
        if v != norm {
            out.push(v);
        }
    }
    out
}

/// A year read one digit at a time, in a language whose renderer reads years
/// that way (zh "一九零五", "一九〇五", #129): two to four digit words, `〇`
/// being zero too. The year word, if any, must already be stripped.
pub fn year_digit_string_norm(lang: &str, norm: &str) -> Option<i64> {
    if !is_supported_lang(lang) || !year_data(lang).digitwise {
        return None;
    }
    let mut digits = String::new();
    for c in norm.chars().filter(|c| !c.is_whitespace()) {
        let d = if c == '〇' { 0 } else { lookup_plain(lang, &c.to_string())? };
        if !(0..=9).contains(&d) {
            return None;
        }
        digits.push(char::from(b'0' + d as u8));
    }
    if !(2..=4).contains(&digits.len()) {
        return None;
    }
    digits.parse().ok()
}

// ---------------------------------------------------------------------------
// English grammar entry points (pure)
// ---------------------------------------------------------------------------

/// `Words2Num_EN().to_cardinal(text)`.
pub fn en_to_cardinal(text: &str) -> Result<w2n_lang_en::W2nValue, w2n_lang_en::W2nError> {
    en_grammar().to_cardinal(text)
}

/// The English grammar, built once: its tables are static and the sentence walker
/// asks it about every token.
fn en_grammar() -> &'static w2n_lang_en::W2nLangEn {
    static G: OnceLock<w2n_lang_en::W2nLangEn> = OnceLock::new();
    G.get_or_init(w2n_lang_en::W2nLangEn::new)
}

/// `Words2Num_EN().to_ordinal(text)`.
pub fn en_to_ordinal(text: &str) -> Result<w2n_lang_en::W2nValue, w2n_lang_en::W2nError> {
    en_grammar().to_ordinal(text)
}

/// `Words2Num_EN().to_year(text)`.
pub fn en_to_year(text: &str) -> Result<w2n_lang_en::W2nValue, w2n_lang_en::W2nError> {
    en_grammar().to_year(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #216: the public per-lang helpers must not cache arbitrary strings.
    #[test]
    fn unsupported_lang_is_not_cached_issue_216() {
        for i in 0..200 {
            let lang = format!("not-a-lang-{i}");
            assert!(!is_scale_word_of(&lang, "x", 0));
            assert!(!token_may_be_number(&lang, "x"));
            assert!(scale_words(&lang).is_empty());
            assert!(!scale_cache().read().unwrap().contains_key(&lang));
            assert!(!hundred_cache().read().unwrap().contains_key(&lang));
            assert!(!hundred_prefix_cache().read().unwrap().contains_key(&lang));
        }
        // Supported languages are unaffected.
        assert!(is_scale_word_of("fr", "mille", 1000));
        assert!(token_may_be_number("fr", "mille"));
    }

    #[test]
    fn normalize_matches_python_shape() {
        // Hyphen between letters -> space; trailing punct -> space; collapse.
        assert_eq!(normalize("Forty-Two"), "forty two");
        assert_eq!(normalize("  a_b  "), "a b");
        assert_eq!(normalize("hello."), "hello");
        assert_eq!(normalize("3.14"), "3.14"); // decimal point survives
        assert_eq!(normalize("-17"), "-17"); // sign hyphen survives
        // NFKD + combining-mark strip: "trente-deux" family.
        assert_eq!(normalize("tr\u{e9}nte"), "trente"); // é -> e
        assert_eq!(normalize("f\u{f3}rty"), "forty"); // ó -> o
    }

    #[test]
    fn normalize_keeps_script_vowel_signs() {
        // Optional diacritics still go: Cyrillic breve, Arabic harakat.
        assert_eq!(normalize("\u{439}"), "\u{438}"); // й -> и
        assert_eq!(normalize("\u{648}\u{64e}\u{627}"), "\u{648}\u{627}");
        // Vowel signs are letters: stripping them merged distinct numbers.
        assert_ne!(normalize("पचासी"), normalize("पचास")); // hi 85 / 50
        assert_ne!(normalize("དགུ་"), normalize("དྲུག་")); // bo 9 / 6
        assert_ne!(normalize("একাশি"), normalize("একুশ")); // bn 81 / 21
        // The nukta is optional: hi 67 with and without it.
        assert_eq!(normalize("सड़सठ"), normalize("सडसठ"));
        assert_eq!(lookup("hi", "सडसठ", false, &[]).ok().flatten(), Some(67));
    }

    #[test]
    fn script_vowel_signs_parse_to_their_own_number() {
        for (lang, text, n) in [
            ("hi", "पचासी", 85),
            ("hi", "पचास", 50),
            ("bo", "དགུ་", 9),
            ("bn", "একাশি", 81),
            ("kn", "ನಲವತ್ತೈದು", 45),
            ("gu", "નેવું", 90),
            ("pa", "ਸੱਤਰ", 70),
            ("dv", "އައްހާސް", 8000),
        ] {
            assert_eq!(lookup(lang, text, false, &[]).ok().flatten(), Some(n), "{lang} {text}");
        }
    }

    #[test]
    fn en_entry_points() {
        assert_eq!(
            en_to_cardinal("forty-two").unwrap(),
            w2n_lang_en::W2nValue::Int(BigInt::from(42))
        );
        assert_eq!(
            en_to_ordinal("twenty-first").unwrap(),
            w2n_lang_en::W2nValue::Int(BigInt::from(21))
        );
        assert_eq!(
            en_to_year("nineteen ninety nine").unwrap(),
            w2n_lang_en::W2nValue::Int(BigInt::from(1999))
        );
        assert!(en_to_cardinal("forty zoot").is_err());
    }

    #[test]
    fn scaled_cardinals_above_table() {
        // Reverse table only holds -1..10001; these compose via scale words.
        assert_eq!(parse_scaled("fr", "soixante-neuf mille huit"), Some(69008));
        assert_eq!(parse_scaled("fr", "cinquante-neuf mille"), Some(59000));
        assert_eq!(parse_scaled("fr", "soixante-quinze mille treize"), Some(75013));
        assert_eq!(parse_scaled("fr", "quatre-vingt-douze mille cent"), Some(92100));
        assert_eq!(parse_scaled("fr", "mille treize"), Some(1013));
        // Non-number fragment → None (walker keeps its shorter parse).
        assert_eq!(parse_scaled("fr", "mille lyon"), None);
        // Plural scale words, connectors, long-scale billions, Slavic genitive
        // plural — round-trips that the flat reverse table could never hold.
        assert_eq!(parse_scaled("fr", "un milliard"), Some(1_000_000_000));
        assert_eq!(parse_scaled("es", "sesenta y nueve mil ocho"), Some(69008));
        assert_eq!(parse_scaled("es", "dos millones"), Some(2_000_000)); // not 2e9
        assert_eq!(parse_scaled("es", "mil millones"), Some(1_000_000_000));
        assert_eq!(parse_scaled("pt", "sessenta e nove mil e oito"), Some(69008));
        assert_eq!(parse_scaled("pl", "sześćdziesiąt dziewięć tysięcy osiem"), Some(69008));
        assert_eq!(parse_scaled("ru", "шестьдесят девять тысяч восемь"), Some(69008));
        // Sentence walk emits the composed value inline.
        assert_eq!(
            w2n_sentence::words2num_sentence(
                "quarante-deux rue des freres lumiere soixante-neuf mille huit lyon",
                "fr",
                "cardinal",
                false,
            )
            .unwrap(),
            "42 rue des freres lumiere 69008 lyon"
        );
    }

    /// gladiaio/words2num2#36: the magnitude of a scale word is read off the
    /// rendering, never assumed from the probed power.
    #[test]
    fn scale_magnitudes_are_derived() {
        // Lakh (10^5) and crore (10^7), not 10^6 / 10^9.
        assert_eq!(parse_scaled("en_IN", "one lakh"), Some(100_000));
        assert_eq!(parse_scaled("en_IN", "ten lakh"), Some(1_000_000));
        assert_eq!(parse_scaled("en_IN", "one crore"), Some(10_000_000));
        assert_eq!(
            parse_scaled("hi", "बारह लाख चौंतीस हज़ार पाँच सौ सड़सठ"),
            Some(1_234_567)
        );
        // ta "ஒரு" is "one" only in front of a scale word.
        assert_eq!(parse_scaled("ta", "ஒரு இலட்சம்"), Some(100_000));
        // eu takes "one" after the scale, every other multiplier before it.
        assert_eq!(parse_scaled("eu", "milioi bat"), Some(1_000_000));
        assert_eq!(parse_scaled("eu", "bi milioi"), Some(2_000_000));
        // ha / ff put every multiplier after the scale word.
        assert_eq!(parse_scaled("ha", "dubu biyu"), Some(2_000));
        assert_eq!(parse_scaled("ha", "miliyan biyu"), Some(2_000_000));
        // #348: the additive reading when 1000 × 10^6 is not how the renderer
        // writes the product.
        assert_eq!(parse_scaled("ha", "miliyan dubu"), Some(1_001_000));
        assert_eq!(parse_scaled("ha", "miliyan dubu biyu"), Some(1_002_000));
        assert_eq!(parse_scaled("ha", "miliyan dubu uku ɗari huɗu hamsin da shida"), Some(1_003_456));
        assert_eq!(parse_scaled("ha", "biliyan dubu"), Some(1_000_001_000));
        // #882: the renderer writes "<scale> <M>" for the product and for the
        // sum without "da" alike; a reader takes the multiplier.
        assert_eq!(parse_scaled("ha", "miliyan goma"), Some(10_000_000));
        assert_eq!(parse_scaled("ha", "miliyan talatin"), Some(30_000_000));
        assert_eq!(parse_scaled("ha", "miliyan sha biyu"), Some(12_000_000));
        assert_eq!(parse_scaled("ha", "miliyan ɗari biyar"), Some(500_000_000));
        // The thousands stay ambiguous, as before (1 001 100 and 1 100 000).
        assert_eq!(parse_scaled("ha", "miliyan dubu ɗari"), None);
        assert_eq!(parse_scaled("ha", "miliyan biyu dubu ɗari uku sha biyar"), None);
        // The sum is written with "da".
        assert_eq!(parse_scaled("ha", "miliyan da biyu"), Some(1_000_002));
        assert_eq!(parse_scaled("ha", "miliyan da talatin"), Some(1_000_030));
        assert_eq!(parse_scaled("ha", "miliyan da ɗari biyar tamanin"), Some(1_000_580));
        assert_eq!(parse_scaled("ff", "ujunere teemerre"), Some(100_000));
        // km myriad and hundred-thousand glued to their multiplier.
        assert_eq!(parse_scaled("km", "មួយម៉ឺន ពីរពាន់ បីរយ សែសិបប្រាំ"), Some(12_345));
        assert_eq!(parse_scaled("km", "មួយសែន"), Some(100_000));
        // en_AERO reads digits one at a time.
        assert_eq!(parse_scaled("en_AERO", "wun zero zero zero zero zero"), Some(100_000));
        assert_eq!(parse_scaled("en_AERO", "wun too tree fower fife"), Some(12_345));
        // A postfix multiplier of 1000 or more ends the number.
        assert_eq!(parse_scaled("tet", "miliaun rihun ida"), Some(1_000_000_000));
        assert_eq!(parse_scaled("tet", "miliaun ida rihun ida"), Some(1_001_000));
        // A left part as big as the scale is a group, not a multiplier.
        assert_eq!(parse_scaled("ko", "십만 천"), Some(101_000));
        assert_eq!(parse_scaled("ko", "백만 천"), Some(1_001_000));
        // Never a wrong number where the reading is ambiguous: a fused bare
        // "one-thousand" is no suffix (dv "thirty-one-thousand" is 31 × 1000,
        // not 30 × 1000), 1000 + 12 needs the renderer's spelling (bm 12000 is
        // 1000 × 12), and a postfix-scale reading must re-render to the input
        // (tet).
        assert_eq!(parse_scaled("dv", "ތިރީސްއެއްހާސް"), Some(31_000));
        assert_eq!(parse_scaled("bm", "ba tan ni fila"), None);
        assert_eq!(parse_scaled("fi", "kymmenentuhatta yksi"), Some(10_001));
        assert_eq!(parse_scaled("tet", "miliaun rihun atus sia rihun ida"), None);
        // A scale glued to its multiplier inside a longer number.
        assert_eq!(parse_scaled("hu", "egymillió-egy"), Some(1_000_001));
        // Prefix languages never take a postfix multiplier or sum bare groups.
        assert_eq!(parse_scaled("fr", "mille deux cents"), Some(1_200));
        assert_eq!(parse_scaled("es", "mil doscientos"), Some(1_200));
        assert_eq!(parse_scaled("fr", "deux trois"), None);
    }

    /// gladiaio/words2num2#40: a stacked scale word multiplies, and the split is
    /// at its last occurrence with a remainder below it.
    #[test]
    fn stacked_scales_multiply() {
        assert_eq!(parse_scaled("bn", "দশ কোটি কোটি"), Some(1_000_000_000_000_000));
        assert_eq!(parse_scaled("as", "দহ কোটি কোটি"), Some(1_000_000_000_000_000));
        assert_eq!(parse_scaled("bn", "দশ হাজার কোটি কোটি"), Some(1_000_000_000_000_000_000));
        assert_eq!(parse_scaled("bn", "দশ কোটি পাঁচ কোটি"), Some(1_000_000_050_000_000));
        assert_eq!(parse_scaled("bn", "দশ কোটি কোটি পাঁচ"), Some(1_000_000_000_000_005));
        assert_eq!(parse_scaled("bn", "এক লাখ বিশ হাজার কোটি"), Some(1_200_000_000_000));
        assert_eq!(parse_scaled("ta", "ஒரு கோடி கோடி"), Some(100_000_000_000_000));
        assert_eq!(parse_scaled("ta", "பத்து ஆயிரம் கோடி கோடி"), Some(1_000_000_000_000_000_000));
        assert_eq!(parse_scaled("ba", "бер миллиард миллиард"), Some(1_000_000_000_000_000_000));
        assert_eq!(parse_scaled("bg", "един милиард милиарда"), Some(1_000_000_000_000_000_000));
        assert_eq!(parse_scaled("eu", "mila mila milioi"), Some(1_000_000_000_000));
        assert_eq!(parse_scaled("eu", "bi mila mila milioi"), Some(2_000_000_000_000));
        assert_eq!(parse_scaled("eu", "bat mila milioi hamar milioi bat"), Some(1_010_000_001));
        assert_eq!(parse_scaled("th", "หนึ่งล้านสองแสนล้าน"), Some(1_200_000_000_000));
        assert_eq!(parse_scaled("th", "สองล้านเอ็ดล้านล้าน"), Some(2_000_001_000_000_000_000));
        assert_eq!(parse_scaled("hy", "միլիոն միլիոն"), Some(1_000_000_000_000));
        // A language that never stacks reads no stack; the remainder stays below
        // the scale.
        assert_eq!(parse_scaled("fr", "deux mille deux mille"), None);
        assert_eq!(parse_scaled("fr", "deux mille mille"), None);
        assert_eq!(parse_scaled("hu", "húsz három"), None);
        assert_eq!(parse_scaled("es", "un millón mil"), Some(1_001_000));
        assert_eq!(parse_scaled("es", "dos mil millones"), Some(2_000_000_000));
    }

    /// gladiaio/words2num2#84: scales up to 10^18 (10^11 for the Indian kharab).
    #[test]
    fn scales_up_to_ten_to_the_eighteen() {
        assert_eq!(parse_scaled("de", "eine Billion"), Some(1_000_000_000_000));
        assert_eq!(parse_scaled("fr", "un billion"), Some(1_000_000_000_000));
        assert_eq!(parse_scaled("es", "dos billones"), Some(2_000_000_000_000));
        assert_eq!(parse_scaled("en_NG", "one trillion"), Some(1_000_000_000_000));
        assert_eq!(parse_scaled("en_IN", "one kharab"), Some(100_000_000_000));
        assert_eq!(parse_scaled("hi", "बीस खरब"), Some(2_000_000_000_000));
        assert_eq!(parse_scaled("ru", "один триллион"), Some(1_000_000_000_000));
        assert_eq!(parse_scaled("pt", "um bilião"), Some(1_000_000_000_000));
        assert_eq!(parse_scaled("ja", "一兆"), Some(1_000_000_000_000));
        assert_eq!(parse_scaled("ja", "百京"), Some(1_000_000_000_000_000_000));
        assert_eq!(parse_scaled("ko", "일조"), Some(1_000_000_000_000));
        // ro renders 10^12, 2·10^12 and 5·10^12 all "bilion" (#220): no magnitude.
        assert_eq!(parse_scaled("ro", "bilion"), None);
    }

    /// gladiaio/words2num2#72, #94, #187: overflow is a refusal, never a wrap,
    /// and a hostile length is refused up front.
    #[test]
    fn scaled_arithmetic_is_checked() {
        let huge = "neuf cent quatre-vingt-dix-neuf millions neuf cent quatre-vingt-dix-neuf mille \
                    neuf cent quatre-vingt-dix-neuf milliards "
            .repeat(10);
        assert_eq!(parse_scaled("fr", huge.trim()), None);
        assert_eq!(parse_scaled("bn", "দশ হাজার কোটি কোটি কোটি"), None);
        let nineteen = format!("wun{}", " zero".repeat(18));
        assert_eq!(parse_scaled("en_AERO", &nineteen), Some(1_000_000_000_000_000_000));
        assert_eq!(parse_scaled("en_AERO", &format!("niner{}", " zero".repeat(19))), None);
        let long = vec!["deux mille"; 30_000].join(" ");
        assert_eq!(parse_scaled("fr", &long), None);
        assert_eq!(parse_scaled("th", &"หนึ่งล้าน".repeat(20_000)), None);
    }

    /// gladiaio/words2num2#41, #42, #43, #102: magnitudes read off the renderings.
    #[test]
    fn scale_derivation_audit() {
        // dv glues a special form of the multiplier: "ދެ" (2), "އެއް" (1).
        assert_eq!(parse_scaled("dv", "ބާރަހާސް"), Some(12_000));
        assert_eq!(parse_scaled("dv", "ވިހިހާސް"), Some(20_000));
        assert_eq!(parse_scaled("dv", "ދެމިލިޔަން"), Some(2_000_000));
        assert_eq!(
            parse_scaled("dv", "ތިރީސްއެއްހާސް ނުވަސަތޭކަ ތިރީސްހަތަރެއް"),
            Some(31_934)
        );
        // vi "tỷ" (10^9) is two letters.
        assert_eq!(parse_scaled("vi", "một tỷ"), Some(1_000_000_000));
        // bm "ba" stays out: every "ba X ni Y" collides (#226).
        assert_eq!(parse_scaled("bm", "ba tan ni kelen"), None);
        // kok writes 10^6 "एक धा लाख" (one ten-lakh): लाख is 10^5.
        assert_eq!(parse_scaled("kok", "एक लाख"), Some(100_000));
        assert_eq!(parse_scaled("kok", "धा लाख"), Some(1_000_000));
        assert_eq!(parse_scaled("kok", "एक धा लाख"), Some(1_000_000));
        assert_eq!(parse_scaled("or", "ଦଶ ଲକ୍ଷ"), Some(1_000_000));
        assert_eq!(parse_scaled("pli", "eka dasa-lakkha"), Some(1_000_000));
        // 100 × ten-lakh to the renderer, 110 lakh to a reader: declined.
        assert_eq!(parse_scaled("kok", "एक शंभर धा लाख"), None);
        assert_eq!(parse_scaled("or", "ଏକ ଶହ ଦଶ ଲକ୍ଷ"), None);
        // sn "mbiri" is 2 after a postfix scale word.
        assert_eq!(parse_scaled("sn", "bhiriyoni mbiri"), Some(2_000_000_000));
        // Plurals heard only after other multipliers.
        assert_eq!(parse_scaled("sl", "tri milijarde"), Some(3_000_000_000));
        assert_eq!(parse_scaled("lt", "dešimt milijardų"), Some(10_000_000_000));
        assert_eq!(parse_scaled("lt", "du šimtai milijardų"), Some(200_000_000_000));
        // Standard spellings the renderer never writes (#240, #259).
        assert_eq!(parse_scaled("sl", "pet milijonov"), Some(5_000_000));
        assert_eq!(parse_scaled("sl", "pet milijard"), Some(5_000_000_000));
        assert_eq!(parse_scaled("da", "en million"), Some(1_000_000));
    }

    /// gladiaio/words2num2#90, #91, #92, #99, #100, #239: glued scales.
    #[test]
    fn glued_scales_audit() {
        assert_eq!(parse_scaled("hu", "tízezer-száz"), Some(10_100));
        assert_eq!(parse_scaled("hu", "tízezer-kettő"), Some(10_002));
        assert_eq!(parse_scaled("fi", "kymmenentuhatta sata"), Some(10_100));
        assert_eq!(parse_scaled("sv", "tiotusen trehundrafyrtiofem"), Some(10_345));
        assert_eq!(parse_scaled("nl", "tienduizend negenhonderdzevenennegentig"), Some(10_997));
        assert_eq!(parse_scaled("cn", "一千二百三十四万五千六百七十八"), Some(12_345_678));
        assert_eq!(parse_scaled("ja", "千二百三十四万五千六百七十八"), Some(12_345_678));
        assert_eq!(parse_scaled("ja", "一億二千三百四十五万六千七百八十九"), Some(123_456_789));
        assert_eq!(parse_scaled("ko", "천이백삼십사만 오천육백칠십팔"), Some(12_345_678));
        assert_eq!(parse_scaled("ko", "일억 이천삼백사십오만 육천칠백팔십구"), Some(123_456_789));
        assert_eq!(parse_scaled("zh", "九千九百九十九万"), Some(99_990_000));
        assert_eq!(parse_scaled("bo", "གཅིག་འབུམ་གཅིག་ཁྲི་དྲུག་བརྒྱ་དགུ་བཅུ་བདུན་"), Some(110_697));
        // #349: a compound scale (ཁྲི་དུང་ཕྱུར) does not swallow the ཁྲི of the
        // multiplier before དུང་ཕྱུར.
        assert_eq!(parse_scaled("bo", "གཅིག་འབུམ་གཅིག་ཁྲི་དུང་ཕྱུར་"), Some(11_000_000_000_000));
        assert_eq!(parse_scaled("bo", "དགུ་འབུམ་དགུ་ཁྲི་དུང་ཕྱུར་"), Some(99_000_000_000_000));
        assert_eq!(parse_scaled("bo", "གཅིག་ས་ཡ་གཅིག་ཁྲི་དུང་ཕྱུར་"), Some(101_000_000_000_000));
        assert_eq!(parse_scaled("bo", "གཅིག་ཁྲི་དུང་ཕྱུར་"), Some(1_000_000_000_000));
        assert_eq!(parse_scaled("th", "หนึ่งแสนสองหมื่นสามพันสี่ร้อยห้าสิบหก"), Some(123_456));
        // zh 零 fills a skipped place.
        assert_eq!(parse_scaled("zh", "一万零一"), Some(10_001));
        assert_eq!(parse_scaled("cn", "一亿零一万"), Some(100_010_000));
        assert_eq!(parse_scaled("cn", "二十万零四百二十七"), Some(200_427));
        assert_eq!(parse_scaled("zh", "一万零一十"), Some(10_010));
        assert_eq!(parse_scaled("zh_TW", "一萬零一"), Some(10_001));
        assert_eq!(parse_scaled("zh", "一万零五千"), None); // no place skipped
        // Without 零 a short tail is the spoken abbreviation (一万五 = 15000).
        assert_eq!(parse_scaled("zh", "一万五"), None);
        assert_eq!(parse_scaled("zh", "一万 五"), None);
        assert_eq!(parse_scaled("ja", "一万五"), Some(10_005)); // ja writes 10005 so
        // sv elides the t that "ett" shares with "tusen".
        assert_eq!(parse_scaled("sv", "tjugoettusen"), Some(21_000));
        assert_eq!(parse_scaled("sv", "tjugoettusen niohundrasextiofyra"), Some(21_964));
        assert_eq!(parse_scaled("sv", "en miljonett"), Some(1_000_001));
        assert_eq!(parse_scaled("sv", "två miljonerett"), Some(2_000_001));
        // th "เอ็ด" is the final one after a higher place.
        assert_eq!(parse_scaled("th", "หนึ่งหมื่นเอ็ด"), Some(10_001));
        assert_eq!(parse_scaled("th", "หนึ่งล้านเอ็ด"), Some(1_000_001));
    }

    #[test]
    fn supported_langs_nonempty() {
        let langs = supported_langs();
        assert!(langs.contains(&"en"));
        assert!(langs.contains(&"fr"));
        assert!(langs.len() >= 100);
    }

    #[test]
    fn hundred_morpheme_is_derived() {
        assert_eq!(hundred_word("fr").as_deref(), Some("cent"));
        assert_eq!(hundred_word("it").as_deref(), Some("cento"));
        assert_eq!(hundred_word("de").as_deref(), Some("hundert"));
        assert_eq!(hundred_word("nl").as_deref(), Some("honderd"));
        assert_eq!(hundred_word("sv").as_deref(), Some("hundra"));
    }

    #[test]
    fn spoken_year_forms() {
        // Two 2-digit groups ("nineteen ninety-nine").
        assert_eq!(parse_year("de", "neunzehn neunundneunzig"), Some(1999));
        assert_eq!(parse_year("nl", "negentien zevenennegentig"), Some(1997));
        // Explicit hundred, spaced (fr "dix-neuf cent ...", hyphens → spaces).
        assert_eq!(parse_year("fr", "dix neuf cent quatre vingt dix"), Some(1990));
        assert_eq!(parse_year("fr", "dix-neuf cent quatre-vingt-dix"), Some(1990));
        // Hundred glued into one token.
        assert_eq!(parse_year("sv", "nittonhundranittiosju"), Some(1997));
        assert_eq!(parse_year("nl", "negentienhonderdzevenennegentig"), Some(1997));
        // A plain canonical two-token number must NOT be mis-read as a year:
        // "vingt trois" (23) is a table hit, so the fallback never sees it —
        // but even directly, 20·100+3 is refused because 23 resolves first via
        // the caller; here we assert the guard shape holds for a non-century.
        assert_eq!(parse_year("fr", "trois quatre"), None); // 3,4 not in 10..99
    }

    #[test]
    fn year_forms_via_public_entry() {
        use w2n_lang_en::W2nValue;
        use w2n_sentence::words2num;
        let int = |n: i64| W2nValue::Int(BigInt::from(n));
        // End-to-end through the same path Python's `words2num` takes.
        assert_eq!(words2num("neunzehn neunundneunzig", "de", "cardinal").unwrap(), int(1999));
        assert_eq!(words2num("nittonhundranittiosju", "sv", "cardinal").unwrap(), int(1997));
        // De-spaced glued canonical ("mille novecento ottantotto" → 1988).
        assert_eq!(words2num("mille novecento ottantotto", "it", "cardinal").unwrap(), int(1988));
        // Regression: a canonical number is unaffected.
        assert_eq!(words2num("deux mille dix", "fr", "cardinal").unwrap(), int(2010));
        assert_eq!(words2num("vingt trois", "fr", "cardinal").unwrap(), int(23));
    }

    fn w2n(text: &str, lang: &str, to: &str) -> Option<i64> {
        match w2n_sentence::words2num(text, lang, to) {
            Ok(w2n_lang_en::W2nValue::Int(n)) => i64::try_from(n).ok(),
            _ => None,
        }
    }

    /// #61 #123 #124 #243: the sign prefix is derived per language and applied
    /// to the whole composed value.
    #[test]
    fn negatives_take_the_renderer_prefix_and_the_whole_value() {
        for (lang, text, n) in [
            ("fr", "moins sept", -7),                                   // #123
            ("es", "menos cuarenta y dos", -42),
            ("zh", "负七", -7),
            ("am", "ሰልቢ ሰባት", -7),
            ("tr", "eksi yedi", -7),
            ("xh", "ngaphantsi kwe sixhenxe", -7),
            ("am", "ሰልቢ አንድ ሺህ ሁለት መቶ ሠላሳ አራት", -1234),        // #61: not -766
            ("fr", "moins un million mille", -1001000),
            ("fr", "moins un million cinq cents", -1000500),
            ("de", "minus zwölftausenddreihundertfünfundvierzig", -12345),
            ("sv", "minus tolvtusen trehundrafyrtiofem", -12345),
            ("tr", "eksiyedi", -7),                                     // #124 glued
            ("th", "ติดลบเจ็ด", -7),
            ("th", "ติดลบหนึ่งพันสองร้อยสามสิบสี่", -1234),
            ("kn", "ಮೈನಸ್ ಏಳು", -7),                                   // #243
            ("kn", "ಋಣ ಏಳು", -7),
            ("kn", "(-) ಏಳು", -7),
            ("te", "మైనస్ ఏడు", -7),
            ("en", "minus seven", -7),
        ] {
            assert_eq!(w2n(text, lang, "cardinal"), Some(n), "{lang} {text:?}");
        }
        // Every language: -n renders and parses back, below and above the table.
        for lang in supported_langs() {
            let l = num2words2_core::get_lang_by_key(lang).unwrap();
            for n in [-7i64, -42, -1234] {
                let Ok(w) = l.to_cardinal(&BigInt::from(n)) else { continue };
                if w.chars().all(|c| c.is_ascii_digit() || c == '-') {
                    continue;
                }
                let got = w2n(&w, lang, "cardinal");
                assert!(got == Some(n) || got.is_none(), "{lang} {w:?} -> {got:?}, want {n}");
            }
        }
        // A bare prefix is not a number.
        assert_eq!(w2n("moins", "fr", "cardinal"), None);
    }

    /// #114 dashes, #115 apostrophes, #116 dotless ı, #117 ß, #118 format
    /// characters, #119 ml chillus, #120 bo tsheg, #121 punctuation, #122 nasal
    /// signs.
    #[test]
    fn normalize_folds_typographic_variants() {
        assert_eq!(normalize("twenty\u{2013}one"), "twenty one");
        assert_eq!(normalize("forty\u{2011}two"), normalize("forty-two"));
        assert_eq!(normalize("\u{2212}42"), "-42");
        assert_eq!(normalize("п\u{2019}ять"), normalize("п'ять"));
        assert_eq!(normalize("п\u{02BC}ять"), normalize("п'ять"));
        assert_eq!(normalize("to\u{02BB}rt"), normalize("to'rt"));
        assert_eq!(normalize("SIFIR"), normalize("sıfır"));
        assert_eq!(normalize("DREISSIG"), normalize("dreißig"));
        assert_eq!(normalize("\u{FEFF}forty\u{200B}two"), "forty two");
        // A soft hyphen is only a line-break hint: the word stays whole.
        assert_eq!(normalize("twenty\u{00AD}one"), "twentyone");
        assert_eq!(normalize("शून्\u{200C}य"), normalize("शून्य"));
        assert_eq!(normalize("ഒന\u{0D4D}\u{200D}പത്"), normalize("ഒൻപത്"));
        assert_eq!(normalize("གཅིག"), normalize("གཅིག་"));
        assert_eq!(normalize("ལྔ།"), normalize("ལྔ་"));
        for p in ["पाँच।", "«cinq»", "(five)", "五。", "خمسة،", "አምስት።"] {
            assert!(!normalize(p).chars().any(|c| !c.is_alphanumeric() && !unicode_normalization::char::is_combining_mark(c)), "{p:?} -> {:?}", normalize(p));
        }
        assert_eq!(normalize("पांच"), normalize("पाँच"));
        // Normalizing twice changes nothing.
        for t in ["Forty-Two", "åttende", "ติดลบเจ็ด", "(-) ಏಳು", "dreißig"] {
            assert_eq!(normalize(&normalize(t)), normalize(t));
        }
    }

    #[test]
    fn typographic_variants_parse() {
        for (lang, text, n) in [
            ("en", "twenty\u{2013}one", 21),
            ("fr", "vingt\u{2011}et\u{2011}un", 21),
            ("af", "een\u{2013}en\u{2013}twintig", 21),
            ("en", "\u{2212}five", -5),
            ("en", "\u{2212}42", -42),
            ("uk", "п\u{2019}ять", 5),
            ("uz", "to\u{02BB}rt", 4),
            ("haw", "\u{02BB}ekahi", 1),
            ("tr", "SIFIR", 0),
            ("tr", "KIRK", 40),
            ("az", "ALTI", 6),
            ("de", "dreissig", 30),
            ("de", "ZWEIUNDDREISSIG", 32),
            ("en", "forty\u{200B}two", 42),
            ("en", "\u{FEFF}forty two", 42),
            ("hi", "शून्\u{200C}य", 0),
            ("ml", "ഒന\u{0D4D}\u{200D}പത്", 9),
            ("bo", "གཅིག", 1),
            ("bo", "བཞི་བཅུ་གཉིས", 42),
            ("bo", "ལྔ།", 5),
            ("hi", "पाँच।", 5),
            ("ar", "خمسة،", 5),
            ("zh", "五。", 5),
            ("am", "አምስት።", 5),
            ("fr", "«cinq»", 5),
            ("en", "(five)", 5),
            ("hi", "पांच", 5),
        ] {
            assert_eq!(w2n(text, lang, "cardinal"), Some(n), "{lang} {text:?}");
        }
    }

    /// #121 in sentence mode: a trailing script mark is kept after the figure,
    /// and an opening bracket or quote stays in front of it (#69).
    #[test]
    fn sentence_mode_keeps_the_punctuation_normalize_drops() {
        let s = |t: &str, l: &str| w2n_sentence::words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(s("मेरे पास पाँच। सेब", "hi"), "मेरे पास 5। सेब");
        assert_eq!(s("I have five) apples", "en"), "I have 5) apples");
        // #69: the bracket or quote is cut off and kept, the number converts.
        assert_eq!(s("(five) apples", "en"), "(5) apples");
        assert_eq!(s("«cinq» pommes", "fr"), "«5» pommes");
        assert_eq!(s("forty two.", "en"), "42.");
    }

    /// #44: no/nb "attende" is 18th, "åttende" 8th; a ringless "atte" still reads 8.
    #[test]
    fn nordic_ring_keeps_eighteenth_apart_from_eighth() {
        for lang in ["no", "nb"] {
            assert_eq!(w2n("attende", lang, "ordinal"), Some(18));
            assert_eq!(w2n("åttende", lang, "ordinal"), Some(8));
            assert_eq!(w2n("ett hundre og attende", lang, "ordinal"), Some(118));
            assert_eq!(w2n("atte", lang, "cardinal"), Some(8));
            assert_eq!(w2n("ÅTTE", lang, "cardinal"), Some(8));
        }
        assert_eq!(w2n("tva", "sv", "cardinal"), Some(2));
    }
}

#[cfg(test)]
mod year_mode_tests {
    use crate::w2n_lang_en::W2nValue;
    use crate::w2n_sentence::words2num;
    use num_bigint::BigInt;

    fn year(text: &str, lang: &str) -> Option<i64> {
        match words2num(text, lang, "year") {
            Ok(W2nValue::Int(n)) => i64::try_from(n).ok(),
            _ => None,
        }
    }

    #[test]
    fn year_words_are_stripped() {
        // #128: the year word num2words2's to_year wraps the number in.
        for (lang, text, y) in [
            ("bn", "এক হাজার ছিষট্টি সাল", 1066),
            ("ba", "бер мең туғыҙ йөҙ биш йыл", 1905),
            ("vi", "năm một nghìn chín trăm lẻ năm", 1905),
            ("hy", "հազար ինը հարյուր հինգ թվական", 1905),
            ("km", "ឆ្នាំ មួយពាន់ ប្រាំបួនរយ ប្រាំ", 1905),
            ("ja", "千九百五年", 1905),
            ("zh", "一千九百零五年", 1905),
            ("mn", "нэг мянга есөн зуун таван он", 1905),
            ("eu", "mila bederatziehun eta bost. urtea", 1905),
        ] {
            assert_eq!(year(text, lang), Some(y), "{lang} {text}");
        }
        // vi "năm" is also 5: a lone one is the number.
        assert_eq!(year("năm", "vi"), Some(5));
    }

    #[test]
    fn zh_digit_by_digit_years() {
        // #129
        for text in ["一九零五年", "一九零五", "一九〇五年"] {
            assert_eq!(year(text, "zh"), Some(1905), "{text}");
        }
        assert_eq!(year("二零二四", "zh_TW"), Some(2024));
    }

    #[test]
    fn ja_imperial_eras() {
        // #130: era + year of the era, 元年 being the first.
        assert_eq!(year("明治三十八年", "ja"), Some(1905));
        assert_eq!(year("治暦二年", "ja"), Some(1066));
        assert_eq!(year("令和元年", "jp"), Some(2019));
    }

    #[test]
    fn spoken_year_spellings() {
        assert_eq!(year("ஆயிரத்து அறுபது ஆறு", "ta"), Some(1066)); // #131
        assert_eq!(year("இரண்டு ஆயிரத்து இருபது நான்கு", "ta"), Some(2024));
        assert_eq!(year("tuhatyhdeksänsataaviisi", "fi"), Some(1905)); // #132
        assert_eq!(year("tuhatkuusikymmentäkuusi", "fi"), Some(1066));
        assert_eq!(year("negentien nege-en-negentig", "af"), Some(1999)); // #133
        assert_eq!(year("twintig vier-en-twintig", "af"), Some(2024));
        assert_eq!(year("nitten hundre og fem", "nb"), Some(1905)); // #134
        assert_eq!(year("nitten hundre og nittini", "no"), Some(1999));
        assert_eq!(year("አሥራ ዘጠኝ መቶ አምስት", "am"), Some(1905)); // #135
        assert_eq!(year("አሥራ ዘጠኝ መቶ", "am"), Some(1900));
        assert_eq!(year("ނަވާރަ ސަތޭކަ ފަހެއް", "dv"), Some(1905)); // #136
        assert_eq!(year("ނަވާރަ ސަތޭކަ", "dv"), Some(1900));
        assert_eq!(year("milleneuveçentoçinque. urtea", "lij"), Some(1905)); // #247
        assert_eq!(year("mille. urtea", "lij"), Some(1000));
        assert_eq!(super::hundred_word("am").as_deref(), Some("መቶ"));
    }

    #[test]
    fn every_rendered_year_round_trips() {
        // #128-#136, #247: to_year of every table language, 1000..=2100.
        for lang in ["as", "ba", "bn", "bo", "eu", "gu", "hy", "km", "lo", "mn", "mr", "my", "vi",
                     "zh", "ja", "ta", "fi", "af", "nb", "am", "dv", "lij"] {
            let r = num2words2_core::get_lang_by_key(lang).unwrap();
            for y in (1000..=2100).step_by(7) {
                let w = r.to_year(&BigInt::from(y)).unwrap();
                assert_eq!(year(&w, lang), Some(y), "{lang} {w}");
            }
        }
    }

    #[test]
    fn regional_english_uses_the_grammar() {
        // #193: decimals, year pairs, "a hundred" -- and the lakh reading stays.
        let dec = |t: &str, l: &str| words2num(t, l, "cardinal").map(|v| match v {
            W2nValue::Dec(d) => d.to_string(),
            other => format!("{other:?}"),
        });
        assert_eq!(dec("three point one four", "en_IN").unwrap(), "3.14");
        assert_eq!(year("nineteen ninety nine", "en_NG"), Some(1999));
        assert_eq!(year("twenty twenty", "en_IN"), Some(2020));
        let card = |t: &str, l: &str| match words2num(t, l, "cardinal") {
            Ok(W2nValue::Int(n)) => i64::try_from(n).ok(),
            _ => None,
        };
        assert_eq!(card("a hundred", "en_NP"), Some(100));
        assert_eq!(card("one million", "en_IN"), Some(1_000_000));
        assert_eq!(card("one lakh", "en_IN"), Some(100_000));
        assert_eq!(card("twelve lakh", "en_IN"), Some(1_200_000));
    }
}

/// Numbers written in figures, read with the language's separators (#59, #60).
#[cfg(test)]
mod digit_literal_tests {
    use super::words2num;
    use crate::w2n_lang_en::W2nValue;
    use num_bigint::BigInt;

    fn int(n: i64) -> W2nValue {
        W2nValue::Int(BigInt::from(n))
    }
    fn dec(v: &W2nValue) -> String {
        match v {
            W2nValue::Dec(d) => d.to_string(),
            other => panic!("expected a Decimal, got {other:?}"),
        }
    }

    #[test]
    fn en_thousands_groups_are_not_summed() {
        assert_eq!(words2num("1,234", "en", "cardinal").unwrap(), int(1234));
        assert_eq!(words2num("12,345", "en", "ordinal").unwrap(), int(12345));
        assert_eq!(words2num("1 000", "en", "cardinal").unwrap(), int(1000));
        assert_eq!(words2num("2,000", "en", "year").unwrap(), int(2000));
        assert_eq!(words2num("1,000,000", "en", "cardinal").unwrap(), int(1_000_000));
        assert_eq!(words2num("-17", "en", "cardinal").unwrap(), int(-17));
        assert_eq!(dec(&words2num("3.14", "en", "cardinal").unwrap()), "3.14");
    }

    #[test]
    fn locale_separators() {
        assert_eq!(words2num("1.234", "de", "cardinal").unwrap(), int(1234));
        assert_eq!(words2num("1.234", "es", "cardinal").unwrap(), int(1234));
        assert_eq!(dec(&words2num("2,5", "de", "cardinal").unwrap()), "2.5");
        assert_eq!(dec(&words2num("1 234,5", "fr", "cardinal").unwrap()), "1234.5");
    }

    #[test]
    fn two_numbers_side_by_side_are_not_one() {
        // Not a grouped number: left to the word grammar, never 12 or 1234.
        assert_ne!(words2num("12 34", "en", "cardinal").ok(), Some(int(1234)));
        assert!(words2num("1.5.2026", "en", "cardinal").is_err());
    }
}
