//! ca: num2words2 renders the masculine singular (primer, segon, tercer,
//! quart, cinquè, vint-i-unè). Feminine and plural: primera, segona, quarta,
//! cinquena, primers, cinquens, cinquenes, vint-i-unena.

use super::endings::{last_token, swap};

const RULES: &[(&str, &[&str])] = &[
    // cinquena, cinquens, cinquenes -> cinquè ("cinque" once normalized).
    ("ena", &["e"]),
    ("ens", &["e"]),
    ("enes", &["e"]),
    // primera, segona, tercera, quarta; primers, quarts; primeres, quartes.
    ("a", &[""]),
    ("s", &[""]),
    ("es", &[""]),
];

pub fn variants(norm: &str) -> Vec<String> {
    last_token(norm, |t| swap(t, RULES, 2))
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_149_inflected_ordinals_parse() {
        assert_eq!(ord("ca", "primera"), Some(1), "primera");
        assert_eq!(ord("ca", "segona"), Some(2), "segona");
        assert_eq!(ord("ca", "tercera"), Some(3), "tercera");
        assert_eq!(ord("ca", "quarta"), Some(4), "quarta");
        assert_eq!(ord("ca", "cinquena"), Some(5), "cinquena");
        assert_eq!(ord("ca", "primers"), Some(1), "primers");
        assert_eq!(ord("ca", "cinquens"), Some(5), "cinquens");
        assert_eq!(ord("ca", "vint-i-unena"), Some(21), "vint-i-unena");
    }
}
