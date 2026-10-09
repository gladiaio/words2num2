//! nb / no: num2words2 renders "andre" for 2nd; bokmål also writes "annen"
//! (m./f.) and "annet" (n.), alone or at the end of a compound ("tjueannen").

pub fn variants(norm: &str) -> Vec<String> {
    let (head, last) = match norm.rsplit_once(' ') {
        Some((h, l)) => (format!("{h} "), l),
        None => (String::new(), norm),
    };
    match last
        .strip_suffix("annen")
        .or_else(|| last.strip_suffix("annet"))
    {
        Some(stem) => vec![format!("{head}{stem}andre")],
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_161_inflected_ordinals_parse() {
        assert_eq!(ord("nb", "annen"), Some(2), "annen");
        assert_eq!(ord("nb", "annet"), Some(2), "annet");
    }
}
