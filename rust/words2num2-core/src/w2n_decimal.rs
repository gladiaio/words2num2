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
        // ar: the renderer writes "،" (#371), which normalizing drops.
        "ar" => &["فاصلة"],
        _ => &[],
    }
}

/// The separator mark ar num2words writes between the integer and the
/// hundredths, as a word of its own: "ثلاثة ، أربع عشرة" is 3.14, "، خمسون"
/// 0.50 (#371). Normalizing deletes it, so it is looked for in the text as
/// written.
fn hundredths_mark(lang: &str) -> Option<&'static str> {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    (base == "ar").then_some("\u{060C}")
}

/// The words the renderer writes around the mark that the cardinal table does
/// not read, normalized -> value, taken from its own renderings.
#[derive(Default)]
struct MarkForms {
    /// The hundredths after the mark: ar writes them feminine ("أربع عشرة"
    /// 14, "خمس" 5), the cardinal table holds the masculine forms.
    hundredths: HashMap<String, u32>,
    /// The integer before the mark as the renderer writes it, for 1..999 and
    /// k·10^e: the ar dual construct "ألفا" (2000), "مليونا" (2·10^6), not
    /// "ألفان", and the scales the cardinal reader does not read.
    integers: HashMap<String, i64>,
}

fn mark_forms(lang: &str) -> &'static MarkForms {
    static CACHE: OnceLock<RwLock<HashMap<String, &'static MarkForms>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(t) = cache.read().ok().and_then(|m| m.get(lang).copied()) {
        return t;
    }
    let mut forms = MarkForms::default();
    if let (Some(r), Some(mark)) = (num2words2_core::get_lang_by_key(lang), hundredths_mark(lang)) {
        let render = |s: &str| {
            let value = BigDecimal::from_str(s).expect("literal");
            r.to_cardinal_float(&FloatValue::Decimal { value, precision: 2 }, None).ok()
        };
        for n in 1..100u32 {
            let Some(text) = render(&format!("0.{:02}", n)) else { continue };
            if let Some(words) = text.trim().strip_prefix(mark) {
                forms.hundredths.entry(crate::normalize(words)).or_insert(n);
            }
        }
        let mut clashes = std::collections::HashSet::new();
        let scales = (0..=18).map(|e| 10i64.pow(e));
        let ints = (1..1000).chain(scales.flat_map(|p| (1..=10).filter_map(move |k| p.checked_mul(k))));
        for v in ints {
            let Some(text) = render(&format!("{v}.5")) else { continue };
            let Some((before, _)) = text.split_once(mark) else { continue };
            // Every form, not only those that differ from the cardinal: the
            // cardinal reader does not read "كوينتليونا" (2·10^18) at all.
            let before = crate::normalize(before);
            match forms.integers.get(&before) {
                Some(&w) if w != v => {
                    clashes.insert(before);
                }
                _ => {
                    forms.integers.insert(before, v);
                }
            }
        }
        // Two values written alike are neither (raise beats a wrong value).
        for c in clashes {
            forms.integers.remove(&c);
        }
    }
    let built: &'static MarkForms = Box::leak(Box::new(forms));
    if let Ok(mut m) = cache.write() {
        m.entry(lang.to_string()).or_insert(built);
    }
    built
}

/// Whether `text` holds the language's written decimal mark as a word of its
/// own, which a cardinal read would silently drop ("مائة ، خمسون" is 100.50,
/// not 150).
pub fn has_written_mark(lang: &str, text: &str) -> bool {
    let Some(m) = hundredths_mark(lang) else { return false };
    mark_words(text, m).iter().any(|w| w == m)
}

