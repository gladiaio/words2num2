//! `words2num_sentence(..., phone=True)`: phone numbers written the way their
//! country writes them.
//!
//! The walk already turns a dictated number into digits ("06 12 34 56 78",
//! "2124567890", "plus 44 7400123456"); this pass takes each run of digit
//! groups (an optional "plus" / "+" first, commas allowed between groups)
//! and formats it with Google libphonenumber's numbering plans:
//!
//! | spoken | written |
//! |---|---|
//! | fr zéro six douze trente-quatre cinquante-six soixante-dix-huit | 06 12 34 56 78 |
//! | en two one two four five six seven eight nine oh | (212) 456-7890 |
//! | en oh seven four double oh one two three four five six (en_GB) | 07400 123456 |
//! | de null drei null eins zwei drei vier fünf sechs | 030 123456 |
//! | plus thirty three six twelve thirty four fifty six seventy eight | +33 6 12 34 56 78 |
//!
//! Which country:
//!
//! * a spoken country code decides ("plus 33"), and the number is written in
//!   the international format;
//! * otherwise `region` ("CH"), or the region of the language code ("fr_CA"),
//!   or the regions where the language is spoken, the main one first (fr:
//!   FR, BE, CH, CA, LU). The first region where the number is valid **and**
//!   dialled exactly so (its trunk prefix included: "19842026" is no Irish
//!   "(01) 984 2026") writes it. "0612345678" is French for fr, Swiss with
//!   `region="CH"` ("061 234 56 78").
//!
//! A run of fewer than eight digits, or one no plan knows, stays as the walk
//! wrote it.

use phonenumber::country::Id;
use phonenumber::Mode;

const TRAILING: &[char] = &[',', ';', ':', '!', '?', '.', ')', '"', '»'];

fn split_trail(tok: &str) -> (&str, &str) {
    let core = tok.trim_end_matches(TRAILING);
    (core, &tok[core.len()..])
}

/// The regions a language's speakers dial without a country code, the main
/// one first.
fn language_regions(base: &str) -> &'static [&'static str] {
    match base {
        "en" => &["US", "GB", "CA", "AU", "IE", "NZ", "IN", "ZA"],
        "fr" => &["FR", "BE", "CH", "CA", "LU"],
        "de" => &["DE", "AT", "CH"],
        "es" => &["ES", "MX", "AR", "CO", "CL", "PE"],
        "it" => &["IT", "CH"],
        "pt" => &["PT", "BR"],
        "nl" => &["NL", "BE"],
        _ => &[],
    }
}

/// Where a number without a country code may be from: `region`, else the
/// region of the language code, else the regions of the language.
fn regions(lang: &str, region: Option<&str>) -> Vec<Id> {
    let mut it = lang.split(&['_', '-'][..]);
    let base = it.next().unwrap_or(lang).to_lowercase();
    let named = region.map(str::to_owned).or_else(|| it.next().map(str::to_owned));
    match named.map(|r| r.to_uppercase()) {
        Some(r) => (if r == "UK" { "GB".to_string() } else { r }).parse::<Id>().into_iter().collect(),
        None => language_regions(&base).iter().filter_map(|r| r.parse::<Id>().ok()).collect(),
    }
}

/// A run of digit groups at token `i`.
struct Run {
    plus: bool,
    digits: String,
    taken: usize,
    trail: String,
}

fn run_at(toks: &[&str], i: usize) -> Option<Run> {
    let first = split_trail(toks.get(i)?).0.to_lowercase();
    let word_plus = first == "plus" || first == "+";
    let mut k = i + usize::from(word_plus);
    let mut plus = word_plus;
    let mut digits = String::new();
    let mut trail = String::new();
    while let Some(tok) = toks.get(k) {
        let (core, t) = split_trail(tok);
        let core = match core.strip_prefix('+') {
            Some(rest) if k == i => {
                plus = true;
                rest
            }
            _ => core,
        };
        if core.is_empty() || !core.bytes().all(|b| b.is_ascii_digit()) {
            break;
        }
        digits.push_str(core);
        trail = t.to_string();
        k += 1;
        // A comma may sit between groups ("06, 12, 34"); anything else ends it.
        if !(t.is_empty() || t == ",") {
            break;
        }
    }
    (!digits.is_empty()).then_some(Run { plus, digits, taken: k - i, trail })
}

