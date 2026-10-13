//! Clock times in French, German and Spanish sentence mode.
//!
//! A time phrase is read on the words, before the walk, and written in
//! figures; the walk leaves a token that holds digits as it is:
//!
//! | lang | spoken | written |
//! |---|---|---|
//! | fr | à neuf heures quinze / et quart / et demie / moins le quart | à 9h15 / 9h15 / 9h30 / 8h45 |
//! | fr | à neuf heures | à 9h |
//! | de | um neun Uhr fünfzehn / um neun Uhr | um 9:15 Uhr / um 9 Uhr |
//! | de | um halb zehn / Viertel nach neun / Viertel vor zehn | um 9:30 / 9:15 / 9:45 |
//! | es | a las nueve y cuarto / y media / menos cuarto / en punto | a las 9:15 / 9:30 / 8:45 / 9:00 |
//!
//! An hour with minutes is a time. An hour alone is one only after a time
//! cue (fr "à", "vers"...; de "um", "gegen"...): "trois heures de route" is a
//! duration and stays for the walk ("3 heures de route").
//! English clock times are read by the walk itself (`en_time_run`).

/// The words of the sentence: (start, end) byte offsets, the token lower-cased
/// without its trailing punctuation, and that punctuation.
struct Tok {
    start: usize,
    end: usize,
    word: String,
    trail: String,
}

const TRAILING: &[char] = &[',', ';', ':', '!', '?', '.', ')', '"', '»'];

