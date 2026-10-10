//! Number words num2words2 does not spell the way speech (or its own longer
//! renderings) do: connector words between groups, connectors glued onto the
//! next word, gender / construct / attributive forms of a multiplier, and
//! oblique forms of a scale word in front of a remainder.
//!
//! The reverse table holds exactly one spelling per value, so each of these
//! forms used to make a parse raise — or, in a sentence, split one number into
//! two ("две тысячи" read as "две 1000"). This module maps them back to the
//! spelling the table knows. Where the renderer itself produces the form
//! (ru "двадцать одна тысяча", kn "ಹತ್ತು ಸಾವಿರದ ಒಂದು") it is derived from
//! renderings; only forms the renderer never writes (pt "duas", ar "ألفين") are
//! listed by hand.

use num_bigint::BigInt;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock, RwLock};

fn base_lang(lang: &str) -> &str {
    lang.split(&['_', '-'][..]).next().unwrap_or(lang)
}

/// Connector/particle words that may sit between number groups and are
/// stripped from a fragment's edges before lookup: es « y », pt/it « e »,
/// ro « de » (« nouă **de** mii »), fa « و » (« ده هزار **و** یک »), da/nb/is
/// « og ». Only the edges of a fragment are trimmed, so a table word that holds
/// the connector inside ("tuttugu og einn" = 21) is untouched.
pub(crate) fn connectors(lang: &str) -> &'static [&'static str] {
    match base_lang(lang) {
        "es" | "gl" => &["y", "e"],
        "pt" | "it" => &["e"],
        "fr" => &["et"],
        "ca" => &["i"],
        "de" => &["und"],
        "nl" | "af" => &["en"],
        "ro" => &["si", "și", "de"],
        "pl" => &["i"],
        "en" => &["and"],
        "ff" => &["e"],  // "ujunere sappo e ɗiɗi" = 12000
        "bg" => &["и"],  // "двадесет и две хиляди"
        "cy" => &["ar"], // "tair ar hugain mil" = 23000
        "sw" => &["na"], // "elfu kumi na mbili" = 12000
        "fa" | "ckb" => &["و"],
        "ku" => &["u"], // "û", normalized
        "is" | "da" | "dk" | "nb" | "no" | "nn" => &["og"],
        "ki" | "rw" | "zu" => &["na"],
        "xh" => &["na", "ana"], // "isigidi na nye", "amashumi amabini ana nye"
        "bm" => &["ni"],
        "ha" => &["da"],
        "lg" => &["mu"],
        "om" => &["fi"],
        "sq" | "lij" => &["e"],
        "rm" if lang.contains("surmiran") || lang.contains("sutsilv") => &["a", "ad"],
        "rm" => &["e", "ed"],
        "tet" => &["resin"],
        "ti" => &["ን"],
        "vi" => &["le", "linh"], // "mười nghìn lẻ một" = 10001
        _ => &[],
    }
}

/// Does the token after a postfix scale word open with a connector? Then it
/// starts the remainder, not the multiplier: bm "miliyɔn **ni** kelen" and ha
/// "miliyan **da** ɗaya" are 10^6 + 1, while "miliyɔn kelen" would be 1 × 10^6.
pub(crate) fn opens_with_connector(lang: &str, tok: Option<&&str>) -> bool {
    tok.is_some_and(|t| connectors(lang).contains(t) || glued_connector(lang, t).is_some())
}

/// "and" written as a prefix of the next word: ar "مليون **و**واحد", he
/// "מיליון **ו**אחת", sn "miriyoni **ne**imwe". The rest of the token, or
/// `None` when it carries no such prefix.
fn glued_connector<'a>(lang: &str, tok: &'a str) -> Option<&'a str> {
    let prefixes: &[&str] = match base_lang(lang) {
        "ar" => &["و"],
        "he" => &["ו"],
        "sn" => &["ne", "na"],
        _ => return None,
    };
    prefixes
        .iter()
        .find_map(|p| tok.strip_prefix(p))
        .filter(|rest| !rest.is_empty())
}

// ---------------------------------------------------------------------------
// Spoken forms the renderer never writes (hand data)
// ---------------------------------------------------------------------------

