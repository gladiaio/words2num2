//! The sentence walker's token pre-pass.
//!
//! [`super::tokenize`] cuts the text on whitespace only. That is not enough to
//! find the number words of real text: punctuation sticks to them ("(forty",
//! "«vingt-deux»", "forty…"), and many languages glue a number to a
//! preposition, a case ending or a counter. This pass cuts such a token into
//! zero-width-joined parts, so the walker sees the number word on its own, and
//! records how each part is glued so the text is rebuilt byte for byte when
//! nothing converts. `parts.concat() == sentence` always holds.

/// How a part is attached to its neighbour, when it was cut out of a
/// whitespace token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Glue {
    /// A whole whitespace token, a space run, or punctuation split off a word.
    Free,
    /// A proclitic or elided article cut off the number word after it (ar
    /// "ب", he "ב", fr "d'", it "dell'"). When that word converts, the
    /// separator is written between them (he "ב-1999", ar "ب300000", fr
    /// "d'11").
    Prefix(&'static str),
    /// A case ending or classifier cut off the number word before it (tr
    /// "de" of "üçte", hu "kor", bn "টা", fi "llä"). When the number
    /// converts, the separator is written between them (tr "3'te", hu
    /// "3-kor", bn "12টা", fi "5:llä").
    Suffix(&'static str),
}

/// Punctuation that may open a word and is kept outside the number: brackets,
/// quotes (straight, curly, guillemets, CJK corner brackets), the Spanish
/// inverted marks.
fn is_leading_punct(c: char) -> bool {
    matches!(
        c,
        '(' | '[' | '{' | '"' | '\'' | '`' | '«' | '‹' | '“' | '”' | '„' | '‟' | '‘' | '’' | '‚'
            | '‛' | '¿' | '¡' | '（' | '「' | '『' | '【' | '〈' | '《' | '〔' | '…' | '—' | '–'
    )
}

/// The trailing punctuation the walker has always handled itself: it strips
/// it for the parse and re-appends it ([`super::rstrip_punct`],
/// [`super::trailing_punct`]), and `,` / `.` close a run.
fn is_legacy_trailing(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'')
}

/// Punctuation that may close a word. A trailing run made only of the legacy
/// class stays on the word; one holding anything else ("two)", "forty…",
/// "vingt-deux»", "पाँच।") is cut off whole and kept verbatim.
fn is_trailing_punct(c: char) -> bool {
    is_legacy_trailing(c)
        || matches!(
            c,
            ')' | ']' | '}' | '%' | '…' | '»' | '«' | '›' | '‹' | '“' | '”' | '‘' | '’' | '„'
                | '）' | '」' | '』' | '】' | '〉' | '》' | '〕' | '、' | '。' | '，' | '！'
                | '？' | '：' | '；' | '।' | '॥' | '،' | '؛' | '؟' | '٪' | '።' | '፣' | '፤'
                | '·' | '—' | '–' | '¡' | '¿'
        )
}

/// Cut leading punctuation and a non-legacy trailing punctuation run off a
/// whitespace token: `(forty` -> `(`, `forty`; `forty…` -> `forty`, `…`;
/// `«vingt-deux»` -> `«`, `vingt-deux`, `»`. A token made only of
/// punctuation stays whole.
fn split_punct(tok: &str) -> Vec<&str> {
    let lead_end = tok
        .char_indices()
        .find(|&(_, c)| !is_leading_punct(c))
        .map_or(tok.len(), |(k, _)| k);
    if lead_end == tok.len() {
        return vec![tok];
    }
    let (lead, rest) = tok.split_at(lead_end);
    let mut trail_start = rest.len();
    for (k, c) in rest.char_indices().rev() {
        if is_trailing_punct(c) {
            trail_start = k;
        } else {
            break;
        }
    }
    let (core, trail) = rest.split_at(trail_start);
    let mut out = Vec::with_capacity(3);
    if !lead.is_empty() {
        out.push(lead);
    }
    if !trail.is_empty() && trail.chars().any(|c| !is_legacy_trailing(c)) && !core.is_empty() {
        out.push(core);
        out.push(trail);
    } else {
        out.push(rest);
    }
    out
}

/// Is `part` punctuation split off a word (or a token made only of
/// punctuation)? It never belongs to a number run.
pub(super) fn is_punct_part(part: &str) -> bool {
    !part.is_empty() && part.chars().all(|c| is_leading_punct(c) || is_trailing_punct(c))
}

