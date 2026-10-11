//! Per-language word lists of the sentence walker that the renderer can
//! provide, derived from it and cached.

use num_bigint::BigInt;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// The values whose renderings are scanned for connectors: every number to
/// 1100, the round thousands, millions and billions, and a few multi-scale
/// values, so each place a connector can sit ("og" after a hundred, "de"
/// before "mii", "leh" between groups) is seen, and every number word shows
/// up at the end of some rendering (cs "dvě stě", da "to tusinde").
fn sample_values() -> Vec<i64> {
    let mut v: Vec<i64> = (1..=1100).collect();
    v.extend((1000..=20_000).step_by(1000));
    v.extend((100_000..=900_000).step_by(100_000));
    for p in [1_000_000i64, 1_000_000_000] {
        v.extend((1..=9).map(|k| k * p));
        v.extend([10 * p, 100 * p]);
    }
    v.extend([
        1234, 2001, 2345, 5555, 12_345, 21_000, 99_999, 100_001, 123_456, 300_500, 1_000_001,
        1_234_567, 1_234_567_891,
    ]);
    v
}

/// The words num2words2 writes *between* the number words of a cardinal and
/// never at its start or end: is "og", fa "و", sq "e", bg "и", ha "da", lus
/// "leh", vi "lẻ", ro "de", sw "na". Normalised.
///
/// A connector is allowed inside a number run but a run never ends on one, so
/// "fjörutíu og tveir" is read whole (42, not "40 og 2") and "douăzeci de lei"
/// keeps its "de" ("20 de lei", not "20 lei") (#65). Derived from the
/// renderer rather than listed by hand: the hand list covered eleven
/// languages. A word that ends a rendering ("cents" in fr "deux cents") is a
/// number word in its own right and is left out.
pub(super) fn derived_connectors(lang: &str) -> &'static [String] {
    static CACHE: OnceLock<RwLock<HashMap<String, &'static [String]>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.read().ok().and_then(|m| m.get(lang).copied()) {
        return v;
    }
    let derived: &'static [String] = Box::leak(compute_connectors(lang).into_boxed_slice());
    if let Ok(mut m) = cache.write() {
        m.insert(lang.to_string(), derived);
    }
    derived
}

fn compute_connectors(lang: &str) -> Vec<String> {
    let Some(renderer) = num2words2_core::get_lang_by_key(lang) else {
        return Vec::new();
    };
    let mut interior: Vec<String> = Vec::new();
    let mut edge: Vec<String> = Vec::new();
    for v in sample_values() {
        let Ok(text) = renderer.to_cardinal(&BigInt::from(v)) else { continue };
        let norm = crate::normalize(&text);
        let toks: Vec<&str> = norm.split_whitespace().collect();
        for (k, t) in toks.iter().enumerate() {
            let list = if k == 0 || k + 1 == toks.len() { &mut edge } else { &mut interior };
            if !list.iter().any(|w| w == t) {
                list.push(t.to_string());
            }
        }
    }
    interior
        .into_iter()
        .filter(|t| !edge.contains(t))
        .filter(|t| !t.chars().any(|c| c.is_ascii_digit()))
        .filter(|t| !is_number_fragment(lang, t))
        .filter(|t| !glued_number(lang, t))
        .collect()
}

fn is_number_fragment(lang: &str, t: &str) -> bool {
    matches!(crate::lookup(lang, t, false, &[]), Ok(Some(_))) || crate::parse_scaled_norm(lang, t).is_some()
}

/// A number word with a short particle glued on — ar "وتسعماية" (and nine
/// hundred), tg "ҳазору", sn "nemakumi" — is part of the number, not a
/// connector between two.
fn glued_number(lang: &str, t: &str) -> bool {
    let chars: Vec<char> = t.chars().collect();
    let n = chars.len();
    (1..=2).any(|k| {
        n >= k + 3
            && (is_number_fragment(lang, &chars[k..].iter().collect::<String>())
                || is_number_fragment(lang, &chars[..n - k].iter().collect::<String>()))
    })
}