/// Gender and attributive forms of a number word, natural spelling first, then
/// the renderer's spelling of the same value. num2words2 renders one form per
/// value (ru "два", pt "dois", el "τρία"), while a count agrees with its noun:
/// ru "две тысячи", pt "duas libras", el "τρεις χιλιάδες", mn "гурван зуун".
fn spoken_forms(lang: &str) -> &'static [(&'static str, &'static str)] {
    match base_lang(lang) {
        "ru" => &[("одна", "один"), ("одно", "один"), ("две", "два")],
        "uk" => &[
            ("одна", "один"),
            ("одне", "один"),
            ("одно", "один"),
            ("дві", "два"),
        ],
        "be" => &[("адна", "адзін"), ("адно", "адзін"), ("дзве", "два")],
        "bg" => &[("една", "един"), ("едно", "един"), ("две", "два")],
        // cs renders the feminine "jedna" for 1.
        "cs" | "cz" => &[("jeden", "jedna"), ("jedno", "jedna"), ("dvě", "dva")],
        "sk" => &[("jedna", "jeden"), ("jedno", "jeden"), ("dve", "dva")],
        "hr" => &[("jedna", "jedan"), ("jedno", "jedan"), ("dvije", "dva")],
        "sr" if lang.contains("Latn") => &[
            ("jedna", "jedan"),
            ("jedno", "jedan"),
            ("dve", "dva"),
            ("dvije", "dva"),
        ],
        "sr" => &[("једна", "један"), ("једно", "један"), ("две", "два")],
        "pt" => &[
            ("uma", "um"),
            ("duas", "dois"),
            ("duzentas", "duzentos"),
            ("trezentas", "trezentos"),
            ("quatrocentas", "quatrocentos"),
            ("quinhentas", "quinhentos"),
            ("seiscentas", "seiscentos"),
            ("setecentas", "setecentos"),
            ("oitocentas", "oitocentos"),
            ("novecentas", "novecentos"),
        ],
        "es" => &[
            ("doscientas", "doscientos"),
            ("trescientas", "trescientos"),
            ("cuatrocientas", "cuatrocientos"),
            ("quinientas", "quinientos"),
            ("seiscientas", "seiscientos"),
            ("setecientas", "setecientos"),
            ("ochocientas", "ochocientos"),
            ("novecientas", "novecientos"),
        ],
        // ca "dues-centes" = 200 (renderer "dos-cents").
        "ca" => &[("una", "un"), ("dues", "dos"), ("centes", "cents")],
        "ro" => &[("două", "doi")],
        "el" => &[
            ("μία", "ένα"),
            ("μια", "ένα"),
            ("ένας", "ένα"),
            ("τρεις", "τρία"),
            ("τέσσερις", "τέσσερα"),
            // gladiaio/words2num2#237: num2words2 writes "εκατό δώδεκα" for 112;
            // Greek says "εκατόν" before any remainder.
            ("εκατόν", "εκατό"),
            ("διακόσιες", "διακόσια"),
            ("διακόσιοι", "διακόσια"),
            ("τριακόσιες", "τριακόσια"),
            ("τριακόσιοι", "τριακόσια"),
            ("τετρακόσιες", "τετρακόσια"),
            ("τετρακόσιοι", "τετρακόσια"),
            ("πεντακόσιες", "πεντακόσια"),
            ("πεντακόσιοι", "πεντακόσια"),
            ("εξακόσιες", "εξακόσια"),
            ("εξακόσιοι", "εξακόσια"),
            ("επτακόσιες", "επτακόσια"),
            ("επτακόσιοι", "επτακόσια"),
            ("οκτακόσιες", "οκτακόσια"),
            ("οκτακόσιοι", "οκτακόσια"),
            ("εννιακόσιες", "εννιακόσια"),
            ("εννιακόσιοι", "εννιακόσια"),
        ],
        "is" => &[
            ("ein", "einn"),
            ("eitt", "einn"),
            ("tvær", "tveir"),
            ("tvö", "tveir"),
            ("þrjár", "þrír"),
            ("þrjú", "þrír"),
            ("fjórar", "fjórir"),
            ("fjögur", "fjórir"),
        ],
        // Feminine 2-4, also soft- and aspirate-mutated.
        "cy" => &[
            ("dwy", "dau"),
            ("tair", "tri"),
            ("pedair", "pedwar"),
            ("thair", "thri"),
            ("phedair", "phedwar"),
        ],
        "hu" => &[("két", "kettő")],
        // et "sada" is the everyday 100; num2words2 writes "ükssada".
        "et" => &[("sada", "ükssada")],
        // Attributive forms, used in front of a noun or a bigger unit.
        "mn" => &[
            ("нэгэн", "нэг"),
            ("гурван", "гурав"),
            ("дөрвөн", "дөрөв"),
            ("таван", "тав"),
            ("зургаан", "зургаа"),
            ("долоон", "долоо"),
            ("найман", "найм"),
            ("есөн", "ес"),
            ("арван", "арав"),
            ("хорин", "хорь"),
            ("гучин", "гуч"),
            ("дөчин", "дөч"),
            ("тавин", "тавь"),
            ("жаран", "жар"),
            ("далан", "дал"),
            ("наян", "ная"),
            ("ерэн", "ер"),
            ("зуун", "зуу"),
        ],
        // Class-concord "one" after a scale word ("miriyoni neimwe").
        "sn" => &[("imwe", "motsi")],
        _ => &[],
    }
}