fn tokens(s: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut start = None;
    for (k, c) in s.char_indices().chain(std::iter::once((s.len(), ' '))) {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(k),
            (true, Some(a)) => {
                let raw = &s[a..k];
                let core = raw.trim_end_matches(TRAILING);
                out.push(Tok {
                    start: a,
                    end: k,
                    word: core.to_lowercase(),
                    trail: raw[core.len()..].to_string(),
                });
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// A number written in figures or in words of `lang` ("neuf", "dix-sept",
/// "una"), as one to three tokens.
fn number(lang: &str, words: &[&str]) -> Option<i64> {
    let text = words.join(" ");
    if text.bytes().all(|b| b.is_ascii_digit()) {
        return text.parse().ok();
    }
    // The feminine one of "une heure", "la una": an article to the tables.
    if matches!((lang, text.as_str()), ("fr", "une") | ("es", "una")) {
        return Some(1);
    }
    match crate::w2n_sentence::words2num(&text, lang, "cardinal") {
        Ok(crate::w2n_lang_en::W2nValue::Int(v)) => i64::try_from(&v).ok(),
        _ => None,
    }
}

/// The longest number of 1..=3 words at `toks[i..]` within `range`: its value
/// and how many words it takes. Every word must be clean (no punctuation)
/// but the last.
fn number_at(lang: &str, toks: &[Tok], i: usize, range: std::ops::RangeInclusive<i64>) -> Option<(i64, usize)> {
    (1..=3).rev().find_map(|n| {
        let span = toks.get(i..i + n)?;
        if span[..n - 1].iter().any(|t| !t.trail.is_empty()) {
            return None;
        }
        let words: Vec<&str> = span.iter().map(|t| t.word.as_str()).collect();
        // A connector after the number is no part of it: "diez y | las tres"
        // kept its "y" (#937).
        if n > 1 && matches!(words[n - 1], "y" | "et" | "und" | "e") {
            return None;
        }
        number(lang, &words).filter(|v| range.contains(v)).map(|v| (v, n))
    })
}

/// A word that counts or measures what the number before it counts: after
/// it, plain minutes are a quantity ("huit heures trois fois", "um neun Uhr
/// zwanzig Leute", "las dos y cinco personas", #933).
fn counted(lang: &str, tok: &Tok) -> bool {
    let w = tok.word.as_str();
    let nouns: &[&str] = match lang {
        "fr" => &["fois", "jour", "jours", "minute", "minutes", "seconde", "secondes", "semaine", "semaines", "mois", "an", "ans", "année", "années", "personnes", "enfants", "hommes", "femmes", "élèves", "kilos", "kilo", "litres", "litre", "francs", "euro", "euros", "dollar", "dollars", "livre", "livres", "%", "fr", "km", "kg", "kilomètres", "mètres", "grammes", "€", "$", "£"],
        "de" => &["leute", "kinder", "männer", "frauen", "schüler", "stück", "kilo", "liter", "minuten", "minute", "sekunden", "tage", "tag", "jahre", "jahr", "personen", "euro", "dollar", "pfund", "prozent", "%", "km", "kg", "meter", "kilometer", "gramm", "€", "$", "£"],
        "es" => &["veces", "vez", "personas", "niños", "hombres", "mujeres", "alumnos", "kilos", "kilo", "litros", "litro", "minutos", "segundos", "días", "dias", "años", "anos", "semanas", "meses", "euros", "dólares", "dolares", "libras", "%", "km", "kg", "kilómetros", "metros", "gramos", "€", "$", "£"],
        _ => &[],
    };
    // Any currency name of the language too ("pesos", "Pfund") (#1194),
    // but not one that is mostly another word or a name ("Franco llamó",
    // "peso la harina", #1197 audit).
    nouns.contains(&w)
        || (!crate::w2n_currency::time_homograph(&crate::normalize(w))
            && crate::w2n_currency::is_currency_word(lang, &crate::normalize(w)))
}

/// The time would cut a longer number: a number word or "et"/"y"/"und" and a
/// number right before its hour or right after it ("cent vingt heures quinze",
/// "trente et une heures", "moins trente et un", #935).
fn cuts_number(lang: &str, toks: &[Tok], start: usize, end: usize) -> bool {
    let val = |k: usize| toks.get(k).and_then(|t| number(lang, &[t.word.as_str()]));
    let is_num = |k: usize| val(k).is_some();
    let conn = |k: usize| toks.get(k).is_some_and(|t| matches!(t.word.as_str(), "et" | "y" | "e"));
    let clean = |k: usize| toks.get(k).is_some_and(|t| t.trail.is_empty());
    // Only a scale word makes the minutes part of a larger number ("quinze
    // mille"), another number after them is the next count ("zwanzig zwei
    // Leute", "quinze un homme", #1190 audit).
    let scale = |k: usize| {
        toks.get(k).is_some_and(|t| {
            matches!(
                t.word.as_str(),
                "cent" | "cents" | "mille" | "million" | "millions" | "milliard" | "milliards"
                    | "hundert" | "tausend" | "millionen" | "cien" | "ciento" | "cientos" | "mil"
                    | "millón" | "millones"
            )
        // ... and only when the two make one number: es "veinte cien" does
        // not (#1190 audit).
        }) && number(lang, &[toks[k - 1].word.as_str(), toks[k].word.as_str()]).is_some()
    };
    // "et"/"y" joins one number only as tens and a unit ("trente et un",
    // "treinta y uno", "soixante et onze"); "neuf et dix", "zwischen neun und
    // zehn" are ranges of two (#1193).
    let compound = |tens: usize, unit: usize| {
        val(tens).is_some_and(|t| t >= 20 && t < 100 && t % 10 == 0) && val(unit).is_some_and(|u| (1..=9).contains(&u) || u == 11)
    };
    let before = start > 0
        && clean(start - 1)
        && (is_num(start - 1) || (start > 1 && conn(start - 1) && clean(start - 2) && compound(start - 2, start)));
    // Only when the span ends on its minutes: "neuf heures et dix heures"
    // is a range of two times.
    // The minutes may have taken the connector with them ("treinta y").
    let after = end > 0
        && clean(end - 1)
        && ((is_num(end - 1) && (scale(end) || (conn(end) && clean(end) && compound(end - 1, end + 1))))
            || (end > 1 && conn(end - 1) && compound(end - 2, end)));
    before || after
}

/// One time span: its tokens [i, i + len) and what replaces them.
struct Span {
    len: usize,
    text: String,
}

fn word(toks: &[Tok], k: usize) -> Option<&str> {
    toks.get(k).map(|t| t.word.as_str())
}

/// The minutes after "heures" / "Uhr" / "y": a number 1..=59 or a word for
/// a quarter / a half. Returns (minutes to add, minus: a quarter to, words).
fn minutes_fr(lang: &str, toks: &[Tok], k: usize) -> Option<(i64, usize)> {
    match (word(toks, k), word(toks, k + 1), word(toks, k + 2)) {
        (Some("et"), Some("quart"), _) => Some((15, 2)),
        // Three quarters of an hour (#934).
        (Some("et"), Some("trois"), Some("quarts")) => Some((45, 3)),
        (Some("trois"), Some("quarts"), _) => Some((45, 2)),
        (Some("et"), Some("demie" | "demi"), _) => Some((30, 2)),
        (Some("moins"), Some("le"), Some("quart")) => Some((-15, 3)),
        (Some("moins"), Some("quart"), _) => Some((-15, 2)),
        (Some("moins"), _, _) => number_at(lang, toks, k + 1, 1..=30).map(|(m, n)| (-m, n + 1)),
        // "neuf heures | et dix heures": a connector opens no minutes, and a
        // number followed by "heures" is the next hour.
        (Some("et"), _, _) => None,
        _ => number_at(lang, toks, k, 1..=59)
            .filter(|&(_, n)| !matches!(word(toks, k + n), Some("heure" | "heures" | "h"))),
    }
}

fn minutes_es(lang: &str, toks: &[Tok], k: usize) -> Option<(i64, usize)> {
    match (word(toks, k), word(toks, k + 1)) {
        (Some("y"), Some("cuarto")) => Some((15, 2)),
        // "y tres cuartos": three quarters of an hour (#934).
        (Some("y"), Some("tres")) if word(toks, k + 2) == Some("cuartos") => Some((45, 3)),
        (Some("y"), Some("media")) => Some((30, 2)),
        (Some("menos"), Some("cuarto")) => Some((-15, 2)),
        (Some("en"), Some("punto")) => Some((0, 2)),
        (Some("y"), _) => number_at(lang, toks, k + 1, 1..=59).map(|(m, n)| (m, n + 1)),
        (Some("menos"), _) => number_at(lang, toks, k + 1, 1..=30).map(|(m, n)| (-m, n + 1)),
        _ => number_at(lang, toks, k, 1..=59),
    }
}

/// An hour and the minutes to add (negative: "moins le quart") -> (h, m).
fn clock(h: i64, add: i64, hours: i64) -> (i64, i64) {
    if add >= 0 {
        (h, add)
    } else {
        ((h - 1).rem_euclid(hours), 60 + add)
    }
}

fn cue(lang: &str, prev: Option<&str>) -> bool {
    let cues: &[&str] = match lang {
        "fr" => &["à", "vers", "dès", "avant", "après", "apres", "entre", "jusqu'à", "jusqu'a"],
        "de" => &["um", "gegen", "ab", "bis", "vor", "nach", "zwischen"],
        _ => &[],
    };
    prev.is_some_and(|p| cues.contains(&p))
}

/// An hour of 1..=3 words at `i` whose next word passes `then`: its value
/// and length (the shortest that works: "nueve | y cuarto", not "nueve y").
fn hour_at(lang: &str, toks: &[Tok], i: usize, max: i64, then: impl Fn(usize) -> bool) -> Option<(i64, usize)> {
    // The first word is a number on its own: "et dix heures" is no hour.
    number(lang, &[toks.get(i)?.word.as_str()])?;
    (1..=3).find_map(|n| {
        let span = toks.get(i..i + n)?;
        if span.iter().any(|t| !t.trail.is_empty()) || !then(i + n) {
            return None;
        }
        let words: Vec<&str> = span.iter().map(|t| t.word.as_str()).collect();
        // A connector after the number is no part of it: "diez y | las tres"
        // kept its "y" (#937).
        if n > 1 && matches!(words[n - 1], "y" | "et" | "und" | "e") {
            return None;
        }
        number(lang, &words).filter(|h| (0..=max).contains(h)).map(|h| (h, n))
    })
}

fn span_fr(toks: &[Tok], i: usize) -> Option<Span> {
    let is_key = |k: usize| matches!(word(toks, k), Some("heure" | "heures" | "h"));
    let (h, n) = hour_at("fr", toks, i, 24, is_key)?;
    let key = i + n;
    let after = if toks[key].trail.is_empty() { minutes_fr("fr", toks, key + 1) } else { None };
    let prev = i.checked_sub(1).and_then(|k| word(toks, k));
    let span = match after {
        Some((add, m)) => {
            // Plain minutes before a counted noun are a quantity (#933).
            let counts = add > 0
                && word(toks, key + 1).is_some_and(|w| !matches!(w, "et" | "moins"))
                && toks[key + m].trail.is_empty()
                && toks.get(key + 1 + m).is_some_and(|t| counted("fr", t));
            let (h2, mm) = clock(h, add, 24);
            let span = Span { len: n + 1 + m, text: format!("{h2}h{mm:02}") };
            if !counts && !cuts_number("fr", toks, i, i + span.len) {
                return Some(span);
            }
            // After a cue the hour alone is still the time ("à 9h 15 fois
            // deux") (#1190 audit).
            return (cue("fr", prev) && !cuts_number("fr", toks, i, i + n + 1))
                .then(|| Span { len: n + 1, text: format!("{h}h") });
        }
        None if cue("fr", prev) => Span { len: n + 1, text: format!("{h}h") },
        None => return None,
    };
    (!cuts_number("fr", toks, i, i + span.len)).then_some(span)
}

fn span_de(toks: &[Tok], i: usize) -> Option<Span> {
    let prev = i.checked_sub(1).and_then(|k| word(toks, k));
    // "halb zehn" = 9:30, "Viertel nach neun" = 9:15, "Viertel vor zehn" /
    // "dreiviertel zehn" = 9:45: after a cue only ("halb so viel").
    if cue("de", prev) {
        let rel = match (word(toks, i)?, word(toks, i + 1)) {
            ("halb", _) => Some((1, -30)),
            ("dreiviertel", _) => Some((1, -15)),
            ("viertel", Some("nach")) => Some((2, 15)),
            ("viertel", Some("vor")) => Some((2, -15)),
            _ => None,
        };
        if let Some((skip, add)) = rel {
            if toks[i..i + skip].iter().all(|t| t.trail.is_empty()) {
                if let Some((h, n)) = number_at("de", toks, i + skip, 1..=12) {
                    let (h, mm) = clock(h, add, 12);
                    let h = if h == 0 { 12 } else { h };
                    return Some(Span { len: skip + n, text: format!("{h}:{mm:02}") });
                }
            }
        }
    }
    let (h, n) = hour_at("de", toks, i, 24, |k| word(toks, k) == Some("uhr"))?;
    let key = i + n;
    let uhr = &toks[key];
    let after = if uhr.trail.is_empty() { number_at("de", toks, key + 1, 1..=59) } else { None };
    // Minutes before a counted noun are a quantity: "um neun Uhr zwanzig
    // Leute" keeps "20 Leute" (#933).
    let after = after.filter(|&(_, m)| !(toks[key + m].trail.is_empty() && toks.get(key + 1 + m).is_some_and(|t| counted("de", t))));
    let span = match after {
        Some((mm, m)) => Span { len: n + 1 + m, text: format!("{h}:{mm:02} Uhr") },
        None => Span { len: n + 1, text: format!("{h} Uhr") },
    };
    (!cuts_number("de", toks, i, i + span.len)).then_some(span)
}

fn span_es(toks: &[Tok], i: usize) -> Option<Span> {
    // "las nueve y cuarto", "la una en punto": the article opens the time.
    if !matches!(word(toks, i)?, "la" | "las") || !toks[i].trail.is_empty() {
        return None;
    }
    let (h, n) = hour_at("es", toks, i + 1, 24, |k| minutes_es("es", toks, k).is_some())?;
    let key = i + 1 + n;
    let (add, m) = minutes_es("es", toks, key)?;
    let plain = !matches!(word(toks, key + 1), Some("cuarto" | "media" | "punto"));
    if plain && toks[key + m - 1].trail.is_empty() && toks.get(key + m).is_some_and(|t| counted("es", t)) {
        return None; // "las dos y cinco personas" (#933)
    }
    if cuts_number("es", toks, i + 1, key + m) {
        return None; // "las dos menos treinta y uno" (#935)
    }
    // "entre las dos y tres": after "entre" a "y" and an hour is the end of
    // a range, not minutes, unless the range goes on ("entre las dos y diez
    // y las tres") (#937).
    let range = i > 0
        && word(toks, i - 1) == Some("entre")
        && word(toks, key) == Some("y")
        && plain
        && (1..=12).contains(&add)
        && !matches!(word(toks, key + m), Some("y" | "minutos"));
    if range {
        // The first hour alone: "entre la 1 y 3", not "la una" left a word.
        let article = &toks[i].word;
        return Some(Span { len: 1 + n, text: format!("{article} {h}") });
    }
    let (h, mm) = clock(h, add, 24);
    let article = &toks[i].word;
    Some(Span { len: 1 + n + m, text: format!("{article} {h}:{mm:02}") })
}

/// Rewrite every clock time of `sentence` in figures (fr, de, es); other
/// languages come back unchanged.
pub fn fold_times(sentence: &str, lang: &str) -> String {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang).to_lowercase();
    let find: fn(&[Tok], usize) -> Option<Span> = match base.as_str() {
        "fr" => span_fr,
        "de" => span_de,
        "es" => span_es,
        _ => return sentence.to_string(),
    };
    // Most sentences hold no time: one cheap look for the words a time needs.
    let keys: &[&str] = match base.as_str() {
        "fr" => &["heure", "h"],
        "de" => &["uhr", "halb", "viertel"],
        _ => &["la"],
    };
    let lower = sentence.to_lowercase();
    if !keys.iter().any(|k| lower.contains(k)) {
        return sentence.to_string();
    }
    let toks = tokens(sentence);
    let mut out = String::with_capacity(sentence.len());
    let mut copied = 0;
    let mut i = 0;
    while i < toks.len() {
        match find(&toks, i) {
            Some(span) => {
                let last = &toks[i + span.len - 1];
                // Keep the case of a capitalised first word ("Las nueve").
                let text = if toks[i].start < sentence.len()
                    && sentence[toks[i].start..].starts_with(|c: char| c.is_uppercase())
                    && span.text.starts_with(|c: char| c.is_alphabetic())
                {
                    let mut c = span.text.chars();
                    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
                } else {
                    span.text
                };
                out.push_str(&sentence[copied..toks[i].start]);
                out.push_str(&text);
                out.push_str(&last.trail);
                copied = last.end;
                i += span.len;
            }
            None => i += 1,
        }
    }
    out.push_str(&sentence[copied..]);
    out
}

#[cfg(test)]
mod tests {
    fn s(text: &str, lang: &str) -> String {
        crate::w2n_sentence::words2num_sentence(text, lang, "cardinal", false).unwrap()
    }

    #[test]
    fn french() {
        assert_eq!(s("à neuf heures quinze", "fr"), "à 9h15");
        assert_eq!(s("rendez-vous à neuf heures.", "fr"), "rendez-vous à 9h.");
        assert_eq!(s("vers dix-sept heures trente", "fr"), "vers 17h30");
        assert_eq!(s("à neuf heures et quart", "fr"), "à 9h15");
        assert_eq!(s("à neuf heures et demie", "fr"), "à 9h30");
        assert_eq!(s("à dix heures moins le quart", "fr"), "à 9h45");
        assert_eq!(s("à dix heures moins cinq", "fr"), "à 9h55");
        assert_eq!(s("à une heure", "fr"), "à 1h");
        assert_eq!(s("il est vingt-trois heures cinquante-neuf", "fr"), "il est 23h59");
        assert_eq!(s("à minuit moins le quart", "fr"), "à minuit moins le quart");
        assert_eq!(s("entre neuf heures et dix heures trente", "fr"), "entre 9h et 10h30");
        // A duration stays a duration.
        assert_eq!(s("trois heures de route", "fr"), "3 heures de route");
        assert_eq!(s("pendant deux heures", "fr"), "pendant 2 heures");
        assert_eq!(s("il a trois heures de retard", "fr"), "il a 3 heures de retard");
    }

    #[test]
    fn german() {
        assert_eq!(s("um neun Uhr fünfzehn", "de"), "um 9:15 Uhr");
        assert_eq!(s("um neun Uhr", "de"), "um 9 Uhr");
        assert_eq!(s("um siebzehn Uhr dreißig.", "de"), "um 17:30 Uhr.");
        assert_eq!(s("um halb zehn", "de"), "um 9:30");
        assert_eq!(s("um Viertel nach neun", "de"), "um 9:15");
        assert_eq!(s("um Viertel vor zehn", "de"), "um 9:45");
        assert_eq!(s("gegen dreiviertel zehn", "de"), "gegen 9:45");
        assert_eq!(s("um halb eins", "de"), "um 12:30");
        // No cue: "halb" and "Viertel" are words.
        assert_eq!(s("halb so viel", "de"), "halb so viel");
        assert_eq!(s("drei Stunden", "de"), "3 Stunden");
    }

    #[test]
    fn spanish() {
        assert_eq!(s("a las nueve y cuarto", "es"), "a las 9:15");
        assert_eq!(s("a las nueve y quince", "es"), "a las 9:15");
        assert_eq!(s("a las nueve y media", "es"), "a las 9:30");
        assert_eq!(s("a las diez menos cuarto", "es"), "a las 9:45");
        assert_eq!(s("a la una en punto", "es"), "a la 1:00");
        assert_eq!(s("las diecisiete treinta", "es"), "las 17:30");
        assert_eq!(s("Las nueve y media, vale", "es"), "Las 9:30, vale");
        // An hour alone is left to the walk.
        assert_eq!(s("a las nueve", "es"), "a las 9");
        assert_eq!(s("las tres casas", "es"), "las 3 casas");
    }

    #[test]
    fn other_languages_pass_through() {
        assert_eq!(super::fold_times("om negen uur", "nl"), "om negen uur");
    }
}
