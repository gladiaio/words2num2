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
//! It needs a four-digit year and a real calendar date. With no separator
//! (no "slash", no de dots, no es/pt "de … de") it also needs a date cue
//! right before it (en "on", fr "le", de "am", es "el" ...), a date it is the
//! second end of ("from … to …"), or to stand alone (a whole sentence, or
//! a clock time after it): "we need three twelve two thousand people" is
//! three quantities (#1031). After a preposition cue ("by", "from", de "ab")
//! a round year with a word after it stays a quantity.
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
        Style::EnUs | Style::EnGb => &["on", "born", "dob", "dated", "date", "since", "until", "till", "from", "by", "before", "after", "between"],
        Style::Fr => &["le", "du", "au", "depuis", "jusqu'au", "entre", "né", "née", "daté", "datée", "date"],
        Style::De => &["am", "vom", "bis", "seit", "ab", "zwischen", "datum", "geboren", "zum", "dem"],
        Style::Es => &["el", "del", "al", "desde", "hasta", "entre", "fecha", "nació", "nacido", "nacida"],
        Style::Pt => &["em", "no", "dia", "de", "desde", "até", "entre", "data", "nasceu", "nascido", "nascida"],
        Style::It => &["il", "dal", "al", "tra", "fra", "nato", "nata", "data", "fino"],
        Style::Nl => &["op", "van", "tot", "sinds", "tussen", "datum", "geboren"],
    };
    cues.contains(&w)
}

/// How strongly a cue announces a date: 2 for a date word ("on", "born",
/// fr "le", de "am"), 1 for a preposition that also takes quantities ("by",
/// "from", de "ab"), 0 for a very common word (pt "de", de "dem" / "zum").
fn cue_strength(st: Style, w: &str) -> u8 {
    let (weak, very_weak): (&[&str], &[&str]) = match st {
        Style::EnUs | Style::EnGb => (&["by", "from", "after", "before", "since", "until", "till"], &[]),
        Style::Fr => (&["au", "du", "depuis", "jusqu'au"], &[]),
        Style::De => (&["ab", "bis", "seit", "vom"], &["dem", "zum"]),
        Style::Es => (&["hasta", "desde", "al", "del"], &[]),
        Style::Pt => (&["em", "no", "desde", "até"], &["de"]),
        Style::It => (&["al", "dal", "fino"], &[]),
        Style::Nl => (&["op", "van", "tot", "sinds"], &[]),
    };
    if very_weak.contains(&w) {
        0
    } else if weak.contains(&w) {
        1
    } else {
        2
    }
}

/// A word that joins the two ends of a date range: "from … to …".
fn range_word(st: Style, w: &str) -> bool {
    let words: &[&str] = match st {
        Style::EnUs | Style::EnGb => &["to", "and", "through", "till", "until", "the"],
        Style::Fr => &["au", "et", "jusqu'au", "le"],
        Style::De => &["bis", "und", "zum", "dem"],
        Style::Es => &["al", "y", "hasta", "el"],
        Style::Pt => &["a", "e", "até", "ao", "o"],
        Style::It => &["al", "e", "fino", "il"],
        Style::Nl => &["tot", "en", "met"],
    };
    words.contains(&w)
}

/// An article or particle a range link may carry ("to the", de "bis zum",
/// pt "até o", it "fino al"), but no link on its own: "… 2026 the 3 12 2000
/// people" is no range (#1076).
fn range_article(st: Style, w: &str) -> bool {
    let words: &[&str] = match st {
        Style::EnUs | Style::EnGb => &["the"],
        Style::Fr => &["le"],
        // "zum" (zu dem) links a range on its own: "vom … zum …" (#1079).
        Style::De => &["dem"],
        Style::Es => &["el"],
        Style::Pt => &["o"],
        Style::It => &["fino", "il"],
        Style::Nl => &[],
    };
    words.contains(&w)
}

/// Are `links` a range link: range words only, at least one of them more
/// than an article?
fn range_link(st: Style, links: &[&str]) -> bool {
    let words: Vec<String> = links
        .iter()
        .map(|t| {
            let (w, trail) = split_trail(t);
            if trail.is_empty() { w.to_lowercase() } else { String::new() }
        })
        .collect();
    !words.is_empty()
        && words.iter().all(|w| range_word(st, w))
        && words.iter().any(|w| !range_article(st, w))
}

/// A clock time spoken as a word ("at noon").
fn time_word(w: &str) -> bool {
    matches!(
        w,
        "noon" | "midnight" | "midi" | "minuit" | "mittag" | "mitternacht" | "mediodía" | "medianoche"
            | "meio-dia" | "meia-noite" | "mezzogiorno" | "mezzanotte" | "middernacht"
    )
}

/// A word that introduces a clock time after a date ("… at 3:30").
fn clock_cue(st: Style, w: &str) -> bool {
    let words: &[&str] = match st {
        Style::EnUs | Style::EnGb => &["at"],
        Style::Fr => &["à"],
        Style::De => &["um"],
        Style::Es => &["a"],
        Style::Pt => &["às"],
        Style::It => &["alle"],
        Style::Nl => &["om"],
    };
    words.contains(&w)
}

