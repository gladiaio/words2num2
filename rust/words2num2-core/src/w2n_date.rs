//! `words2num_sentence(..., date=True)`: dates written the way the language
//! writes them.
//!
//! The walk already turns the numbers of a date into figures ("march 12th
//! 2026", "le 1er avril 2026", "am 12. märz"); this pass rewrites each day +
//! month (+ year) span it left into the written form:
//!
//! | lang | after the walk | written |
//! |---|---|---|
//! | en | march 12th 2026 / the 12th of march | March 12, 2026 / March 12 |
//! | en_GB (AU, NZ, IE, IN, ZA) | the 12th of march 2026 | 12 March 2026 |
//! | fr | le 1er avril 2026 | le 1er avril 2026 |
//! | de | am 12. märz 2026 | am 12. März 2026 |
//! | es | el 1º de mayo de 2026 | el 1 de mayo de 2026 |
//! | pt | 1º de maio de 2026 | 1º de maio de 2026 |
//! | it | il 1 maggio | il 1º maggio |
//! | nl | 12 maart 2026 | 12 maart 2026 |
//!
//! Only a span with a month name next to a day (or a month and a year) is
//! touched; the words around it stay. Other languages pass through unchanged.

/// The language a date is written for, from the code ("en_GB" -> day first).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    EnUs,
    EnGb,
    Fr,
    De,
    Es,
    Pt,
    It,
    Nl,
}

fn style(lang: &str) -> Option<Style> {
    let mut it = lang.split(&['_', '-'][..]);
    let base = it.next().unwrap_or(lang).to_lowercase();
    let region = it.next().unwrap_or("").to_uppercase();
    Some(match base.as_str() {
        "en" if matches!(region.as_str(), "GB" | "UK" | "AU" | "NZ" | "IE" | "IN" | "ZA") => Style::EnGb,
        "en" => Style::EnUs,
        "fr" => Style::Fr,
        "de" => Style::De,
        "es" => Style::Es,
        "pt" => Style::Pt,
        "it" => Style::It,
        "nl" => Style::Nl,
        _ => return None,
    })
}

const TRAILING: &[char] = &[',', ';', ':', '!', '?', '.', ')', '"', '\'', '»'];

/// The token without its trailing punctuation, and that punctuation.
fn split_trail(tok: &str) -> (&str, &str) {
    let core = tok.trim_end_matches(TRAILING);
    (core, &tok[core.len()..])
}

/// A day of the month as the walk writes it: "12", "12th", "1er", "1º",
/// "12." (de). Returns the day and the punctuation after it.
fn day(st: Style, tok: &str) -> Option<(u32, String)> {
    let (core, trail) = split_trail(tok);
    let (digits, trail) = if st == Style::De && trail.starts_with('.') {
        (core, &trail[1..]) // "12." is the German ordinal
    } else {
        (core, trail)
    };
    let digits = match st {
        Style::EnUs | Style::EnGb => ["st", "nd", "rd", "th"]
            .iter()
            .find_map(|s| digits.strip_suffix(s))
            .unwrap_or(digits),
        Style::Fr => ["er", "re", "e"].iter().find_map(|s| digits.strip_suffix(s)).unwrap_or(digits),
        Style::Es | Style::Pt | Style::It => {
            ["º", "ª", "°", "o"].iter().find_map(|s| digits.strip_suffix(s)).unwrap_or(digits)
        }
        Style::De | Style::Nl => digits,
    };
    if digits.is_empty() || digits.len() > 2 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let d: u32 = digits.parse().ok()?;
    (1..=31).contains(&d).then(|| (d, trail.to_string()))
}

/// A month name of the language, as spoken (the walk leaves it in words).
fn month(lang: &str, tok: &str) -> Option<(String, String)> {
    let (core, trail) = split_trail(tok);
    let norm = crate::normalize(core);
    crate::w2n_sentence::months(lang)
        .split_whitespace()
        .any(|m| m == norm)
        .then(|| (core.to_string(), trail.to_string()))
}

/// A year in figures, 1000..=2999 (what a spoken date names).
fn year(tok: &str) -> Option<(String, String)> {
    let (core, trail) = split_trail(tok);
    let ok = core.len() == 4 && core.bytes().all(|b| b.is_ascii_digit()) && matches!(core.as_bytes()[0], b'1' | b'2');
    ok.then(|| (core.to_string(), trail.to_string()))
}

