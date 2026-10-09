//! el: num2words2 renders the masculine nominative (πρώτος, δεύτερος,
//! τρίτος), accent-stripped "πρωτος". Feminine / neuter / case / plural
//! endings: πρώτη, πρώτης, πρώτο, πρώτου, πρώτον, πρώτοι, πρώτες, πρώτα,
//! πρώτων, πρώτους. Every element of a compound inflects ("εικοστή πρώτη").

use super::endings::{per_token, swap};

const M: &[&str] = &["ος"];

const RULES: &[(&str, &[&str])] = &[
    ("η", M),
    ("ης", M),
    ("ο", M),
    ("ου", M),
    ("ον", M),
    ("οι", M),
    ("ες", M),
    ("α", M),
    ("ων", M),
    ("ους", M),
    ("ε", M),
];

pub fn variants(norm: &str) -> Vec<String> {
    per_token(norm, |t| swap(t, RULES, 2))
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_150_inflected_ordinals_parse() {
        assert_eq!(ord("el", "πρώτη"), Some(1), "πρώτη");
        assert_eq!(ord("el", "πρώτο"), Some(1), "πρώτο");
        assert_eq!(ord("el", "πρώτου"), Some(1), "πρώτου");
        assert_eq!(ord("el", "πρώτης"), Some(1), "πρώτης");
        assert_eq!(ord("el", "δεύτερη"), Some(2), "δεύτερη");
        assert_eq!(ord("el", "δεύτερο"), Some(2), "δεύτερο");
        assert_eq!(ord("el", "τρίτη"), Some(3), "τρίτη");
        assert_eq!(ord("el", "εικοστή πρώτη"), Some(21), "εικοστή πρώτη");
    }

    #[test]
    fn issue_230_and_244_compounds_inflect_every_element() {
        assert_eq!(ord("el", "δέκατος όγδοος"), Some(18));
        assert_eq!(ord("el", "εκατοστός εικοστός πρώτος"), Some(121));
        assert_eq!(ord("el", "εκατοστή εικοστή πρώτη"), Some(121));
        // The renderer's "εκατοστός εικοστός" (written for 121..129) is 120th.
        assert_eq!(ord("el", "εκατοστός εικοστός"), Some(120));
        // "δέκα δεύτερα" ("ten seconds"): a cardinal element, not a compound ordinal.
        assert_eq!(ord("el", "δέκα δεύτερα"), None);
    }
}