/// Does num2words2 spell `value` exactly as `text` (both normalised, hyphens
/// read as spaces)? The authority for a reading the walker cannot check
/// arithmetically.
pub(super) fn renders_as(lang: &str, value: &BigInt, text: &str) -> bool {
    let Some(renderer) = num2words2_core::get_lang_by_key(lang) else {
        return false;
    };
    let Ok(rendered) = renderer.to_cardinal(value) else {
        return false;
    };
    let canon = |s: &str| crate::normalize(s).replace('-', " ").split_whitespace().collect::<Vec<_>>().join(" ");
    canon(&rendered) == canon(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #65: the connectors come from the renderer, and a word that ends a
    /// rendering (fr "cents", cs "stě") is not one.
    #[test]
    fn connectors_are_derived_from_the_renderer() {
        for (lang, conn) in [("is", "og"), ("no", "og"), ("fa", "و"), ("sq", "e"), ("bg", "и"),
                             ("ha", "da"), ("lus", "leh"), ("ro", "de"), ("sw", "na"), ("vi", "le")] {
            assert!(derived_connectors(lang).iter().any(|c| c == conn), "{lang} {conn}");
        }
        assert!(!derived_connectors("fr").iter().any(|c| c == "cents" || c == "vingts"));
        assert!(!derived_connectors("cs").iter().any(|c| c == "ste"));
    }
}

// ===========================================================================
// Spelling variants and case forms the renderer does not write
// ===========================================================================

fn base_lang(lang: &str) -> &str {
    lang.split(&['_', '-'][..]).next().unwrap_or(lang)
}

/// Case-declined cardinals (#74): the reverse table holds the renderer's
/// nominative, but after a preposition or a quantity the oblique case is the
/// normal form ("около трёх тысяч", "около trzech tysięcy", "сто двадцати").
/// Each row is (value, forms); the form is read as the nominative of the
/// value. Gender variants of the nominative are a separate matter and are not
/// listed here.
const RU_CASES: &[(i64, &str)] = &[
    (1, "одного одному одним одном одной одну одною"),
    (2, "двух двум двумя"),
    (3, "трёх трех трём трем тремя"),
    (4, "четырёх четырех четырём четырем четырьмя"),
    (5, "пяти пятью"),
    (6, "шести шестью"),
    (7, "семи семью"),
    (8, "восьми восемью восьмью"),
    (9, "девяти девятью"),
    (10, "десяти десятью"),
    (11, "одиннадцати одиннадцатью"),
    (12, "двенадцати двенадцатью"),
    (13, "тринадцати тринадцатью"),
    (14, "четырнадцати четырнадцатью"),
    (15, "пятнадцати пятнадцатью"),
    (16, "шестнадцати шестнадцатью"),
    (17, "семнадцати семнадцатью"),
    (18, "восемнадцати восемнадцатью"),
    (19, "девятнадцати девятнадцатью"),
    (20, "двадцати двадцатью"),
    (30, "тридцати тридцатью"),
    (40, "сорока"),
    (50, "пятидесяти пятьюдесятью"),
    (60, "шестидесяти шестьюдесятью"),
    (70, "семидесяти семьюдесятью"),
    (80, "восьмидесяти восьмьюдесятью восемьюдесятью"),
    (90, "девяноста"),
    (100, "ста"),
    (200, "двухсот двумстам двумястами двухстах"),
    (300, "трёхсот трехсот тремстам тремястами трёхстах трехстах"),
    (400, "четырёхсот четырехсот четырёмстам четыремстам четырьмястами четырёхстах четырехстах"),
    (500, "пятисот пятистам пятьюстами пятистах"),
    (600, "шестисот шестистам шестьюстами шестистах"),
    (700, "семисот семистам семьюстами семистах"),
    (800, "восьмисот восьмистам восьмьюстами восемьюстами восьмистах"),
    (900, "девятисот девятистам девятьюстами девятистах"),
];
const RU_SCALES: &[(&str, &str)] = &[
    ("тысяч", "тысячам тысячами тысячах тысячу тысячей тысячею тысяче"),
    ("миллионов", "миллиону миллионам миллионом миллионами миллионе миллионах"),
    ("миллиардов", "миллиарду миллиардам миллиардом миллиардами миллиарде миллиардах"),
];

/// Ukrainian; the apostrophe of "п'яти" is dropped from the keys (the
/// normaliser turns it into a space, see [`alias_rewrite`]).
const UK_CASES: &[(i64, &str)] = &[
    (1, "одного одному одним одній однієї одну однією"),
    (2, "двох двом двома"),
    (3, "трьох трьом трьома"),
    (4, "чотирьох чотирьом чотирма"),
    (5, "пяти пятьом пятьма"),
    (6, "шести шістьом шістьма шести"),
    (7, "семи сімом сьома"),
    (8, "восьми вісьмом вісьма вісьмома"),
    (9, "девяти девятьом девятьма"),
    (10, "десяти десятьом десятьма"),
    (11, "одинадцяти одинадцятьом одинадцятьма"),
    (12, "дванадцяти дванадцятьом дванадцятьма"),
    (13, "тринадцяти тринадцятьом тринадцятьма"),
    (14, "чотирнадцяти чотирнадцятьом чотирнадцятьма"),
    (15, "пятнадцяти пятнадцятьом пятнадцятьма"),
    (16, "шістнадцяти шістнадцятьом шістнадцятьма"),
    (17, "сімнадцяти сімнадцятьом сімнадцятьма"),
    (18, "вісімнадцяти вісімнадцятьом вісімнадцятьма"),
    (19, "девятнадцяти девятнадцятьом девятнадцятьма"),
    (20, "двадцяти двадцятьом двадцятьма"),
    (30, "тридцяти тридцятьом тридцятьма"),
    (40, "сорока"),
    (50, "пятдесяти пятдесятьом пятдесятьма"),
    (60, "шістдесяти шістдесятьом шістдесятьма"),
    (70, "сімдесяти сімдесятьом сімдесятьма"),
    (80, "вісімдесяти вісімдесятьом вісімдесятьма"),
    (90, "девяноста"),
    (100, "ста"),
    (200, "двохсот двомстам двомастами двохстах"),
    (300, "трьохсот трьомстам трьомастами трьохстах"),
    (400, "чотирьохсот чотирьомстам чотирмастами чотирьохстах"),
    (500, "пятисот пятистам пятьмастами пятистах"),
    (600, "шестисот шестистам шістьмастами шестистах"),
    (700, "семисот семистам сьомастами семистах"),
    (800, "восьмисот восьмистам вісьмастами восьмистах"),
    (900, "девятисот девятистам девятьмастами девятистах"),
];
const UK_SCALES: &[(&str, &str)] = &[
    ("тисяч", "тисячам тисячами тисячах тисячу тисячею тисячі"),
    ("мільйонів", "мільйону мільйонам мільйоном мільйонами мільйоні мільйонах"),
];

const PL_CASES: &[(i64, &str)] = &[
    (1, "jednego jednemu jednym jedną jednej"),
    (2, "dwóch dwu dwom dwoma dwiema"),
    (3, "trzech trzem trzema"),
    (4, "czterech czterem czterema"),
    (5, "pięciu pięcioma"),
    (6, "sześciu sześcioma"),
    (7, "siedmiu siedmioma"),
    (8, "ośmiu ośmioma"),
    (9, "dziewięciu dziewięcioma"),
    (10, "dziesięciu dziesięcioma"),
    (11, "jedenastu jedenastoma"),
    (12, "dwunastu dwunastoma"),
    (13, "trzynastu trzynastoma"),
    (14, "czternastu czternastoma"),
    (15, "piętnastu piętnastoma"),
    (16, "szesnastu szesnastoma"),
    (17, "siedemnastu siedemnastoma"),
    (18, "osiemnastu osiemnastoma"),
    (19, "dziewiętnastu dziewiętnastoma"),
    (20, "dwudziestu dwudziestoma"),
    (30, "trzydziestu trzydziestoma"),
    (40, "czterdziestu czterdziestoma"),
    (50, "pięćdziesięciu pięćdziesięcioma"),
    (60, "sześćdziesięciu sześćdziesięcioma"),
    (70, "siedemdziesięciu siedemdziesięcioma"),
    (80, "osiemdziesięciu osiemdziesięcioma"),
    (90, "dziewięćdziesięciu dziewięćdziesięcioma"),
    (100, "stu stoma"),
    (200, "dwustu dwustoma"),
    (300, "trzystu trzystoma"),
    (400, "czterystu czterystoma"),
    (500, "pięciuset"),
    (600, "sześciuset"),
    (700, "siedmiuset"),
    (800, "ośmiuset"),
    (900, "dziewięciuset"),
];
const PL_SCALES: &[(&str, &str)] = &[
    ("tysięcy", "tysiąca tysiącom tysiącami tysiącach tysiącu tysiącem"),
    ("milionów", "miliona milionom milionami milionach milionie milionem"),
];

/// Arabic genitive/accusative (#74): the tens in -ين and the duals in -ين are
/// the default spoken form ("ثلاثة وعشرين", "اثنين"). Read as the -ون / -ان
/// nominative the renderer writes.
const AR_CASES: &[(&str, &str)] = &[
    ("عشرون", "عشرين"),
    ("ثلاثون", "ثلاثين"),
    ("أربعون", "أربعين"),
    ("خمسون", "خمسين"),
    ("ستون", "ستين"),
    ("سبعون", "سبعين"),
    ("ثمانون", "ثمانين"),
    ("تسعون", "تسعين"),
    ("اثنان", "اثنين"),
    ("اثنتان", "اثنتين"),
    ("اثنتا", "اثنتي"),
    ("مئتان", "مئتين مائتين مائتان"),
    ("ألفان", "ألفين"),
];

/// Common spellings outside the renderer's one (#78, #180): hi "छह" (the
/// renderer writes "छः"), ro "unșpe" ... "nouășpe" for -sprezece (spoken
/// Romanian contracts the teens).
const WORD_VARIANTS: &[(&str, &str, &str)] = &[
    ("hi", "छः", "छह छे"),
    ("ro", "unsprezece", "unșpe unspe"),
    ("ro", "doisprezece", "doișpe doispe"),
    ("ro", "douăsprezece", "douășpe douaspe"),
    ("ro", "treisprezece", "treișpe treispe"),
    ("ro", "paisprezece", "paișpe paispe"),
    ("ro", "cincisprezece", "cinșpe cinspe cincișpe"),
    ("ro", "șaisprezece", "șaișpe saispe"),
    ("ro", "șaptesprezece", "șaptișpe șapteșpe saptispe"),
    ("ro", "optsprezece", "optișpe optșpe optispe"),
    ("ro", "nouăsprezece", "nouășpe nouaspe"),
    // The oblique stems a case ending is cut from (#73, #74): hu "hármat",
    // "hetet", "ezret", "kettőt"; bn "দুটো"; fi "viidellä", "kahdessa",
    // "kolmea", "sadan", "tuhannen".
    ("hu", "három", "hárm"),
    ("hu", "hét", "het"),
    ("hu", "ezer", "ezr"),
    ("bn", "দুই", "দু"),
    ("fi", "yksi", "yhde yhte yht"),
    ("fi", "kaksi", "kahde kahte kaht"),
    ("fi", "neljä", "neljä"),
    ("fi", "viisi", "viide viite viit"),
    ("fi", "kuusi", "kuude kuute kuut"),
    ("fi", "seitsemän", "seitsemä"),
    ("fi", "kahdeksan", "kahdeksa"),
    ("fi", "yhdeksän", "yhdeksä"),
    ("fi", "kymmenen", "kymmene kymmen"),
    ("fi", "sata", "sada"),
    ("fi", "tuhat", "tuhanne"),
];

/// Substring spellings, accepted only when the rewritten word is a number
/// word: el demotic "εννιά", "οχτώ", "εφτά" and their compounds
/// ("δεκαοχτώ", "εφτακόσια"), bn "পঁয়ত্রিশ" for the renderer's "পঁইত্রিশ",
/// hi anusvara for chandrabindu ("पांच" / "पाँच"), da "tres" for "treds"
/// (also in "halvtres", "toogtres") (#78).
const SUBSTRING_VARIANTS: &[(&str, &str, &str)] = &[
    ("el", "εννια", "εννεα"),
    ("el", "οχτ", "οκτ"),
    ("el", "εφτ", "επτ"),
    ("bn", "পঁয", "পঁই"),
    ("bn", "পঁই", "পঁয"),
    ("hi", "ं", "ँ"),
    ("hi", "ँ", "ं"),
    ("da", "tres", "treds"),
    // zh 两 is "two" before a measure word and in 两百, 两千 (#177).
    ("zh", "两", "二"),
    ("zh", "兩", "二"),
    ("cn", "两", "二"),
    ("cn", "兩", "二"),
];

type AliasMap = HashMap<String, String>;

/// The exact-word aliases of a language, normalised: variant -> replacement.
fn word_aliases(lang: &str) -> &'static AliasMap {
    static CACHE: OnceLock<RwLock<HashMap<String, &'static AliasMap>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let base = base_lang(lang);
    if let Some(m) = cache.read().ok().and_then(|m| m.get(base).copied()) {
        return m;
    }
    let mut map = AliasMap::new();
    let mut add = |forms: &str, to: String| {
        for f in forms.split_whitespace() {
            let key = crate::normalize(f).replace(' ', "");
            map.entry(key).or_insert_with(|| to.clone());
        }
    };
    let render = |v: i64| {
        num2words2_core::get_lang_by_key(base)
            .and_then(|r| r.to_cardinal(&BigInt::from(v)).ok())
            .map(|t| crate::normalize(&t))
    };
    let (cases, scales): (&[(i64, &str)], &[(&str, &str)]) = match base {
        "ru" => (RU_CASES, RU_SCALES),
        "uk" => (UK_CASES, UK_SCALES),
        "pl" => (PL_CASES, PL_SCALES),
        _ => (&[], &[]),
    };
    for (v, forms) in cases {
        if let Some(nom) = render(*v) {
            add(forms, nom);
        }
    }
    for (to, forms) in scales {
        add(forms, crate::normalize(to));
    }
    if base == "ar" {
        for (to, forms) in AR_CASES {
            add(forms, crate::normalize(to));
        }
    }
    for (l, to, forms) in WORD_VARIANTS {
        if *l == base {
            add(forms, crate::normalize(to));
        }
    }
    let leaked: &'static AliasMap = Box::leak(Box::new(map));
    if let Ok(mut m) = cache.write() {
        m.insert(base.to_string(), leaked);
    }
    leaked
}