/// The words of `text` with the mark as a word of its own however it is
/// typed — glued ("مائة، خمسون", "مائة،خمسون") or, as classic num2words wrote
/// it, an ASCII comma standing alone ("مائة , خمسون"; #616, #658) — and
/// trailing marks dropped, as punctuation ("ثلاثة ،" is 3).
fn mark_words(text: &str, mark: &str) -> Vec<String> {
    // An ASCII comma between two letters is the mark too ("مائة,خمسون");
    // between digits it groups ("1,000"), and stays.
    let chars: Vec<char> = text.chars().collect();
    let ascii: String = chars
        .iter()
        .enumerate()
        .map(|(k, &c)| {
            let letter = |d: Option<&char>| d.is_some_and(|c| c.is_alphabetic());
            let before = chars[..k].iter().rev().find(|c| !c.is_whitespace());
            let after = chars[k + 1..].iter().find(|c| !c.is_whitespace());
            if c == ',' && (letter(before) || before.is_none()) && letter(after) {
                mark.to_string()
            } else {
                c.to_string()
            }
        })
        .collect();
    let spaced = ascii.replace(mark, &format!(" {mark} "));
    let mut words: Vec<String> = spaced.split_whitespace().map(str::to_string).collect();
    while words.last().is_some_and(|w| w == mark) {
        words.pop();
    }
    words
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
    // "moins zéro virgule cinq" is -0.5: the sign is read apart, so a zero
    // integer keeps it (#364). One sign, no recursion: "moins moins …" is
    // no decimal, and a long run of signs is refused in one step.
    if let Some(rest) = split_sign(lang, text) {
        if rest.is_empty() || split_sign(lang, &rest).is_some() {
            return None;
        }
        let (int, sep, frac) = read_unsigned(lang, &rest, hand_sep, default_sep, cardinal)?;
        if int.starts_with('-') {
            return None;
        }
        return Some((format!("-{int}"), sep, frac));
    }
    read_unsigned(lang, text, hand_sep, default_sep, cardinal)
}

/// The text after a leading sign, normalized: a dash of any kind ("-",
/// "‐", "–", "−"), the language's sign words however many ("moins", lg
/// "wansi wa", zu "ngaphansi kwe") or one glued to the number (zh "负零点五",
/// ja "マイナス零点五"; #703). `None` without a sign.
fn split_sign(lang: &str, text: &str) -> Option<String> {
    let t = text.trim_start();
    let dash = |c: char| matches!(c, '-' | '\u{2010}'..='\u{2015}' | '\u{2212}' | '\u{FE58}' | '\u{FE63}' | '\u{FF0D}');
    if let Some(rest) = t.strip_prefix(dash) {
        return Some(rest.trim_start().to_string());
    }
    // The rest stays as written: a hyphenated compound is one fraction digit
    // (wo "juróom-ñaar" is 7).
    let raw: Vec<&str> = t.split_whitespace().collect();
    let norm: Vec<String> = raw.iter().map(|w| crate::normalize(w)).collect();
    let first = norm.first()?;
    for p in crate::sign_prefixes(lang).iter() {
        let words: Vec<&str> = p.text.split_whitespace().collect();
        if words.len() <= norm.len() && norm[..words.len()].iter().zip(&words).all(|(a, b)| a == b) {
            return Some(raw[words.len()..].join(" "));
        }
        if p.glued && words.len() == 1 {
            if let Some(tail) = first.strip_prefix(words[0]).filter(|r| r.chars().next().is_some_and(char::is_alphabetic)) {
                let mut rest = vec![tail.to_string()];
                rest.extend(raw[1..].iter().map(|w| w.to_string()));
                return Some(rest.join(" "));
            }
        }
    }
    None
}

