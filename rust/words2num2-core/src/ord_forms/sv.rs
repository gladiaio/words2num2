//! sv: num2words2 renders the general -a form (första, andra); the masculine
//! -e form names a male ("Karl den förste", "andre") and ends compounds too
//! ("tjugoförste").

pub fn variants(norm: &str) -> Vec<String> {
    let (head, last) = match norm.rsplit_once(' ') {
        Some((h, l)) => (format!("{h} "), l),
        None => (String::new(), norm),
    };
    for (m, g) in [("forste", "forsta"), ("andre", "andra")] {
        if let Some(stem) = last.strip_suffix(m) {
            return vec![format!("{head}{stem}{g}")];
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_162_inflected_ordinals_parse() {
        assert_eq!(ord("sv", "förste"), Some(1), "förste");
        assert_eq!(ord("sv", "andre"), Some(2), "andre");
    }
}
