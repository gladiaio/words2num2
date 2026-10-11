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
//!
//! A date spoken all in numbers, day / month / year ("three twelve twenty
//! twenty six", "oh three slash twelve slash twenty twenty six", de "12. 3.
//! 2026", es "12 del 3 de 2026"), is written in figures, in the order and
//! with the separator of the region: en (US) month first "3/12/2026"; en_GB,
//! fr, es, it, pt day first "12/03/2026"; de "12.03.2026"; nl "12-03-2026".
//! It needs a four-digit year and a real calendar date.
//!
//! `region` ("US", "GB"...) overrides the region of the language code;
//! `iso` writes every date that has a year as "2026-03-12".

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

fn style(lang: &str, region: Option<&str>) -> Option<Style> {
    let mut it = lang.split(&['_', '-'][..]);
    let base = it.next().unwrap_or(lang).to_lowercase();
    let region = region.unwrap_or_else(|| it.next().unwrap_or("")).to_uppercase();
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

/// The number of a month name of `lang` (1..=12): its place among the
/// language's names, abbreviations and regional spellings folded in (en
/// "sept", es "setiembre", de "jänner").
fn month_number(lang: &str, name: &str) -> Option<u32> {
    let norm = crate::normalize(name);
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang).to_lowercase();
    match (base.as_str(), norm.as_str()) {
        ("es", "setiembre") => return Some(9),
        ("de", "janner") => return Some(1),
        ("en", "sept") => return Some(9),
        _ => {}
    }
    let names: Vec<&str> = crate::w2n_sentence::months(lang)
        .split_whitespace()
        .filter(|m| !matches!(*m, "setiembre" | "janner" | "sept"))
        .collect();
    let full = &names[..names.len().min(12)];
    if let Some(k) = full.iter().position(|m| *m == norm) {
        return u32::try_from(k + 1).ok();
    }
    // en abbreviations: the first three letters of a full name.
    full.iter().position(|m| m.len() > 3 && m[..3] == norm[..]).and_then(|k| u32::try_from(k + 1).ok())
}

/// A number token in figures, 1..=99, with its trailing punctuation; de
/// "3." is the ordinal (the dot is no punctuation there).
fn small(st: Style, tok: &str) -> Option<(u32, String)> {
    let (core, trail) = split_trail(tok);
    let trail = if st == Style::De && trail.starts_with('.') { &trail[1..] } else { trail };
    let ok = !core.is_empty() && core.len() <= 2 && core.bytes().all(|b| b.is_ascii_digit());
    ok.then(|| core.parse().ok()).flatten().filter(|v| (1..=99).contains(v)).map(|v| (v, trail.to_string()))
}

