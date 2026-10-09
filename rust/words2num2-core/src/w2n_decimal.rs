//! Spoken decimals, read the way the renderer writes them.
//!
//! num2words writes a decimal in one of three shapes, and each language's
//! shape is derived here from its own `to_cardinal_float` output rather than
//! from a hand list of separator words:
//!
//! * **a separator word** between the integer and the fraction digits — fr
//!   "trois virgule un quatre", sv "tre komma ett fyra", hi "तीन दशमलव एक
//!   चार", ko "삼 점 일 사", sk "tri celých štrnásť" (the fraction as one
//!   number). The separator is the part of the 3.5 rendering between
//!   "three" and "five".
//! * **a whole-word and a denominator** — ru "три целых четырнадцать сотых",
//!   hu "három egész tizennégy század", fa "سه و چهارده صدم", mn "гурав,
//!   зууны арван дөрөв": the value is int + n / 10^k, with the words for each
//!   k taken from the 3.5 / 3.14 / 3.125 renderings.
//! * **a separator glued inside one word** — ja "三点一四", th "สามจุดหนึ่งสี่",
//!   tr "üçvirgülondört". Only a whole-text read (`words2num`) can split it:
//!   the sentence walker never opens a run on a word that is not a number.
//!
//! A derived word is only trusted when no integer rendering of the language
//! contains it (and, for a separator, when it is not a number word itself),
//! so an integer is never read as a decimal.

use bigdecimal::BigDecimal;
use num2words2_core::floatpath::FloatValue;
use num_bigint::BigInt;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{OnceLock, RwLock};

/// How one language writes a decimal (all words normalized).
#[derive(Debug, Default)]
pub struct DecimalForms {
    /// Separator words between the integer and the fraction digits.
    pub pointwords: Vec<String>,
    /// Separators written inside one word (ja "点", tr "virgul").
    pub glued: Vec<String>,
    /// `(whole-words, denominator-words, k)`: `<int> <whole> <n> <denom>` is
    /// int + n / 10^k.
    pub fractions: Vec<(Vec<String>, Vec<String>, u32)>,
    /// The separator and whole-words as the walker sees them — lowercased
    /// but not normalized ("čárka", "점", "egész"), for its includable list.
    pub run_words: Vec<String>,
}

/// Separator words num2words does not write but the language does: the
/// standard spelling where the renderer falls back to English "point" (gl,
/// mk, nn, fo, lb, oc, mt; #267).
fn native_pointwords(base: &str) -> &'static [&'static str] {
    match base {
        "gl" => &["coma", "punto"],
        "mk" => &["запирка"],
        "nn" | "fo" | "lb" => &["komma"],
        "oc" => &["virgula"],
        "mt" => &["punt"],
        _ => &[],
    }
}

/// The derived forms of renderer key `lang`, built once and cached.
pub fn forms(lang: &str) -> &'static DecimalForms {
    static CACHE: OnceLock<RwLock<HashMap<String, &'static DecimalForms>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(f) = cache.read().ok().and_then(|m| m.get(lang).copied()) {
        return f;
    }
    let built: &'static DecimalForms = Box::leak(Box::new(derive(lang)));
    if let Ok(mut m) = cache.write() {
        m.entry(lang.to_string()).or_insert(built);
    }
    built
}

/// The words a decimal adds inside a number run, for the walker's includable
/// list: the separators and the whole-words (never a denominator, which ends
/// the run).
pub fn run_words(lang: &str) -> Vec<&'static str> {
    let f = forms(lang);
    let mut out: Vec<&'static str> = f.pointwords.iter().map(String::as_str).collect();
    let wholes = f.fractions.iter().flat_map(|(whole, _, _)| whole.iter());
    for w in wholes.chain(f.run_words.iter()) {
        if !out.contains(&w.as_str()) {
            out.push(w.as_str());
        }
    }
    out
}

fn derive(lang: &str) -> DecimalForms {
    let mut forms = derive_with(lang, &|s: &str| crate::normalize(s));
    // The same derivation on the text as the walker's tokens read it.
    let raw = derive_with(lang, &|s: &str| {
        s.to_lowercase().replace(',', " ").split_whitespace().collect::<Vec<_>>().join(" ")
    });
    let wholes = raw.fractions.into_iter().flat_map(|(whole, _, _)| whole);
    forms.run_words = raw.pointwords.into_iter().chain(wholes).collect();
    forms
}

