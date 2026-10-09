//! Korean. num2words2 writes "N 번째" with the attributive native numeral
//! (첫 번째, 두 번째, 스무 번째). The native ordinals 첫째, 둘째, 셋째 — the
//! counting form + 째 — are just as standard (#176), and 번째 is often typed
//! without its space (두번째).
//!
//! The normalized text is NFKD, which splits Hangul syllables into jamo; the
//! rules work on recomposed syllables.

use unicode_normalization::UnicodeNormalization;

/// Counting numeral -> attributive form, at the end of the stem.
const ATTRIBUTIVE: &[(&str, &str)] = &[
    ("하나", "한"),
    ("스물", "스무"),
    ("둘", "두"),
    ("셋", "세"),
    ("넷", "네"),
];

pub fn variants(norm: &str) -> Vec<String> {
    // 째 is ᄍ + ᅢ: a cheap gate before recomposing.
    if !norm.ends_with('\u{1162}') {
        return Vec::new();
    }
    let s: String = norm.nfc().collect();
    let Some(stem) = s.strip_suffix('째') else {
        return Vec::new();
    };
    if let Some(num) = stem.strip_suffix('번') {
        let num = num.trim_end();
        return if num.is_empty() {
            Vec::new()
        } else {
            vec![format!("{num} 번째")]
        };
    }
    let stem = stem.trim_end();
    if stem.is_empty() {
        return Vec::new();
    }
    let attributive = ATTRIBUTIVE
        .iter()
        .find_map(|(from, to)| stem.strip_suffix(from).map(|head| format!("{head}{to}")))
        .unwrap_or_else(|| stem.to_string());
    vec![format!("{attributive} 번째")]
}

#[cfg(test)]
mod tests {
    #[test]
    fn korean_native_jjae_ordinals_issue_176() {
        for (input, want) in [
            ("첫째", 1),
            ("둘째", 2),
            ("셋째", 3),
            ("넷째", 4),
            ("다섯째", 5),
            ("열째", 10),
            ("열둘째", 12),
            ("스물째", 20),
            ("두번째", 2),
        ] {
            assert_eq!(
                crate::lookup("ko", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
