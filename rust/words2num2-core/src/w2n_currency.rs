//! Currency fold for sentence mode (`words2num_sentence(..., currency=True)`).
//!
//! Runs on the walker's output, where the numbers are already figures: an amount
//! spoken as number + currency word (+ connector + subunit) is written the way the
//! language writes that currency — en `$1,355.28`, fr `1 355,28 €` / `43,20 $`,
//! es `$154.92` (dollars, pesos) / `500,20 €`, de `23,50 €`, nl `€ 20,50`,
//! pt `R$ 200,50`.
//!
//! Rules:
//! * the currency drives the symbol and its position, the language drives the
//!   thousands and decimal separators (en `1,355.28`, fr `1 355,28`, de/es/it/pt/nl
//!   `1.355,28`), except for dollars and pesos in Spanish which follow the
//!   Latin-American `$1,234.56`;
//! * subunits: a subunit word after a connector (`and`, `et`, `con`, `y`, `und`,
//!   `e`) or bare (`two euros fifty`), or on their own (`ninety nine cents` ->
//!   `$0.99`, the language's default currency);
//! * no decimals unless a subunit was spoken (`twelve dollars` -> `$12`); JPY and
//!   KRW never carry decimals;
//! * a currency word without a number in front stays a word (`des euros`,
//!   `quelques dollars`); `1 euro` is `1 €`.

/// A currency the fold knows: ISO code, symbol, minor-unit exponent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Currency {
    pub code: &'static str,
    pub symbol: &'static str,
    pub decimals: u8,
}

const USD: Currency = Currency {
    code: "USD",
    symbol: "$",
    decimals: 2,
};
const EUR: Currency = Currency {
    code: "EUR",
    symbol: "€",
    decimals: 2,
};
const GBP: Currency = Currency {
    code: "GBP",
    symbol: "£",
    decimals: 2,
};
const JPY: Currency = Currency {
    code: "JPY",
    symbol: "¥",
    decimals: 0,
};
const CNY: Currency = Currency {
    code: "CNY",
    symbol: "CN¥",
    decimals: 2,
};
const CHF: Currency = Currency {
    code: "CHF",
    symbol: "CHF",
    decimals: 2,
};
const CAD: Currency = Currency {
    code: "CAD",
    symbol: "CA$",
    decimals: 2,
};
const AUD: Currency = Currency {
    code: "AUD",
    symbol: "A$",
    decimals: 2,
};
const NZD: Currency = Currency {
    code: "NZD",
    symbol: "NZ$",
    decimals: 2,
};
const MXN: Currency = Currency {
    code: "MXN",
    symbol: "$",
    decimals: 2,
};
const PESO: Currency = Currency {
    code: "PESO",
    symbol: "$",
    decimals: 2,
};
const BRL: Currency = Currency {
    code: "BRL",
    symbol: "R$",
    decimals: 2,
};
const INR: Currency = Currency {
    code: "INR",
    symbol: "₹",
    decimals: 2,
};
const KRW: Currency = Currency {
    code: "KRW",
    symbol: "₩",
    decimals: 0,
};
const RUB: Currency = Currency {
    code: "RUB",
    symbol: "₽",
    decimals: 2,
};
const TRY: Currency = Currency {
    code: "TRY",
    symbol: "₺",
    decimals: 2,
};
const PLN: Currency = Currency {
    code: "PLN",
    symbol: "zł",
    decimals: 2,
};
const SEK: Currency = Currency {
    code: "SEK",
    symbol: "kr",
    decimals: 2,
};
const NOK: Currency = Currency {
    code: "NOK",
    symbol: "kr",
    decimals: 2,
};
const DKK: Currency = Currency {
    code: "DKK",
    symbol: "kr",
    decimals: 2,
};
const XOF: Currency = Currency {
    code: "XOF",
    symbol: "FCFA",
    decimals: 0,
};
const MAD: Currency = Currency {
    code: "MAD",
    symbol: "MAD",
    decimals: 2,
};
const ZAR: Currency = Currency {
    code: "ZAR",
    symbol: "R",
    decimals: 2,
};

