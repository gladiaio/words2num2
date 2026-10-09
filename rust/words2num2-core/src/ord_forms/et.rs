//! et: num2words2 renders the nominative (esimene, teine, kolmas, viies).
//! Every other case is built on the genitive stem: esimese, teise, kolmanda,
//! viienda, + the case ending (esimesel, kolmandat, kolmandas, viiendale).

use super::endings::last_token;

/// Case endings that follow the genitive stem (the genitive itself is "").
const CASES: &[&str] = &[
    "", "t", "s", "l", "lt", "le", "ks", "st", "sse", "ni", "na", "ta", "ga", "d", "id",
];

fn token(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Partitive of the -ne ordinals: esimest -> esimene, teist -> teine.
    if let Some(s) = t.strip_suffix("st") {
        if s.ends_with('e') || s.ends_with('i') {
            out.push(format!("{s}ne"));
        }
    }
    for c in CASES {
        let Some(g) = t.strip_suffix(c) else { continue };
        if g.chars().count() < 4 {
            continue;
        }
        // kolmanda -> kolmas, viienda -> viies, kümnenda -> kümnes.
        if let Some(s) = g.strip_suffix("nda") {
            out.push(format!("{s}s"));
        }
        // esimese -> esimene, teise -> teine.
        if let Some(s) = g.strip_suffix("se") {
            out.push(format!("{s}ne"));
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
    fn issue_166_inflected_ordinals_parse() {
        assert_eq!(ord("et", "esimese"), Some(1), "esimese");
        assert_eq!(ord("et", "teise"), Some(2), "teise");
        assert_eq!(ord("et", "kolmanda"), Some(3), "kolmanda");
        assert_eq!(ord("et", "esimest"), Some(1), "esimest");
        assert_eq!(ord("et", "viiendal"), Some(5), "viiendal");
    }
}
