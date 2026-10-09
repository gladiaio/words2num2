//! pl: num2words2 renders the masculine nominative (pierwszy, drugi,
//! trzeci). Polish inflects every element of a compound ("dwudziesta
//! pierwsza", "dwudziestego pierwszego"), so each token is canonicalized.

use super::endings::{per_token, swap};

/// Back to both masculine endings: hard -y (pierwszy), velar / soft -i
/// (drugi, trzeci). The table keeps whichever exists.
const M: &[&str] = &["y", "i"];

const RULES: &[(&str, &[&str])] = &[
    ("a", M),
    ("e", M),
    ("ego", M),
    ("ej", M),
    ("emu", M),
    ("ym", M),
    ("ych", M),
    ("ymi", M),
    ("ia", M), // trzecia, drugiej: soft and velar stems keep the i
    ("ie", M),
    ("iego", M),
    ("iej", M),
    ("iemu", M),
    ("im", M),
    ("ich", M),
    ("imi", M),
    // Masculine-personal plural: pierwsi, drudzy, czwarci, siódmi.
    ("si", &["szy"]),
    ("dzy", &["gi"]),
    ("ci", &["ty"]),
    ("mi", &["my"]),
];

pub fn variants(norm: &str) -> Vec<String> {
    per_token(norm, |t| swap(t, RULES, 3))
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_140_inflected_ordinals_parse() {
        assert_eq!(ord("pl", "pierwsza"), Some(1), "pierwsza");
        assert_eq!(ord("pl", "pierwsze"), Some(1), "pierwsze");
        assert_eq!(ord("pl", "pierwszego"), Some(1), "pierwszego");
        assert_eq!(ord("pl", "pierwszej"), Some(1), "pierwszej");
        assert_eq!(ord("pl", "pierwsi"), Some(1), "pierwsi");
        assert_eq!(ord("pl", "druga"), Some(2), "druga");
        assert_eq!(ord("pl", "trzecia"), Some(3), "trzecia");
        assert_eq!(ord("pl", "trzeciego"), Some(3), "trzeciego");
        assert_eq!(
            ord("pl", "dwudziesta pierwsza"),
            Some(21),
            "dwudziesta pierwsza"
        );
    }
}