/// Forms of a scale noun that must become the plain scale word (or a
/// multiplier + scale word) before the number is split on its scales: ar dual
/// "مليونان" = 2 × 10^6, accusative "ألفاً" after 11..99, construct "مئتا ألف".
/// Applied to the whole input, so a rewrite may change the token count.
fn scale_forms(lang: &str) -> &'static [(&'static str, &'static str)] {
    match base_lang(lang) {
        "ar" => &[
            ("ألفاً", "ألف"),
            ("ألفا", "ألف"),
            ("مليوناً", "مليون"),
            ("مليونا", "مليون"),
            ("ملياراً", "مليار"),
            ("مليارا", "مليار"),
            ("مليونان", "اثنان مليون"),
            ("مليونين", "اثنان مليون"),
            ("ملياران", "اثنان مليار"),
            ("مليارين", "اثنان مليار"),
            ("ألفين", "ألفان"),
            ("مئتا", "مئتان"),
            ("مئتين", "مئتان"),
        ],
        _ => &[],
    }
}

/// A hand table, normalized once per language.
type FormMap = HashMap<String, String>;

fn normalized_map(
    cache: &'static OnceLock<RwLock<HashMap<String, Arc<FormMap>>>>,
    lang: &str,
    src: fn(&str) -> &'static [(&'static str, &'static str)],
) -> Arc<FormMap> {
    let c = cache.get_or_init(|| RwLock::new(HashMap::new()));
    if let Some(m) = c.read().unwrap().get(lang) {
        return m.clone();
    }
    let m: Arc<FormMap> = Arc::new(
        src(lang)
            .iter()
            .map(|(a, b)| (crate::normalize(a), crate::normalize(b)))
            .filter(|(a, b)| a != b)
            .collect(),
    );
    c.write().unwrap().insert(lang.to_string(), m.clone());
    m
}

fn spoken_map(lang: &str) -> Arc<FormMap> {
    static C: OnceLock<RwLock<HashMap<String, Arc<FormMap>>>> = OnceLock::new();
    normalized_map(&C, lang, spoken_forms)
}

fn scale_map(lang: &str) -> Arc<FormMap> {
    static C: OnceLock<RwLock<HashMap<String, Arc<FormMap>>>> = OnceLock::new();
    normalized_map(&C, lang, scale_forms)
}

// ---------------------------------------------------------------------------
// Forms the renderer writes inside longer numbers (derived)
// ---------------------------------------------------------------------------

/// What the longer renderings of a language teach about its number words.
#[derive(Default)]
struct Derived {
    /// A multiplier or remainder phrase spelled differently from the
    /// standalone number: ru "двадцать одна" (тысяча) = 21, he "שני" (מיליון)
    /// = 2, nb "hundre" (tusen) = 100, tg "яксад" after ҳазор = 100.
    phrases: HashMap<String, i64>,
    /// Oblique form of a scale word in front of a remainder -> the plain
    /// scale word: kn "ಸಾವಿರದ" -> "ಸಾವಿರ", te "లక్షల" -> "లక్ష",
    /// tg "ҳазору" -> "ҳазор".
    oblique: HashMap<String, String>,
    /// Every token of a rendered number that is not a number by itself: kn
    /// "ನೂರ", mn "дөчин", he "עשרת". The sentence walker lets such a token
    /// carry a run on, since only the full phrase is a number.
    parts: HashSet<String>,
    /// The tens morphemes the renderer only writes glued: what is left of a
    /// tens word after its leading unit (fi "kolme|kymmentä", ro
    /// "doua|zeci"), or of a word from 21 to 99 after its trailing unit (es
    /// "veinti|tres", hu "huszon|ketto"). Never a teen's piece: sv "ton" of
    /// "femton" is also the word for a tonne (#982).
    morphemes: HashSet<String>,
}

fn derived_cache() -> &'static RwLock<HashMap<String, Arc<Derived>>> {
    static C: OnceLock<RwLock<HashMap<String, Arc<Derived>>>> = OnceLock::new();
    C.get_or_init(|| RwLock::new(HashMap::new()))
}

/// The derived forms of `lang`. `tab` is its cardinal reverse table; only the
/// renderer and `tab` are consulted, so a derivation never depends on the
/// lookup it feeds.
fn derived(lang: &str, tab: &HashMap<String, i64>) -> Arc<Derived> {
    if let Some(d) = derived_cache().read().unwrap().get(lang) {
        return d.clone();
    }
    let d = Arc::new(derive(lang, tab));
    derived_cache()
        .write()
        .unwrap()
        .insert(lang.to_string(), d.clone());
    d
}

/// Is `form` the same number as `canon`, only spelled another way? Same
/// tokens up to inflection (each differing pair starts alike: "две"/"два",
/// "עשרת"/"עשר"), or a "one" said or dropped in front ("hundre" /
/// "ett hundre", "яксад" / "сад", am "አንድ መቶ" / "መቶ"). This keeps a probe
/// whose rendering restructured the number (en_IN "ten lakh") from being taken
/// for an inflected multiplier.
/// Two spellings of one word: same first letter, and what they share at both
/// ends is at least a third of the shorter one ("две"/"два", "שנים"/"שתים",
/// "douăsprezece"/"doisprezece", "tair"/"tri").
fn inflected(a: &str, b: &str) -> bool {
    let head = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    let tail = a
        .chars()
        .rev()
        .zip(b.chars().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let shorter = a.chars().count().min(b.chars().count());
    head >= 1 && (head + tail).min(shorter) * 3 >= shorter
}

fn same_number_shape(form: &str, canon: &str, ones: &[String]) -> bool {
    if form == canon {
        return false;
    }
    let a: Vec<&str> = form.split_whitespace().collect();
    let b: Vec<&str> = canon.split_whitespace().collect();
    if a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x == y || inflected(x, y)) {
        return true;
    }
    ones.iter().any(|one| {
        form == format!("{one} {canon}")
            || canon == format!("{one} {form}")
            || form == format!("{one}{canon}")
            || canon == format!("{one}{form}")
    })
}

