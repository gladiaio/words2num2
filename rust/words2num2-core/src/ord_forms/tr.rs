//! Turkish. num2words2 writes the bare ordinal (birinci, ikinci, üçüncü);
//! in a sentence it takes case and possessive suffixes, with vowel harmony
//! and a buffer consonant: ikinciye, birinciyi, birincisi, üçüncüsünde
//! (#173). The suffix is cut after the ordinal ending -inci / -ıncı /
//! -üncü / -uncu.
//!
//! Normalized text has lost the diaeresis and cedilla (üçüncü -> ucuncu,
//! ş -> s), while the dotless ı stays a letter of its own.

use super::suffix::map_last;

const ENDINGS: &[&str] = &["inci", "ıncı", "uncu"];

/// Case, possessive and plural suffixes after an ordinal, normalized.
const SUFFIXES: &[&str] = &[
    "yi", "yı", "yu", "ye", "ya", "de", "da", "te", "ta", "den", "dan", "ten", "tan", "nin", "nın",
    "nun", "le", "la", "yle", "yla", "si", "sı", "su", "sini", "sını", "sunu", "sine", "sına",
    "suna", "sinde", "sında", "sunda", "sinden", "sından", "sundan", "sinin", "sının", "sunun",
    "siyle", "sıyla", "suyla", "ydi", "ydı", "ydu", "dir", "dır", "dur", "ler", "lar", "leri",
    "ları", "lere", "lara", "lerde", "larda", "lerden", "lardan", "lerin", "ların",
];

pub fn variants(norm: &str) -> Vec<String> {
    map_last(norm, |t| {
        let (at, end) = ENDINGS
            .iter()
            .filter_map(|e| t.rfind(e).map(|i| (i, i + e.len())))
            .max()?;
        (at > 0 && SUFFIXES.contains(&&t[end..])).then(|| t[..end].to_string())
    })
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn turkish_case_and_possessive_suffixes_issue_173() {
        for (input, want) in [
            ("ikinciye", 2),
            ("birinciyi", 1),
            ("birincisi", 1),
            ("üçüncüsü", 3),
            ("dokuzuncuda", 9),
            ("yirmibirinciden", 21),
            ("onuncular", 10),
        ] {
            assert_eq!(
                crate::lookup("tr", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