/// How a language writes an amount of a currency.
#[derive(Clone, Copy, Debug)]
struct Style {
    /// Symbol before the number (`$12`, `€ 20,50`) or after (`12 €`).
    prefix: bool,
    /// A space between symbol and number (`€ 20,50`, `12 €`, `CHF 10`).
    space: bool,
    thousands: &'static str,
    decimal: char,
}

const EN_STYLE: Style = Style {
    prefix: true,
    space: false,
    thousands: ",",
    decimal: '.',
};
const LATAM: Style = Style {
    prefix: true,
    space: false,
    thousands: ",",
    decimal: '.',
};
const FR_STYLE: Style = Style {
    prefix: false,
    space: true,
    thousands: " ",
    decimal: ',',
};
const CONTINENTAL: Style = Style {
    prefix: false,
    space: true,
    thousands: ".",
    decimal: ',',
};
const NL_STYLE: Style = Style {
    prefix: true,
    space: true,
    thousands: ".",
    decimal: ',',
};
const PT_STYLE: Style = Style {
    prefix: true,
    space: true,
    thousands: ".",
    decimal: ',',
};
const CODE_PREFIX: Style = Style {
    prefix: true,
    space: true,
    thousands: ",",
    decimal: '.',
};

/// The symbol and layout for `cur` in `lang`.
fn style(lang: &str, cur: Currency) -> (String, Style) {
    let sym = cur.symbol;
    match lang {
        "en" => match cur.code {
            "CHF" => (sym.to_string(), CODE_PREFIX),
            "SEK" | "NOK" | "DKK" | "PLN" => (
                sym.to_string(),
                Style {
                    prefix: false,
                    ..EN_STYLE
                },
            ),
            _ => (sym.to_string(), EN_STYLE),
        },
        "fr" => {
            let s = match cur.code {
                "CAD" => "$ CA".to_string(), // OQLF: « 5 $ CA »
                "AUD" => "$ AU".to_string(),
                "NZD" => "$ NZ".to_string(),
                "USD" => "$".to_string(),
                _ => sym.to_string(),
            };
            (s, FR_STYLE)
        }
        "es" => match cur.code {
            // Latin-American dollars and pesos: `$1,234.56`.
            "USD" | "MXN" | "PESO" | "CAD" | "AUD" | "NZD" => (sym.to_string(), LATAM),
            "CHF" => (sym.to_string(), CODE_PREFIX),
            _ => (sym.to_string(), CONTINENTAL),
        },
        "pt" => match cur.code {
            "USD" => ("US$".to_string(), PT_STYLE),
            "BRL" => (sym.to_string(), PT_STYLE),
            "EUR" | "GBP" | "JPY" | "CHF" | "INR" | "RUB" | "PLN" | "SEK" | "NOK" | "DKK"
            | "TRY" | "XOF" | "MAD" | "ZAR" => (sym.to_string(), CONTINENTAL),
            _ => (sym.to_string(), PT_STYLE),
        },
        "nl" => (sym.to_string(), NL_STYLE),
        // de, it, ca, gl and everything else on the continent.
        _ => match cur.code {
            "CHF" => (sym.to_string(), CODE_PREFIX),
            _ => (sym.to_string(), CONTINENTAL),
        },
    }
}

/// A word of the currency lexicon: the currency it names, and whether it is a
/// subunit (`cents`) or a main unit (`dollars`).
#[derive(Clone, Copy, Debug)]
struct Word {
    cur: Currency,
    sub: bool,
}

const fn unit(cur: Currency) -> Word {
    Word { cur, sub: false }
}
const fn sub(cur: Currency) -> Word {
    Word { cur, sub: true }
}

/// The default currency of a bare subunit ("ninety nine cents") per language.
fn default_currency(lang: &str) -> Currency {
    match lang {
        "en" | "es" => USD, // es: the Latin-American callers of an IVR
        "pt" => BRL,
        _ => EUR,
    }
}

/// Is `norm` (normalized) a subunit word of `lang`: "cents", "centimes",
/// "centesimi"? The walker asks before reading it as an ordinal (it
/// "centesimo" is both the hundredth and the cent).
pub fn is_subunit_word(lang: &str, norm: &str) -> bool {
    lexicon(lang).iter().any(|(name, w)| w.sub && *name == norm)
}

