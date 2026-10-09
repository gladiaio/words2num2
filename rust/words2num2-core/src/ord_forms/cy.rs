//! cy: num2words2 renders the radical masculine (cyntaf, trydydd, pedwerydd).
//! Speech adds the feminine (trydedd, pedwaredd), the soft mutation after an
//! article or preposition (gyntaf, drydedd, bedwaredd, ddegfed), and the
//! standard compound / twenty forms the renderer does not use: "unfed ar
//! hugain" (it writes "cyntaf ar hugain") and "ugeinfed" (it writes
//! "ugainfed", without the ai -> ei vowel affection).

use super::endings::per_token;

/// Radical first letters a soft-mutated word may come from.
fn unmutate(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |p: &str, rest: &str| out.push(format!("{p}{rest}"));
    if let Some(r) = t.strip_prefix("dd") {
        push("d", r);
    } else if let Some(r) = t.strip_prefix('d') {
        push("t", r);
    }
    if let Some(r) = t.strip_prefix('g') {
        push("c", r);
    }
    if let Some(r) = t.strip_prefix('b') {
        push("p", r);
    }
    if let Some(r) = t.strip_prefix('f') {
        push("m", r);
        push("b", r);
    }
    if let Some(r) = t.strip_prefix('l') {
        if !r.starts_with('l') {
            push("ll", r);
        }
    }
    if let Some(r) = t.strip_prefix('r') {
        if !r.starts_with('h') {
            push("rh", r);
        }
    }
    // g- drops under soft mutation: "ail" is radical, but "olfed" <- "golfed".
    if t.starts_with(['a', 'e', 'i', 'o', 'u', 'w', 'y']) {
        push("g", t);
    }
    out
}

/// Radical-form rewrites of one token (feminine and vowel-affection forms).
fn radical(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    match t {
        "trydedd" => out.push("trydydd".to_string()),
        "pedwaredd" => out.push("pedwerydd".to_string()),
        _ => {}
    }
    // ugeinfed / deugeinfed / trigeinfed -> the renderer's -gainfed.
    if let Some(s) = t.strip_suffix("geinfed") {
        out.push(format!("{s}gainfed"));
    }
    out
}

fn token(t: &str) -> Vec<String> {
    let mut out = radical(t);
    for u in unmutate(t) {
        out.extend(radical(&u));
        out.push(u);
    }
    out
}

pub fn variants(norm: &str) -> Vec<String> {
    let mut out = per_token(norm, token);
    // "unfed ar hugain" (21st), "unfed a deugain" (41st): the renderer uses
    // "cyntaf" for the unit of these compounds.
    let mut more = Vec::new();
    for cand in std::iter::once(norm.to_string()).chain(out.iter().cloned()) {
        let toks: Vec<&str> = cand.split(' ').collect();
        if toks.len() >= 3
            && toks[0] == "unfed"
            && matches!(toks[1], "ar" | "a")
            && !matches!(toks[2], "ddeg" | "bymtheg")
        {
            more.push(format!("cyntaf {}", toks[1..].join(" ")));
        }
    }
    out.extend(more);
    out
}

/// num2words2 raises for every cy ordinal above 100. The round hundreds and
/// 1000th are regular: the multiplier + "canfed" (100th) under the mutation
/// it triggers (dau ganfed, tri chanfed, chwe chanfed), and "milfed".
pub fn extra_keys(n: i64, rendered: Option<&str>) -> Vec<String> {
    if rendered.is_some() {
        return Vec::new();
    }
    let key = match n {
        200 => "dau ganfed",
        300 => "tri chanfed",
        400 => "pedwar canfed",
        500 => "pum canfed",
        600 => "chwe chanfed",
        700 => "saith canfed",
        800 => "wyth canfed",
        900 => "naw canfed",
        1000 => "milfed",
        _ => return Vec::new(),
    };
    vec![key.to_string()]
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_163_inflected_ordinals_parse() {
        assert_eq!(ord("cy", "trydedd"), Some(3), "trydedd");
        assert_eq!(ord("cy", "pedwaredd"), Some(4), "pedwaredd");
        assert_eq!(ord("cy", "drydedd"), Some(3), "drydedd");
        assert_eq!(ord("cy", "gyntaf"), Some(1), "gyntaf");
    }

    #[test]
    fn issue_251_standard_compound_and_twenty_forms() {
        assert_eq!(ord("cy", "unfed ar hugain"), Some(21));
        assert_eq!(ord("cy", "unfed a deugain"), Some(41));
        assert_eq!(ord("cy", "ugeinfed"), Some(20));
        assert_eq!(ord("cy", "deugeinfed"), Some(40));
        assert_eq!(ord("cy", "cyntaf ar hugain"), Some(21));
        assert_eq!(ord("cy", "unfed ar ddeg"), Some(11));
    }

    #[test]
    fn issue_252_round_hundreds_and_thousandth() {
        assert_eq!(ord("cy", "dau ganfed"), Some(200));
        assert_eq!(ord("cy", "tri chanfed"), Some(300));
        assert_eq!(ord("cy", "milfed"), Some(1000));
    }
}