fn is_table_word(lang: &str, w: &str) -> bool {
    matches!(crate::lookup(lang, w, false, &[]), Ok(Some(_)))
}

/// cs "pětadvacet" = 25: the unit, "a", the tens, glued (#179). The renderer
/// writes "dvacet pět". Normalised (diacritics stripped).
fn cs_inverted(tok: &str) -> Option<String> {
    const UNITS: [&str; 10] = ["jedna", "jeden", "dva", "tri", "ctyri", "pet", "sest", "sedm", "osm", "devet"];
    const TENS: [&str; 8] = ["dvacet", "tricet", "ctyricet", "padesat", "sedesat", "sedmdesat", "osmdesat", "devadesat"];
    for u in UNITS {
        let Some(rest) = tok.strip_prefix(u) else { continue };
        // "pět-a-dvacet"; "jedna" ends in the "a" itself ("jednadvacet").
        let rest = match rest.strip_prefix('a') {
            Some(r) if TENS.contains(&r) => r,
            _ if u.ends_with('a') => rest,
            _ => continue,
        };
        if TENS.contains(&rest) {
            let unit = if u == "jeden" { "jedna" } else { u };
            return Some(format!("{} {}", rest, unit));
        }
    }
    None
}

/// The replacement for one normalised word, or `None`. `prev` is the word
/// before it in the same number (vi "tư" is 4 only after "mươi").
fn word_alias(lang: &str, tok: &str, prev: Option<&str>) -> Option<String> {
    let base = base_lang(lang);
    if let Some(r) = word_aliases(lang).get(tok) {
        return Some(r.clone());
    }
    match base {
        // The conjunction "و" glued to a declined ten: "وعشرين" -> "وعشرون".
        "ar" => {
            if let Some(rest) = tok.strip_prefix('و') {
                if let Some(r) = word_aliases(lang).get(rest) {
                    return Some(format!("و{}", r));
                }
            }
        }
        "cs" => {
            if let Some(r) = cs_inverted(tok) {
                return Some(r);
            }
        }
        // Native Korean numerals, read as the Sino-Korean the renderer writes
        // (#178): 서른다섯 -> 삼십오.
        "ko" => {
            // `normalize` leaves Hangul decomposed into jamo (NFKD).
            use unicode_normalization::UnicodeNormalization;
            let composed: String = tok.nfc().collect();
            if let Some((v, _)) = ko_native(&composed) {
                return num2words2_core::get_lang_by_key("ko")
                    .and_then(|r| r.to_cardinal(&BigInt::from(v)).ok())
                    .map(|t| crate::normalize(&t));
            }
        }
        // #77: "tư" for 4 after "mươi" (ba mươi tư), "linh" the northern "lẻ".
        "vi" => match tok {
            "tu" if prev == Some("muoi") => return Some("bon".to_string()),
            "linh" => return Some("le".to_string()),
            _ => {}
        },
        _ => {}
    }
    for (l, from, to) in SUBSTRING_VARIANTS {
        if *l != base {
            continue;
        }
        let (from, to) = (crate::normalize(from), crate::normalize(to));
        if from.is_empty() || !tok.contains(from.as_str()) {
            continue;
        }
        let cand = tok.replace(from.as_str(), &to);
        if is_table_word(lang, &cand) {
            return Some(cand);
        }
    }
    None
}

