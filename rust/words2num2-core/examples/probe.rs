use words2num2_core::parse_scaled;
fn main() {
    let rows: Vec<(&str, &str, &str, &str)> = vec![
        // lang, meaning, NATIVE script form, Latin transliteration in the tables
        ("ti", "zero",   "ባዶ",            "bado"),
        ("ti", "one",    "ሓደ",            "ḥade"),
        ("ti", "eleven", "ዓሰርተ ን ሓደ",     "'aserte n ḥade"),
        ("ky", "zero",   "нөл",           "nöl"),
        ("ky", "forty",  "кырк",          "kırk"),
        ("ky", "42",     "кырк эки",      "kırk eki"),
        ("or", "one",    "ଏକ",            "eka"),
        ("or", "forty",  "ଚାଳିଶ",          "calīśa"),
        ("ckb", "one",   "یەک",           "yek"),
        ("ckb", "1000",  "یەک هەزار",     "yek hezar"),
        ("kok", "one",   "एक",            "ek"),
        ("kok", "100",   "एक शंभर",        "ek xambhar"),
        // Controls: languages whose script was never wrong.
        ("am", "one",    "አንድ",           "and"),
        ("hi", "one",    "एक",            "ek"),
        ("ru", "forty",  "сорок",         "sorok"),
    ];
    println!("{:<5} {:<8} {:>8}  {:<18} {:>8}  {}", "lang", "meaning", "native", "", "latin", "");
    for (lang, meaning, native, latin) in rows {
        println!("{:<5} {:<8} {:>8?}  {:<18} {:>8?}  {:?}",
            lang, meaning, parse_scaled(lang, native), native,
            parse_scaled(lang, latin), latin);
    }
}
