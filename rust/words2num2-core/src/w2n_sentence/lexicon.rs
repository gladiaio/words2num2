//! Per-language word lists of the sentence walker that the renderer can
//! provide, derived from it and cached.

use num_bigint::BigInt;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// The values whose renderings are scanned for connectors: every number to
/// 1100, the round thousands, millions and billions, and a few multi-scale
/// values, so each place a connector can sit ("og" after a hundred, "de"
/// before "mii", "leh" between groups) is seen, and every number word shows
/// up at the end of some rendering (cs "dvě stě", da "to tusinde").
fn sample_values() -> Vec<i64> {
    let mut v: Vec<i64> = (1..=1100).collect();
    v.extend((1000..=20_000).step_by(1000));
    v.extend((100_000..=900_000).step_by(100_000));
    for p in [1_000_000i64, 1_000_000_000] {
        v.extend((1..=9).map(|k| k * p));
        v.extend([10 * p, 100 * p]);
    }
    v.extend([
        1234, 2001, 2345, 5555, 12_345, 21_000, 99_999, 100_001, 123_456, 300_500, 1_000_001,
        1_234_567, 1_234_567_891,
    ]);
    v
}

/// The words num2words2 writes *between* the number words of a cardinal and
/// never at its start or end: is "og", fa "و", sq "e", bg "и", ha "da", lus
/// "leh", vi "lẻ", ro "de", sw "na". Normalised.
///
/// A connector is allowed inside a number run but a run never ends on one, so
/// "fjörutíu og tveir" is read whole (42, not "40 og 2") and "douăzeci de lei"
/// keeps its "de" ("20 de lei", not "20 lei") (#65). Derived from the
/// renderer rather than listed by hand: the hand list covered eleven
/// languages. A word that ends a rendering ("cents" in fr "deux cents") is a
/// number word in its own right and is left out.
pub(super) fn derived_connectors(lang: &str) -> &'static [String] {
    static CACHE: OnceLock<RwLock<HashMap<String, &'static [String]>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.read().ok().and_then(|m| m.get(lang).copied()) {
        return v;
    }
    let derived: &'static [String] = Box::leak(compute_connectors(lang).into_boxed_slice());
    if let Ok(mut m) = cache.write() {
        m.insert(lang.to_string(), derived);
    }
    derived
}

fn compute_connectors(lang: &str) -> Vec<String> {
    let Some(renderer) = num2words2_core::get_lang_by_key(lang) else {
        return Vec::new();
    };
    let mut interior: Vec<String> = Vec::new();
    let mut edge: Vec<String> = Vec::new();
    for v in sample_values() {
        let Ok(text) = renderer.to_cardinal(&BigInt::from(v)) else { continue };
        let norm = crate::normalize(&text);
        let toks: Vec<&str> = norm.split_whitespace().collect();
        for (k, t) in toks.iter().enumerate() {
            let list = if k == 0 || k + 1 == toks.len() { &mut edge } else { &mut interior };
            if !list.iter().any(|w| w == t) {
                list.push(t.to_string());
            }
        }
    }
    interior
        .into_iter()
        .filter(|t| !edge.contains(t))
        .filter(|t| !t.chars().any(|c| c.is_ascii_digit()))
        .filter(|t| !matches!(crate::lookup(lang, t, false, &[]), Ok(Some(_))))
        .filter(|t| crate::parse_scaled_norm(lang, t).is_none())
        .collect()
}

/// Does num2words2 spell `value` exactly as `text` (both normalised, hyphens
/// read as spaces)? The authority for a reading the walker cannot check
/// arithmetically.
pub(super) fn renders_as(lang: &str, value: &BigInt, text: &str) -> bool {
    let Some(renderer) = num2words2_core::get_lang_by_key(lang) else {
        return false;
    };
    let Ok(rendered) = renderer.to_cardinal(value) else {
        return false;
    };
    let canon = |s: &str| crate::normalize(s).replace('-', " ").split_whitespace().collect::<Vec<_>>().join(" ");
    canon(&rendered) == canon(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #65: the connectors come from the renderer, and a word that ends a
    /// rendering (fr "cents", cs "stě") is not one.
    #[test]
    fn connectors_are_derived_from_the_renderer() {
        for (lang, conn) in [("is", "og"), ("no", "og"), ("fa", "و"), ("sq", "e"), ("bg", "и"),
                             ("ha", "da"), ("lus", "leh"), ("ro", "de"), ("sw", "na"), ("vi", "le")] {
            assert!(derived_connectors(lang).iter().any(|c| c == conn), "{lang} {conn}");
        }
        assert!(!derived_connectors("fr").iter().any(|c| c == "cents" || c == "vingts"));
        assert!(!derived_connectors("cs").iter().any(|c| c == "ste"));
    }
}