fn derive(lang: &str, tab: &HashMap<String, i64>) -> Derived {
    let mut d = Derived::default();
    let Some(l) = num2words2_core::get_lang_by_key(lang) else {
        return d;
    };
    let render = |n: i64| {
        l.to_cardinal(&BigInt::from(n))
            .ok()
            .map(|w| crate::normalize(&w))
            .filter(|w| !w.is_empty() && !w.chars().any(|c| c.is_ascii_digit()))
    };
    let conns = connectors(lang);
    let trim = |toks: &[&str]| crate::trim_connectors(&toks.join(" "), conns);
    // The words for "one" a multiplier may carry or drop: nb "ett" (hundre),
    // tg "як" (сад) — not always the standalone 1 (nb "en").
    let mut ones: Vec<String> = Vec::new();
    for n in [1i64, 100, 1000] {
        if let Some(w) = render(n) {
            if let Some(first) = w.split_whitespace().next() {
                if !ones.iter().any(|o| o == first) {
                    ones.push(first.to_string());
                }
            }
        }
    }
    let mut phrases: HashMap<String, Option<i64>> = HashMap::new();
    let mut add_phrase = |form: String, v: i64, canon: &str| {
        if form.is_empty() || tab.contains_key(&form) || !same_number_shape(&form, canon, &ones) {
            return;
        }
        let slot = phrases.entry(form).or_insert(Some(v));
        if *slot != Some(v) {
            *slot = None; // two values spelled alike: neither is kept
        }
    };
    let mut oblique: HashMap<String, Option<String>> = HashMap::new();
    for p in [3u32, 6, 9] {
        let mag = 10i64.pow(p);
        // Multipliers: the words left of the scale in k · 10^p.
        let mut glued: Vec<(i64, String)> = Vec::new();
        for k in 1..1000i64 {
            let Some(text) = render(k * mag) else {
                continue;
            };
            let toks: Vec<&str> = text.split_whitespace().collect();
            if toks.len() == 1 && k > 1 {
                glued.push((k, text.clone()));
            }
            // A last token that is a number is a multiplier after its scale (ha
            // "dubu biyu") or a group of its own (ko "이만 천" = 20000 + 1000).
            if toks.len() < 2 || tab.contains_key(toks[toks.len() - 1]) {
                continue;
            }
            if let Some(canon) = render(k) {
                add_phrase(trim(&toks[..toks.len() - 1]), k, &canon);
            }
        }
        // The same for a multiplier glued to its scale: rm "ventgaduamilli" =
        // "ventgadua" (22, feminine) + "milli". The scale morpheme is the
        // tail every such rendering shares.
        // Thousands only: above them glued scales stack (th "สองพันล้าน" =
        // 2 · 1000 · 10^6) and the shared tail is no single morpheme.
        if p == 3 && glued.len() >= 10 {
            let mut morpheme = glued[0].1.clone();
            for (_, g) in &glued[1..] {
                morpheme = crate::common_suffix(&morpheme, g);
            }
            if morpheme.chars().count() >= 3 {
                for (k, g) in &glued {
                    let form = &g[..g.len() - morpheme.len()];
                    // A bare stem of the number (dv "ދިހަ" in "ދިހަހާސް", 10 =
                    // "ދިހައެއް") is how the language glues, not a gender form:
                    // the scale derivation already reads those tokens whole. One
                    // dropped letter is an ending (rm "ventgatrai(s)").
                    let stem =
                        |c: &String| c.strip_prefix(form).is_some_and(|t| t.chars().count() > 1);
                    if let Some(canon) = render(*k).filter(|c| !stem(c)) {
                        add_phrase(form.to_string(), *k, &canon);
                    }
                }
            }
        }
        // Remainders and oblique scale words: k · 10^p + r against k · 10^p.
        for k in [1i64, 2, 10, 21] {
            let Some(round) = render(k * mag) else {
                continue;
            };
            let rt: Vec<&str> = round.split_whitespace().collect();
            let Some(&scale) = rt.last() else { continue };
            for r in 1..1000i64 {
                let (Some(text), Some(canon)) = (render(k * mag + r), render(r)) else {
                    continue;
                };
                let ct: Vec<&str> = text.split_whitespace().collect();
                if ct.len() <= rt.len() || ct[..rt.len() - 1] != rt[..rt.len() - 1] {
                    continue;
                }
                let at = ct[rt.len() - 1];
                if at != scale {
                    // kn "ಹತ್ತು ಸಾವಿರದ ಒಂದು" next to "ಹತ್ತು ಸಾವಿರ".
                    if tab.contains_key(at) || at.chars().next() != scale.chars().next() {
                        continue;
                    }
                    let slot = oblique
                        .entry(at.to_string())
                        .or_insert(Some(scale.to_string()));
                    if slot.as_deref() != Some(scale) {
                        *slot = None;
                    }
                }
                add_phrase(trim(&ct[rt.len()..]), r, &canon);
            }
        }
    }
    d.phrases = phrases
        .into_iter()
        .filter_map(|(k, v)| Some((k, v?)))
        .collect();
    d.oblique = oblique
        .into_iter()
        .filter_map(|(k, v)| Some((k, v?)))
        .collect();
    let mut parts: HashSet<String> = HashSet::new();
    for key in tab.keys().chain(d.phrases.keys()) {
        for t in key.split_whitespace() {
            if !tab.contains_key(t) && !conns.contains(&t) && t.chars().count() >= 3 {
                parts.insert(t.to_string());
            }
        }
    }
    d.parts = parts;
    let units: Vec<(i64, &str)> = tab.iter().filter(|(k, v)| (1..=10).contains(*v) && !k.contains(' ')).map(|(k, v)| (*v, k.as_str())).collect();
    let mut morphemes: HashSet<String> = HashSet::new();
    let mut keep = |m: &str| {
        // Four letters at least: hu "van" (60 = "hat|van") is also "is".
        if m.chars().count() >= 4 && !tab.contains_key(m) && !conns.contains(&m) {
            morphemes.insert(m.to_string());
        }
    };
    for (k, v) in tab.iter().filter(|(k, v)| (20..=99).contains(*v) && !k.contains(' ')) {
        for (u, uw) in &units {
            if v % 10 == 0 && (2..=9).contains(u) && *v == u * 10 {
                if let Some(rest) = k.strip_prefix(uw) {
                    keep(rest);
                }
            }
            // Units 1-9 only: nb "førti" less "ti" (10) leaves "før", which
            // is the word for "before" (#982).
            if *v % 10 != 0 && (1..=9).contains(u) && *v % 10 == *u {
                if let Some(head) = k.strip_suffix(uw) {
                    keep(head);
                }
            }
        }
    }
    d.morphemes = morphemes;
    d
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Every combination of replacing the tokens of `norm` that have a spoken
/// form by the renderer's spelling (at most four such tokens). All of them,
/// because a token can be right as it stands in one place and need the
/// replacement in another (mn "арван гурван": 13 is "арван гурав").
fn spoken_variants(map: &FormMap, norm: &str) -> Vec<String> {
    let toks: Vec<&str> = norm.split_whitespace().collect();
    let slots: Vec<usize> = (0..toks.len())
        .filter(|&i| map.contains_key(toks[i]))
        .collect();
    if slots.is_empty() || slots.len() > 4 {
        return Vec::new();
    }
    (1u32..(1 << slots.len()))
        .map(|mask| {
            let mut v = toks.clone();
            for (k, &pos) in slots.iter().enumerate() {
                if mask & (1 << k) != 0 {
                    v[pos] = map[toks[pos]].as_str();
                }
            }
            v.join(" ")
        })
        .collect()
}

/// The value of a cardinal fragment the table misses, read through its forms,
/// and whether that reading is *bound*: valid only inside a longer number (a
/// derived multiplier / remainder phrase, or a glued "and"). A spoken gender
/// form ("две", "duas") is a number on its own; ar "وواحد" ("and one") is not.
/// Called by the table lookup with the table in hand.
pub(crate) fn fragment_value(
    lang: &str,
    tab: &HashMap<String, i64>,
    norm: &str,
) -> Option<(i64, bool)> {
    let spoken = spoken_map(lang);
    let free = |s: &str| -> Option<i64> {
        tab.get(s).copied().or_else(|| {
            spoken_variants(&spoken, s)
                .iter()
                .find_map(|v| tab.get(v).copied())
        })
    };
    if let Some(v) = free(norm) {
        return Some((v, false));
    }
    let d = derived(lang, tab);
    if let Some(&v) = d.phrases.get(norm) {
        return Some((v, true));
    }
    // ar "مليون وواحد": the remainder "وواحد" is "and" + "واحد". Only the
    // first token: inside the table forms the prefix is part of the word.
    let first = norm.split_whitespace().next()?;
    let rest = glued_connector(lang, first)?;
    let stripped = format!("{rest}{}", &norm[first.len()..]);
    free(&stripped)
        .or_else(|| d.phrases.get(&stripped).copied())
        .map(|v| (v, true))
}

/// The input respelled with the scale words the scale split knows: oblique
/// scale forms (kn "ಸಾವಿರದ", te "వేయిల", tg "ҳазору"), ar dual and accusative
/// scale nouns, and da "tusind(e)" glued into its neighbours. `None` when
/// nothing changes. A last resort for input every other reading declined.
pub(crate) fn scale_spelling(lang: &str, norm: &str) -> Option<String> {
    let scale = scale_map(lang);
    let oblique = crate::with_cardinal_table(lang, |tab| derived(lang, tab).oblique.clone())?;
    let mut out: Vec<String> = Vec::new();
    for t in norm.split_whitespace() {
        if let Some(s) = scale.get(t).or_else(|| oblique.get(t)) {
            out.push(s.clone());
        } else if matches!(base_lang(lang), "da" | "dk") {
            out.extend(split_tusind(t));
        } else {
            out.push(t.to_string());
        }
    }
    let s = out.join(" ");
    (s != norm).then_some(s)
}

/// da glues the thousand to its neighbours and writes "tusinde" before a
/// remainder: "tolvtusinde" -> "tolv tusind", "ettusindsyvhundrede" ->
/// "et tusind syvhundrede".
fn split_tusind(tok: &str) -> Vec<String> {
    let Some(pos) = tok.find("tusind") else {
        return vec![tok.to_string()];
    };
    let pre = &tok[..pos];
    let mut suf = &tok[pos + "tusind".len()..];
    if suf == "e" {
        suf = "";
    }
    [pre, "tusind", suf]
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Is `norm` (one token) only a number inside a longer one — a derived
/// multiplier/remainder form (he "שני" before מיליון) or a glued "and" (ar
/// "وواحد")? The sentence walker keeps such a word unless a run grows past it.
pub fn is_bound_form(lang: &str, norm: &str) -> bool {
    !norm.contains(' ')
        && crate::with_cardinal_table(lang, |tab| {
            matches!(fragment_value(lang, tab, norm), Some((_, true)))
        })
        .unwrap_or(false)
}

/// Is `norm` a word of the rendered numbers that is no number by itself (kn
/// "ನೂರ", mn "дөчин", he "עשרת", ha "sha")? The sentence walker lets it carry a
/// run on; connectors and words under three letters are left out.
pub fn is_number_part(lang: &str, norm: &str) -> bool {
    crate::with_cardinal_table(lang, |tab| derived(lang, tab).parts.contains(norm)).unwrap_or(false)
}

/// Is `norm` (one token) a tens morpheme the renderer only writes glued, as
/// spoken text may split it off: fi "kymmentä", ro "zeci", es "veinti", hu
/// "huszon" (#947)? See `Derived::morphemes`; a stray piece is none: fr
/// "eux", sv "ton", de "sie", nb "før" (#285, #982).
pub fn is_glued_morpheme(lang: &str, norm: &str) -> bool {
    !norm.contains(' ')
        && crate::with_cardinal_table(lang, |tab| derived(lang, tab).morphemes.contains(norm)).unwrap_or(false)
}

/// Does the renderer spell another value exactly like `norm` too? Where the
/// multiplier follows its unit and "and" also joins tens and units, bm
/// "ba tan ni kelen" is both 10000 + 1 and 1000 × 11: the renderer writes
/// 10001 and 11000 alike. Here the rival reading is a thousand-unit followed by
/// a multiplier that spans a connector; when it also renders as `norm`, the
/// text is ambiguous and must not be read as either.
pub(crate) fn has_rival_reading(lang: &str, v: i64, norm: &str) -> bool {
    let conns = connectors(lang);
    let toks: Vec<&str> = norm.split_whitespace().collect();
    (0..toks.len().saturating_sub(1)).any(|i| {
        let rest = &toks[i + 1..];
        if !rest.iter().any(|t| conns.contains(t)) {
            return false;
        }
        let Some(unit) =
            crate::lookup_plain(lang, toks[i]).filter(|&u| u >= 1000 && crate::is_power_of_ten(u))
        else {
            return false;
        };
        let Some(m) = crate::lookup_plain(lang, &rest.join(" ")).filter(|m| (2..1000).contains(m))
        else {
            return false;
        };
        let head = crate::trim_connectors(&toks[..i].join(" "), conns);
        let head = if head.is_empty() {
            Some(0)
        } else {
            crate::parse_scaled_norm_unchecked(lang, &head)
        };
        head.is_some_and(|h| {
            let rival = h + unit * m;
            rival != v && crate::renders_as(lang, rival, norm)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::w2n_sentence::{words2num, words2num_sentence};

    fn card(lang: &str, text: &str) -> Option<i64> {
        match words2num(text, lang, "cardinal") {
            Ok(crate::w2n_lang_en::W2nValue::Int(v)) => i64::try_from(v).ok(),
            _ => None,
        }
    }

    fn sent(lang: &str, text: &str) -> String {
        words2num_sentence(text, lang, "cardinal", false).unwrap()
    }

    /// Every hand-listed form maps to a word the renderer really writes, so a
    /// typo or a reversed pair (cs renders "jedna" for 1) cannot hide.
    #[test]
    fn spoken_forms_map_to_rendered_words() {
        for lang in [
            "ru", "uk", "be", "bg", "cs", "sk", "hr", "sr", "sr_Latn", "pt", "es", "ca", "ro",
            "el", "is", "cy", "hu", "mn", "sn", "et",
        ] {
            let words: HashSet<String> = crate::with_cardinal_table(lang, |tab| {
                tab.keys()
                    .flat_map(|k| k.split_whitespace().map(str::to_string))
                    .collect()
            })
            .unwrap();
            for (form, canon) in spoken_map(lang).iter() {
                assert!(
                    words.contains(canon),
                    "{lang}: {form} -> {canon} is no rendered word"
                );
            }
        }
    }

    /// gladiaio/words2num2#85: a connector after a scale word.
    #[test]
    fn connector_after_scale_word() {
        for (lang, text, v) in [
            ("fa", "ده هزار و یک", 10001),
            ("fa", "یک میلیون و یک", 1000001),
            ("ckb", "دە هەزار و یەک", 10001),
            ("ku", "yek milyon û yek", 1000001),
            ("ki", "ikũmi ngiri na ĩmwe", 10001),
            ("is", "ein milljón og einn", 1000001),
            ("da", "ti tusind og et", 10001),
            ("nb", "ti tusen og en", 10001),
            ("sq", "dhjetë mijë e një", 10001),
            ("vi", "mười nghìn lẻ một", 10001),
            ("ti", "ዓሰርተ ሽሕ ን ሓደ", 10001),
            ("zu", "ishumi inkulungwane na kunye", 10001),
            ("tet", "rihun sanulu resin ida resin ida", 11001),
            // Postfix scales: the connector starts the remainder.
            ("bm", "miliyɔn ni kelen", 1000001),
            ("ha", "miliyan da ɗaya", 1000001),
        ] {
            assert_eq!(card(lang, text), Some(v), "{lang} {text}");
        }
        // Table words holding the connector are untouched.
        assert_eq!(card("is", "tuttugu og einn"), Some(21));
        // bm writes 10001 and 11000 alike: neither is guessed.
        assert_eq!(card("bm", "ba tan ni kelen"), None);
        // A run never ends on the connector, nor starts with one.
        assert_eq!(sent("da", "tre og fire"), "3 og 4");
        assert_eq!(sent("ha", "da biyu"), "da 2");
        assert_eq!(sent("ro", "două mii de lei"), "2000 de lei");
    }

    /// gladiaio/words2num2#86: ar و / he ו glued to the remainder.
    #[test]
    fn glued_conjunction_after_scale_word() {
        assert_eq!(card("ar", "عشرة آلاف وواحد"), Some(10001));
        assert_eq!(card("ar", "مليون وواحد"), Some(1000001));
        assert_eq!(card("he", "עשרת אלפים ואחת"), Some(10001));
        assert_eq!(card("he", "מיליון ואחת"), Some(1000001));
        // "and five" is no digit of "three five"; alone, the conjunction is
        // peeled off as a proclitic and kept in front of the figure.
        assert_eq!(sent("ar", "وخمسة"), "و5");
        assert_eq!(sent("ar", "ثلاثة وخمسة"), "3 وخمسة");
    }

    /// gladiaio/words2num2#87: the multiplier in a gender / construct /
    /// attributive form.
    #[test]
    fn inflected_multiplier() {
        for (lang, text, v) in [
            (
                "be",
                "дваццаць адна тысяча дзевяцьсот шэсцьдзясят чатыры",
                21964,
            ),
            (
                "bg",
                "двадесет и две хиляди деветстотин шестдесет и един",
                22961,
            ),
            (
                "hr",
                "dvadeset jedna tisuća devetsto šezdeset četiri",
                21964,
            ),
            ("cs", "dvě miliardy", 2_000_000_000),
            ("el", "είκοσι μία χιλιάδες", 21000),
            ("cy", "tair ar hugain mil", 23000),
            (
                "is",
                "tuttugu og eitt þúsund níu hundruð sextíu og fjórir",
                21964,
            ),
            ("he", "אחד עשר אלף", 11000),
            ("he", "שני מיליון", 2_000_000),
            ("hu", "kétmillió", 2_000_000),
            ("et", "sada tuhat", 100_000),
        ] {
            assert_eq!(card(lang, text), Some(v), "{lang} {text}");
        }
    }

    /// gladiaio/words2num2#88: ar dual and accusative scale nouns.
    #[test]
    fn arabic_dual_and_accusative_scales() {
        assert_eq!(card("ar", "مليونان"), Some(2_000_000));
        assert_eq!(card("ar", "ملياران"), Some(2_000_000_000));
        assert_eq!(card("ar", "مليونان وواحد"), Some(2_000_001));
        assert_eq!(
            card("ar", "أحد عشر ألفاً وتسعمائة وأربعة وتسعون"),
            Some(11994)
        );
        assert_eq!(card("ar", "مئتا ألف"), Some(200_000));
        assert_eq!(card("ar", "ألفين"), Some(2000));
    }

    /// gladiaio/words2num2#89: da "tusinde" and the thousand glued to its neighbours.
    #[test]
    fn danish_glued_thousand() {
        assert_eq!(
            card("da", "tolvtusinde og trehundrede og femogfyrre"),
            Some(12345)
        );
        assert_eq!(card("da", "ti tusinde og et"), Some(10001));
        assert_eq!(
            card("da", "ethundrede og ettusindsyvhundrede og fireogtyve"),
            Some(101724)
        );
        assert_eq!(
            card(
                "da",
                "en millioner tohundrede og fireogtredivetusindfemhundrede og syvogtreds"
            ),
            Some(1_234_567)
        );
    }

    /// gladiaio/words2num2#93, #96, #97: oblique scale words before a remainder.
    #[test]
    fn oblique_scale_words() {
        assert_eq!(card("kn", "ಹತ್ತು ಸಾವಿರದ ಒಂದು"), Some(10001));
        assert_eq!(card("kn", "ಒಂದು ಲಕ್ಷದ ಒಂದು"), Some(100001));
        assert_eq!(card("te", "పది వేయిల ఒకటి"), Some(10001));
        assert_eq!(card("te", "ఒకటి లక్షల ఒకటి"), Some(100001));
        assert_eq!(card("tg", "даҳ ҳазору як"), Some(10001));
        assert_eq!(card("tg", "як миллиону як"), Some(1_000_001));
    }

    /// gladiaio/words2num2#95: rm connector glued after the thousand.
    #[test]
    fn romansh_glued_connector() {
        assert_eq!(card("rm", "dieschmilliedin"), Some(10001));
        assert_eq!(card("rm", "dieschmillietschient"), Some(10100));
        assert_eq!(card("rm_surmiran", "dieschmelladegn"), Some(10001));
    }

    /// gladiaio/words2num2#98: the bare hundred inside a scaled number.
    #[test]
    fn bare_hundred_in_scaled_number() {
        assert_eq!(card("nb", "hundre tusen"), Some(100_000));
        assert_eq!(card("no", "en million hundre tusen"), Some(1_100_000));
        assert_eq!(card("tg", "даҳ ҳазор яксад"), Some(10100));
    }

    /// gladiaio/words2num2#101: sn glued ne- and class-concord "imwe".
    #[test]
    fn shona_glued_connector() {
        assert_eq!(
            card(
                "sn",
                "zvuru gumi nepiri nemazana matatu nemakumi mana neshanu"
            ),
            Some(12345)
        );
        assert_eq!(card("sn", "miriyoni neimwe"), Some(1_000_001));
        assert_eq!(card("sn", "bhiriyoni nemotsi"), Some(1_000_000_001));
    }

    /// gladiaio/words2num2#105: feminine numerals on their own.
    #[test]
    fn feminine_numerals() {
        for (lang, text, v) in [
            ("pt", "duas", 2),
            ("pt", "duzentas", 200),
            ("pt", "uma", 1),
            ("es", "doscientas", 200),
            ("ru", "две", 2),
            ("uk", "дві", 2),
            ("cs", "dvě", 2),
            ("ca", "dues", 2),
            ("ca", "dues-centes", 200),
            ("el", "τρεις", 3),
            ("is", "tvö", 2),
        ] {
            assert_eq!(card(lang, text), Some(v), "{lang} {text}");
        }
    }

    /// gladiaio/words2num2#66: gendered and combining forms keep a run whole.
    #[test]
    fn gendered_forms_keep_sentence_runs() {
        for (lang, text, want) in [
            (
                "ru",
                "я заплатил две тысячи двадцать четыре рубля",
                "я заплатил 2024 рубля",
            ),
            ("ru", "одна тысяча рублей", "1000 рублей"),
            ("hr", "ima dvije tisuće dvadeset četiri", "ima 2024"),
            ("is", "þrjú hundruð krónur", "300 krónur"),
            ("cs", "dvě stě korun", "200 korun"),
            ("ro", "două mii de lei", "2000 de lei"),
            ("kn", "qqx ಹನ್ನೆರಡು ಸಾವಿರ ಮೂರು ನೂರ ನಲವತ್ತೈದು zzx", "qqx 12345 zzx"),
            (
                "mn",
                "qqx арван хоёр мянга гурван зуун дөчин тав zzx",
                "qqx 12345 zzx",
            ),
            ("he", "שני מיליון שקל", "2000000 שקל"),
        ] {
            assert_eq!(sent(lang, text), want, "{lang} {text}");
        }
        // A bound form alone stays a word: he "שני" is also "Monday".
        assert_eq!(sent("he", "נפגשנו ביום שני בבוקר"), "נפגשנו ביום שני בבוקר");
    }

    /// gladiaio/words2num2#237: el "εκατόν" before a remainder (the renderer
    /// writes "εκατό δώδεκα").
    #[test]
    fn greek_ekaton() {
        assert_eq!(card("el", "εκατόν δώδεκα"), Some(112));
        assert_eq!(card("el", "εκατό δώδεκα"), Some(112));
        assert_eq!(
            sent("el", "Μένω στην οδό Ερμού εκατόν δώδεκα."),
            "Μένω στην οδό Ερμού 112."
        );
    }
}
