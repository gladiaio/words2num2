//! ru: num2words2 renders the masculine nominative (первый, второй, третий),
//! normalized with й -> и ("первыи", "второи", "третии"). Only the last
//! element of a compound inflects: "двадцать первая", "сто третьего".

use super::endings::{last_token, swap};

/// Hard stems (перв-ый, втор-ой, сот-ый): every case / gender / number ending
/// back to both masculine endings; the table keeps the one that exists.
const HARD: &[&str] = &["ыи", "ои"];
/// Soft третий: третья, третьего, третьим … -> "третии".
const SOFT: &[&str] = &["ии"];

const RULES: &[(&str, &[&str])] = &[
    ("ая", HARD),
    ("ое", HARD),
    ("ого", HARD),
    ("ому", HARD),
    ("ым", HARD),
    ("ом", HARD),
    ("ую", HARD),
    ("ые", HARD),
    ("ых", HARD),
    ("ыми", HARD),
    ("ои", HARD), // первой (f. gen/dat/instr), normalized like the m. -ой
    ("ою", HARD),
    ("ья", SOFT),
    ("ье", SOFT),
    ("ьего", SOFT),
    ("ьему", SOFT),
    ("ьим", SOFT),
    ("ьем", SOFT),
    ("ью", SOFT),
    ("ьи", SOFT),
    ("ьих", SOFT),
    ("ьими", SOFT),
    ("ьеи", SOFT),
];

pub fn variants(norm: &str) -> Vec<String> {
    last_token(norm, |t| swap(t, RULES, 3))
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_137_inflected_ordinals_parse() {
        assert_eq!(ord("ru", "первая"), Some(1), "первая");
        assert_eq!(ord("ru", "первое"), Some(1), "первое");
        assert_eq!(ord("ru", "первого"), Some(1), "первого");
        assert_eq!(ord("ru", "первому"), Some(1), "первому");
        assert_eq!(ord("ru", "первых"), Some(1), "первых");
        assert_eq!(ord("ru", "вторая"), Some(2), "вторая");
        assert_eq!(ord("ru", "третья"), Some(3), "третья");
        assert_eq!(ord("ru", "третьего"), Some(3), "третьего");
        assert_eq!(ord("ru", "сотая"), Some(100), "сотая");
        assert_eq!(ord("ru", "двадцать первая"), Some(21), "двадцать первая");
        assert_eq!(ord("ru", "второй"), Some(2), "второй");
        assert_eq!(ord("ru", "первоя"), None, "первоя");
    }
}
