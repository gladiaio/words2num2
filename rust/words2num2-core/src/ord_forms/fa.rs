//! Persian. num2words2 writes the postpositive ordinal in -م (دوم, یکم,
//! بیست و یکم); the prenominal form adds -ین (دومین, بیست و یکمین), and
//! "first" is just as often اول / اولین / نخست / نخستین (#174).

use super::suffix::map_last;

pub fn variants(norm: &str) -> Vec<String> {
    if matches!(norm, "اول" | "اولین" | "نخست" | "نخستین") {
        return vec!["یکم".to_string()];
    }
    map_last(norm, |t| {
        t.strip_suffix("ین")
            .filter(|s| s.ends_with('م') && s.chars().count() > 1)
            .map(str::to_string)
    })
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn persian_min_ordinals_and_first_issue_174() {
        for (input, want) in [
            ("دومین", 2),
            ("سومین", 3),
            ("یکمین", 1),
            ("بیست و یکمین", 21),
            ("اول", 1),
            ("نخست", 1),
            ("نخستین", 1),
        ] {
            assert_eq!(
                crate::lookup("fa", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
