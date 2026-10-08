//! Punjabi. num2words2 writes the masculine direct ordinal (ਪਹਿਲਾ, ਪੰਜਵਾਂ);
//! the feminine is ਪਹਿਲੀ / ਪੰਜਵੀਂ and the oblique ਪਹਿਲੇ / ਪੰਜਵੇਂ (#157).

use super::suffix::{map_last, swap_first};

const ENDINGS: &[(&str, &str)] = &[("ਵੀਂ", "ਵਾਂ"), ("ਵੇਂ", "ਵਾਂ"), ("ੀ", "ਾ"), ("ੇ", "ਾ")];

pub fn variants(norm: &str) -> Vec<String> {
    map_last(norm, |t| swap_first(t, ENDINGS))
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn punjabi_feminine_and_oblique_ordinals_issue_157() {
        for (input, want) in [
            ("ਪਹਿਲੀ", 1),
            ("ਦੂਜੀ", 2),
            ("ਪਹਿਲੇ", 1),
            ("ਪੰਜਵੀਂ", 5),
            ("ਪੰਜਵੇਂ", 5),
        ] {
            assert_eq!(
                crate::lookup("pa", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
