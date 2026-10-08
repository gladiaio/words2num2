//! Vietnamese. num2words2's to_ordinal returns the bare cardinal, so the
//! ordinal table holds the cardinals; the real ordinal is "thứ" + cardinal,
//! with the suppletive "thứ nhất" (1st) and "thứ tư" (4th) (#245).

pub fn variants(norm: &str) -> Vec<String> {
    let Some(rest) = norm.strip_prefix("thu ") else {
        return Vec::new();
    };
    let rest = match rest {
        "nhat" => "mot",
        "tu" => "bon",
        r => r,
    };
    vec![rest.to_string()]
}

#[cfg(test)]
mod tests {
    #[test]
    fn vietnamese_thu_ordinals_issue_245() {
        for (input, want) in [
            ("thứ mười một", 11),
            ("thứ nhất", 1),
            ("thứ tư", 4),
            ("thứ hai", 2),
        ] {
            assert_eq!(
                crate::lookup("vi", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
