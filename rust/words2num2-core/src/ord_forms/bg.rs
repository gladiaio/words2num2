//! bg: num2words2 renders the bare masculine (първи, втори, трети; стотен,
//! хиляден). Speech adds gender and the definite article: първа, първо,
//! първия(т), първата, първите, стотната. Only the last element inflects:
//! "двадесет и първа".

use super::endings::last_token;

const ENDINGS: &[&str] = &["а", "о", "ия", "ият", "ата", "ото", "ите"];

fn token(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    for e in ENDINGS {
        let Some(stem) = t.strip_suffix(e) else {
            continue;
        };
        if stem.chars().count() < 2 {
            continue;
        }
        out.push(format!("{stem}и"));
        // стотна -> стотен, хилядната -> хиляден: the fleeting е of -ен.
        if let Some(s) = stem.strip_suffix('н') {
            out.push(format!("{s}ен"));
        }
    }
    out
}

pub fn variants(norm: &str) -> Vec<String> {
    last_token(norm, token)
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_145_inflected_ordinals_parse() {
        assert_eq!(ord("bg", "първа"), Some(1), "първа");
        assert_eq!(ord("bg", "първо"), Some(1), "първо");
        assert_eq!(ord("bg", "първия"), Some(1), "първия");
        assert_eq!(ord("bg", "първият"), Some(1), "първият");
        assert_eq!(ord("bg", "първата"), Some(1), "първата");
        assert_eq!(ord("bg", "втора"), Some(2), "втора");
        assert_eq!(ord("bg", "трета"), Some(3), "трета");
        assert_eq!(ord("bg", "третия"), Some(3), "третия");
        assert_eq!(ord("bg", "двадесет и първа"), Some(21), "двадесет и първа");
        assert_eq!(ord("bg", "стотна"), Some(100), "стотна");
    }
}
