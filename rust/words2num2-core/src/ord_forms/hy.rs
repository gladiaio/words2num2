//! Armenian. num2words2 glues 11-19 (տասնմեկերորդ) but writes the other
//! compounds as tens + the standalone ordinal of the unit (քսան առաջին).
//! Standard Armenian glues the cardinal and adds -երորդ once (քսանմեկերորդ,
//! քսաներկուերորդ) (#250): strip the suffix, read the cardinal, and give the
//! renderer's spelling of that ordinal.

use super::suffix::map_last;

pub fn variants(norm: &str) -> Vec<String> {
    let Some(cardinal) = map_last(norm, |t| {
        ["երորդ", "րորդ"].iter().find_map(|suf| {
            t.strip_suffix(suf)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
    }) else {
        return Vec::new();
    };
    let Ok(Some(n)) = crate::lookup("hy", &cardinal, false, &[]) else {
        return Vec::new();
    };
    num2words2_core::get_lang_by_key("hy")
        .and_then(|l| l.to_ordinal(&num_bigint::BigInt::from(n)).ok())
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn armenian_glued_compound_ordinals_issue_250() {
        for (input, want) in [
            ("քսանմեկերորդ", 21),
            ("քսաներկուերորդ", 22),
            ("քսան առաջին", 21),
            ("հինգերորդ", 5),
        ] {
            assert_eq!(
                crate::lookup("hy", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
