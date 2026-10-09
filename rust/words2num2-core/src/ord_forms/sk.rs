//! sk: num2words2 renders the masculine nominative (prvý, druhý, tretí),
//! normalized "prvy", "druhy", "treti". Slovak inflects every element of a
//! compound ("dvadsiata prvá"), so each token is canonicalized.

use super::endings::{per_token, swap};

const HARD: &[&str] = &["y"];
const SOFT: &[&str] = &["i"];

const RULES: &[(&str, &[&str])] = &[
    // Hard: prvá, prvé, prvého, prvému, prvom, prvou, prvých, prvým(i).
    ("a", HARD),
    ("e", HARD),
    ("eho", HARD),
    ("emu", HARD),
    ("om", HARD),
    ("ou", HARD),
    ("ych", HARD),
    ("ym", HARD),
    ("ymi", HARD),
    ("i", HARD), // prví (m. anim. pl.)
    // Soft tretí / tisíci: tretia, tretie, tretieho, tretiemu, tretiu …
    ("ia", SOFT),
    ("ie", SOFT),
    ("ieho", SOFT),
    ("iemu", SOFT),
    ("iu", SOFT),
    ("ich", SOFT),
    ("im", SOFT),
    ("imi", SOFT),
];

pub fn variants(norm: &str) -> Vec<String> {
    per_token(norm, |t| swap(t, RULES, 2))
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_142_inflected_ordinals_parse() {
        assert_eq!(ord("sk", "prvá"), Some(1), "prvá");
        assert_eq!(ord("sk", "prvé"), Some(1), "prvé");
        assert_eq!(ord("sk", "prvého"), Some(1), "prvého");
        assert_eq!(ord("sk", "druhá"), Some(2), "druhá");
        assert_eq!(ord("sk", "piata"), Some(5), "piata");
        assert_eq!(ord("sk", "tretia"), Some(3), "tretia");
    }

    #[test]
    fn issue_244_compound_ordinals() {
        assert_eq!(ord("sk", "dvadsiaty prvý"), Some(21));
        assert_eq!(ord("sk", "dvadsiata prvá"), Some(21));
    }
}
