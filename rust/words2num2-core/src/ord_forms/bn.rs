//! Bengali. num2words2's to_ordinal returns the bare cardinal, so the ordinal
//! table holds the cardinals; the real ordinals are the ones its
//! to_ordinal_num renders: প্রথম, দ্বিতীয় … দশম, then cardinal + -তম
//! (একুশতম) (#245, #246). The suppletive ones are read off the renderer.

use std::sync::OnceLock;

/// `(normalized ordinal, normalized cardinal)` for the ordinals that are not
/// cardinal + -তম.
fn suppletive() -> &'static [(String, String)] {
    static S: OnceLock<Vec<(String, String)>> = OnceLock::new();
    S.get_or_init(|| {
        let Some(l) = num2words2_core::get_lang_by_key("bn") else {
            return Vec::new();
        };
        (1..=20i64)
            .filter_map(|n| {
                let n = num_bigint::BigInt::from(n);
                let ord = crate::normalize(&l.to_ordinal_num(&n).ok()?);
                let card = crate::normalize(&l.to_cardinal(&n).ok()?);
                (!ord.ends_with("তম")).then_some((ord, card))
            })
            .collect()
    })
}

pub fn variants(norm: &str) -> Vec<String> {
    if let Some(stem) = norm
        .strip_suffix("তম")
        .filter(|s| !s.is_empty() && !s.ends_with(' '))
    {
        return vec![stem.to_string()];
    }
    suppletive()
        .iter()
        .filter(|(ord, _)| ord == norm)
        .map(|(_, card)| card.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn bengali_real_ordinals_issue_245() {
        for (input, want) in [
            ("প্রথম", 1),
            ("দ্বিতীয়", 2),
            ("দশম", 10),
            ("একুশতম", 21),
            ("এক হাজারতম", 1000),
        ] {
            assert_eq!(
                crate::lookup("bn", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