/// Could the normalised word be read through an alias? The walker's cheap
/// gate, so a declined or variant number word may open or extend a run.
pub(super) fn is_alias_word(lang: &str, tok: &str) -> bool {
    if tok.is_empty() || tok.contains(' ') {
        return false;
    }
    let prev = if base_lang(lang) == "vi" { Some("muoi") } else { None };
    word_alias(lang, tok, prev).is_some()
}

/// The text rewritten through the language's aliases, or `None` when nothing
/// changed. Only consulted after every reading of the text as written has
/// failed, and only kept if the rewrite then parses — so it can turn a raise
/// into a value, never change a value.
pub(super) fn alias_rewrite(lang: &str, norm: &str) -> Option<String> {
    let base = base_lang(lang);
    let toks: Vec<&str> = norm.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(toks.len());
    let mut changed = false;
    let mut k = 0;
    while k < toks.len() {
        let tok = toks[k];
        // uk "п'яти": the normaliser split the apostrophe into a space.
        if base == "uk" && tok.chars().count() == 1 && k + 1 < toks.len() {
            let joined = format!("{}{}", tok, toks[k + 1]);
            if let Some(r) = word_alias(lang, &joined, None) {
                out.push(r);
                changed = true;
                k += 2;
                continue;
            }
        }
        let prev = out.last().map(String::as_str);
        if let Some(r) = word_alias(lang, tok, prev) {
            out.push(r);
            changed = true;
        } else {
            out.push(tok.to_string());
        }
        k += 1;
    }
    // vi "hai nghìn không trăm hai mươi": an empty hundreds group spoken as
    // "không trăm" after a scale word; the renderer leaves it out (#77).
    if base == "vi" {
        let mut k = 1;
        while k + 1 < out.len() {
            if out[k] == "khong"
                && out[k + 1] == "tram"
                && matches!(out[k - 1].as_str(), "nghin" | "ngan" | "trieu" | "ty")
            {
                out.drain(k..k + 2);
                changed = true;
            } else {
                k += 1;
            }
        }
    }
    changed.then(|| out.join(" "))
}

