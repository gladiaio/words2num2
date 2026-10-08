//! mt: num2words2 bakes the assimilated article into the ordinal (l-ewwel,
//! it-tieni, ir-raba', il-ħames), so the bare ordinal ("ewwel", "tieni") and
//! the feminine "tienja" missed. Strip whatever article the input carries
//! and try the bare form under every article, spaced (the hyphen of
//! "it-tieni" normalizes to a space) or hyphen-glued (kept before ħ, għ).

const ARTICLES: &[&str] = &[
    "l", "il", "it", "ir", "is", "id", "in", "iz", "ix", "ic", "ig",
];

pub fn variants(norm: &str) -> Vec<String> {
    let toks: Vec<&str> = norm.split(' ').collect();
    // The article as its own token ("it tieni"), or glued ("il-ħames").
    let mut bare: Vec<String> = toks.iter().map(|t| t.to_string()).collect();
    if toks.len() > 1 && ARTICLES.contains(&toks[0]) {
        bare.remove(0);
    } else if let Some((a, rest)) = toks[0].split_once('-') {
        if ARTICLES.contains(&a) {
            bare[0] = rest.to_string();
        }
    }
    if bare[0] == "tienja" {
        bare[0] = "tieni".to_string();
    }
    let rest = bare[1..].join(" ");
    let mut out = Vec::new();
    for a in ARTICLES {
        for sep in [" ", "-"] {
            let head = format!("{a}{sep}{}", bare[0]);
            out.push(if rest.is_empty() {
                head
            } else {
                format!("{head} {rest}")
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_170_inflected_ordinals_parse() {
        assert_eq!(ord("mt", "ewwel"), Some(1), "ewwel");
        assert_eq!(ord("mt", "tieni"), Some(2), "tieni");
        assert_eq!(ord("mt", "tielet"), Some(3), "tielet");
        assert_eq!(ord("mt", "tienja"), Some(2), "tienja");
    }
}
