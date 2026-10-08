//! ro: num2words2 renders the masculine with its article (primul, al doilea,
//! al douăzeci și unulea). The feminine takes "a" and -a: "a doua", "a
//! treia", "a douăsprezecea", "a douăzeci și una"; "prima" / "întâia" /
//! "întâiul" are 1st. The renderer also writes 100th / 1000th with the
//! pronoun "una" ("al una sutălea") where Romanian uses the article "o"
//! ("al o sutălea").

/// Masculine ordinal endings for a feminine last token: stem (minus -a or
/// -ea) + -lea / -ilea / -elea / -ulea / -alea; the table keeps the real one
/// (treia -> treilea, patra -> patrulea, cincea -> cincilea, zecea -> zecelea,
/// noua -> noualea, una -> unulea, suta -> sutalea, mia -> mielea).
fn masculine(last: &str) -> Vec<String> {
    if last == "doua" {
        return vec!["doilea".to_string()];
    }
    // douăsprezecea -> doisprezecelea.
    let last = match last.strip_prefix("douasprezec") {
        Some(rest) => format!("doisprezec{rest}"),
        None => last.to_string(),
    };
    let mut stems = Vec::new();
    if let Some(s) = last.strip_suffix("ea") {
        stems.push(s.to_string());
    }
    if let Some(s) = last.strip_suffix('a') {
        stems.push(s.to_string());
    }
    let mut out = Vec::new();
    for s in stems {
        for suf in ["lea", "ilea", "elea", "ulea", "alea"] {
            out.push(format!("{s}{suf}"));
        }
    }
    out
}

pub fn variants(norm: &str) -> Vec<String> {
    if matches!(
        norm,
        "prima" | "intaia" | "intaiul" | "intaiului" | "primei" | "primului"
    ) {
        return vec!["primul".to_string()];
    }
    // "o" before sută / mie (100, 1000) -> the renderer's "una".
    let toks: Vec<&str> = norm.split(' ').collect();
    let fixed: Vec<String> = toks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let next = toks.get(i + 1).copied().unwrap_or("");
            if *t == "o" && (next.starts_with("sut") || next.starts_with("mi")) {
                "una".to_string()
            } else {
                t.to_string()
            }
        })
        .collect();
    let mut out = vec![fixed.join(" ")];
    // Feminine "a X…-a" -> masculine "al X…-lea".
    if fixed.len() >= 2 && fixed[0] == "a" {
        let mut m = fixed.clone();
        m[0] = "al".to_string();
        let last = m.pop().unwrap_or_default();
        for cand in masculine(&last) {
            let mut v = m.clone();
            v.push(cand);
            out.push(v.join(" "));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_148_inflected_ordinals_parse() {
        assert_eq!(ord("ro", "prima"), Some(1), "prima");
        assert_eq!(ord("ro", "a doua"), Some(2), "a doua");
        assert_eq!(ord("ro", "a treia"), Some(3), "a treia");
        assert_eq!(ord("ro", "a patra"), Some(4), "a patra");
        assert_eq!(ord("ro", "a cincea"), Some(5), "a cincea");
        assert_eq!(ord("ro", "a douăsprezecea"), Some(12), "a douăsprezecea");
        assert_eq!(
            ord("ro", "a douăzeci și una"),
            Some(21),
            "a douăzeci și una"
        );
        assert_eq!(ord("ro", "întâiul"), Some(1), "întâiul");
    }

    #[test]
    fn issue_253_article_o_before_suta_and_mie() {
        assert_eq!(ord("ro", "al o sutălea"), Some(100));
        assert_eq!(ord("ro", "al o sută unulea"), Some(101));
        assert_eq!(ord("ro", "al o mielea"), Some(1000));
        assert_eq!(ord("ro", "al una sutălea"), Some(100));
    }
}
