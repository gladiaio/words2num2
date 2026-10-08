//! hr / sr / bs: num2words2 renders the masculine nominative (prvi / први,
//! drugi, treći -> "treci"). One ending map serves the three standards in
//! both scripts: prva, prvo, prvog(a), prvom(u), prvu, prvih -> prvi.

use super::endings::{last_token, swap};

const RULES: &[(&str, &[&str])] = &[
    ("a", &["i"]),
    ("o", &["i"]),
    ("e", &["i"]),
    ("og", &["i"]),
    ("oga", &["i"]),
    ("om", &["i"]),
    ("omu", &["i"]),
    ("u", &["i"]),
    ("ih", &["i"]),
    ("im", &["i"]),
    ("ima", &["i"]),
    // Soft stems (treći): trećeg(a), trećem(u).
    ("eg", &["i"]),
    ("ega", &["i"]),
    ("em", &["i"]),
    ("emu", &["i"]),
    ("а", &["и"]),
    ("о", &["и"]),
    ("е", &["и"]),
    ("ог", &["и"]),
    ("ога", &["и"]),
    ("ом", &["и"]),
    ("ому", &["и"]),
    ("у", &["и"]),
    ("их", &["и"]),
    ("им", &["и"]),
    ("има", &["и"]),
    ("ег", &["и"]),
    ("ега", &["и"]),
    ("ем", &["и"]),
    ("ему", &["и"]),
];

pub fn variants(norm: &str) -> Vec<String> {
    last_token(norm, |t| swap(t, RULES, 2))
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_144_inflected_ordinals_parse() {
        assert_eq!(ord("hr", "prva"), Some(1), "prva");
        assert_eq!(ord("hr", "prvo"), Some(1), "prvo");
        assert_eq!(ord("hr", "prvog"), Some(1), "prvog");
        assert_eq!(ord("hr", "druga"), Some(2), "druga");
        assert_eq!(ord("hr", "treća"), Some(3), "treća");
        assert_eq!(ord("hr", "trećeg"), Some(3), "trećeg");
    }

    #[test]
    fn issue_144_serbian_cyrillic_and_bosnian() {
        assert_eq!(ord("sr", "прва"), Some(1));
        assert_eq!(ord("sr", "трећа"), Some(3));
        assert_eq!(ord("sr", "трећег"), Some(3));
        assert_eq!(ord("bs", "prva"), Some(1));
    }

    #[test]
    fn issue_244_cardinal_prefix_and_ordinal_last_element() {
        assert_eq!(ord("hr", "dvadeset prvi"), Some(21));
        assert_eq!(ord("sr", "двадесет први"), Some(21));
        // Not a compound ordinal: "five others".
        assert_eq!(ord("hr", "pet drugih"), None);
    }
}
