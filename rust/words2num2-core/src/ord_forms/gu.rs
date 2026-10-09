//! Gujarati. num2words2 writes the masculine ordinal (પહેલો, પાંચમો); the
//! feminine ends in -ી (પહેલી, બીજી), the neuter in -ું (પહેલું) and the
//! plural / oblique in -ા (પહેલા) (#156).

use super::suffix::{map_last, swap_first};

pub fn variants(norm: &str) -> Vec<String> {
    map_last(norm, |t| {
        swap_first(t, &[("ી", "ો"), ("ું", "ો"), ("ા", "ો")])
    })
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn gujarati_feminine_and_neuter_ordinals_issue_156() {
        for (input, want) in [("પહેલી", 1), ("બીજી", 2), ("પહેલું", 1), ("પાંચમી", 5)]
        {
            assert_eq!(
                crate::lookup("gu", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