static LEX_EN: &[(&str, Word)] = &[
    ("us dollars", unit(USD)),
    ("us dollar", unit(USD)),
    ("american dollars", unit(USD)),
    ("american dollar", unit(USD)),
    ("canadian dollars", unit(CAD)),
    ("canadian dollar", unit(CAD)),
    ("australian dollars", unit(AUD)),
    ("australian dollar", unit(AUD)),
    ("new zealand dollars", unit(NZD)),
    ("new zealand dollar", unit(NZD)),
    ("swiss francs", unit(CHF)),
    ("swiss franc", unit(CHF)),
    ("mexican pesos", unit(MXN)),
    ("mexican peso", unit(MXN)),
    ("pounds sterling", unit(GBP)),
    ("pound sterling", unit(GBP)),
    ("south african rand", unit(ZAR)),
    ("dollars", unit(USD)),
    ("dollar", unit(USD)),
    ("bucks", unit(USD)),
    ("buck", unit(USD)),
    ("euros", unit(EUR)),
    ("euro", unit(EUR)),
    ("pounds", unit(GBP)),
    ("pound", unit(GBP)),
    ("quid", unit(GBP)),
    ("sterling", unit(GBP)),
    ("yen", unit(JPY)),
    ("yuan", unit(CNY)),
    ("renminbi", unit(CNY)),
    ("francs", unit(CHF)),
    ("franc", unit(CHF)),
    ("pesos", unit(PESO)),
    ("peso", unit(PESO)),
    ("reais", unit(BRL)),
    ("real", unit(BRL)),
    ("rupees", unit(INR)),
    ("rupee", unit(INR)),
    ("won", unit(KRW)),
    ("rubles", unit(RUB)),
    ("ruble", unit(RUB)),
    ("roubles", unit(RUB)),
    ("rouble", unit(RUB)),
    ("lira", unit(TRY)),
    ("liras", unit(TRY)),
    ("zloty", unit(PLN)),
    ("zlotys", unit(PLN)),
    ("kronor", unit(SEK)),
    ("krona", unit(SEK)),
    ("kroner", unit(NOK)),
    ("krone", unit(NOK)),
    ("rand", unit(ZAR)),
    ("dirhams", unit(MAD)),
    ("dirham", unit(MAD)),
    ("cents", sub(USD)),
    ("cent", sub(USD)),
    ("pence", sub(GBP)),
    ("penny", sub(GBP)),
    ("p", sub(GBP)),
    ("centavos", sub(PESO)),
    ("centavo", sub(PESO)),
    ("kopeks", sub(RUB)),
    ("kopek", sub(RUB)),
];
static LEX_FR: &[(&str, Word)] = &[
    ("dollars americains", unit(USD)),
    ("dollar americain", unit(USD)),
    ("dollars us", unit(USD)),
    ("dollar us", unit(USD)),
    ("dollars canadiens", unit(CAD)),
    ("dollar canadien", unit(CAD)),
    ("dollars australiens", unit(AUD)),
    ("dollar australien", unit(AUD)),
    ("dollars neo-zelandais", unit(NZD)),
    ("dollar neo-zelandais", unit(NZD)),
    ("francs suisses", unit(CHF)),
    ("franc suisse", unit(CHF)),
    ("francs cfa", unit(XOF)),
    ("franc cfa", unit(XOF)),
    ("livres sterling", unit(GBP)),
    ("livre sterling", unit(GBP)),
    ("pesos mexicains", unit(MXN)),
    ("peso mexicain", unit(MXN)),
    ("dollars", unit(USD)),
    ("dollar", unit(USD)),
    ("euros", unit(EUR)),
    ("euro", unit(EUR)),
    ("livres", unit(GBP)),
    ("livre", unit(GBP)),
    ("yens", unit(JPY)),
    ("yen", unit(JPY)),
    ("yuans", unit(CNY)),
    ("yuan", unit(CNY)),
    ("pesos", unit(PESO)),
    ("peso", unit(PESO)),
    ("reals", unit(BRL)),
    ("real", unit(BRL)),
    ("reais", unit(BRL)),
    ("roupies", unit(INR)),
    ("roupie", unit(INR)),
    ("wons", unit(KRW)),
    ("won", unit(KRW)),
    ("roubles", unit(RUB)),
    ("rouble", unit(RUB)),
    ("livres turques", unit(TRY)),
    ("livre turque", unit(TRY)),
    ("zlotys", unit(PLN)),
    ("zloty", unit(PLN)),
    ("couronnes suedoises", unit(SEK)),
    ("couronne suedoise", unit(SEK)),
    ("couronnes norvegiennes", unit(NOK)),
    ("couronne norvegienne", unit(NOK)),
    ("couronnes danoises", unit(DKK)),
    ("couronne danoise", unit(DKK)),
    ("dirhams", unit(MAD)),
    ("dirham", unit(MAD)),
    ("rands", unit(ZAR)),
    ("rand", unit(ZAR)),
    ("centimes", sub(EUR)),
    ("centime", sub(EUR)),
    ("cents", sub(EUR)),
    ("cent", sub(EUR)),
    ("pennies", sub(GBP)),
    ("penny", sub(GBP)),
    ("pence", sub(GBP)),
    ("centavos", sub(PESO)),
    ("centavo", sub(PESO)),
    ("kopecks", sub(RUB)),
    ("kopeck", sub(RUB)),
];
static LEX_ES: &[(&str, Word)] = &[
    ("dolares estadounidenses", unit(USD)),
    ("dolar estadounidense", unit(USD)),
    ("dolares americanos", unit(USD)),
    ("dolar americano", unit(USD)),
    ("dolares canadienses", unit(CAD)),
    ("dolar canadiense", unit(CAD)),
    ("dolares australianos", unit(AUD)),
    ("dolar australiano", unit(AUD)),
    ("francos suizos", unit(CHF)),
    ("franco suizo", unit(CHF)),
    ("libras esterlinas", unit(GBP)),
    ("libra esterlina", unit(GBP)),
    ("pesos mexicanos", unit(MXN)),
    ("peso mexicano", unit(MXN)),
    ("dolares", unit(USD)),
    ("dolar", unit(USD)),
    ("euros", unit(EUR)),
    ("euro", unit(EUR)),
    ("libras", unit(GBP)),
    ("libra", unit(GBP)),
    ("yenes", unit(JPY)),
    ("yen", unit(JPY)),
    ("yuanes", unit(CNY)),
    ("yuan", unit(CNY)),
    ("pesos", unit(PESO)),
    ("peso", unit(PESO)),
    ("reales", unit(BRL)),
    ("real", unit(BRL)),
    ("rupias", unit(INR)),
    ("rupia", unit(INR)),
    ("wones", unit(KRW)),
    ("won", unit(KRW)),
    ("rublos", unit(RUB)),
    ("rublo", unit(RUB)),
    ("liras", unit(TRY)),
    ("lira", unit(TRY)),
    ("zlotys", unit(PLN)),
    ("zloty", unit(PLN)),
    ("coronas", unit(SEK)),
    ("corona", unit(SEK)),
    ("dirhams", unit(MAD)),
    ("dirham", unit(MAD)),
    ("rands", unit(ZAR)),
    ("rand", unit(ZAR)),
    ("centavos", sub(USD)),
    ("centavo", sub(USD)),
    ("centimos", sub(EUR)),
    ("centimo", sub(EUR)),
    ("peniques", sub(GBP)),
    ("penique", sub(GBP)),
];
static LEX_DE: &[(&str, Word)] = &[
    ("us-dollar", unit(USD)),
    ("us dollar", unit(USD)),
    ("kanadische dollar", unit(CAD)),
    ("australische dollar", unit(AUD)),
    ("schweizer franken", unit(CHF)),
    ("britische pfund", unit(GBP)),
    ("pfund sterling", unit(GBP)),
    ("dollar", unit(USD)),
    ("dollars", unit(USD)),
    ("euro", unit(EUR)),
    ("euros", unit(EUR)),
    ("pfund", unit(GBP)),
    ("yen", unit(JPY)),
    ("yuan", unit(CNY)),
    ("franken", unit(CHF)),
    ("pesos", unit(PESO)),
    ("peso", unit(PESO)),
    ("real", unit(BRL)),
    ("reais", unit(BRL)),
    ("rupien", unit(INR)),
    ("rupie", unit(INR)),
    ("won", unit(KRW)),
    ("rubel", unit(RUB)),
    ("lira", unit(TRY)),
    ("zloty", unit(PLN)),
    ("kronen", unit(SEK)),
    ("krone", unit(SEK)),
    ("dirham", unit(MAD)),
    ("rand", unit(ZAR)),
    ("cent", sub(EUR)),
    ("cents", sub(EUR)),
    ("pence", sub(GBP)),
    ("penny", sub(GBP)),
    ("rappen", sub(CHF)),
    ("centavos", sub(PESO)),
    ("centavo", sub(PESO)),
    ("kopeken", sub(RUB)),
    ("kopeke", sub(RUB)),
];
static LEX_IT: &[(&str, Word)] = &[
    ("dollari americani", unit(USD)),
    ("dollaro americano", unit(USD)),
    ("dollari canadesi", unit(CAD)),
    ("dollaro canadese", unit(CAD)),
    ("dollari australiani", unit(AUD)),
    ("dollaro australiano", unit(AUD)),
    ("franchi svizzeri", unit(CHF)),
    ("franco svizzero", unit(CHF)),
    ("sterline", unit(GBP)),
    ("sterlina", unit(GBP)),
    ("dollari", unit(USD)),
    ("dollaro", unit(USD)),
    ("euro", unit(EUR)),
    ("euri", unit(EUR)),
    ("yen", unit(JPY)),
    ("yuan", unit(CNY)),
    ("franchi", unit(CHF)),
    ("franco", unit(CHF)),
    ("pesos", unit(PESO)),
    ("peso", unit(PESO)),
    ("real", unit(BRL)),
    ("reais", unit(BRL)),
    ("rupie", unit(INR)),
    ("rupia", unit(INR)),
    ("won", unit(KRW)),
    ("rubli", unit(RUB)),
    ("rublo", unit(RUB)),
    ("lire turche", unit(TRY)),
    ("lira turca", unit(TRY)),
    ("zloty", unit(PLN)),
    ("corone", unit(SEK)),
    ("corona", unit(SEK)),
    ("dirham", unit(MAD)),
    ("rand", unit(ZAR)),
    ("centesimi", sub(EUR)),
    ("centesimo", sub(EUR)),
    ("cent", sub(EUR)),
    ("cents", sub(EUR)),
    ("pence", sub(GBP)),
    ("penny", sub(GBP)),
    ("centavos", sub(PESO)),
    ("centavo", sub(PESO)),
];
static LEX_PT: &[(&str, Word)] = &[
    ("dolares americanos", unit(USD)),
    ("dolar americano", unit(USD)),
    ("dolares canadenses", unit(CAD)),
    ("dolar canadense", unit(CAD)),
    ("dolares australianos", unit(AUD)),
    ("dolar australiano", unit(AUD)),
    ("francos suicos", unit(CHF)),
    ("franco suico", unit(CHF)),
    ("libras esterlinas", unit(GBP)),
    ("libra esterlina", unit(GBP)),
    ("dolares", unit(USD)),
    ("dolar", unit(USD)),
    ("euros", unit(EUR)),
    ("euro", unit(EUR)),
    ("libras", unit(GBP)),
    ("libra", unit(GBP)),
    ("ienes", unit(JPY)),
    ("iene", unit(JPY)),
    ("yuans", unit(CNY)),
    ("yuan", unit(CNY)),
    ("pesos", unit(PESO)),
    ("peso", unit(PESO)),
    ("reais", unit(BRL)),
    ("real", unit(BRL)),
    ("rupias", unit(INR)),
    ("rupia", unit(INR)),
    ("wons", unit(KRW)),
    ("won", unit(KRW)),
    ("rublos", unit(RUB)),
    ("rublo", unit(RUB)),
    ("liras", unit(TRY)),
    ("lira", unit(TRY)),
    ("zlotys", unit(PLN)),
    ("zloty", unit(PLN)),
    ("coroas", unit(SEK)),
    ("coroa", unit(SEK)),
    ("dirhams", unit(MAD)),
    ("dirham", unit(MAD)),
    ("rands", unit(ZAR)),
    ("rand", unit(ZAR)),
    ("centavos", sub(BRL)),
    ("centavo", sub(BRL)),
    ("centimos", sub(EUR)),
    ("centimo", sub(EUR)),
    ("pence", sub(GBP)),
    ("penny", sub(GBP)),
];
static LEX_NL: &[(&str, Word)] = &[
    ("amerikaanse dollar", unit(USD)),
    ("amerikaanse dollars", unit(USD)),
    ("canadese dollar", unit(CAD)),
    ("canadese dollars", unit(CAD)),
    ("australische dollar", unit(AUD)),
    ("australische dollars", unit(AUD)),
    ("zwitserse frank", unit(CHF)),
    ("zwitserse franken", unit(CHF)),
    ("britse pond", unit(GBP)),
    ("pond sterling", unit(GBP)),
    ("dollar", unit(USD)),
    ("dollars", unit(USD)),
    ("euro", unit(EUR)),
    ("euros", unit(EUR)),
    ("euro's", unit(EUR)),
    ("pond", unit(GBP)),
    ("ponden", unit(GBP)),
    ("yen", unit(JPY)),
    ("yuan", unit(CNY)),
    ("frank", unit(CHF)),
    ("franken", unit(CHF)),
    ("pesos", unit(PESO)),
    ("peso", unit(PESO)),
    ("real", unit(BRL)),
    ("reais", unit(BRL)),
    ("roepies", unit(INR)),
    ("roepie", unit(INR)),
    ("won", unit(KRW)),
    ("roebels", unit(RUB)),
    ("roebel", unit(RUB)),
    ("lira", unit(TRY)),
    ("zloty", unit(PLN)),
    ("kronen", unit(SEK)),
    ("kroon", unit(SEK)),
    ("dirham", unit(MAD)),
    ("rand", unit(ZAR)),
    ("cent", sub(EUR)),
    ("centen", sub(EUR)),
    ("cents", sub(EUR)),
    ("pence", sub(GBP)),
    ("penny", sub(GBP)),
    ("centavos", sub(PESO)),
    ("centavo", sub(PESO)),
];
static LEX_CA: &[(&str, Word)] = &[
    ("dolars", unit(USD)),
    ("dolar", unit(USD)),
    ("euros", unit(EUR)),
    ("euro", unit(EUR)),
    ("lliures", unit(GBP)),
    ("lliura", unit(GBP)),
    ("centims", sub(EUR)),
    ("centim", sub(EUR)),
    ("centaus", sub(USD)),
    ("centau", sub(USD)),
];

