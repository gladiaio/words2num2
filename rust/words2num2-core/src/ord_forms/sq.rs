//! sq: num2words2 renders the definite masculine with its article (i pari, i
//! dyti, i treti). The feminine is "e X-a" (e para, e dyta), the indefinite
//! ends -ë (i parë, e parë), and the article may be dropped.

fn last(t: &str) -> Vec<String> {
    let mut out = vec![t.to_string()];
    if let Some(s) = t.strip_suffix('a') {
        out.push(format!("{s}i"));
        out.push(format!("{s}e")); // e njëmbëdhjeta -> i njëmbëdhjetë
    }
    if let Some(s) = t.strip_suffix('e') {
        out.push(format!("{s}i"));
    }
    out
}

pub fn variants(norm: &str) -> Vec<String> {
    let toks: Vec<&str> = norm.split(' ').collect();
    let (article, body): (bool, &[&str]) = match toks.first() {
        Some(&("i" | "e" | "te" | "se")) => (true, &toks[1..]),
        _ => (false, &toks[..]),
    };
    let Some((l, head)) = body.split_last() else {
        return Vec::new();
    };
    // Without the article only the indefinite -ë form ("parë") is an ordinal:
    // a bare "para" is "money" / "before".
    if !article && !l.ends_with('e') {
        return Vec::new();
    }
    let mut out = Vec::new();
    for cand in last(l) {
        let mut v = vec!["i"];
        v.extend_from_slice(head);
        v.push(&cand);
        out.push(v.join(" "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_169_inflected_ordinals_parse() {
        assert_eq!(ord("sq", "e para"), Some(1), "e para");
        assert_eq!(ord("sq", "e dyta"), Some(2), "e dyta");
        assert_eq!(ord("sq", "e treta"), Some(3), "e treta");
        assert_eq!(ord("sq", "i parë"), Some(1), "i parë");
    }
}