fn only_digits(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

/// The written form of a run, if a numbering plan knows it.
fn written(run: &Run, candidates: &[Id]) -> Option<String> {
    if !(8..=15).contains(&run.digits.len()) {
        return None;
    }
    if run.plus {
        let n = phonenumber::parse(None, format!("+{}", run.digits)).ok()?;
        return phonenumber::is_valid(&n).then(|| n.format().mode(Mode::International).to_string());
    }
    candidates.iter().find_map(|&c| {
        let n = phonenumber::parse(Some(c), &run.digits).ok()?;
        let national = n.format().mode(Mode::National).to_string();
        // Dialled exactly so in that country: the trunk prefix too.
        (phonenumber::is_valid(&n) && only_digits(&national) == run.digits).then_some(national)
    })
    // A plan with no grouping for it ("061234567890"): the groups as spoken
    // beat one long run of digits.
    .filter(|w| w.contains(|c: char| !c.is_ascii_digit()))
}

/// Rewrite every phone number of `text` (the walk's output) as its country
/// writes it. A run no numbering plan knows comes back unchanged.
pub fn fold_phones(text: &str, lang: &str, region: Option<&str>) -> String {
    let candidates = regions(lang, region);
    let toks: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(toks.len());
    let mut i = 0;
    while i < toks.len() {
        match run_at(&toks, i) {
            Some(run) => {
                match written(&run, &candidates) {
                    Some(w) => {
                        // A comma after the last group is the sentence's.
                        let trail = if run.trail == "," { "," } else { run.trail.as_str() };
                        out.push(format!("{w}{trail}"));
                    }
                    // Not a phone number: the whole run stays as it was (its
                    // tail is not tried again on its own).
                    None => out.extend(toks[i..i + run.taken].iter().map(|t| t.to_string())),
                }
                i += run.taken;
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
    fn walk(text: &str, lang: &str, region: Option<&str>) -> String {
        crate::w2n_sentence::words2num_sentence_full(text, lang, "cardinal", false, false, false, region, "written", true)
            .unwrap()
    }

    fn plain(text: &str, lang: &str) -> String {
        crate::w2n_sentence::words2num_sentence(text, lang, "cardinal", false).unwrap()
    }

    #[test]
    fn national() {
        assert_eq!(
            walk("mon numéro est zéro six douze trente-quatre cinquante-six soixante-dix-huit", "fr", None),
            "mon numéro est 06 12 34 56 78"
        );
        assert_eq!(
            walk("zéro six, douze, trente-quatre, cinquante-six, soixante-dix-huit.", "fr", None),
            "06 12 34 56 78."
        );
        assert_eq!(walk("call me at two one two four five six seven eight nine oh", "en", None), "call me at (212) 456-7890");
        assert_eq!(walk("oh seven four double oh one two three four five six", "en_GB", None), "07400 123456");
        assert_eq!(walk("null drei null eins zwei drei vier fünf sechs", "de", None), "030 123456");
        assert_eq!(walk("seis uno dos tres cuatro cinco seis siete ocho", "es", None), "612 34 56 78");
    }

    #[test]
    fn spoken_country_code() {
        assert_eq!(
            walk("plus trente-trois six douze trente-quatre cinquante-six soixante-dix-huit", "fr", None),
            "+33 6 12 34 56 78"
        );
        assert_eq!(
            walk("plus forty four seven four double oh one two three four five six", "en", None),
            "+44 7400 123456"
        );
    }

    #[test]
    fn region() {
        let digits = "zéro six douze trente-quatre cinquante-six soixante-dix-huit";
        assert_eq!(walk(digits, "fr", Some("CH")), "061 234 56 78");
        assert_eq!(walk("zéro six douze trente-quatre cinquante-six septante-huit", "fr_CH", None), "061 234 56 78");
        assert_eq!(walk("oh seven four double oh one two three four five six", "en", Some("GB")), "07400 123456");
        // A region the number does not fit: the digits stay.
        assert_eq!(walk("zero six one two three four five six seven eight", "en", Some("US")), "0612345678");
    }

    #[test]
    fn not_a_phone() {
        for (text, lang) in [
            // A year pair is no Irish landline: the trunk "0" is missing.
            ("it was nineteen eighty four twenty twenty six", "en"),
            ("I have five apples and three pears", "en"),
            ("press one two three", "en"),
            ("douze personnes", "fr"),
            // A fictional number (555) is not valid anywhere.
            ("five five five two three four five six seven eight", "en"),
        ] {
            assert_eq!(walk(text, lang, None), plain(text, lang), "{text}");
        }
        // A rejected run is not retried from its second group.
        assert_eq!(walk("zéro six douze trente-quatre cinquante-six soixante-dix-huit", "fr", Some("US")), "06 12 34 56 78");
    }
}
