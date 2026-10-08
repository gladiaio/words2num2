//! sw: num2words2 renders the class-1 concord "wa" (wa kwanza, wa pili). The
//! ordinal agrees with its noun class: ya / la / cha / vya / za / kwa / pa /
//! mwa kwanza; and "kwanza" / "pili" are also said bare.

const CONCORDS: &[&str] = &["ya", "la", "cha", "vya", "za", "kwa", "pa", "mwa", "wa"];

pub fn variants(norm: &str) -> Vec<String> {
    match norm.split_once(' ') {
        Some((c, rest)) if CONCORDS.contains(&c) => vec![format!("wa {rest}")],
        // Bare: only the two words that are not also cardinals (tatu, nne …
        // stay cardinals).
        None if matches!(norm, "kwanza" | "pili") => vec![format!("wa {norm}")],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_171_inflected_ordinals_parse() {
        assert_eq!(ord("sw", "ya kwanza"), Some(1), "ya kwanza");
        assert_eq!(ord("sw", "la kwanza"), Some(1), "la kwanza");
        assert_eq!(ord("sw", "cha pili"), Some(2), "cha pili");
        assert_eq!(ord("sw", "kwanza"), Some(1), "kwanza");
        assert_eq!(ord("sw", "tatu"), None, "tatu");
    }
}
