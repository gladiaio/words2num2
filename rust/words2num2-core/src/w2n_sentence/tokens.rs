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

/// [`super::tokenize`], then each word cut at its punctuation.
pub(super) fn split_parts(sentence: &str) -> (Vec<String>, Vec<Glue>) {
    let mut parts = Vec::new();
    let mut glue = Vec::new();
    for tok in super::tokenize(sentence) {
        if super::py_str_isspace(&tok) {
            parts.push(tok);
            glue.push(Glue::Free);
            continue;
        }
        for piece in split_punct(&tok) {
            parts.push(piece.to_string());
            glue.push(Glue::Free);
        }
    }
    (parts, glue)
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
            let (parts, _) = split_parts(s);
            assert_eq!(parts.concat(), s);
        }
    }
}
