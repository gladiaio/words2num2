//! Urdu. num2words2 writes the masculine direct ordinal (پہلا, پانچواں); the
//! feminine is پہلی / پانچویں and the oblique پہلے / پانچویں (#158).

use super::suffix::{map_last, swap_first};

const ENDINGS: &[(&str, &str)] = &[("ویں", "واں"), ("ی", "ا"), ("ے", "ا")];

pub fn variants(norm: &str) -> Vec<String> {
    map_last(norm, |t| swap_first(t, ENDINGS))
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn urdu_feminine_and_oblique_ordinals_issue_158() {
        for (input, want) in [
            ("پہلی", 1),
            ("دوسری", 2),
            ("پہلے", 1),
            ("پانچویں", 5),
            ("تیسری", 3),
        ] {
            assert_eq!(
                crate::lookup("ur", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
