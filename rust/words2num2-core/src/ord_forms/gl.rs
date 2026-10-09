//! gl: num2words2 renders the ordinal as the cardinal + "-o" ("un-o",
//! "vinte un-o", "dez un-o" for 11th), never the real ordinal (primeiro,
//! vixésimo primeiro, décimo primeiro). Each ordinal word is replaced by the
//! cardinal of its value and the renderer's "o" appended, which is exactly
//! the key it rendered: "vixésima primeira" -> "vinte un o" (21).

const ORDINALS: &[(&str, i64)] = &[
    ("primeiro", 1),
    ("segundo", 2),
    ("terceiro", 3),
    ("cuarto", 4),
    ("quinto", 5),
    ("sexto", 6),
    ("setimo", 7),
    ("oitavo", 8),
    ("noveno", 9),
    ("decimo", 10),
    ("vixesimo", 20),
    ("trixesimo", 30),
    ("cuadraxesimo", 40),
    ("quincuaxesimo", 50),
    ("sexaxesimo", 60),
    ("septuaxesimo", 70),
    ("octoxesimo", 80),
    ("nonaxesimo", 90),
    ("centesimo", 100),
    ("milesimo", 1000),
];

/// The value of one ordinal word, in any gender / number (primeira,
/// primeiros, primeiras).
fn value(tok: &str) -> Option<i64> {
    let m = ["as", "os", "a", "o"]
        .iter()
        .find_map(|e| tok.strip_suffix(e))
        .map(|stem| format!("{stem}o"))?;
    ORDINALS.iter().find(|(w, _)| *w == m).map(|(_, v)| *v)
}

pub fn variants(norm: &str) -> Vec<String> {
    let Some(renderer) = num2words2_core::get_lang_by_key("gl") else {
        return Vec::new();
    };
    let mut parts = Vec::new();
    for t in norm.split(' ') {
        let Some(v) = value(t) else { return Vec::new() };
        let Ok(w) = renderer.to_cardinal(&num_bigint::BigInt::from(v)) else {
            return Vec::new();
        };
        parts.push(crate::normalize(&w));
    }
    parts.push("o".to_string());
    vec![parts.join(" ")]
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_244_inflected_ordinals_parse() {
        assert_eq!(ord("gl", "primeiro"), Some(1), "primeiro");
        assert_eq!(ord("gl", "primeira"), Some(1), "primeira");
        assert_eq!(
            ord("gl", "vixésimo primeiro"),
            Some(21),
            "vixésimo primeiro"
        );
        assert_eq!(ord("gl", "décimo primeiro"), Some(11), "décimo primeiro");
    }
}
