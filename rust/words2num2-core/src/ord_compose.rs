//! Ordinals above the reverse table (#125).
//!
//! The ordinal table stops at 10000 (plus the powers of ten), and ordinal
//! mode used to be table-only, so "dix mille unième", "zwanzigtausendste" or
//! "douze mille trois cent quarante-cinquième" raised in every language. An
//! ordinal above the table is a cardinal head and an ordinal tail: "dix
//! mille" + "unième" (10000 + 1), "zwanzig" + "tausendste" (20 × 1000). The
//! tail is a table hit, the head goes through the cardinal reader
//! ([`crate::parse_scaled_norm`]), and the candidate value is only accepted
//! when the renderer spells that very ordinal — so a split can never invent a
//! number.

use num_bigint::BigInt;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

/// The value of the normalized ordinal `norm` composed from a cardinal head
/// and an ordinal tail, or `None`.
pub(crate) fn compose(lang: &str, norm: &str) -> Option<i64> {
    let toks: Vec<&str> = norm.split(' ').collect();
    let last = *toks.last()?;
    let renderer = num2words2_core::get_lang_by_key(lang)?;
    let scales = scale_morphemes(lang);
    let ordinal = |t: &str| {
        crate::lookup_norm(lang, t, true, &[])
            .ok()
            .flatten()
            .filter(|&v| v > 0)
    };
    // Cheap gate: a composed ordinal holds a scale word (mille, tausend) or
    // ends on a scale ordinal (fr "millième", which does not contain "mille").
    let has_scale = scales.iter().any(|w| norm.contains(w.as_str()));
    if !has_scale && !ordinal(last).is_some_and(|v| v >= 1000 && crate::is_power_of_ten(v)) {
        return None;
    }
    let render = |v: i64| {
        renderer
            .to_ordinal(&BigInt::from(v))
            .ok()
            .map(|w| crate::normalize(&w))
    };
    // The renderer must spell exactly this value this way, and no close
    // neighbour the same way: az writes 20000th and 21000th alike ("iyirmi
    // mininci"), ff 20001st and 21000th, so such a reading is declined.
    let spells = |v: i64| {
        let Some(own) = render(v) else { return false };
        crate::spells_ordinal(lang, norm, &own)
            && [1, 999, 1000, 1001, 10_000]
                .iter()
                .flat_map(|d| [v - d, v + d])
                .all(|n| n <= 0 || render(n).is_none_or(|r| r != own))
    };
    let cardinal = |t: &str| {
        crate::lookup_norm(lang, t, false, &[])
            .ok()
            .flatten()
            .or_else(|| crate::parse_scaled_norm(lang, t))
    };
    // The tail of a compound ordinal is often not an ordinal on its own: fr
    // "unième" only exists inside "vingt et unième" / "mille unième". Behind
    // the language's own "1000" it is a table entry, 1000 + t.
    let thousand = renderer
        .to_cardinal(&BigInt::from(1000))
        .ok()
        .map(|w| crate::normalize(&w));
    let tail_value = |tail: &str| {
        ordinal(tail).or_else(|| {
            let k = thousand.as_deref()?;
            [format!("{k} {tail}"), format!("{k}{tail}")]
                .iter()
                .find_map(|t| ordinal(t))
                .map(|v| v - 1000)
                .filter(|v| (1..1000).contains(v))
        })
    };
    let split = |head: &str, tail: &str| -> Option<i64> {
        // A glued cut can leave a piece ending on a separator the table keys
        // never end on: bo "གཉིས་ཁྲི་གཅིག་|སྟོང་པ" (21000th) splits after a tsek,
        // and normalize drops a trailing tsek. Normalize the pieces like keys.
        let (head, tail) = (crate::normalize(head), crate::normalize(tail));
        let (head, tail) = (head.as_str(), tail.as_str());
        if head.is_empty() || tail.is_empty() {
            return None;
        }
        let t = tail_value(tail)?;
        let h = cardinal(head)?;
        [h.checked_add(t), h.checked_mul(t)]
            .into_iter()
            .flatten()
            .find(|&v| v > crate::LOOKUP_HI - 1 && spells(v))
    };
    // Spaced: "dix mille" | "unième".
    for k in (1..toks.len()).rev() {
        if let Some(v) = split(&toks[..k].join(" "), &toks[k..].join(" ")) {
            return Some(v);
        }
    }
    // Glued (de, nl, tr): cut the last word at a scale word inside it.
    let head = toks[..toks.len() - 1].join(" ");
    for w in scales.iter() {
        for (pos, _) in last.match_indices(w.as_str()) {
            for cut in [pos, pos + w.len()] {
                if cut == 0 || cut >= last.len() {
                    continue;
                }
                let h = if head.is_empty() {
                    last[..cut].to_string()
                } else {
                    format!("{head} {}", &last[..cut])
                };
                if let Some(v) = split(&h, &last[cut..]) {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// The scale words of `lang` and the scale morphemes glued inside a word:
/// de renders 1000 as "eintausend" but 20000 as "zwanzigtausend", so the
/// morpheme is the common tail of 1000 and 2000, "tausend".
///
/// Cached per language: the sentence walker tries an ordinal reading on every
/// run extension the cardinal reader declines (#186).
fn scale_morphemes(lang: &str) -> Arc<Vec<String>> {
    static C: OnceLock<RwLock<HashMap<String, Arc<Vec<String>>>>> = OnceLock::new();
    let cache = C.get_or_init(Default::default);
    if let Some(v) = cache.read().unwrap().get(lang) {
        return v.clone();
    }
    let v = Arc::new(
        num2words2_core::get_lang_by_key(lang)
            .map(|r| derive_scale_morphemes(lang, r))
            .unwrap_or_default(),
    );
    cache.write().unwrap().insert(lang.to_string(), v.clone());
    v
}

fn derive_scale_morphemes(
    lang: &str,
    renderer: &(dyn num2words2_core::Lang + Sync),
) -> Vec<String> {
    let mut out: Vec<String> = crate::scale_words(lang)
        .into_iter()
        .map(|(w, _)| w)
        .collect();
    let card = |n: i64| {
        renderer
            .to_cardinal(&BigInt::from(n))
            .ok()
            .map(|w| crate::normalize(&w))
    };
    for m in [1_000i64, 1_000_000, 1_000_000_000] {
        if let (Some(one), Some(two)) = (card(m), card(2 * m)) {
            let tail = crate::common_suffix(&one, &two).trim().to_string();
            if tail.chars().count() >= 2 && !out.contains(&tail) {
                out.push(tail);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compose;

    fn ord(lang: &str, text: &str) -> Option<i64> {
        compose(lang, &crate::normalize(text))
    }

    #[test]
    fn ordinals_above_the_table_compose_issue_125() {
        assert_eq!(ord("fr", "dix mille unième"), Some(10001));
        assert_eq!(
            ord("fr", "douze mille trois cent quarante-cinquième"),
            Some(12345)
        );
        assert_eq!(ord("fr", "deux cent millième"), Some(200000));
        assert_eq!(ord("de", "zwanzigtausendste"), Some(20000));
        assert_eq!(
            ord("de", "zwölftausenddreihundertfünfundvierzigste"),
            Some(12345)
        );
        // bo: the glued cut lands after a tsek, which normalize strips.
        assert_eq!(ord("bo", "གཉིས་ཁྲི་གཅིག་སྟོང་པ་"), Some(21000));
        assert_eq!(ord("es", "millonésimo"), None); // a table hit: the powers of ten are in the table
    }

    #[test]
    fn a_reading_the_renderer_makes_ambiguous_declines() {
        // num2words2 az renders 20000th and 21000th both "iyirmi mininci".
        assert_eq!(ord("az", "iyirmi mininci"), None);
    }

    #[test]
    fn a_split_the_renderer_does_not_spell_declines() {
        // "mille" + "unième" would be 1001, inside the table: not this reader's job.
        assert_eq!(ord("fr", "mille unième"), None);
        assert_eq!(ord("fr", "dix mille"), None);
    }
}