/// A date spoken all in numbers at token `i`: N1 [sep] N2 [sep] YEAR, with
/// sep "slash" or "/" (both or none), de "12. 3. 2026", es / pt "12 del 3
/// de 2026". N1 is the month in en (US), the day elsewhere.
fn numeric_at(lang: &str, st: Style, toks: &[&str], i: usize, chained: bool) -> Option<Span> {
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
    // de "12. 3. 2026": the ordinal dots are a separator too.
    let de_dots = st == Style::De && [i, i + 1].iter().all(|&k| at(k).is_some_and(|t| t.ends_with('.')));
    if s1 == 0 && !de_dots && !chained {
        let word = |t: &str| t.chars().any(char::is_alphanumeric);
        let sentence_end = |t: &str| t.trim_end_matches(['"', '\'', ')', '»', '”']).ends_with(['.', '!', '?']);
        let back: Vec<&str> = toks[..i].iter().rev().copied().filter(|t| !t.is_empty()).collect();
        // A cue, with ":" or "," after it (also spaced: "Date : …"), a bracket
        // before it ("(on").
        let cue_at = back.iter().position(|t| word(t));
        let cue = cue_at
            .filter(|&n| back[..n].iter().all(|t| matches!(*t, ":" | ",")))
            .and_then(|n| {
                let (prev, prev_trail) = split_trail(back[n]);
                let prev = prev.trim_start_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
                let strength = cue_strength(st, &prev);
                // A very common word counts behind a real cue ("ab dem", "bis
                // zum"), taking its strength.
                let strength = if strength == 0 {
                    back.get(n + 1).map_or(0, |t| {
                        let w = split_trail(t).0.to_lowercase();
                        if date_cue(st, &w) { cue_strength(st, &w).max(1) } else { 0 }
                    })
                } else {
                    strength
                };
                (matches!(prev_trail, "" | ":" | ",") && date_cue(st, &prev)).then_some(strength)
            });
        let cued = cue.is_some();
        // Alone: at the start of the text or of a sentence (punctuation-only
        // tokens such as a bullet may come between), and up to the end of it,
        // or before a clock time ("at 3:30") (#1057, #1058).
        let starts = match cue_at {
            None => true,
            Some(n) => sentence_end(back[n]) && back[..n].iter().all(|t| !word(t)),
        };
        let after: Vec<&str> = toks[(i + len).min(toks.len())..].iter().copied().filter(|t| !t.is_empty()).collect();
        let time_next = |k: usize| {
            after.get(k).is_some_and(|t| clock_cue(st, &split_trail(t).0.to_lowercase()))
                && after.get(k + 1).is_some_and(|t| {
                    t.starts_with(|c: char| c.is_ascii_digit()) || time_word(&split_trail(t).0.to_lowercase())
                })
        };
        let ends = if trail.is_empty() {
            match after.iter().position(|t| word(t)) {
                None => after.iter().all(|t| t.chars().all(|c| matches!(c, '.' | '!' | '?' | '"' | ')' | '»' | '”'))),
                Some(k) => after[..k].is_empty() && time_next(0),
            }
        } else {
            sentence_end(&trail)
        };
        if !cued && !(starts && ends) {
            return None;
        }
        // After a preposition cue, a round year with a word after it is a
        // quantity: "by three twelve two thousand people" (#1055); after a
        // very common word (pt "de", de "dem"), any year is. A range word
        // after it ("from … to …") is no such word; a date word ("born on",
        // "am") always gives a date.
        // Range words, then a second date: three numbers or one in figures
        // ("to 4 12 2000", "tot en met 13 3 2000", "to 4/12/2000") (#1051 N1).
        let r = after
            .iter()
            .take(3)
            .take_while(|t| {
                let (w, trail) = split_trail(t);
                trail.is_empty() && range_word(st, &w.to_lowercase())
            })
            .count();
        // The second end in any date form the pass reads ("4/12/2000", "4
        // slash 12 slash 2000", "december fourth 2000", es "cuatro de
        // diciembre de …"): the same span lookup, as a chained span.
        let second_at = (i + len..toks.len()).filter(|&k| !toks[k].is_empty()).nth(r);
        let range_next = r > 0
            && range_link(st, &after[..r])
            && second_at.is_some_and(|k| {
                let t = toks[k];
                let core = split_trail(t).0;
                let parts: Vec<&str> = core.split(['/', '.', '-']).collect();
                let figure_date = parts.len() == 3
                    && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
                    && parts.iter().any(|p| p.len() == 4);
                figure_date || span_at(lang, st, toks, k, true).is_some()
            });
        let word_after = !ends && !range_next;
        match cue {
            Some(1) if y.ends_with("00") && word_after => return None,
            // pt "de" alone only opens a range: "de 12 3 2026 a 13 3 2026".
            Some(0) if !range_next => return None,
            _ => {}
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
fn span_at(lang: &str, st: Style, toks: &[&str], i: usize, chained: bool) -> Option<Span> {
    if let Some(s) = numeric_at(lang, st, toks, i, chained) {
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
    // The token right after the last date folded: a range word there makes
    // the next span the second end of a range ("from 3/12/2026 to 4 1 2027").
    let mut after_date: Option<usize> = None;
    while i < toks.len() {
        // Up to three range words ("to", nl "tot en met").
        let chained = after_date.is_some_and(|k| k < i && i - k <= 3 && range_link(st, &toks[k..i]));
        match span_at(lang, st, &toks, i, chained) {
            Some(s) => {
                out.push(render(st, &s, iso));
                i += s.len;
                after_date = open(&s.trail).then_some(i);
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
