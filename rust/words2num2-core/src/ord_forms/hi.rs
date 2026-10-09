//! Hindi. num2words2 writes the masculine direct ordinal (पहला, पाँचवाँ);
//! speech agrees it with the noun: feminine पहली / पाँचवीं, oblique पहले /
//! पाँचवें (#153). The nasal of -वाँ is often typed as an anusvara (पांचवां).

use super::suffix::{map_last, swap_first};

const ENDINGS: &[(&str, &str)] = &[
    ("वीं", "वाँ"),
    ("वें", "वाँ"),
    ("वी", "वाँ"),
    ("वे", "वाँ"),
    ("वां", "वाँ"),
    ("ी", "ा"),
    ("े", "ा"),
];

pub fn variants(norm: &str) -> Vec<String> {
    let mut out: Vec<String> = map_last(norm, |t| swap_first(t, ENDINGS))
        .into_iter()
        .collect();
    // पांच -> पाँच: the anusvara spelling of the candrabindu, anywhere.
    for v in [norm.to_string()].iter().chain(out.clone().iter()) {
        if v.contains("ां") {
            out.push(v.replace("ां", "ाँ"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn hindi_feminine_and_oblique_ordinals_issue_153() {
        for (input, want) in [
            ("पहली", 1),
            ("दूसरी", 2),
            ("तीसरी", 3),
            ("पहले", 1),
            ("पाँचवीं", 5),
            ("पाँचवें", 5),
            ("पांचवीं", 5),
            ("इक्कीसवीं", 21),
        ] {
            assert_eq!(
                crate::lookup("hi", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