/// Days in `month` of `year` (leap years included).
fn days_in(month: u32, year: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
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

/// One date span: its parts, with the punctuation after its last token. A
/// numeric date has no month name (`month` empty).
struct Span {
    day: Option<u32>,
    month: String,
    month_no: Option<u32>,
    year: Option<String>,
    trail: String,
    len: usize,
}

/// A date in figures: the region's order and separator, or ISO.
fn numeric(st: Style, d: u32, m: u32, y: &str, iso: bool) -> String {
    if iso {
        return format!("{y}-{m:02}-{d:02}");
    }
    match st {
        Style::EnUs => format!("{m}/{d}/{y}"),
        Style::De => format!("{d:02}.{m:02}.{y}"),
        Style::Nl => format!("{d:02}-{m:02}-{y}"),
        _ => format!("{d:02}/{m:02}/{y}"),
    }
}

fn render(st: Style, s: &Span, iso: bool) -> String {
    if let (Some(d), Some(m), Some(y)) = (s.day, s.month_no, &s.year) {
        if iso || s.month.is_empty() {
            return format!("{}{}", numeric(st, d, m, y, iso), s.trail);
        }
    }
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

/// A word that announces a date: "on", "born", "le", "am", "el" ...
fn date_cue(st: Style, w: &str) -> bool {
    let cues: &[&str] = match st {
        Style::EnUs | Style::EnGb => &["on", "born", "dated", "date", "since", "until", "till", "from", "by", "before", "after"],
        Style::Fr => &["le", "du", "au", "depuis", "jusqu'au", "né", "née", "daté", "datée", "date"],
        Style::De => &["am", "vom", "bis", "seit", "ab", "datum", "geboren"],
        Style::Es => &["el", "desde", "hasta", "fecha", "nació", "nacido", "nacida"],
        Style::Pt => &["em", "no", "dia", "desde", "até", "data", "nasceu", "nascido", "nascida"],
        Style::It => &["il", "dal", "al", "nato", "nata", "data", "fino"],
        Style::Nl => &["op", "van", "tot", "sinds", "datum", "geboren"],
    };
    cues.contains(&w)
}

/// A date spoken all in numbers at token `i`: N1 [sep] N2 [sep] YEAR, with
/// sep "slash" or "/" (both or none), de "12. 3. 2026", es / pt "12 del 3
/// de 2026". N1 is the month in en (US), the day elsewhere.
fn numeric_at(st: Style, toks: &[&str], i: usize) -> Option<Span> {
    let at = |k: usize| toks.get(k).copied().filter(|t| !t.is_empty());
    let low = |k: usize| at(k).map(|t| split_trail(t).0.to_lowercase());
    let (a, a_trail) = small(st, at(i)?)?;
    if !a_trail.is_empty() {
        return None;
    }
    let es = matches!(st, Style::Es | Style::Pt);
    let sep = |k: usize| -> usize {
        match low(k).as_deref() {
            Some("slash" | "/") => 1,
            Some("del" | "de") if es => 1,
            _ => 0,
        }
    };
    let s1 = sep(i + 1);
    let (b, b_trail) = small(st, at(i + 1 + s1)?)?;
    if !b_trail.is_empty() {
        return None;
    }
    let s2 = sep(i + 2 + s1);
    if (s1 == 0) != (s2 == 0) {
        return None; // "3 slash 12 2026", "tres de doce 2000": half a date
    }
    let (y, trail) = at(i + 2 + s1 + s2).and_then(year)?;
    // With no separator, three numbers are a date only after a date cue ("on
    // 3 12 2026", "le 12 3 2026") or as the whole text: "we need 3 12 2000
    // people" is three quantities (#1031).
    let len = 3 + s1 + s2;
    if s1 == 0 {
        let cued = i > 0 && {
            let (prev, prev_trail) = split_trail(toks[i - 1]);
            prev_trail.is_empty() && date_cue(st, &prev.to_lowercase())
        };
        // Nothing but punctuation after it: "three twelve twenty twenty six ."
        let whole = i == 0
            && toks[(i + len).min(toks.len())..]
                .iter()
                .all(|t| t.chars().all(|c| !c.is_alphanumeric()));
        if !cued && !whole {
            return None;
        }
    }
    let (d, m) = if st == Style::EnUs { (b, a) } else { (a, b) };
    let yn: u32 = y.parse().ok()?;
    if !(1..=12).contains(&m) || d == 0 || d > days_in(m, yn) {
        return None;
    }
    Some(Span { day: Some(d), month: String::new(), month_no: Some(m), year: Some(y), trail, len })
}

/// The date span starting at token `i`, if any.
fn span_at(lang: &str, st: Style, toks: &[&str], i: usize) -> Option<Span> {
    if let Some(s) = numeric_at(st, toks, i) {
        return Some(s);
    }
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
                return Some(Span { day: Some(d), month_no: month_number(lang, &m), month: m, year: y, trail, len: m_at + 1 + n - i });
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
                return Some(Span { day: Some(d), month_no: month_number(lang, &m), month: m, year: y, trail, len: 2 + skip + n });
            }
        }
        // A month and a year: "march 2026", "mars 2026", "marzo de 2026".
        let (y, trail, n) = with_year(i + 1, &m_trail);
        if n > 0 {
            return Some(Span { day: None, month_no: month_number(lang, &m), month: m, year: y, trail, len: 1 + n });
        }
    }
    None
}