/// Multi-word currency names come first (longest match): "francs suisses",
/// "canadian dollars", "livres sterling".
fn lexicon(lang: &str) -> &'static [(&'static str, Word)] {
    match lang {
        "en" => LEX_EN,
        "fr" => LEX_FR,
        "es" => LEX_ES,
        "de" => LEX_DE,
        "it" => LEX_IT,
        "pt" => LEX_PT,
        "nl" => LEX_NL,
        "ca" => LEX_CA,
        _ => &[],
    }
}

/// Connectors between the main amount and the subunit: "and", "et", "con", "y",
/// "und", "e", "en".
fn connectors(lang: &str) -> &'static [&'static str] {
    match lang {
        "en" => &["and"],
        "fr" => &["et"],
        "es" | "gl" => &["con", "y"],
        "ca" => &["amb", "i"],
        "de" => &["und"],
        "it" | "pt" => &["e"],
        "nl" => &["en"],
        _ => &[],
    }
}

/// One part of the text: a run of non-blank characters (`word`) or of blanks.
#[derive(Debug)]
struct Part {
    text: String,
    blank: bool,
}

fn split_parts(text: &str) -> Vec<Part> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut blank: Option<bool> = None;
    for c in text.chars() {
        let b = c.is_whitespace();
        if blank.is_some_and(|x| x != b) {
            parts.push(Part {
                text: std::mem::take(&mut cur),
                blank: blank.unwrap(),
            });
        }
        blank = Some(b);
        cur.push(c);
    }
    if let Some(b) = blank {
        parts.push(Part {
            text: cur,
            blank: b,
        });
    }
    parts
}

