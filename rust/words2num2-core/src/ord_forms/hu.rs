//! hu: num2words2 renders the nominative (első, második, harmadik). Speech
//! adds the accusative (elsőt, másodikat) and the date possessive + case
//! ("május elsején", "másodikán", "harmadikáig"): elseje -> első,
//! másodika -> második.

use super::endings::last_token;

const CASES: &[&str] = &[
    "", "t", "at", "et", "ot", "n", "an", "en", "on", "ig", "ra", "re", "nak", "nek", "ban", "ben",
    "bol", "tol", "rol", "hoz", "hez", "val", "vel",
];
const POSSESSIVE: &[&str] = &["", "a", "e", "ja", "je"];

fn token(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    for c in CASES {
        let Some(a) = t.strip_suffix(c) else { continue };
        for p in POSSESSIVE {
            let Some(r) = a.strip_suffix(p) else { continue };
            if r.chars().count() < 3 {
                continue;
            }
            // elseje / elsején: the possessive of első is built on "elsej-".
            if let Some(s) = r.strip_suffix("elsej") {
                out.push(format!("{s}elso"));
            }
            out.push(r.to_string());
        }
    }
    out
}

pub fn variants(norm: &str) -> Vec<String> {
    last_token(norm, token)
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_168_inflected_ordinals_parse() {
        assert_eq!(ord("hu", "elsőt"), Some(1), "elsőt");
        assert_eq!(ord("hu", "másodikat"), Some(2), "másodikat");
        assert_eq!(ord("hu", "elsején"), Some(1), "elsején");
        assert_eq!(ord("hu", "másodikán"), Some(2), "másodikán");
        assert_eq!(ord("hu", "harmadikán"), Some(3), "harmadikán");
    }
}
