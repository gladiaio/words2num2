//! lt: num2words2 renders the indefinite masculine (pirmas, antras, trečias;
//! 100th "šimtasis" is definite). Feminine, definite (pronominal) and case
//! forms: pirma, pirmoji, pirmasis, pirmą, pirmąją, pirmojo, pirmosios.

use super::endings::{last_token, swap};

const M: &[&str] = &["as", "asis"];

const RULES: &[(&str, &[&str])] = &[
    ("a", M),
    ("as", M),
    ("asis", M),
    ("oji", M),
    ("aja", M), // pirmąją
    ("ojo", M),
    ("osios", M),
    ("o", M),
    ("os", M),
    ("ai", M),
    ("ieji", M),
    ("u", M),
    ("us", M),
    ("ame", M),
    ("ajame", M),
    ("oje", M),
];

pub fn variants(norm: &str) -> Vec<String> {
    last_token(norm, |t| swap(t, RULES, 3))
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_146_inflected_ordinals_parse() {
        assert_eq!(ord("lt", "pirma"), Some(1), "pirma");
        assert_eq!(ord("lt", "pirmoji"), Some(1), "pirmoji");
        assert_eq!(ord("lt", "pirmasis"), Some(1), "pirmasis");
        assert_eq!(ord("lt", "antra"), Some(2), "antra");
        assert_eq!(ord("lt", "trečia"), Some(3), "trečia");
    }

    #[test]
    fn issue_244_compound_ordinals() {
        assert_eq!(ord("lt", "dvidešimt pirmas"), Some(21));
        assert_eq!(ord("lt", "dvidešimt pirmoji"), Some(21));
    }
}
