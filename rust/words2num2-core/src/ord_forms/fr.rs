//! fr: num2words2 renders the singular (premier, deuxième, vingt-et-unième).
//! The plural adds -s: premiers, premières, deuxièmes, vingt-et-unièmes. The
//! feminine singular and "second(e)" are canonicalized in the core
//! (`ordinal_canonical`); this covers the plural.

pub fn variants(norm: &str) -> Vec<String> {
    let (head, last) = match norm.rsplit_once(' ') {
        Some((h, l)) => (format!("{h} "), l),
        None => (String::new(), norm),
    };
    let sing = match last {
        "premiers" | "premieres" => "premier".to_string(),
        // "secondes" / "seconds" are also the time unit ("trente secondes"):
        // only as the whole input, never at the end of a compound.
        "secondes" | "seconds" if head.is_empty() => "deuxieme".to_string(),
        l if l.ends_with("iemes") => l[..l.len() - 1].to_string(),
        _ => return Vec::new(),
    };
    vec![format!("{head}{sing}")]
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_172_inflected_ordinals_parse() {
        assert_eq!(ord("fr", "premiers"), Some(1), "premiers");
        assert_eq!(ord("fr", "premières"), Some(1), "premières");
        assert_eq!(ord("fr", "deuxièmes"), Some(2), "deuxièmes");
        assert_eq!(ord("fr", "secondes"), Some(2), "secondes");
        assert_eq!(ord("fr", "vingt-et-unièmes"), Some(21), "vingt-et-unièmes");
        assert_eq!(ord("fr", "trente secondes"), None, "trente secondes");
    }
}
