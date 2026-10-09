//! eo: num2words2 renders the nominative singular (unua, dua); the
//! accusative adds -n, the plural -j: unuan, unuaj, unuajn.

use super::endings::per_token;

fn token(t: &str) -> Vec<String> {
    for suf in ["jn", "n", "j"] {
        if let Some(stem) = t.strip_suffix(suf) {
            if stem.ends_with('a') && stem.chars().count() >= 2 {
                return vec![stem.to_string()];
            }
        }
    }
    Vec::new()
}

pub fn variants(norm: &str) -> Vec<String> {
    per_token(norm, token)
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_165_inflected_ordinals_parse() {
        assert_eq!(ord("eo", "unuan"), Some(1), "unuan");
        assert_eq!(ord("eo", "duan"), Some(2), "duan");
        assert_eq!(ord("eo", "unuaj"), Some(1), "unuaj");
    }
}
