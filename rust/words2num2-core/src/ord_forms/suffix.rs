//! Ending rewrites shared by the ordinal-form files.

/// `norm` with its last whitespace token rewritten by `f` (`None`: no rule
/// applies). Most ordinal agreement sits on the last word only: hi "सौ
/// पाँचवीं" (105th, feminine) inflects "पाँचवीं" alone.
pub(super) fn map_last(norm: &str, f: impl Fn(&str) -> Option<String>) -> Option<String> {
    let (head, last) = match norm.rsplit_once(' ') {
        Some((h, l)) => (Some(h), l),
        None => (None, norm),
    };
    let new = f(last)?;
    Some(match head {
        Some(h) => format!("{h} {new}"),
        None => new,
    })
}

/// `tok` with its ending `from` replaced by `to`, when the stem left is not
/// empty.
pub(super) fn swap_end(tok: &str, from: &str, to: &str) -> Option<String> {
    tok.strip_suffix(from)
        .filter(|stem| !stem.is_empty())
        .map(|stem| format!("{stem}{to}"))
}

/// The first ending of `pairs` that `tok` carries, swapped for its
/// canonical counterpart. Order matters: longer endings first.
pub(super) fn swap_first(tok: &str, pairs: &[(&str, &str)]) -> Option<String> {
    pairs.iter().find_map(|(from, to)| swap_end(tok, from, to))
}