/// [`super::tokenize`], then each word cut at its punctuation, and for the
/// languages that glue a number to a particle, at that particle.
pub(super) fn split_parts(
    sentence: &str,
    converter: &super::Converter,
    resolved: &str,
) -> (Vec<String>, Vec<Glue>) {
    let mut parts: Vec<String> = Vec::new();
    let mut glue = Vec::new();
    // Was the previous word a number word? A suffixed homograph ("yüzde",
    // percent) is only peeled inside a number.
    let mut prev_number = false;
    for tok in super::tokenize(sentence) {
        if super::py_str_isspace(&tok) {
            parts.push(tok);
            glue.push(Glue::Free);
            continue;
        }
        for piece in split_punct(&tok) {
            if is_punct_part(piece) {
                parts.push(piece.to_string());
                glue.push(Glue::Free);
                continue;
            }
            let pieces = peel(converter, resolved, piece, prev_number);
            prev_number = pieces.iter().any(|(p, g)| *g == Glue::Free && number_like(converter, resolved, p));
            for (p, g) in pieces {
                parts.push(p.to_string());
                glue.push(g);
            }
        }
    }
    (parts, glue)
}

fn is_number(converter: &super::Converter, word: &str) -> bool {
    let w = super::rstrip_punct(word).to_lowercase();
    !w.is_empty() && !super::holds_digit(&w) && converter.is_number_word(&w)
}

/// A number word, or (ar / he) one with the conjunction glued on: "وتسعمائة"
/// (and nine hundred) continues a number.
fn number_like(converter: &super::Converter, resolved: &str, word: &str) -> bool {
    if is_number(converter, word) {
        return true;
    }
    let rest = match base(resolved) {
        "ar" => word.strip_prefix('و'),
        "he" => word.strip_prefix('ו'),
        _ => None,
    };
    rest.is_some_and(|r| is_number(converter, r))
}

fn base(resolved: &str) -> &str {
    resolved.split(&['_', '-'][..]).next().unwrap_or(resolved)
}

/// Cut a proclitic or a case ending off `piece` when what remains is a number
/// word; otherwise the piece whole.
fn peel<'a>(
    converter: &super::Converter,
    resolved: &str,
    piece: &'a str,
    prev_number: bool,
) -> Vec<(&'a str, Glue)> {
    let whole = vec![(piece, Glue::Free)];
    let lang = base(resolved);
    if !matches!(lang, "ar" | "he" | "fr" | "it" | "ca" | "tr" | "hu" | "bn" | "fi") {
        return whole;
    }
    let core = super::rstrip_punct(piece);
    if core.is_empty() || super::holds_digit(core) || is_number(converter, core) {
        return whole;
    }
    // Inside a number the glued conjunction belongs to it (ar "ثلاثة
    // وعشرين", he "תשעים ותשע"), so a proclitic is only cut at a run head.
    let in_number_conjunction = prev_number && matches!(lang, "ar" | "he");
    if let Some((k, sep)) = prefix_cut(converter, lang, core).filter(|_| !in_number_conjunction) {
        return vec![(&piece[..k], Glue::Prefix(sep)), (&piece[k..], Glue::Free)];
    }
    if let Some((k, sep)) = suffix_cut(converter, lang, core, prev_number) {
        // Trailing legacy punctuation stays on the suffix: "üçte." -> "üç", "te.".
        return vec![(&piece[..k], Glue::Free), (&piece[k..], Glue::Suffix(sep))];
    }
    whole
}

/// The byte length of a proclitic of `core` and the separator to write after
/// it, when the rest is a number word (#76, #184).
fn prefix_cut(converter: &super::Converter, lang: &str, core: &str) -> Option<(usize, &'static str)> {
    let rest_is_number = |k: usize| {
        let rest = &core[k..];
        let w = rest.to_lowercase();
        // A lone article-number after an elision ("l'un d'entre eux") is cut
        // too: the walker's article rule keeps it in words.
        is_number(converter, rest)
            || converter.apocope(&w).is_some()
            || super::is_article_word(lang, &w)
    };
    match lang {
        // fr "d'onze", "l'un"; it "dell'otto", "dall'undici", "l'otto"; ca "d'onze".
        "fr" | "it" | "ca" => {
            let (k, c) = core.char_indices().find(|&(_, c)| c == '\'' || c == '’')?;
            let head = &core[..k];
            let k = k + c.len_utf8();
            let elided = head.chars().count() <= 6
                && !head.is_empty()
                && head.chars().all(char::is_alphabetic)
                && k < core.len();
            (elided && rest_is_number(k)).then_some((k, ""))
        }
        // One-letter prepositions and the conjunction glued to the word: ar ب ل
        // ك و ف, he ב ל מ כ ש ו (and two of them: "וב", "שב"). Not the
        // article (ar "ال", he "ה"): "המאה" is "the century".
        "ar" | "he" => {
            let letters: &[char] = if lang == "ar" {
                &['ب', 'ل', 'ك', 'و', 'ف']
            } else {
                &['ב', 'ל', 'מ', 'כ', 'ש', 'ו']
            };
            let sep = if lang == "he" { "-" } else { "" };
            let chars: Vec<(usize, char)> = core.char_indices().collect();
            for n in 1..=2usize {
                if chars.len() <= n + 1 || !chars[..n].iter().all(|(_, c)| letters.contains(c)) {
                    break;
                }
                let k = chars[n].0;
                let rest = crate::normalize(&core[k..]);
                // ar "لست" is "I am not", not "ل" + "ست" (six).
                if lang == "ar" && matches!(rest.as_str(), "ست" | "سبع") {
                    continue;
                }
                if is_number(converter, &core[k..]) {
                    return Some((k, sep));
                }
            }
            None
        }
        _ => None,
    }
}