/// en month names that are also everyday words ("may", "march") or
/// abbreviations ("mar", "dec"): a date only with more than a bare number.
fn en_homograph(m: &str) -> bool {
    let m = m.to_lowercase();
    m == "may" || m == "march" || (m.len() <= 4 && !matches!(m.as_str(), "june" | "july"))
}

/// The month as the language writes it: capitalised in en and de.
fn written_month(st: Style, m: &str) -> String {
    let lower = m.to_lowercase();
    if matches!(st, Style::EnUs | Style::EnGb | Style::De) {
        let mut c = lower.chars();
        c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
    } else {
        lower
    }
}

fn written_day(st: Style, d: u32) -> String {
    match (st, d) {
        (Style::Fr, 1) => "1er".to_string(),
        (Style::Pt | Style::It, 1) => "1º".to_string(),
        (Style::De, _) => format!("{d}."),
        _ => d.to_string(),
    }
}

/// One date span: its parts, with the punctuation after its last token.
struct Span {
    day: Option<u32>,
    month: String,
    year: Option<String>,
    trail: String,
    len: usize,
}

fn render(st: Style, s: &Span) -> String {
    let m = written_month(st, &s.month);
    let mut out = match (st, s.day) {
        (_, None) => m,
        (Style::EnUs, Some(d)) => format!("{m} {d}"),
        (Style::Es | Style::Pt, Some(d)) => format!("{} de {m}", written_day(st, d)),
        (_, Some(d)) => format!("{} {m}", written_day(st, d)),
    };
    if let Some(y) = &s.year {
        out.push_str(match (st, s.day) {
            (Style::EnUs, Some(_)) => ", ",
            (Style::Es | Style::Pt, _) => " de ",
            _ => " ",
        });
        out.push_str(y);
    }
    out.push_str(&s.trail);
    out
}

/// A clean token: no punctuation after it, so a span may go on past it.
fn open(trail: &str) -> bool {
    trail.is_empty()
}

/// The date span starting at token `i`, if any.
fn span_at(lang: &str, st: Style, toks: &[&str], i: usize) -> Option<Span> {
    let at = |k: usize| toks.get(k).copied().filter(|t| !t.is_empty());
    let low = |k: usize| at(k).map(|t| split_trail(t).0.to_lowercase());
    let en = matches!(st, Style::EnUs | Style::EnGb);
    let de_of = matches!(st, Style::Es | Style::Pt);
    // Optional year after the month (es/pt: "de 2026").
    let with_year = |k: usize, month_trail: &str| -> (Option<String>, String, usize) {
        if !open(month_trail) {
            return (None, month_trail.to_string(), 0);
        }
        let (k, extra) = if de_of && low(k).as_deref() == Some("de") { (k + 1, 1) } else { (k, 0) };
        match at(k).and_then(year) {
            Some((y, trail)) => (Some(y), trail, extra + 1),
            None => (None, String::new(), 0),
        }
    };
    // en "the 12th of march", "12th of march"; es/pt "12 de marzo"; others "12 mars".
    let d_at = if en && low(i).as_deref() == Some("the") && at(i + 1).and_then(|t| day(st, t)).is_some() {
        i + 1
    } else {
        i
    };
    if let Some((d, d_trail)) = at(d_at).and_then(|t| day(st, t)) {
        if open(&d_trail) {
            let link = (en && low(d_at + 1).as_deref() == Some("of")) || (de_of && low(d_at + 1).as_deref() == Some("de"));
            let m_at = d_at + 1 + usize::from(link);
            if let Some((m, m_trail)) = at(m_at).and_then(|t| month(lang, t)) {
                let (y, trail, n) = with_year(m_at + 1, &m_trail);
                let trail = if n == 0 { m_trail } else { trail };
                return Some(Span { day: Some(d), month: m, year: y, trail, len: m_at + 1 + n - i });
            }
        }
    }
    // en "march 12th", "march the 12th".
    let (m, m_trail) = at(i).and_then(|t| month(lang, t))?;
    if open(&m_trail) {
        if en {
            let skip = usize::from(low(i + 1).as_deref() == Some("the"));
            let d_tok = at(i + 1 + skip);
            if let Some((d, d_trail)) = d_tok.and_then(|t| day(st, t)) {
                let (y, trail, n) = with_year(i + 2 + skip, &d_trail);
                // "they march 12 miles", "you may 2": a verb, unless the day is an
                // ordinal ("march 12th"), "the" opens it, or a year follows.
                let ordinal = d_tok.is_some_and(|t| split_trail(t).0.ends_with(|c: char| c.is_ascii_alphabetic()));
                if en_homograph(&m) && !(ordinal || skip == 1 || n > 0) {
                    return None;
                }
                let trail = if n == 0 { d_trail } else { trail };
                return Some(Span { day: Some(d), month: m, year: y, trail, len: 2 + skip + n });
            }
        }
        // A month and a year: "march 2026", "mars 2026", "marzo de 2026".
        let (y, trail, n) = with_year(i + 1, &m_trail);
        if n > 0 {
            return Some(Span { day: None, month: m, year: y, trail, len: 1 + n });
        }
    }
    None
}