// ===========================================================================
// Native Korean numerals (#178)
// ===========================================================================

/// A native Korean numeral, 1..99: the value and whether it is the
/// attributive form used before a counter (한 / 두 / 세 / 네 / 스무, and a
/// compound ending in one: 열두, 스물한). num2words2 writes only the
/// Sino-Korean series, but objects, hours and ages are counted natively.
pub(super) fn ko_native(tok: &str) -> Option<(i64, bool)> {
    const TENS: [(&str, i64); 9] = [
        ("열", 10), ("스물", 20), ("서른", 30), ("마흔", 40), ("쉰", 50), ("예순", 60), ("일흔", 70),
        ("여든", 80), ("아흔", 90),
    ];
    const UNITS: [(&str, i64, bool); 13] = [
        ("하나", 1, false), ("한", 1, true), ("둘", 2, false), ("두", 2, true), ("셋", 3, false),
        ("세", 3, true), ("넷", 4, false), ("네", 4, true), ("다섯", 5, false), ("여섯", 6, false),
        ("일곱", 7, false), ("여덟", 8, false), ("아홉", 9, false),
    ];
    if tok == "스무" {
        return Some((20, true));
    }
    let unit = |w: &str| UNITS.iter().find(|(u, _, _)| *u == w).map(|&(_, v, a)| (v, a));
    for (t, tv) in TENS {
        if let Some(rest) = tok.strip_prefix(t) {
            if rest.is_empty() {
                return Some((tv, false));
            }
            if let Some((uv, attr)) = unit(rest) {
                return Some((tv + uv, attr));
            }
        }
    }
    unit(tok)
}

