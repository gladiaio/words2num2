//! Japanese. num2words2 writes the ordinal as N番目 (二十一番目); the 第N
//! form (第一, 第二十一章) is just as standard (#175).

pub fn variants(norm: &str) -> Vec<String> {
    let Some(rest) = norm.strip_prefix('第') else {
        return Vec::new();
    };
    let rest = rest.strip_suffix("番目").unwrap_or(rest);
    if rest.is_empty() {
        return Vec::new();
    }
    vec![format!("{rest}番目")]
}

#[cfg(test)]
mod tests {
    #[test]
    fn japanese_dai_ordinals_issue_175() {
        for (input, want) in [("第一", 1), ("第二十一", 21), ("第百", 100)] {
            assert_eq!(
                crate::lookup("ja", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
