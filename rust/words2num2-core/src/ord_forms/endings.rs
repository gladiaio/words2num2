//! Shared string helpers for the per-language ordinal rewrites.

/// Upper bound on the candidates one input expands to (a long compound in a
/// language that inflects every element multiplies the options per token).
const MAX_CANDIDATES: usize = 256;

/// Ending rewrites of one token: for every `(from, to)` rule whose `from`
/// ends `tok` and leaves a stem of at least `min_stem` characters, the stem
/// followed by each of `to`. Every matching rule contributes (not only the
/// longest), so "первого" yields both "перв"+"ыи" and "первог"+… candidates;
/// only the ones the table holds ever match.
pub fn swap(tok: &str, rules: &[(&str, &[&str])], min_stem: usize) -> Vec<String> {
    let mut out = Vec::new();
    for (from, tos) in rules {
        if let Some(stem) = tok.strip_suffix(from) {
            if stem.chars().count() >= min_stem {
                for to in *tos {
                    out.push(format!("{stem}{to}"));
                }
            }
        }
    }
    out
}

/// Every combination of rewriting the tokens of `norm` with `f` (each token
/// may also stay as written), the unchanged input excluded. For the languages
/// that inflect every element of a compound (pl "dwudziesta pierwsza", el
/// "εικοστή πρώτη", cs "dvacátá první").
pub fn per_token(norm: &str, f: impl Fn(&str) -> Vec<String>) -> Vec<String> {
    let toks: Vec<&str> = norm.split(' ').collect();
    let options: Vec<Vec<String>> = toks
        .iter()
        .map(|t| {
            let mut o = vec![t.to_string()];
            o.extend(f(t).into_iter().filter(|c| c != t));
            o
        })
        .collect();
    let mut acc: Vec<Vec<&str>> = vec![Vec::new()];
    for opts in &options {
        let mut next = Vec::new();
        for prefix in &acc {
            for o in opts {
                if next.len() >= MAX_CANDIDATES {
                    break;
                }
                let mut p = prefix.clone();
                p.push(o.as_str());
                next.push(p);
            }
        }
        acc = next;
    }
    acc.into_iter()
        .map(|v| v.join(" "))
        .filter(|v| v != norm)
        .collect()
}

/// Rewrite only the last token of `norm` with `f`: the languages whose
/// compounds inflect just the final element (ru "двадцать первая", bg
/// "двадесет и първа").
pub fn last_token(norm: &str, f: impl Fn(&str) -> Vec<String>) -> Vec<String> {
    let (head, last) = match norm.rsplit_once(' ') {
        Some((h, l)) => (Some(h), l),
        None => (None, norm),
    };
    f(last)
        .into_iter()
        .filter(|c| c != last)
        .map(|c| match head {
            Some(h) => format!("{h} {c}"),
            None => c,
        })
        .collect()
}

/// Test helper: `words2num(text, lang, to="ordinal")` as an `i64`.
#[cfg(test)]
pub fn ord(lang: &str, text: &str) -> Option<i64> {
    match crate::words2num(text, lang, "ordinal") {
        Ok(crate::w2n_lang_en::W2nValue::Int(n)) => i64::try_from(n).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap_keeps_a_minimum_stem() {
        let rules: &[(&str, &[&str])] = &[("a", &["y"]), ("ia", &["i"])];
        assert_eq!(swap("druga", rules, 2), vec!["drugy"]);
        assert_eq!(swap("trzecia", rules, 2), vec!["trzeciy", "trzeci"]);
        assert!(swap("a", rules, 2).is_empty());
    }

    #[test]
    fn per_token_and_last_token() {
        let f = |t: &str| {
            if t == "b" {
                vec!["x".to_string()]
            } else {
                Vec::new()
            }
        };
        assert_eq!(per_token("b c b", f), vec!["b c x", "x c b", "x c x"]);
        assert_eq!(last_token("b c b", f), vec!["b c x"]);
        assert!(last_token("b c", f).is_empty());
    }
}