fn derive_with(lang: &str, nz: &dyn Fn(&str) -> String) -> DecimalForms {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    let mut forms = DecimalForms::default();
    for w in native_pointwords(base) {
        forms.pointwords.push(nz(w));
    }
    let Some(r) = num2words2_core::get_lang_by_key(lang) else {
        return forms;
    };
    let card = |n: i64| r.to_cardinal(&BigInt::from(n)).ok().map(|s| nz(&s));
    // Every word, and the whole text, of the integer renderings: a derived
    // word found there could turn an integer into a decimal.
    let mut int_words: std::collections::HashSet<String> = Default::default();
    let mut int_texts: Vec<String> = Vec::new();
    let samples = (0..=1000).chain([1001, 1100, 1234, 2000, 9999, 10000, 12345, 100_000, 1_000_000, 2_000_000]);
    for n in samples {
        if let Some(t) = card(n) {
            int_words.extend(t.split_whitespace().map(str::to_string));
            int_texts.push(t);
        }
    }
    let in_integers = |s: &str| int_texts.iter().any(|t| t.contains(s));
    let is_number = |w: &str| int_words.contains(w);

    // (k, fraction value): 3.5, 3.14, 3.125.
    let mut post_empty_mids: Vec<String> = Vec::new();
    let mut templates: Vec<(String, String, u32, bool)> = Vec::new(); // (mid, post, k, glued)
    for (k, frac) in [(1u32, 5i64), (2, 14), (3, 125)] {
        let value = BigDecimal::from_str(&format!("3.{}", frac)).expect("literal");
        let fv = FloatValue::Decimal { value, precision: k };
        let Some(rendered) = r.to_cardinal_float(&fv, None).ok().map(|s| nz(&s)) else {
            continue;
        };
        let (Some(c3), Some(cf)) = (card(3), card(frac)) else {
            continue;
        };
        let digits: Option<Vec<String>> = frac
            .to_string()
            .chars()
            .map(|d| card(i64::from(d.to_digit(10).unwrap_or(0))))
            .collect();
        let Some(rest) = rendered.strip_prefix(c3.as_str()) else {
            continue;
        };
        let glued = !rest.is_empty() && !rest.starts_with(' ');
        let rest = rest.trim();
        let mut found = None;
        for f in [Some(cf.clone()), digits.map(|d| d.join(" "))].into_iter().flatten() {
            if glued {
                // Glued: the fraction is the end of the same word.
                if let Some(mid) = rest.strip_suffix(f.as_str()) {
                    found = Some((mid.trim().to_string(), String::new()));
                    break;
                }
                continue;
            }
            // Spaced: match whole words, the last occurrence.
            let rt: Vec<&str> = rest.split_whitespace().collect();
            let ft: Vec<&str> = f.split_whitespace().collect();
            if ft.is_empty() || ft.len() > rt.len() {
                continue;
            }
            if let Some(i) = (0..=rt.len() - ft.len()).rev().find(|&i| rt[i..i + ft.len()] == ft[..]) {
                found = Some((rt[..i].join(" "), rt[i + ft.len()..].join(" ")));
                break;
            }
        }
        let Some((mid, post)) = found else { continue };
        if mid.is_empty() {
            continue;
        }
        if post.is_empty() && !glued && !post_empty_mids.contains(&mid) {
            post_empty_mids.push(mid.clone());
        }
        templates.push((mid, post, k, glued));
    }
    // One separator for every precision: a separator word. Several (mn
    // "аравны" tenths, "зууны" hundredths): denominators written first.
    let one_separator = post_empty_mids.len() == 1;
    for (mid, post, k, glued) in templates {
        if glued {
            if post.is_empty() && !in_integers(&mid) && !forms.glued.contains(&mid) {
                forms.glued.push(mid);
            }
            continue;
        }
        if post.is_empty() && one_separator {
            let single = !mid.contains(' ');
            if single && !is_number(&mid) && !in_integers(&mid) && !forms.pointwords.contains(&mid) {
                forms.pointwords.push(mid);
            }
            continue;
        }
        // A fraction template must hold a word no integer has: its
        // denominator, or (when written first) the whole-word itself.
        let marker = if post.is_empty() { &mid } else { &post };
        if in_integers(marker) {
            continue;
        }
        let words = |s: &str| s.split_whitespace().map(str::to_string).collect::<Vec<_>>();
        let t = (words(&mid), words(&post), k);
        if !forms.fractions.contains(&t) {
            forms.fractions.push(t);
        }
    }
    forms
}

