//! Malay. num2words2 hyphenates the ordinal prefix from 11 on ("ke-sebelas",
//! "ke-dua puluh satu"); Malay glues "ke" to a number word and keeps the
//! hyphen for figures only ("kesebelas", but "ke-11") (#254).

pub fn variants(norm: &str) -> Vec<String> {
    norm.strip_prefix("ke")
        .filter(|rest| rest.chars().next().is_some_and(|c| c.is_alphabetic()))
        .map(|rest| format!("ke {rest}"))
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn malay_glued_ke_ordinals_issue_254() {
        for (input, want) in [
            ("kesebelas", 11),
            ("kedua puluh satu", 21),
            ("keseratus", 100),
            ("ke-sebelas", 11),
            ("kedua", 2),
        ] {
            assert_eq!(
                crate::lookup("ms", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
