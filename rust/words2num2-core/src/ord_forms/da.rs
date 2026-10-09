//! da: num2words2 renders the common gender "anden" for 2nd; the neuter is
//! "andet" (the only gendered Danish ordinal).

pub fn variants(norm: &str) -> Vec<String> {
    match norm {
        "andet" => vec!["anden".to_string()],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_160_inflected_ordinals_parse() {
        assert_eq!(ord("da", "andet"), Some(2), "andet");
    }
}
