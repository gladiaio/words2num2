//! uk: num2words2 renders the masculine nominative (перший, другий, третій),
//! normalized with й -> и, ї -> і ("першии", "другии", "третіи"). Only the
//! last element of a compound inflects: "двадцять перша".

use super::endings::{last_token, swap};

const HARD: &[&str] = &["ии"];
/// Soft третій: третя, третього, третьому … -> "третіи".
const SOFT: &[&str] = &["іи"];

const RULES: &[(&str, &[&str])] = &[
    ("а", HARD),
    ("е", HARD),
    ("ого", HARD),
    ("ому", HARD),
    ("им", HARD),
    ("іи", HARD), // першій (f. dat/loc)
    ("у", HARD),
    ("і", HARD),
    ("их", HARD),
    ("ими", HARD),
    ("ою", HARD),
    ("оі", HARD), // першої (f. gen), ї normalized to і
    ("я", SOFT),
    ("є", SOFT),
    ("ього", SOFT),
    ("ьому", SOFT),
    ("ім", SOFT),
    ("ю", SOFT),
    ("і", SOFT),
    ("іх", SOFT),
    ("іми", SOFT),
    ("ьоі", SOFT),
];

pub fn variants(norm: &str) -> Vec<String> {
    last_token(norm, |t| swap(t, RULES, 3))
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_138_inflected_ordinals_parse() {
        assert_eq!(ord("uk", "перша"), Some(1), "перша");
        assert_eq!(ord("uk", "перше"), Some(1), "перше");
        assert_eq!(ord("uk", "першого"), Some(1), "першого");
        assert_eq!(ord("uk", "друга"), Some(2), "друга");
        assert_eq!(ord("uk", "третя"), Some(3), "третя");
        assert_eq!(ord("uk", "третього"), Some(3), "третього");
        assert_eq!(ord("uk", "двадцять перша"), Some(21), "двадцять перша");
    }
}
