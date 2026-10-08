//! lv: num2words2 renders the definite masculine (pirmais, otrais, trešais).
//! Feminine and case forms: pirmā, otrā, trešā, pirmo, pirmajam, pirmās.

use super::endings::{last_token, swap};

const M: &[&str] = &["ais"];

const RULES: &[(&str, &[&str])] = &[
    ("a", M),
    ("o", M),
    ("as", M),
    ("aja", M),
    ("ajam", M),
    ("ajai", M),
    ("aje", M),
    ("ie", M),
    ("ajiem", M),
    ("ajam", M),
    ("ajos", M),
    ("ajas", M),
    ("aji", M),
    ("u", M),
];

pub fn variants(norm: &str) -> Vec<String> {
    last_token(norm, |t| swap(t, RULES, 3))
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_147_inflected_ordinals_parse() {
        assert_eq!(ord("lv", "pirmā"), Some(1), "pirmā");
        assert_eq!(ord("lv", "otrā"), Some(2), "otrā");
        assert_eq!(ord("lv", "trešā"), Some(3), "trešā");
        assert_eq!(ord("lv", "pirmo"), Some(1), "pirmo");
    }

    #[test]
    fn issue_244_compound_ordinals() {
        assert_eq!(ord("lv", "divdesmit pirmais"), Some(21));
        assert_eq!(ord("lv", "simts divdesmit pirmā"), Some(121));
    }
}