/// Native counters: what a native numeral counts (개 things, 시 hours, 살
/// years of age, 명 people ...). Compared on the start of the next word, so
/// "개를", "시에", "살이에요" count.
const KO_NATIVE_COUNTERS: &[&str] = &[
    "개월", "개", "시간", "시", "살", "명", "마리", "번", "사람", "권", "잔", "병", "장", "대", "달",
    "그루", "켤레", "벌", "송이", "가지", "군데", "곳", "분", "주", "해", "바퀴", "줄", "통", "채",
];

/// Is a native numeral at this position a count? The attributive forms (한
/// 두 세 네 스무) are also "one/a", "two" ... only before a counter, and 네 is
/// "yes", 세 the Sino age counter, 열 "fever", 쉰 "hoarse": all of these need
/// the counter after them. 한 번 ("once", "give it a try") stays words.
pub(super) fn ko_native_needs_counter(tok: &str) -> bool {
    ko_native(tok).is_some_and(|(_, attr)| attr) || matches!(tok, "열" | "쉰")
}

pub(super) fn ko_counter_follows(head: &str, next: Option<&str>) -> bool {
    let Some(next) = next else { return false };
    if ko_native(head).is_some_and(|(v, _)| v == 1) && next.starts_with('번') {
        return false;
    }
    KO_NATIVE_COUNTERS.iter().any(|c| next.starts_with(c))
}