/// Rewrite every date span of `text` (the walk's output) in the written form
/// of `lang`. Text in a language with no date style comes back unchanged.
pub fn fold_dates(text: &str, lang: &str) -> String {
    fold_dates_with(text, lang, None, false)
}

/// [`fold_dates`] with a region overriding the language code's ("GB" for
/// day first) and ISO output (`iso`: "2026-03-12" for a date with a year).
pub fn fold_dates_with(text: &str, lang: &str, region: Option<&str>, iso: bool) -> String {
    let Some(st) = style(lang, region) else {
        return text.to_string();
    };
    let toks: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        match span_at(lang, st, &toks, i) {
            Some(s) => {
                out.push(render(st, &s, iso));
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
        crate::w2n_sentence::words2num_sentence_full(text, lang, "cardinal", false, false, true, None, "written", false).unwrap()
    }

    fn walk_with(text: &str, lang: &str, region: Option<&str>, format: &str) -> String {
        crate::w2n_sentence::words2num_sentence_full(text, lang, "cardinal", false, false, true, region, format, false).unwrap()
    }

    #[test]
    fn numeric_dates() {
        assert_eq!(walk("born on three twelve twenty twenty six", "en"), "born on 3/12/2026");
        assert_eq!(walk("three twelve twenty twenty six", "en_GB"), "03/12/2026");
        assert_eq!(walk("the date is oh three slash twelve slash twenty twenty six.", "en"), "the date is 3/12/2026.");
        assert_eq!(walk("né le douze trois deux mille vingt-six", "fr"), "né le 12/03/2026");
        assert_eq!(walk("am zwölften dritten zweitausendsechsundzwanzig", "de"), "am 12.03.2026");
        assert_eq!(walk("el doce del tres de dos mil veintiséis", "es"), "el 12/03/2026");
        // nl "twaalf drie" is merged by the walk ("1203"): the format on figures.
        assert_eq!(super::fold_dates("op 12 3 2026", "nl"), "op 12-03-2026");
        // Not a calendar date, no year, or half a date: left as it is.
        assert_eq!(walk("thirteen twelve twenty twenty six", "en"), "13 12 2026");
        assert_eq!(walk("two thirty twenty twenty four", "en"), "2 30 2024");
        assert_eq!(walk("two twenty nine twenty twenty four", "en"), "2/29/2024");
        assert_eq!(walk("two twenty nine twenty twenty five", "en"), "2 29 2025");
        assert_eq!(walk("three twelve", "en"), "3 12");
    }

    #[test]
    fn region_override() {
        assert_eq!(walk_with("three twelve twenty twenty six", "en", Some("GB"), "written"), "03/12/2026");
        assert_eq!(walk_with("on march twelfth", "en", Some("au"), "written"), "on 12 March");
        assert_eq!(walk_with("on march twelfth", "en_GB", Some("US"), "written"), "on March 12");
    }

    #[test]
    fn iso() {
        let iso = |t: &str, l: &str| walk_with(t, l, None, "iso");
        assert_eq!(iso("on march twelfth twenty twenty six", "en"), "on 2026-03-12");
        assert_eq!(iso("three twelve twenty twenty six", "en"), "2026-03-12");
        assert_eq!(iso("le premier avril deux mille vingt-six", "fr"), "le 2026-04-01");
        assert_eq!(iso("el doce de setiembre de dos mil veintiséis", "es"), "el 2026-09-12");
        assert_eq!(iso("on sept twelfth twenty twenty six", "en"), "on 2026-09-12");
        // No year: the written form.
        assert_eq!(iso("the first of march", "en"), "March 1");
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