/// A number the walker wrote: integer digits and an optional fraction, as
/// written (`1355`, `3.5`, `2,50`); the trailing punctuation kept apart.
#[derive(Debug, Clone)]
struct Num {
    int: String,
    frac: String,
    trailing: String,
}

fn parse_num(word: &str) -> Option<Num> {
    let trimmed = word.trim_end_matches(|c: char| ",.;:!?)]\"'»".contains(c));
    let trailing = word[trimmed.len()..].to_string();
    if trimmed.is_empty() || !trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    let (int, frac) = match trimmed.find([',', '.']) {
        Some(p) => (&trimmed[..p], &trimmed[p + 1..]),
        None => (trimmed, ""),
    };
    if int.is_empty()
        || !int.chars().all(|c| c.is_ascii_digit())
        || !frac.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some(Num {
        int: int.to_string(),
        frac: frac.to_string(),
        trailing,
    })
}

/// Lower-cased, diacritics stripped, trailing punctuation kept apart.
fn word_key(word: &str) -> (String, String) {
    let trimmed = word.trim_end_matches(|c: char| ",.;:!?)]\"'»".contains(c));
    let trailing = word[trimmed.len()..].to_string();
    (crate::normalize(trimmed).trim().to_string(), trailing)
}

fn group_thousands(int: &str, sep: &str) -> String {
    let digits: Vec<char> = int.chars().collect();
    let mut out = String::new();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push_str(sep);
        }
        out.push(*c);
    }
    out
}

