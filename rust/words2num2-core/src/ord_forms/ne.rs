//! Nepali. num2words2 writes the masculine ordinal (पहिलो, दोस्रो); the
//! feminine ends in -ी (पहिली, दोस्री, तेस्री) (#155).

use super::suffix::{map_last, swap_first};

pub fn variants(norm: &str) -> Vec<String> {
    map_last(norm, |t| swap_first(t, &[("ी", "ो")]))
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn nepali_feminine_ordinals_issue_155() {
        for (input, want) in [("पहिली", 1), ("दोस्री", 2), ("तेस्री", 3)]
        {
            assert_eq!(
                crate::lookup("ne", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