/// [`read`] once any sign is off.
fn read_unsigned(
    lang: &str,
    text: &str,
    hand_sep: &dyn Fn(&str) -> Option<char>,
    default_sep: char,
    cardinal: &dyn Fn(&str) -> Option<BigInt>,
) -> Option<(String, char, String)> {
    let f = forms(lang);
    // 0. ar "<int> ، <hundredths>": the fraction is a count of hundredths.
    if let Some(mark) = hundredths_mark(lang).filter(|_| has_written_mark(lang, text)) {
        // Figures with the mark as their decimal point: "٣،٥" is 3.5, as
        // "3,5" is, never 3.05 (the renderer writes no figures).
        // The sign is off already (see `read`).
        if let Some((a, b)) = text.trim().split_once(mark) {
            let digits = |s: &str| -> Option<String> {
                (!s.is_empty())
                    .then(|| s.chars().map(|c| crate::w2n_sentence::digit_value(c).map(|d| char::from(b'0' + d as u8))).collect())
                    .flatten()
            };
            if let (Some(a), Some(b)) = (digits(a), digits(b)) {
                let int = a.trim_start_matches('0');
                let int = if int.is_empty() { "0" } else { int };
                return Some((int.to_string(), default_sep, b));
            }
        }
        let words = mark_words(text, mark);
        // Two marks are no decimal, whichever comma each is: "مائة ، , خمسون".
        if words.iter().any(|w| w == ",") {
            return None;
        }
        // Two marks are no decimal: "مائة ، ، خمسون" (#616).
        if words.iter().filter(|w| *w == mark).count() > 1 {
            return None;
        }
        let pos = words.iter().position(|t| t == mark)?;
        let forms = mark_forms(lang);
        let int = if pos == 0 {
            BigInt::from(0)
        } else {
            let head = words[..pos].join(" ");
            match forms.integers.get(&crate::normalize(&head)) {
                Some(&v) => BigInt::from(v),
                None => cardinal(&head).filter(|i| *i >= BigInt::from(0))?,
            }
        };
        // Only the hundredths the renderer writes after the mark: "ثلاثة ،
        // أربعة" is a list, not 3.04 (#617).
        let rest = words[pos + 1..].join(" ");
        // A figure is no masculine word: "ثلاثة ، 50" is 3.50.
        let figure = || {
            let digits: Option<String> =
                rest.chars().map(|c| crate::w2n_sentence::digit_value(c).map(|d| char::from(b'0' + d as u8))).collect();
            digits.filter(|d| (1..=2).contains(&d.len())).and_then(|d| d.parse::<u32>().ok())
        };
        let n = forms.hundredths.get(&crate::normalize(&rest)).copied().or_else(figure)?;
        return Some((int.to_string(), default_sep, format!("{:0>2}", n)));
    }
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

    /// #364: a negative decimal below one keeps its sign.
    #[test]
    fn negative_zero_integer() {
        assert_eq!(card("moins zéro virgule cinq", "fr"), "-0.5");
        assert_eq!(card("minus null Komma fünf", "de"), "-0.5");
        assert_eq!(card("menos cero coma cinco", "es"), "-0.5");
        assert_eq!(card("минус ноль целых пять десятых", "ru"), "-0.5");
        assert_eq!(card("minus zero point five", "en_IN"), "-0.5");
        assert_eq!(card("moins trois virgule cinq", "fr"), "-3.5");
        assert!(crate::words2num("moins moins trois virgule cinq", "fr", "cardinal").is_err());
        // #703: a long run of signs is refused in one step, no recursion.
        let many = "moins ".repeat(100_000) + "zéro virgule cinq";
        assert!(crate::words2num(&many, "fr", "cardinal").is_err());
        let dashes = "-".repeat(100_000) + "zéro virgule cinq";
        assert!(crate::words2num(&dashes, "fr", "cardinal").is_err());
        // Glued, multi-word and dash-variant signs.
        assert_eq!(card("负零点五", "zh"), "-0.5");
        assert_eq!(card("マイナス零点五", "ja"), "-0.5");
        assert_eq!(card("\u{2010}zéro virgule cinq", "fr"), "-0.5");
        assert_eq!(card("\u{2013}zéro virgule cinq", "fr"), "-0.5");
        // The rest stays as written: one hyphenated fraction digit.
        assert_eq!(card("minus fukk ñaar point juróom-ñaar juróom", "wo"), "-12.75");
        // ar: one sign rule for the "،" decimals too.
        assert_eq!(card("سالب ٣،٥", "ar"), "-3.5");
        assert!(crate::words2num("سالب سالب صفر ، خمسون", "ar", "cardinal").is_err());
        assert_eq!(card("سالب ، خمسون", "ar"), "-0.50");
        assert_eq!(card("-ثلاثة ، خمسون", "ar"), "-3.50");
    }

    /// #371: ar "،" (the renderer's separator, hundredths) and "فاصلة".
    #[test]
    fn arabic_decimals() {
        assert_eq!(card("ثلاثة ، أربع عشرة", "ar"), "3.14");
        assert_eq!(card("ثلاثة ، خمس", "ar"), "3.05");
        assert_eq!(card("مائة ، خمسون", "ar"), "100.50");
        assert_eq!(card("، خمسون", "ar"), "0.50");
        assert_eq!(card("مليون ، خمس وسبعون", "ar"), "1000000.75");
        assert_eq!(card("ثلاثة فاصلة أربعة عشر", "ar"), "3.14");
        assert_eq!(card("ثلاثة فاصلة واحد أربعة", "ar"), "3.14");
        // A mark with no hundredths after it is no number.
        assert!(crate::words2num("مائة ، خمسمائة", "ar", "cardinal").is_err());
        // The sign is read apart, so -0.50 keeps it.
        assert_eq!(card("سالب ، خمسون", "ar"), "-0.50");
        assert_eq!(card("سالب ثلاثة ، خمسون", "ar"), "-3.50");
        // The dual construct before the mark.
        assert_eq!(card("ألفا ، خمسون", "ar"), "2000.50");
        assert_eq!(card("مليونا ، خمسون", "ar"), "2000000.50");
        assert_eq!(card("مليارا ، خمسون", "ar"), "2000000000.50");
        assert_eq!(card("تريليونا ، خمسون", "ar"), "2000000000000.50");
        assert_eq!(card("كوادريليونا ، خمسون", "ar"), "2000000000000000.50");
        assert_eq!(card("كوينتليونا ، خمسون", "ar"), "2000000000000000000.50");
        assert_eq!(card("مئتا ، خمسون", "ar"), "200.50");
        // A trailing mark is punctuation.
        assert_eq!(crate::words2num("ثلاثة ،", "ar", "cardinal").unwrap(), W2nValue::Int(3.into()));
        assert_eq!(crate::words2num("مائة ،", "ar", "cardinal").unwrap(), W2nValue::Int(100.into()));
        assert_eq!(crate::words2num("ثلاثة ، ،", "ar", "cardinal").unwrap(), W2nValue::Int(3.into()));
        // #617: only the renderer's hundredths follow the mark; a list stays one.
        for list in ["ثلاثة ، أربعة", "، ثلاثة", "ثلاثة ، ثلاثة", "ثلاثة ، خمسة"] {
            assert!(crate::words2num(list, "ar", "cardinal").is_err(), "{list}");
        }
        // #616: a glued mark; a doubled one is no decimal.
        assert_eq!(card("مائة، خمسون", "ar"), "100.50");
        assert_eq!(card("مائة ،خمسون", "ar"), "100.50");
        assert_eq!(card("مائة،خمسون", "ar"), "100.50");
        assert!(crate::words2num("مائة ، ، خمسون", "ar", "cardinal").is_err());
        assert!(crate::words2num("، ، خمسون", "ar", "cardinal").is_err());
        // #658: classic num2words' free-standing ASCII comma.
        assert_eq!(card("مائة , خمسون", "ar"), "100.50");
        assert_eq!(card(", خمسون", "ar"), "0.50");
        assert_eq!(card("سالب , خمسون", "ar"), "-0.50");
        // A glued ASCII comma between words; a figure after the mark.
        assert_eq!(card("مائة, خمسون", "ar"), "100.50");
        assert_eq!(card("مائة,خمسون", "ar"), "100.50");
        assert_eq!(card("مائة ,خمسون", "ar"), "100.50");
        assert_eq!(card("ثلاثة ، 50", "ar"), "3.50");
        assert_eq!(card("3 ، 50", "ar"), "3.50");
        // Figures glued to the mark: its decimal point, as "3,5" is.
        assert_eq!(card("٣،٥", "ar"), "3.5");
        assert_eq!(card("3،5", "ar"), "3.5");
        assert_eq!(card("12،5", "ar"), "12.5");
        assert_eq!(card("٣،٥٠", "ar"), "3.50");
        assert_eq!(card("3،05", "ar"), "3.05");
        assert!(crate::words2num("مائة ، , خمسون", "ar", "cardinal").is_err());
        assert!(crate::words2num("مائة , ، خمسون", "ar", "cardinal").is_err());
        assert!(crate::words2num("ثلاثة , أربعة", "ar", "cardinal").is_err());
        // to="digits" keeps the commas of a dictated string.
        for (t, v) in [("خمسة ، ستة ، سبعة", 567), ("خمسة، ستة، سبعة", 567), ("خمسة , ستة", 56)] {
            assert_eq!(crate::words2num(t, "ar", "digits").unwrap(), W2nValue::Int(v.into()), "{t}");
        }
        // #619: a glued minus.
        assert_eq!(card("-ثلاثة ، خمسون", "ar"), "-3.50");
        // #620: other modes never drop the mark.
        assert_eq!(
            format!("{:?}", crate::words2num("مائة ، خمسون", "ar", "currency").unwrap()),
            format!("{:?}", crate::words2num("مائة ، خمسون", "ar", "cardinal").unwrap())
        );
        assert!(crate::words2num("مائة ، خمسون", "ar", "year").is_err());
        assert!(crate::words2num("مائة ، خمسون", "ar", "ordinal").is_err());
    }
}