/// `12` + `5` -> `$12.05`; `1355` + `28` -> `$1,355.28`; no fraction -> `$12`.
fn render(lang: &str, cur: Currency, int: &str, frac: Option<&str>) -> String {
    let (sym, st) = style(lang, cur);
    let int = int.trim_start_matches('0');
    let int = if int.is_empty() { "0" } else { int };
    let mut number = group_thousands(int, st.thousands);
    if cur.decimals > 0 {
        if let Some(f) = frac {
            let mut f = f.to_string();
            while f.len() < cur.decimals as usize {
                f.push('0');
            }
            f.truncate(cur.decimals as usize);
            number.push(st.decimal);
            number.push_str(&f);
        }
    }
    let sp = if st.space { " " } else { "" };
    if st.prefix {
        format!("{}{}{}", sym, sp, number)
    } else {
        format!("{}{}{}", number, sp, sym)
    }
}

/// Match the longest currency name starting at `parts[i]` (skipping blanks
/// between its words). Returns the word and the index of its last part.
fn match_currency(lang: &str, parts: &[Part], i: usize) -> Option<(Word, usize, String)> {
    let lex = lexicon(lang);
    if lex.is_empty() {
        return None;
    }
    // Gather up to three words with their part indices.
    let mut words: Vec<(String, String, usize)> = Vec::new(); // (key, trailing, idx)
    let mut j = i;
    while j < parts.len() && words.len() < 3 {
        if parts[j].blank {
            j += 1;
            continue;
        }
        let (key, trailing) = word_key(&parts[j].text);
        let stop = !trailing.is_empty();
        words.push((key, trailing, j));
        if stop {
            break;
        }
        j += 1;
    }
    for take in (1..=words.len()).rev() {
        // A currency name never spans trailing punctuation on an inner word.
        if words[..take - 1].iter().any(|(_, t, _)| !t.is_empty()) {
            continue;
        }
        let phrase = words[..take]
            .iter()
            .map(|(k, _, _)| k.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        if let Some((_, w)) = lex.iter().find(|(name, _)| *name == phrase) {
            let (_, trailing, idx) = &words[take - 1];
            return Some((*w, *idx, trailing.clone()));
        }
    }
    None
}

/// Next non-blank part index after `i`.
fn next_word(parts: &[Part], i: usize) -> Option<usize> {
    (i + 1..parts.len()).find(|&j| !parts[j].blank)
}

/// Fold every amount in `text` (the walker's output) for `lang`. Words that
/// block a fold ("des euros", "quelques dollars") are simply not numbers.
pub fn fold_currency(text: &str, lang: &str) -> String {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    if lexicon(base).is_empty() {
        return text.to_string();
    }
    let parts = split_parts(text);
    let conns = connectors(base);
    let mut out = String::new();
    let mut i = 0usize;
    while i < parts.len() {
        let p = &parts[i];
        if p.blank {
            out.push_str(&p.text);
            i += 1;
            continue;
        }
        let Some(num) = parse_num(&p.text) else {
            out.push_str(&p.text);
            i += 1;
            continue;
        };
        // number (no trailing punctuation) followed by a currency word?
        if !num.trailing.is_empty() {
            out.push_str(&p.text);
            i += 1;
            continue;
        }
        let Some(j) = next_word(&parts, i) else {
            out.push_str(&p.text);
            i += 1;
            continue;
        };
        let Some((word, end, trailing)) = match_currency(base, &parts, j) else {
            out.push_str(&p.text);
            i += 1;
            continue;
        };
        if word.sub {
            // "ninety nine cents" -> $0.99 (a bare subunit; a decimal number
            // in front makes no sense and is left alone).
            if !num.frac.is_empty() {
                out.push_str(&p.text);
                i += 1;
                continue;
            }
            let cur = default_currency_for_sub(base, word.cur);
            out.push_str(&render(base, cur, "0", Some(&pad2(&num.int))));
            out.push_str(&trailing);
            i = end + 1;
            continue;
        }
        let cur = word.cur;
        let mut consumed_end = end;
        let mut frac: Option<String> = if num.frac.is_empty() {
            None
        } else {
            Some(num.frac.clone())
        };
        let mut trailing = trailing;
        // Subunit after the currency: "[and] twenty eight cents" | "fifty".
        if trailing.is_empty() && frac.is_none() && cur.decimals > 0 {
            if let Some(k) = next_word(&parts, end) {
                let (kkey, _) = word_key(&parts[k].text);
                let (mut m, mut had_conn) = (k, false);
                if conns.contains(&kkey.as_str()) {
                    had_conn = true;
                    match next_word(&parts, k) {
                        Some(k2) => m = k2,
                        None => m = usize::MAX,
                    }
                }
                if m != usize::MAX {
                    if let Some(sub_num) = parse_num(&parts[m].text) {
                        let small = sub_num.frac.is_empty() && sub_num.int.len() <= 2;
                        if small {
                            // Explicit subunit word after it?
                            let after = next_word(&parts, m);
                            let sub_word = after
                                .and_then(|a| match_currency(base, &parts, a))
                                .filter(|(w, _, _)| w.sub);
                            if let Some((_, sub_end, sub_trailing)) = sub_word {
                                if sub_num.trailing.is_empty() {
                                    frac = Some(pad2(&sub_num.int));
                                    consumed_end = sub_end;
                                    trailing = sub_trailing;
                                }
                            } else if !had_conn {
                                // "two euros fifty": bare subunit, only when nothing
                                // number-like follows (no "fifty three people" risk
                                // beyond what speech allows).
                                let follows_number =
                                    after.is_some_and(|a| parse_num(&parts[a].text).is_some());
                                if !follows_number {
                                    frac = Some(pad2(&sub_num.int));
                                    consumed_end = m;
                                    trailing = sub_num.trailing.clone();
                                }
                            }
                        }
                    }
                }
            }
        }
        out.push_str(&render(base, cur, &num.int, frac.as_deref()));
        out.push_str(&trailing);
        i = consumed_end + 1;
    }
    out
}

/// A bare subunit names the language's default currency when the word is shared
/// ("cents": USD in en, EUR in fr/de/it/nl; "centavos": USD in es, BRL in pt);
/// a subunit that belongs to one currency (pence, kopeks, rappen) names it.
fn default_currency_for_sub(lang: &str, named: Currency) -> Currency {
    match named.code {
        "USD" | "EUR" | "PESO" | "BRL" => default_currency(lang),
        _ => named,
    }
}

fn pad2(int: &str) -> String {
    let t = int.trim_start_matches('0');
    let t = if t.is_empty() { "0" } else { t };
    if t.len() == 1 {
        format!("0{}", t)
    } else {
        t.to_string()
    }
}
