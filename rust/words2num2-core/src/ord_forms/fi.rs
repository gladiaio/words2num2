//! fi: num2words2 renders the nominative (ensimmäinen, toinen, kolmas,
//! viides). The other cases: -inen ordinals on the -ise- stem (toisen,
//! toisena, toista, ensimmäisessä), -s ordinals on -nne- / -nte- / -tta
//! (kolmannen, kolmantena, kolmatta, viidennen).

use super::endings::last_token;

const CASES: &[&str] = &[
    "", "n", "na", "a", "en", "ssa", "sta", "lla", "lta", "lle", "ksi", "t", "ne", "in", "ksi",
];

fn token(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Partitive: toista -> toinen, kolmatta -> kolmas.
    if let Some(s) = t.strip_suffix("ista") {
        out.push(format!("{s}inen"));
    }
    if let Some(s) = t.strip_suffix("tta") {
        out.push(format!("{s}s"));
    }
    for c in CASES {
        let Some(g) = t.strip_suffix(c) else { continue };
        if g.chars().count() < 4 {
            continue;
        }
        if let Some(s) = g.strip_suffix("ise") {
            out.push(format!("{s}inen"));
        }
        for st in ["nne", "nte"] {
            if let Some(s) = g.strip_suffix(st) {
                out.push(format!("{s}s"));
            }
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
    fn issue_167_inflected_ordinals_parse() {
        assert_eq!(ord("fi", "ensimmäisen"), Some(1), "ensimmäisen");
        assert_eq!(ord("fi", "toisen"), Some(2), "toisen");
        assert_eq!(ord("fi", "kolmannen"), Some(3), "kolmannen");
        assert_eq!(ord("fi", "toisena"), Some(2), "toisena");
        assert_eq!(ord("fi", "kolmatta"), Some(3), "kolmatta");
        assert_eq!(ord("fi", "viidennen"), Some(5), "viidennen");
    }
}
