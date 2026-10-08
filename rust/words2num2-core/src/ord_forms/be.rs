//! be: num2words2 renders the masculine nominative (першы, другі, трэці).
//! Hard stems take -ы, stems in г/к/х take -і, and трэці is soft. Only the
//! last element of a compound inflects: "дваццаць першая".

use super::endings::{last_token, swap};

const HARD: &[&str] = &["ы", "і"];
const SOFT: &[&str] = &["і"];

const RULES: &[(&str, &[&str])] = &[
    ("ая", HARD),
    ("ае", HARD),
    ("ага", HARD),
    ("аму", HARD),
    ("ым", HARD),
    ("ую", HARD),
    ("ыя", HARD),
    ("ых", HARD),
    ("ымі", HARD),
    ("аи", HARD), // першай (f. gen/dat), й normalized to и
    ("ое", HARD), // другое
    ("ога", HARD),
    ("ому", HARD),
    ("ім", HARD),
    ("ія", HARD),
    ("іх", HARD),
    ("імі", HARD),
    ("яя", SOFT), // трэцяя
    ("яе", SOFT),
    ("яга", SOFT),
    ("яму", SOFT),
    ("юю", SOFT),
    ("яи", SOFT),
];

/// The renderer writes 900th "дзевяцiсоты" with a Latin i (U+0069); real
/// Cyrillic text "дзевяцісоты" only matches that key once the і is swapped.
fn latin_i(tok: &str) -> Option<String> {
    tok.contains("сот").then(|| tok.replace('і', "i"))
}

pub fn variants(norm: &str) -> Vec<String> {
    let mut out = last_token(norm, |t| {
        let mut c = vec![t.to_string()];
        c.extend(swap(t, RULES, 3));
        let latin: Vec<String> = c.iter().filter_map(|x| latin_i(x)).collect();
        c.extend(latin);
        c
    });
    out.retain(|v| v != norm);
    out
}

/// num2words2 renders 80th as "сямідзясяты" (70th): its suffix test on
/// "семдзесят" also matches "восемдзесят". The x80th key is missing, and the
/// x70th spelling already belongs to x70 (first write wins), so add the
/// correct "васьмідзясяты" for every x80.
pub fn extra_keys(n: i64, rendered: Option<&str>) -> Vec<String> {
    match rendered {
        Some(r) if n % 100 == 80 && r.contains("сямідзясяты") => {
            vec![r.replace("сямідзясяты", "васьмідзясяты")]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_139_inflected_ordinals_parse() {
        assert_eq!(ord("be", "першая"), Some(1), "першая");
        assert_eq!(ord("be", "першае"), Some(1), "першае");
        assert_eq!(ord("be", "першага"), Some(1), "першага");
        assert_eq!(ord("be", "другая"), Some(2), "другая");
        assert_eq!(ord("be", "трэцяя"), Some(3), "трэцяя");
    }

    #[test]
    fn issue_231_eightieth_is_not_seventieth() {
        assert_eq!(ord("be", "васьмідзясяты"), Some(80));
        assert_eq!(ord("be", "тысяча васьмідзясяты"), Some(1080));
        // The renderer's spelling of 80th is the real 70th, and stays 70.
        assert_eq!(ord("be", "сямідзясяты"), Some(70));
    }

    #[test]
    fn issue_241_cyrillic_i_in_nine_hundredth() {
        assert_eq!(ord("be", "дзевяцісоты"), Some(900));
        assert_eq!(ord("be", "дзевяцiсоты"), Some(900)); // the renderer's Latin i
    }
}