/// Rewrite every date span of `text` (the walk's output) in the written form
/// of `lang`. Text in a language with no date style comes back unchanged.
pub fn fold_dates(text: &str, lang: &str) -> String {
    let Some(st) = style(lang) else {
        return text.to_string();
    };
    let toks: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        match span_at(lang, st, &toks, i) {
            Some(s) => {
                out.push(render(st, &s));
                i += s.len;
            }
            None => {
                out.push(toks[i].to_string());
                i += 1;
            }
        }
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::fold_dates;

    fn walk(text: &str, lang: &str) -> String {
        crate::w2n_sentence::words2num_sentence_full(text, lang, "cardinal", false, false, true).unwrap()
    }

    #[test]
    fn english_us() {
        assert_eq!(walk("on march twelfth twenty twenty six", "en"), "on March 12, 2026");
        assert_eq!(walk("the first of march", "en"), "March 1");
        assert_eq!(walk("on the twenty first of june nineteen ninety.", "en"), "on June 21, 1990.");
        assert_eq!(walk("december one twenty twenty five", "en"), "December 1, 2025");
        assert_eq!(walk("march the twelfth", "en"), "March 12");
        assert_eq!(walk("in march twenty twenty six", "en"), "in March 2026");
        assert_eq!(walk("see you march twelfth, ok", "en"), "see you March 12, ok");
        assert_eq!(walk("june twelve", "en"), "June 12");
        assert_eq!(walk("may the fourth", "en"), "May 4");
    }

    #[test]
    fn english_day_first() {
        assert_eq!(walk("the twelfth of march twenty twenty six", "en_GB"), "12 March 2026");
        assert_eq!(walk("on march twelfth", "en_AU"), "on 12 March");
    }

    #[test]
    fn other_languages() {
        assert_eq!(walk("le premier avril deux mille vingt-six", "fr"), "le 1er avril 2026");
        assert_eq!(walk("le douze Mars", "fr"), "le 12 mars");
        assert_eq!(walk("am zwölften märz zweitausendsechsundzwanzig", "de"), "am 12. März 2026");
        assert_eq!(walk("el primero de mayo", "es"), "el 1 de mayo");
        assert_eq!(walk("el doce de marzo de dos mil veintiséis", "es"), "el 12 de marzo de 2026");
        assert_eq!(walk("doze de março de dois mil e vinte e seis", "pt"), "12 de março de 2026");
        assert_eq!(walk("il primo maggio", "it"), "il 1º maggio");
        assert_eq!(walk("twaalf maart tweeduizend zesentwintig", "nl"), "12 maart 2026");
    }

    #[test]
    fn no_date_no_change() {
        for (text, lang) in [
            ("I may two things", "en"),
            ("they march twelve miles", "en"),
            ("march twelve", "en"),
            ("you may go", "en"),
            ("march on", "en"),
            ("twelve people", "en"),
            ("page 12 of the march report", "en"),
            ("douze personnes", "fr"),
        ] {
            assert_eq!(walk(text, lang), crate::w2n_sentence::words2num_sentence(text, lang, "cardinal", false).unwrap(), "{text}");
        }
        // A language with no date style passes through.
        assert_eq!(fold_dates("12 marca 2026", "pl"), "12 marca 2026");
    }

    #[test]
    fn off_by_default() {
        let plain = crate::w2n_sentence::words2num_sentence("march twelfth", "en", "cardinal", false).unwrap();
        assert_eq!(plain, "march 12th");
    }
}
