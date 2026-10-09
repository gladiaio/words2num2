//! is: num2words2 renders the masculine nominative (fyrsti, annar, þriðji).
//! The weak ordinals end -a / -u in the other genders and cases (fyrsta,
//! fyrstu, þriðja); annar is strong and irregular (önnur, annað, annan,
//! aðra …). Every element of a compound inflects ("tuttugasta og fyrsta").

use super::endings::per_token;

const ANNAR: &[&str] = &[
    "onnur", "annað", "annan", "annarri", "annars", "aðra", "aðrir", "aðrar", "oðrum", "oðru",
    "annarrar", "annarra",
];

fn token(t: &str) -> Vec<String> {
    if ANNAR.contains(&t) {
        return vec!["annar".to_string()];
    }
    match t.strip_suffix('a').or_else(|| t.strip_suffix('u')) {
        Some(stem) if stem.chars().count() >= 3 => vec![format!("{stem}i")],
        _ => Vec::new(),
    }
}

pub fn variants(norm: &str) -> Vec<String> {
    per_token(norm, token)
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_159_inflected_ordinals_parse() {
        assert_eq!(ord("is", "fyrsta"), Some(1), "fyrsta");
        assert_eq!(ord("is", "fyrstu"), Some(1), "fyrstu");
        assert_eq!(ord("is", "önnur"), Some(2), "önnur");
        assert_eq!(ord("is", "annað"), Some(2), "annað");
        assert_eq!(ord("is", "þriðja"), Some(3), "þriðja");
    }
}