/// A decimal spoken in words (`text`, as written): `(int, separator
/// character, fraction digits)`. `cardinal` reads a run of number words.
/// `hand_sep` names the separator words the caller knows with their
/// character (fr "virgule" -> ',', es "punto" -> '.'); derived words write
/// `default_sep`.
pub fn read(
    lang: &str,
    text: &str,
    hand_sep: &dyn Fn(&str) -> Option<char>,
    default_sep: char,
    cardinal: &dyn Fn(&str) -> Option<BigInt>,
) -> Option<(String, char, String)> {
    let f = forms(lang);
    // Each written word normalized on its own: a hyphenated compound stays
    // one fraction digit (wo "juróom-ñeent" is 9, not 5 and 4).
    let units: Vec<String> = text
        .split_whitespace()
        .map(crate::normalize)
        .filter(|u| !u.is_empty())
        .collect();
    let norm = units.join(" ");
    let toks: Vec<&str> = norm.split_whitespace().collect();
    let sep_of = |t: &str| {
        hand_sep(t).or_else(|| f.pointwords.iter().any(|p| p == t).then_some(default_sep))
    };
    // 1. A separator word, then fraction digits.
    if let Some(pos) = units.iter().position(|t| sep_of(t).is_some()) {
        let sep = sep_of(&units[pos])?;
        let int = if pos == 0 { BigInt::from(0) } else { cardinal(&units[..pos].join(" "))? };
        let rest: Vec<&str> = units[pos + 1..].iter().map(String::as_str).collect();
        let frac = fraction_digits(&rest, cardinal)?;
        return Some((int.to_string(), sep, frac));
    }
    // 2. A whole-word and a denominator.
    for (whole, denom, k) in &f.fractions {
        let n = toks.len();
        if n < whole.len() + denom.len() + 2 || toks[n - denom.len()..] != denom[..] {
            continue;
        }
        let body = &toks[..n - denom.len()];
        let whole: Vec<&str> = whole.iter().map(String::as_str).collect();
        let Some(at) = (1..body.len()).find(|&p| body[p..].starts_with(&whole)) else {
            continue;
        };
        let num = &body[at + whole.len()..];
        if num.is_empty() {
            continue;
        }
        let int = cardinal(&body[..at].join(" "))?;
        let num = cardinal(&num.join(" "))?;
        let limit = BigInt::from(10u32).pow(*k);
        if num < BigInt::from(0) || num >= limit {
            return None;
        }
        return Some((int.to_string(), default_sep, format!("{:0>w$}", num, w = *k as usize)));
    }
    // 3. A separator glued inside the word.
    if toks.len() == 1 {
        for g in &f.glued {
            let Some(at) = norm.find(g.as_str()) else { continue };
            let (left, right) = (&norm[..at], &norm[at + g.len()..]);
            if left.is_empty() || right.is_empty() {
                continue;
            }
            let int = cardinal(left)?;
            let frac = match cardinal(right) {
                Some(n) if n >= BigInt::from(0) && n < BigInt::from(1000) => n.to_string(),
                _ => {
                    // ja "一四": one digit per character.
                    let chars: Vec<String> = right.chars().map(String::from).collect();
                    let refs: Vec<&str> = chars.iter().map(String::as_str).collect();
                    fraction_digits(&refs, cardinal)?
                }
            };
            return Some((int.to_string(), default_sep, frac));
        }
    }
    None
}

