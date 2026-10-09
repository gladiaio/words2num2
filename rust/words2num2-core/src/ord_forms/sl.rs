//! sl: num2words2 renders the masculine nominative (prvi, drugi, tretji),
//! compounds glued ("enaindvajseti"), so the one token carries the ending.

use super::endings::{per_token, swap};

const M: &[&str] = &["i"];

const RULES: &[(&str, &[&str])] = &[
    ("a", M),
    ("o", M),
    ("e", M),
    ("ega", M),
    ("emu", M),
    ("em", M),
    ("im", M),
    ("ih", M),
    ("ima", M),
    ("imi", M),
];

pub fn variants(norm: &str) -> Vec<String> {
    per_token(norm, |t| swap(t, RULES, 2))
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_143_inflected_ordinals_parse() {
        assert_eq!(ord("sl", "prva"), Some(1), "prva");
        assert_eq!(ord("sl", "prvo"), Some(1), "prvo");
        assert_eq!(ord("sl", "prvega"), Some(1), "prvega");
        assert_eq!(ord("sl", "druga"), Some(2), "druga");
        assert_eq!(ord("sl", "tretja"), Some(3), "tretja");
    }
}
