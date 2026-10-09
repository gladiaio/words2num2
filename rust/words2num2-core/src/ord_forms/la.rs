//! la: num2words2 renders the masculine nominative (prīmus, secundus,
//! tertius), macron-stripped "primus". Feminine / neuter / case forms: prima,
//! primum, primo, primi, primae, primam, primorum, primis.

use super::endings::{per_token, swap};

const M: &[&str] = &["us"];

const RULES: &[(&str, &[&str])] = &[
    ("a", M),
    ("um", M),
    ("o", M),
    ("i", M),
    ("ae", M),
    ("am", M),
    ("orum", M),
    ("arum", M),
    ("is", M),
    ("os", M),
    ("as", M),
    ("e", M),
];

pub fn variants(norm: &str) -> Vec<String> {
    per_token(norm, |t| swap(t, RULES, 3))
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_164_inflected_ordinals_parse() {
        assert_eq!(ord("la", "prima"), Some(1), "prima");
        assert_eq!(ord("la", "prīma"), Some(1), "prīma");
        assert_eq!(ord("la", "primum"), Some(1), "primum");
        assert_eq!(ord("la", "primo"), Some(1), "primo");
        assert_eq!(ord("la", "secunda"), Some(2), "secunda");
        assert_eq!(ord("la", "tertia"), Some(3), "tertia");
    }

    #[test]
    fn issue_244_compound_ordinals() {
        assert_eq!(ord("la", "vicesimus primus"), Some(21));
    }
}
