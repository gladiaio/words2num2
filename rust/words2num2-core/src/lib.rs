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

pub mod w2n_currency;
pub mod w2n_formats;
pub mod w2n_lang_en;
pub mod w2n_sentence;

/// The public single-token entry point (`words2num2.words2num`), re-exported at
/// the crate root. Its dispatch — `_resolve_lang`, the en-vs-reverse-table
/// choice, and the `to` mode selection — lives in [`w2n_sentence`].
pub use w2n_sentence::words2num;

/// Python's `Words2Num_Base.LOOKUP_RANGE`.
const LOOKUP_LO: i64 = -1;
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
pub fn normalize_tail(decomposed: &str) -> String {
    let lowered = decomposed.to_lowercase().replace('_', " ");
    let chars: Vec<char> = lowered.chars().collect();
    let mut out = String::with_capacity(lowered.len());

    for (i, &c) in chars.iter().enumerate() {
        match c {
            // (?<=[a-z])-(?=[a-z]) — a hyphen joining two words becomes a
            // space, but a hyphen before a digit is a sign and must survive.
            '-' => {
                let prev_alpha = i > 0 && chars[i - 1].is_ascii_lowercase();
                let next_alpha = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
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
    // \s+ -> " ", then strip.
    out.split_whitespace().collect::<Vec<_>>().join(" ")
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
pub fn normalize(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    if s.is_ascii() {
        return normalize_tail(s); // NFKD and the combining-mark filter are no-ops on ASCII
    }
    let nfkd: String = s.nfkd().collect();
    let stripped: String = nfkd
        .chars()
        .filter(|&c| !is_optional_diacritic(c))
        .collect();
    normalize_tail(&stripped)
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
/// The per-(lang, kind) cache of reverse tables.
type TableCache = RwLock<HashMap<(String, bool), ReverseTable>>;

/// The reverse tables, built lazily per (lang, kind) exactly as Python does.
fn tables() -> &'static TableCache {
    static T: OnceLock<TableCache> = OnceLock::new();
    T.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Port of `Words2Num_Base._build_table`.
///
/// Python calls `num2words(n, lang, to=kind)` for every n in LOOKUP_RANGE and
/// does `table.setdefault(key, n)` — **first write wins**, so the canonical
/// short form takes precedence over later spellings. That ordering is
/// load-bearing; do not switch to insert-overwrite.
fn build_table(lang: &str, ordinal: bool) -> Result<HashMap<String, i64>, LookupError> {
    let l = num2words2_core::get_lang_by_key(lang)
        .ok_or_else(|| LookupError::NotImplemented(lang.to_string()))?;
    let mut table = HashMap::new();
    for n in LOOKUP_LO..LOOKUP_HI {
        let v = BigInt::from(n);
        let words = if ordinal { l.to_ordinal(&v) } else { l.to_cardinal(&v) };
        let Ok(words) = words else { continue }; // Python swallows every raise
        let key = normalize(&words);
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

    let key = (lang.to_string(), ordinal);
    {
        let t = tables().read().unwrap();
        if let Some(tab) = t.get(&key) {
            return Ok(table_get(tab, lang, normalized, ordinal).map(|v| sign * v));
        }
    }
    let built = build_table(lang, ordinal)?;
    let got = table_get(&built, lang, normalized, ordinal).map(|v| sign * v);
    tables().write().unwrap().insert(key, built);
    Ok(got)
}

/// A table hit for `normalized`, or for one of its spoken variants: the fr
/// plural -s on a cardinal, the gender / declension / apocope of an ordinal.
fn table_get(
    tab: &HashMap<String, i64>,
    lang: &str,
    normalized: &str,
    ordinal: bool,
) -> Option<i64> {
    if let Some(v) = tab.get(normalized) {
        return Some(*v);
    }
    let variants = if ordinal {
        ordinal_variants(lang, normalized)
    } else {
        fr_plural_variants(lang, normalized)
    };
    for variant in variants {
        if let Some(v) = tab.get(&variant) {
            return Some(*v);
        }
    }
    None
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
}

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
    scales_of(lang).words.clone()
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

/// Derive [`Scales`] by rendering round numbers (1, 2 and 5 × 10^3..10^9) and
/// reading the magnitude off each rendering instead of assuming it: in
/// "दस लाख" (10^6) the multiplier "दस" is 10, so "लाख" is 10^5, not 10^6. A
/// token that is itself a table number ("bat" = 1, "zero", "biyu" = 2) is a
/// multiplier, never a scale word. The magnitude must be a power of ten ≥ 1000.
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
    let is_word = |t: &str| {
        t.len() >= 3
            && !t.chars().any(|c| c.is_ascii_digit())
            && lookup_plain(lang, t).is_none_or(|v| v >= 1000 && is_power_of_ten(v))
    };
    fn add(out: &mut Vec<(String, i64)>, w: &str, mag: i64) {
        if !out.iter().any(|(x, _)| x == w) {
            out.push((w.to_string(), mag));
        }
    }
    // A single token = table multiplier + scale morpheme (km "ពីរពាន់" = 2 ·
    // "ពាន់"). Longest multiplier first, so the morpheme is the shortest tail.
    let glued = |tok: &str, n: i64| -> Option<(String, i64)> {
        let cuts: Vec<usize> = tok.char_indices().map(|(i, _)| i).filter(|&i| i > 0).collect();
        cuts.into_iter().rev().find_map(|i| {
            let (a, b) = tok.split_at(i);
            let m = lookup_plain(lang, a).filter(|m| (1..1000).contains(m))?;
            (n % m == 0 && is_power_of_ten(n / m) && n / m >= 1000 && is_word(b))
                .then(|| (b.to_string(), n / m))
        })
    };
    // Multipliers the table cannot read (ru "две тысячи", de "eine Million"):
    // settled once every readable sample has fixed the magnitudes.
    let mut unread: Vec<(String, u32, i64, String)> = Vec::new();
    let mut glued_words: Vec<String> = Vec::new();
    for p in [3u32, 4, 5, 6, 7, 9] {
        for k in [1i64, 2, 5] {
            let n = k * 10i64.pow(p);
            let Some(text) = render(n) else { continue };
            if text.chars().any(|c| c.is_ascii_digit()) {
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
                        add(&mut sc.words, w, n / m);
                        if after && m == 1 {
                            if !sc.one_after.iter().any(|(x, _)| x == w) {
                                sc.one_after.push((w.to_string(), right.clone()));
                            }
                        } else if after && !sc.postfix.iter().any(|x| x == w) {
                            sc.postfix.push(w.to_string());
                        }
                    }
                    None if !after => unread.push((w.to_string(), p, n, left.clone())),
                    _ => {}
                }
            }
        }
    }
    // An unread multiplier keeps the probed power as the magnitude — but only for
    // 10^3/10^6/10^9, where the scale word is that power (he "עשרת אלפים" is
    // 10 · 10^3, not 10^4).
    for (w, p, _, _) in &unread {
        if [3, 6, 9].contains(p) {
            add(&mut sc.words, w, 10i64.pow(*p));
        }
    }
    // The unread prefix of exactly one scale is "one": ta "ஒரு இலட்சம்" = 10^5.
    for (w, _, n, one) in unread {
        if !one.contains(' ')
            && sc.words.iter().any(|(x, mag)| *x == w && *mag == n)
            && !sc.one_forms.contains(&one)
        {
            sc.one_forms.push(one);
        }
    }
    // sv "ettusen" glues "ett" + "tusen" on a shared t, leaving "usen": drop a
    // glued morpheme that is the tail of a longer one of the same magnitude.
    let words = sc.words.clone();
    sc.words.retain(|(w, mag)| {
        !(glued_words.contains(w)
            && words.iter().any(|(x, m)| m == mag && x.len() > w.len() && x.ends_with(w.as_str())))
    });
    // Plus grande magnitude d'abord : « million » se scinde avant « mille ».
    sc.words.sort_by(|a, b| b.1.cmp(&a.1));
    sc.digit_by_digit = match (render(10), render(1), render(0)) {
        (Some(ten), Some(one), Some(zero)) => ten == format!("{one} {zero}"),
        _ => false,
    };
    sc.additive_groups = render(12000).is_some_and(|text| {
        let toks: Vec<&str> = text.split_whitespace().collect();
        toks.len() >= 2
            && toks
                .iter()
                .map(|t| lookup_plain(lang, t).or_else(|| split_glued_scale(lang, t, &sc.words)))
                .sum::<Option<i64>>()
                == Some(12000)
    });
    sc
}

/// Is `word` (normalized) a scale word of `lang` with magnitude >= `min`? Reads the
/// cache in place — the sentence walker asks this for every run head.
pub fn is_scale_word_of(lang: &str, word: &str, min: i64) -> bool {
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

/// Connector/particle words that may sit between number groups and must be
/// stripped from a fragment's edges before lookup: es « y », pt/it « e »,
/// ro « de » (« nouă **de** mii »), etc. Keyed by language prefix.
fn connector_words(lang: &str) -> &'static [&'static str] {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    match base {
        "es" | "gl" => &["y", "e"],
        "pt" | "it" => &["e"],
        "fr" => &["et"],
        "ca" => &["i"],
        "de" => &["und"],
        "nl" | "af" => &["en"],
        "ro" => &["si", "și", "de"],
        "pl" => &["i"],
        "en" => &["and"],
        "ff" => &["e"], // "ujunere sappo e ɗiɗi" = 12000
        "sw" => &["na"], // "elfu kumi na mbili" = 12000
        _ => &[],
    }
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
    if !is_supported_lang(lang) {
        return None;
    }
    parse_scaled_inner(lang, norm, &scales_of(lang), connector_words(lang))
}

/// The article that is also "one" in front of a scale word, for languages whose table
/// does not hold it as a number: de "eine Million", nl "een miljoen".
fn article_one(lang: &str, word: &str) -> bool {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    matches!((base, word), ("de", "ein" | "eine") | ("nl" | "af", "een"))
}

/// A scale word glued into one token, as de / nl / it write it:
/// "dreiundzwanzigtausend" -> (23, 1000, 0), "tweehonderdduizend" -> (200, 1000, 0),
/// "ventitremila" -> (23, 1000, 0). `None` when the token holds no scale morpheme.
fn split_glued_scale(lang: &str, tok: &str, scales: &[(String, i64)]) -> Option<i64> {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    let mut words: Vec<(String, i64)> = scales.to_vec();
    // The thousand morpheme of the agglutinative languages: `scale_words` derives its
    // entries from whole renderings ("zweitausend"), never from the bare morpheme.
    let thousand = match base {
        "de" => Some("tausend"),
        "nl" => Some("duizend"),
        "af" => Some("duisend"),
        "it" => Some("mila"), // "duemila", "ventitremila"
        "sv" | "no" | "nb" | "nn" => Some("tusen"),
        "da" => Some("tusind"),
        _ => None,
    };
    if let Some(t) = thousand {
        if tok == t {
            return Some(1_000); // de "tausend Euro": the bare morpheme is 1000
        }
        words.push((t.to_string(), 1_000));
    }
    for (w, mag) in &words {
        let Some(pos) = tok.find(w.as_str()) else {
            continue;
        };
        if pos == 0 || *mag < 1_000 {
            continue; // the scale word itself, or a hundred morpheme (parse_year's job)
        }
        let pre = &tok[..pos];
        let suf = &tok[pos + w.len()..];
        // de "hunderttausend": the bare hundred morpheme is 100 (num2words writes "einhundert").
        let high = lookup_plain(lang, pre)
            .or_else(|| hundred_word(lang).filter(|h| h == pre).map(|_| 100))?;
        let low = if suf.is_empty() {
            0
        } else {
            lookup_plain(lang, suf)?
        };
        if (1..1000).contains(&high) && (0..*mag).contains(&low) {
            return Some(high * mag + low);
        }
    }
    None
}

/// en_AERO "wun zero zero zero zero zero" = 100000: every token one digit.
fn digit_run(lang: &str, toks: &[&str]) -> Option<i64> {
    if toks.len() < 2 || toks.len() > 18 {
        return None;
    }
    toks.iter().try_fold(0i64, |acc, t| {
        lookup_plain(lang, t)
            .filter(|d| (0..10).contains(d))
            .map(|d| acc * 10 + d)
    })
}

/// km "មួយម៉ឺន ពីរពាន់ បីរយ សែសិបប្រាំ" = 10000 + 2000 + 300 + 45: each token a
/// group, strictly decreasing in place value so "two three" never sums.
fn additive_groups(lang: &str, toks: &[&str], scales: &[(String, i64)]) -> Option<i64> {
    let mut total = 0i64;
    let mut cap = i64::MAX;
    for t in toks {
        let v = lookup_plain(lang, t).or_else(|| split_glued_scale(lang, t, scales))?;
        if v <= 0 || v >= cap {
            return None;
        }
        total += v;
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
        if let Some(v) = split_glued_scale(lang, text, scales) {
            return Some(v);
        }
    }
    let toks: Vec<&str> = text.split_whitespace().collect();
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
    // Split on the largest scale word present (whole-token match).
    for (word, mag) in scales {
        if let Some(pos) = toks.iter().position(|t| t == word) {
            let left = toks[..pos].join(" ");
            let right = toks[pos + 1..].join(" ");
            let left_trim = trim_connectors(left.trim(), conns);
            let after = &toks[pos + 1..];
            if left_trim.is_empty() {
                // eu "milioi bat ...": the "one" follows the scale word.
                if let Some((_, one)) = sc.one_after.iter().find(|(w, _)| w == word) {
                    let one: Vec<&str> = one.split_whitespace().collect();
                    if after.starts_with(&one) {
                        if let Some(r) = rest(&after[one.len()..], *mag) {
                            return Some(mag + r);
                        }
                    }
                }
                // ha "dubu sha biyu ɗari uku ..." : the longest multiplier that
                // leaves a remainder below the scale.
                if sc.postfix.contains(word) {
                    for j in (1..=after.len()).rev() {
                        let m = trim_connectors(&after[..j].join(" "), conns);
                        let Some(m) = lookup_plain(lang, &m).filter(|m| (1..1000).contains(m)) else {
                            continue;
                        };
                        if let Some(r) = rest(&after[j..], *mag) {
                            return Some(m * mag + r);
                        }
                    }
                }
            }
            // « mille » nu (pas de multiplicateur à gauche) = 1×mille ; de « eine Million ».
            let l = if left_trim.is_empty()
                || article_one(lang, &left_trim)
                || sc.one_forms.contains(&left_trim)
            {
                1
            } else {
                parse_scaled_inner(lang, &left, sc, conns)?
            };
            let r = if trim_connectors(right.trim(), conns).is_empty() {
                0
            } else {
                parse_scaled_inner(lang, &right, sc, conns)?
            };
            return Some(l * mag + r);
        }
    }
    if sc.additive_groups && toks.len() > 1 {
        if let Some(v) = additive_groups(lang, &toks, scales) {
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
    for i in 1..toks.len() {
        let left = trim_connectors(&toks[..i].join(" "), conns);
        let right = trim_connectors(&toks[i..].join(" "), conns);
        if let (Some(l), Some(r)) = (lookup_plain(lang, &left), lookup_plain(lang, &right)) {
            if l >= 100 && l % 100 == 0 && (1..100).contains(&r) {
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
        let mut suf = forms[0].clone();
        for f in &forms[1..] {
            suf = common_suffix(&suf, f);
        }
        // Keep only the trailing alphabetic run (drop any leading unit letters).
        let tail: String = {
            let mut rev: Vec<char> = suf.chars().rev().take_while(|c| c.is_alphabetic()).collect();
            rev.reverse();
            rev.into_iter().collect()
        };
        (tail.chars().count() >= 3).then_some(tail)
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

    // (B) explicit hundred token: LEFT <hundred> RIGHT -> LEFT*100 + RIGHT.
    if let Some(ref h) = hw {
        if let Some(pos) = toks.iter().position(|t| *t == h.as_str()) {
            let left = toks[..pos].join(" ");
            let right = toks[pos + 1..].join(" ");
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
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    let pairs = matches!(base, "de" | "nl" | "af" | "da" | "sv" | "no" | "nb" | "nn" | "is");
    if pairs && toks.len() == 2 {
        if let (Some(a), Some(b)) = (lookup_plain(lang, toks[0]), lookup_plain(lang, toks[1])) {
            if (10..=99).contains(&a) && (0..=99).contains(&b) {
                return Some(a * 100 + b);
            }
        }
    }
    None
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
        assert_eq!(parse_scaled("ff", "ujunere teemerre"), Some(100_000));
        // km myriad and hundred-thousand glued to their multiplier.
        assert_eq!(parse_scaled("km", "មួយម៉ឺន ពីរពាន់ បីរយ សែសិបប្រាំ"), Some(12_345));
        assert_eq!(parse_scaled("km", "មួយសែន"), Some(100_000));
        // en_AERO reads digits one at a time.
        assert_eq!(parse_scaled("en_AERO", "wun zero zero zero zero zero"), Some(100_000));
        assert_eq!(parse_scaled("en_AERO", "wun too tree fower fife"), Some(12_345));
        // Prefix languages never take a postfix multiplier or sum bare groups.
        assert_eq!(parse_scaled("fr", "mille deux cents"), Some(1_200));
        assert_eq!(parse_scaled("es", "mil doscientos"), Some(1_200));
        assert_eq!(parse_scaled("fr", "deux trois"), None);
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
}