/// The digits of a spoken fraction: single digit words read one by one
/// ("un quatre" -> "14"), or leading zero words and then one number below
/// 1000 ("zéro vingt-cinq" -> "025", "cent vingt-cinq" -> "125"; #52). A
/// scale word is not part of a fraction: "deux virgule cinq millions" is 2.5
/// million, so the fraction stops before it.
fn fraction_digits(toks: &[&str], cardinal: &dyn Fn(&str) -> Option<BigInt>) -> Option<String> {
    if toks.is_empty() {
        return None;
    }
    let digit = |t: &str| cardinal(t).filter(|d| *d >= BigInt::from(0) && *d <= BigInt::from(9));
    let zeros = toks.iter().take_while(|t| digit(t).is_some_and(|d| d == BigInt::from(0))).count();
    let rest = &toks[zeros..];
    let mut out = "0".repeat(zeros);
    if rest.is_empty() {
        return Some(out);
    }
    let singles: Option<Vec<BigInt>> = rest.iter().map(|t| digit(t)).collect();
    match singles {
        Some(ds) => ds.iter().for_each(|d| out.push_str(&d.to_string())),
        None => {
            let n = cardinal(&rest.join(" "))
                .filter(|n| *n >= BigInt::from(0) && *n < BigInt::from(1000))?;
            out.push_str(&n.to_string());
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use crate::w2n_lang_en::W2nValue;
    use crate::w2n_sentence::words2num_sentence;

    fn sent(text: &str, lang: &str) -> String {
        words2num_sentence(text, lang, "cardinal", false).unwrap()
    }
    fn card(text: &str, lang: &str) -> String {
        match crate::words2num(text, lang, "cardinal").unwrap() {
            W2nValue::Dec(d) => d.to_string(),
            other => format!("{other:?}"),
        }
    }

    /// #52: a fraction of leading zeros and a number of any size.
    #[test]
    fn fraction_with_zeros_or_hundreds() {
        assert_eq!(sent("trois virgule zéro vingt-cinq", "fr"), "3,025");
        assert_eq!(sent("trois virgule cent vingt-cinq", "fr"), "3,125");
        assert_eq!(sent("deux virgule cinquante", "fr"), "2,50");
        assert_eq!(sent("trois virgule un quatre", "fr"), "3,14");
    }

    /// #107: the separator word comes from the renderer, for every language.
    #[test]
    fn derived_separator_words() {
        assert_eq!(sent("tre komma ett fyra", "sv"), "3,14");
        assert_eq!(sent("tři čárka jedna čtyři", "cs"), "3,14");
        assert_eq!(sent("तीन दशमलव एक चार", "hi"), "3.14");
        assert_eq!(sent("삼 점 일 사", "ko"), "3.14");
        assert_eq!(sent("three point one four", "en_IN"), "3.14");
        assert_eq!(sent("tree decimal wun fower", "en_AERO"), "3.14");
        assert_eq!(sent("ba phẩy mười bốn", "vi"), "3,14");
        // A lone separator word stays a word.
        assert_eq!(sent("이십 점 받았다", "ko"), "20 점 받았다");
        // Glued separators: read by words2num.
        assert_eq!(card("三点一四", "ja"), "3.14");
        assert_eq!(card("üçvirgülondört", "tr"), "3.14");
    }

    /// #108: whole-word + denominator decimals.
    #[test]
    fn whole_and_denominator() {
        assert_eq!(sent("три целых четырнадцать сотых", "ru"), "3,14");
        assert_eq!(sent("három egész tizennégy század", "hu"), "3,14");
        assert_eq!(sent("سه و چهارده صدم", "fa"), "3.14");
        assert_eq!(sent("tri celých štrnásť", "sk"), "3,14");
        assert_eq!(sent("үш бүтін он төрт", "kz"), "3.14");
        assert_eq!(card("три целых пять сотых", "ru"), "3.05");
        assert_eq!(card("гурав, зууны арван дөрөв", "mn"), "3.14");
        assert_eq!(card("гурав, зууны тав", "mn"), "3.05");
    }

    /// #191: words2num reads a decimal in every language, not only en.
    #[test]
    fn words2num_reads_decimals() {
        assert_eq!(card("trois virgule un quatre", "fr"), "3.14");
        assert_eq!(card("tres punto uno cuatro", "es"), "3.14");
        assert_eq!(card("drei Komma eins vier", "de"), "3.14");
        assert_eq!(card("tre komma ett fyra", "sv"), "3.14");
        // A hyphenated compound is one fraction digit (wo 9 = 5 + 4).
        assert_eq!(card("juróom-ñeent point juróom-ñeent juróom-ñeent", "wo"), "9.99");
    }

    /// #267: the standard separator where the renderer writes "point".
    #[test]
    fn native_separator_spellings() {
        assert_eq!(sent("tres coma un catro", "gl"), "3,14");
        assert_eq!(sent("три запирка еден четири", "mk"), "3.14");
        assert_eq!(sent("tre komma ein fire", "nn"), "3,14");
    }
}
