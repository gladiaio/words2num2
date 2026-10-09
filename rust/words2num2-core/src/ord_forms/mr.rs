//! Marathi. num2words2 writes the masculine ordinal (पहिला, पाचवा); the
//! feminine ends in -ी (पहिली, पाचवी) and the neuter in -े (पहिले) (#154).

use super::suffix::{map_last, swap_first};

pub fn variants(norm: &str) -> Vec<String> {
    map_last(norm, |t| swap_first(t, &[("ी", "ा"), ("े", "ा")]))
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn marathi_feminine_and_neuter_ordinals_issue_154() {
        for (input, want) in [("पहिली", 1), ("दुसरी", 2), ("पहिले", 1), ("पाचवी", 5)]
        {
            assert_eq!(
                crate::lookup("mr", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
