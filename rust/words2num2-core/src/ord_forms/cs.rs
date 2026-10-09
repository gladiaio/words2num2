//! cs: num2words2 renders the masculine nominative (první, druhý, třetí),
//! normalized "prvni", "druhy", "treti". Czech inflects every element of a
//! compound ("dvacátá první"), so each token is canonicalized.

use super::endings::{per_token, swap};

const HARD: &[&str] = &["y"];
const SOFT: &[&str] = &["i"];

const RULES: &[(&str, &[&str])] = &[
    // Hard: druhá, druhé, druhého, druhému, druhém, druhou, druhých, druhým(i).
    ("a", HARD),
    ("e", HARD),
    ("eho", HARD),
    ("emu", HARD),
    ("em", HARD),
    ("ou", HARD),
    ("ych", HARD),
    ("ym", HARD),
    ("ymi", HARD),
    // Soft: prvního, prvnímu, prvním, prvních, prvními.
    ("iho", SOFT),
    ("imu", SOFT),
    ("im", SOFT),
    ("ich", SOFT),
    ("imi", SOFT),
];

pub fn variants(norm: &str) -> Vec<String> {
    per_token(norm, |t| swap(t, RULES, 2))
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_141_inflected_ordinals_parse() {
        assert_eq!(ord("cs", "druhá"), Some(2), "druhá");
        assert_eq!(ord("cs", "druhé"), Some(2), "druhé");
        assert_eq!(ord("cs", "druhého"), Some(2), "druhého");
        assert_eq!(ord("cs", "pátá"), Some(5), "pátá");
        assert_eq!(ord("cs", "prvního"), Some(1), "prvního");
        assert_eq!(ord("cs", "třetího"), Some(3), "třetího");
    }

    #[test]
    fn issue_244_compound_ordinals() {
        assert_eq!(ord("cs", "dvacátý první"), Some(21));
        assert_eq!(ord("cs", "dvacátá první"), Some(21));
    }

    #[test]
    fn issue_244_two_ordinals_are_not_a_compound() {
        assert_eq!(ord("cs", "první druhý"), None);
    }
}