/// Case endings and classifiers, longest first within each language.
fn suffixes(lang: &str) -> &'static [&'static str] {
    match lang {
        // Locative, ablative, dative, accusative, genitive, instrumental, with
        // the buffer letters.
        "tr" => &[
            "nden", "ndan", "den", "dan", "ten", "tan", "nin", "nın", "nun", "nün", "yle", "yla",
            "de", "da", "te", "ta", "ye", "ya", "yi", "yı", "yu", "yü", "in", "ın", "un", "ün",
            "le", "la", "e", "a", "i", "ı", "u", "ü",
        ],
        "hu" => &[
            "szor", "szer", "ször", "kor", "ban", "ben", "ból", "ből", "nál", "nél", "tól", "től",
            "hoz", "hez", "höz", "ról", "ről", "ra", "re", "at", "et", "öt", "ot", "an", "en",
            "ön", "on", "t",
        ],
        "bn" => &["খানা", "খানি", "গুলো", "গুলি", "জন", "টা", "টি", "টো"],
        "fi" => &[
            "ssa", "ssä", "sta", "stä", "lla", "llä", "lta", "ltä", "lle", "ksi", "tta", "ttä",
            "een", "na", "nä", "en", "n", "a", "ä",
        ],
        _ => &[],
    }
}

/// The separator a language writes between figures and a case ending.
fn suffix_sep(lang: &str) -> &'static str {
    match lang {
        "tr" => "'",
        "hu" => "-",
        "fi" => ":",
        _ => "",
    }
}

/// Number words whose suffixed forms are ordinary words, peeled only inside a
/// number: tr "on" ("ona", "onda": to him, in him), "altı" ("altında":
/// under), "yüz" ("yüzde": percent), "bir" ("biri": someone); hu "egy"
/// ("egykor": formerly), "hét" ("héten": in the week, but "hétkor" is at
/// seven); fi "sata" ("sataa": it rains). Normalised.
fn lone_suffix_excluded(lang: &str, stem: &str, suffix: &str) -> bool {
    match (lang, stem) {
        ("tr", "on" | "alti" | "yuz" | "bir") => true,
        ("hu", "egy") => true,
        ("hu", "het") => suffix != "kor",
        ("fi", "sata") => true,
        _ => false,
    }
}

/// The byte offset where a case ending or classifier starts in `core`, when
/// the stem before it is a number word (#73, #74 fi).
fn suffix_cut(
    converter: &super::Converter,
    lang: &str,
    core: &str,
    prev_number: bool,
) -> Option<(usize, &'static str)> {
    for suf in suffixes(lang) {
        let Some(stem) = core.strip_suffix(suf).or_else(|| {
            // Case-insensitive ending ("ÜÇTE" is rare; the stem keeps its case).
            let lower = core.to_lowercase();
            lower.ends_with(suf).then(|| &core[..core.len() - suf.len()]).filter(|_| core.len() == lower.len())
        }) else {
            continue;
        };
        if stem.chars().count() < 2 {
            continue;
        }
        let norm = crate::normalize(stem);
        // The oblique stems (hu "hárm-", fi "viide-") are read through the
        // aliases ([`super::lexicon`]).
        if !is_number(converter, stem) {
            continue;
        }
        if !prev_number && lone_suffix_excluded(lang, &norm, suf) {
            continue;
        }
        return Some((stem.len(), suffix_sep(lang)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punctuation_is_cut_off_and_kept() {
        assert_eq!(split_punct("(forty"), vec!["(", "forty"]);
        assert_eq!(split_punct("two)"), vec!["two", ")"]);
        assert_eq!(split_punct("two)."), vec!["two", ")."]);
        assert_eq!(split_punct("forty…"), vec!["forty", "…"]);
        assert_eq!(split_punct("«vingt-deux»"), vec!["«", "vingt-deux", "»"]);
        assert_eq!(split_punct("¿veinte?"), vec!["¿", "veinte?"]);
        // The legacy trailing class stays on the word.
        assert_eq!(split_punct("forty,"), vec!["forty,"]);
        assert_eq!(split_punct("\"forty\""), vec!["\"", "forty\""]);
        assert_eq!(split_punct("o'clock"), vec!["o'clock"]);
        assert_eq!(split_punct("…"), vec!["…"]);
        for s in ["“forty two”", "(a) b", "x « y »"] {
            let (parts, _) = split_parts(s, &super::super::converter_for("en"), "en");
            assert_eq!(parts.concat(), s);
        }
    }
}
