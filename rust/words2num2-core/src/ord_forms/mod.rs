//! Inflected ordinals -> the renderer's canonical ordinal spelling.
//!
//! num2words2 renders one ordinal form per number, usually the masculine
//! nominative singular (ru "первый", pl "pierwszy", el "πρώτος", ca "primer").
//! Speech agrees the ordinal with its noun: ru "первая", "третьего", pl
//! "dwudziesta pierwsza", el "πρώτη", ca "cinquena", hu "másodikán". The
//! reverse table only holds the rendered form, so every other form raised.
//!
//! [`variants`] maps a normalized inflected input onto candidate spellings in
//! the renderer's canonical form. The candidates are only ever used as table
//! keys: a candidate that is not a rendered ordinal is a miss, never a guess.
//! The functions here are pure string work — they must not call back into the
//! lookup (the caller holds the table lock while it tries the candidates).
//!
//! One file per language; [`variants`] dispatches on the base language code.

mod endings;

mod be;
mod bg;
mod ca;
mod cs;
mod cy;
mod da;
mod el;
mod eo;
mod et;
mod fi;
mod fr;
mod gl;
mod he;
mod hu;
mod is;
mod la;
mod lt;
mod lv;
mod mt;
mod nb;
mod pl;
mod ro;
mod ru;
mod sh;
mod sk;
mod sl;
mod sq;
mod sv;
mod sw;
mod uk;

/// Candidate spellings, in the renderer's canonical ordinal form, of the
/// inflected normalized ordinal `norm` (empty when the language has no rule or
/// nothing in `norm` is inflected). The input itself is never included.
pub fn variants(lang: &str, norm: &str) -> Vec<String> {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    let mut out = match base {
        "be" => be::variants(norm),
        "bg" => bg::variants(norm),
        "bs" | "hr" | "sr" => sh::variants(norm),
        "ca" => ca::variants(norm),
        "cs" => cs::variants(norm),
        "cy" => cy::variants(norm),
        "da" => da::variants(norm),
        "el" => el::variants(norm),
        "eo" => eo::variants(norm),
        "et" => et::variants(norm),
        "fi" => fi::variants(norm),
        "fr" => fr::variants(norm),
        "gl" => gl::variants(norm),
        "he" => he::variants(norm),
        "hu" => hu::variants(norm),
        "is" => is::variants(norm),
        "la" => la::variants(norm),
        "lt" => lt::variants(norm),
        "lv" => lv::variants(norm),
        "mt" => mt::variants(norm),
        "nb" | "no" => nb::variants(norm),
        "pl" => pl::variants(norm),
        "ro" => ro::variants(norm),
        "ru" => ru::variants(norm),
        "sk" => sk::variants(norm),
        "sl" => sl::variants(norm),
        "sq" => sq::variants(norm),
        "sv" => sv::variants(norm),
        "sw" => sw::variants(norm),
        "uk" => uk::variants(norm),
        _ => Vec::new(),
    };
    out.retain(|v| v != norm);
    let mut seen = std::collections::HashSet::new();
    out.retain(|v| seen.insert(v.clone()));
    out
}

/// How a language builds a compound ordinal: every element ordinal (cs
/// "dvacátý první", el "εικοστός πρώτος", la "vicesimus primus") or a
/// cardinal prefix + an ordinal last element (hr "dvadeset prvi", lt
/// "dvidešimt pirmas", lv "divdesmit pirmais").
enum Compound {
    AllOrdinal,
    LastOrdinal,
}

/// The value of a grammatical compound ordinal the renderer does not write.
///
/// num2words2 forms these ordinals as `cardinal + suffix` on the whole phrase
/// (cs "dvacet jednaý", lt "dvidešimt vienasas", el 121 "εκατοστός εικοστός"),
/// so the real compound never reaches the table. Each ordinal element is read
/// through the ordinal table (its inflections included), replaced by the
/// renderer's cardinal of its value, and the result must parse as a cardinal:
/// "dvacátý první" -> "dvacet jedna" -> 21, "δέκατος όγδοος" -> "δεκα οκτω"
/// -> 18. Anything the cardinal parser rejects ("první druhý" -> "jedna
/// dva") stays a miss. Called outside the table lock, after the table missed.
pub fn compose(lang: &str, norm: &str) -> Option<i64> {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    let style = match base {
        "cs" | "sk" | "el" | "la" => Compound::AllOrdinal,
        "hr" | "sr" | "bs" | "lt" | "lv" => Compound::LastOrdinal,
        _ => return None,
    };
    let toks: Vec<&str> = norm.split(' ').collect();
    if toks.len() < 2 {
        return None;
    }
    let renderer = num2words2_core::get_lang_by_key(lang)?;
    let ordinal_value = |t: &str| -> Option<i64> {
        // A word that is also a cardinal ("sto", "εκατό") is not an ordinal element.
        if matches!(crate::lookup_norm(lang, t, false, &[]), Ok(Some(_))) {
            return None;
        }
        crate::lookup_norm(lang, t, true, &[]).ok().flatten()
    };
    let as_cardinal = |v: i64| -> Option<String> {
        renderer
            .to_cardinal(&num_bigint::BigInt::from(v))
            .ok()
            .map(|w| crate::normalize(&w))
    };
    let mut parts = Vec::with_capacity(toks.len());
    for (i, t) in toks.iter().enumerate() {
        let last = i + 1 == toks.len();
        let ordinal = match style {
            Compound::AllOrdinal => true,
            Compound::LastOrdinal => last,
        };
        if ordinal {
            parts.push(as_cardinal(ordinal_value(t)?)?);
        } else {
            parts.push(t.to_string());
        }
    }
    let card = parts.join(" ");
    if let Ok(Some(v)) = crate::lookup_norm(lang, &card, false, &[]) {
        return Some(v);
    }
    // el "δεκα οκτω": the renderer glues 11-19 ("δεκαοκτω").
    let glued: String = card.split(' ').collect();
    if let Ok(Some(v)) = crate::lookup_norm(lang, &glued, false, &[]) {
        return Some(v);
    }
    crate::parse_scaled_norm(lang, &card)
}

/// Extra reverse-table keys for the ordinal of `n`, where the renderer's own
/// spelling (`rendered`, normalized; `None` when it raised) is wrong or
/// missing and no rewrite of the input could reach a correct key: be renders
/// every x80th as the x70th ("сямідзясяты"), cy raises above 100. Inserted
/// after the rendered key, first write wins, so they never shadow it.
pub fn extra_keys(lang: &str, n: i64, rendered: Option<&str>) -> Vec<String> {
    match lang {
        "be" => be::extra_keys(n, rendered),
        "cy" => cy::extra_keys(n, rendered),
        _ => Vec::new(),
    }
}
