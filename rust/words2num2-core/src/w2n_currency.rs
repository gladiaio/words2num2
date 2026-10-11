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

/// Every currency the fold can write, most common first: when two currencies
/// share a word in the renderer's forms ("dollar" is USD, CAD, AUD and NZD),
/// the first one names it.
const ALL: [Currency; 22] = [
    USD, EUR, GBP, JPY, CNY, CHF, CAD, AUD, NZD, MXN, BRL, INR, KRW, RUB, TRY, PLN, SEK, NOK,
    DKK, XOF, MAD, ZAR,
];

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
            // A symbol written after the number is spaced: "20 kr", "2 zł" (#182).
            "SEK" | "NOK" | "DKK" | "PLN" => (
                sym.to_string(),
                Style {
                    prefix: false,
                    space: true,
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
    lexicon(lang).iter().any(|(name, w)| w.sub && name == norm)
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

/// Every other currency code a num2words language may write (ISO 4217 and the
/// legacy codes the renderer still carries). `to="currency"` reads their names
/// too — vi "đồng", id "rupiah", ru "рублей" — though the fold, which needs a
/// symbol, only writes the currencies in [`ALL`].
const OTHER_CODES: &[&str] = &[
    "EUR", "USD", "INR", "GBP", "JPY", "KWD", "CNY", "CHF", "RUB", "BHD", "CAD", "AUD",
    "KRW", "NOK", "ISK", "RON", "MXN", "UZS", "BYN", "SAR", "HUF", "EEK", "MYR", "ZAR",
    "NZD", "NGN", "HKD", "SGD", "NIO", "PLN", "IDR", "IQD", "SEK", "OMR", "UAH", "TND",
    "PKR", "NPR", "LYD", "LTL", "JOD", "DKK", "BRL", "MMK", "LAK", "XOF", "PHP", "KZT",
    "ETB", "AED", "TRY", "CRC", "RSD", "QAR", "GEL", "LKR", "ILS", "BAM", "THB", "LVL",
    "GTQ", "BGN", "UGX", "DZD", "CZK", "RWF", "MGA", "KPW", "KHR", "KGS", "HNL", "FRF",
    "MOP", "KES", "HTG", "ERN", "CDF", "AFN", "VND", "VES", "TMT", "MKD", "ITL", "IRR",
    "HRK", "BDT", "ANG", "AMD", "ALL", "TZS", "TWD", "MNT", "EGP", "COP", "BND", "AWG",
    "ZMW", "YER", "WST", "TJS", "SOS", "SLL", "SCR", "MWK", "MVR", "MUR", "MRU", "LBP",
    "GHS", "BWP", "AZN", "AOA", "ZWL", "XCD", "UYU", "TTD", "TOP", "SZL", "SYP", "SSP",
    "SRD", "SHP", "SDG", "SBD", "PYG", "PGK", "PEN", "PAB", "NAD", "MZN", "MDL", "MAD",
    "LSL", "LRD", "KYD", "KMF", "JMD", "IRT", "GYD", "GNF", "GMD", "GIP", "FKP", "FJD",
    "DOP", "DJF", "CVE", "CUP", "CLP", "BZD", "BTN", "BSD", "BOB", "BMD", "BIF", "BBD",
    "ARS", "XPF", "XAF", "VUV", "VEF", "SVC", "STD", "STN", "NTD", "MRO", "ESP", "ECS",
    "CYP", "BYR", "SKK", "FIM", "DEM", "CUC",
];

/// The hand-written names: colloquial words the renderer never writes
/// ("bucks", "quid", "balles"), the adjective forms ("francs suisses",
/// "canadian dollars"), and the shared subunits mapped to the language's own
/// currency. Multi-word names may sit anywhere: the match is longest-first.
fn hand_lexicon(lang: &str) -> &'static [(&'static str, Word)] {
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

/// Renderer forms that collide with an ordinary word of the language once
/// diacritics are stripped: en "öre" (the SEK subunit) is "ore". Kept out of
/// the fold, which reads running text; `to="currency"` reads an amount and
/// keeps them.
fn denied(lang: &str, name: &str) -> bool {
    matches!((lang, name), ("en", "ore"))
}

/// The currency lexicon of `lang` (a renderer key: "en", "fr", "en_IN"): the
/// hand-written names of its base language first, then every unit and
/// subunit name the renderer's `to_currency` writes for the currencies in
/// [`ALL`] — so "paise", "kopeykas", "copechi", "Fen", "rappen", es
/// "francos", ca "iens"/"rupies" are read because num2words writes them (#104).
///
/// A derived name is skipped when it is a number word of the language (fr
/// "cent" is 100), when it is [`denied`], and, for a subunit, when the
/// currency has no two-digit minor unit here (JPY "sen", KRW "jeon") or is
/// CNY's "jiao", a tenth rather than a hundredth that num2words renders as
/// the subunit. The first name wins: hand entries, then [`ALL`] order.
///
/// Built once per key and cached: the fold and the walker ask for it on
/// every word.
fn lexicon(lang: &str) -> &'static [(String, Word)] {
    cached_lexicon(lang, false)
}

/// [`lexicon`] plus the names of every code in [`OTHER_CODES`]: what
/// `to="currency"` reads.
fn amount_lexicon(lang: &str) -> &'static [(String, Word)] {
    cached_lexicon(lang, true)
}

fn cached_lexicon(lang: &str, other_codes: bool) -> &'static [(String, Word)] {
    type Lex = &'static [(String, Word)];
    type Cache = std::sync::RwLock<std::collections::HashMap<(String, bool), Lex>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (lang.to_string(), other_codes);
    if let Some(l) = cache.read().ok().and_then(|m| m.get(&key).copied()) {
        return l;
    }
    let built: Lex = Box::leak(build_lexicon(lang, other_codes).into_boxed_slice());
    if let Ok(mut m) = cache.write() {
        m.entry(key).or_insert(built);
    }
    built
}

fn build_lexicon(lang: &str, other_codes: bool) -> Vec<(String, Word)> {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    let mut out: Vec<(String, Word)> = hand_lexicon(base)
        .iter()
        .map(|(n, w)| (n.to_string(), *w))
        .collect();
    let key = if num2words2_core::get_lang_by_key(lang).is_some() { lang } else { base };
    let Some(r) = num2words2_core::get_lang_by_key(key) else {
        return out;
    };
    let is_number = |name: &str| {
        matches!(crate::lookup_norm(key, name, false, &[]), Ok(Some(_)))
            || crate::parse_scaled_norm(key, name).is_some()
    };
    let others = OTHER_CODES
        .iter()
        .filter(|_| other_codes)
        .filter(|code| !ALL.iter().any(|c| c.code == **code))
        .map(|&code| Currency {
            code,
            symbol: "",
            decimals: match r.currency_precision(code) {
                1 => 0,
                1000 => 3,
                _ => 2,
            },
        });
    for cur in ALL.into_iter().chain(others) {
        let Some(forms) = r.currency_forms(cur.code) else {
            continue;
        };
        let precision = 10i64.pow(u32::from(cur.decimals));
        let subunits_ok = cur.decimals > 0 && r.currency_precision(cur.code) == precision;
        let unit_names = forms.unit.iter().map(|f| (f, false));
        let sub_names = forms.subunit.iter().filter(|_| subunits_ok).map(|f| (f, true));
        for (form, sub) in unit_names.chain(sub_names) {
            let name = crate::normalize(form);
            if name.is_empty()
                || (sub && cur.code == "CNY" && name.starts_with("jiao"))
                || (!other_codes && denied(base, &name))
                || out.iter().any(|(n, _)| *n == name)
                || is_number(&name)
            {
                continue;
            }
            out.push((name, Word { cur, sub }));
        }
    }
    out
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
/// written (`1355`, `3.5`, `2,50`); the trailing punctuation kept apart, and
/// the invisible format characters after it ("20\u{200b}", #1021).
#[derive(Debug, Clone)]
struct Num {
    int: String,
    frac: String,
    trailing: String,
    invisible: String,
}

fn parse_num(word: &str) -> Option<Num> {
    let core = word.trim_end_matches(crate::w2n_sentence::is_format_char);
    let invisible = word[core.len()..].to_string();
    let trimmed = core.trim_end_matches(|c: char| ",.;:!?)]\"'»".contains(c));
    let trailing = core[trimmed.len()..].to_string();
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
        invisible,
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
/// The fraction is never truncated: the caller leaves an amount whose spoken
/// fraction is longer than the currency's minor unit unfolded (#46).
fn render(lang: &str, cur: Currency, int: &str, frac: Option<&str>) -> String {
    let (sym, st) = style(lang, cur);
    let int = int.trim_start_matches('0');
    let int = if int.is_empty() { "0" } else { int };
    let mut number = group_thousands(int, st.thousands);
    if let Some(f) = frac.filter(|_| cur.decimals > 0) {
        let mut f = f.to_string();
        while f.len() < cur.decimals as usize {
            f.push('0');
        }
        number.push(st.decimal);
        number.push_str(&f);
    }
    let sp = if st.space { " " } else { "" };
    if st.prefix {
        format!("{}{}{}", sym, sp, number)
    } else {
        format!("{}{}{}", number, sp, sym)
    }
}

/// A currency name matched in the text: what it names, the index of its last
/// part, the punctuation after it, and the name itself ("pounds").
struct Match {
    word: Word,
    end: usize,
    trailing: String,
    name: String,
}

/// Match the longest currency name starting at `parts[i]` (skipping blanks
/// between its words).
fn match_currency(lang: &str, parts: &[Part], i: usize) -> Option<Match> {
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
        if let Some((name, w)) = lex.iter().find(|(name, _)| *name == phrase) {
            let (_, trailing, idx) = &words[take - 1];
            return Some(Match {
                word: *w,
                end: *idx,
                trailing: trailing.clone(),
                name: name.clone(),
            });
        }
    }
    None
}

/// Next non-blank part index after `i`.
fn next_word(parts: &[Part], i: usize) -> Option<usize> {
    (i + 1..parts.len()).find(|&j| !parts[j].blank)
}

/// Previous non-blank part index before `i`.
fn prev_word(parts: &[Part], i: usize) -> Option<usize> {
    (0..i).rev().find(|&j| !parts[j].blank)
}

/// en "pounds" is also a weight: "twenty pounds of flour" stays words; "quid",
/// "sterling" and "pounds sterling" are money wherever they stand (#81).
fn is_weight(lang: &str, m: &Match, parts: &[Part]) -> bool {
    lang == "en"
        && matches!(m.name.as_str(), "pound" | "pounds")
        && m.trailing.is_empty()
        && next_word(parts, m.end).is_some_and(|k| word_key(&parts[k].text).0 == "of")
}

/// `n` (digits) as a fraction of the currency's minor unit, if it is one:
/// below `10^decimals`, two digits (`5` -> `05`).
fn minor_digits(n: &str, cur: Currency) -> Option<String> {
    let t = n.trim_start_matches('0');
    let d = cur.decimals as usize;
    if d == 0 || t.len() > d {
        return None;
    }
    Some(format!("{:0>width$}", t, width = d))
}

/// Is part `k` the end of the text, or followed only by terminal punctuation?
fn ends_amount(parts: &[Part], k: usize, num: &Num) -> bool {
    next_word(parts, k).is_none() || matches!(num.trailing.as_str(), "." | "!" | "?")
}

/// Fold every amount in `text` (the walker's output) for `lang`. Words that
/// block a fold ("des euros", "quelques dollars") are simply not numbers.
pub fn fold_currency(text: &str, lang: &str) -> String {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    if hand_lexicon(base).is_empty() {
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
        let m = next_word(&parts, i)
            .and_then(|j| match_currency(base, &parts, j))
            .filter(|m| !is_weight(base, m, &parts));
        let Some(m) = m else {
            out.push_str(&p.text);
            i += 1;
            continue;
        };
        if m.word.sub {
            // "ninety nine cents" -> $0.99, "one hundred fifty cents" -> $1.50
            // (#45). Left alone: a decimal number in front, and a subunit
            // after a connector — "doce francos con 34 céntimos", "twelve
            // dollars and 100 cents": it belongs to the amount before the
            // connector, which was not (or could not be) folded, and the
            // language's default currency would be a guess (#48).
            let cur = default_currency_for_sub(base, m.word.cur);
            let after_conn = prev_word(&parts, i)
                .is_some_and(|k| conns.contains(&word_key(&parts[k].text).0.as_str()));
            if !num.frac.is_empty() || after_conn || cur.decimals == 0 {
                out.push_str(&p.text);
                i += 1;
                continue;
            }
            let digits = num.int.trim_start_matches('0');
            let d = cur.decimals as usize;
            let (int, frac) = if digits.len() > d {
                digits.split_at(digits.len() - d)
            } else {
                ("0", digits)
            };
            let frac = format!("{:0>width$}", frac, width = d);
            out.push_str(&render(base, cur, int, Some(&frac)));
            out.push_str(&num.invisible);
            out.push_str(&m.trailing);
            i = m.end + 1;
            continue;
        }
        let cur = m.word.cur;
        // A spoken fraction longer than the minor unit ("12.345 euros", "3.5
        // yen") is never truncated: the amount stays as it was said (#46).
        if num.frac.len() > cur.decimals as usize {
            out.push_str(&p.text);
            i += 1;
            continue;
        }
        let mut consumed_end = m.end;
        let mut frac: Option<String> = if num.frac.is_empty() {
            None
        } else {
            Some(num.frac.clone())
        };
        let mut trailing = m.trailing.clone();
        let mut sub_invisible = String::new();
        // Subunit after the currency: "[and] twenty eight cents" | "fifty". A comma
        // after the unit is a pause, not the end of the amount, when a spoken
        // subunit follows: "zéro dollar, et cinquante-cinq centimes" (the
        // punctuation of a transcript), "twelve euros, thirty-four cents" (how
        // num2words writes English amounts, #47).
        let comma = trailing == ",";
        if (trailing.is_empty() || comma) && frac.is_none() && cur.decimals > 0 {
            if let Some(k) = next_word(&parts, m.end) {
                let (kkey, _) = word_key(&parts[k].text);
                let (mut sub_at, mut had_conn) = (Some(k), false);
                if conns.contains(&kkey.as_str()) {
                    had_conn = true;
                    sub_at = next_word(&parts, k);
                }
                let sub_num = sub_at.and_then(|s| parse_num(&parts[s].text).map(|n| (s, n)));
                if let Some((s, sub_num)) = sub_num.filter(|(_, n)| n.frac.is_empty()) {
                    let minor = minor_digits(&sub_num.int, cur);
                    let after = next_word(&parts, s);
                    // Explicit subunit word after it?
                    let sub_word = after
                        .and_then(|a| match_currency(base, &parts, a))
                        .filter(|w| w.word.sub);
                    if let Some(sw) = sub_word {
                        if let Some(minor) = minor.filter(|_| sub_num.trailing.is_empty()) {
                            frac = Some(minor);
                            consumed_end = sw.end;
                            trailing = sw.trailing;
                            sub_invisible = sub_num.invisible.clone();
                        }
                    } else if let Some(minor) = minor.filter(|_| !comma) {
                        // "two euros fifty": bare subunit, only when nothing
                        // number-like follows. After a connector ("two dollars
                        // and fifty", "cinco euros con cincuenta") only when it
                        // ends the amount: "two dollars and fifty people" is
                        // not $2.50 (#181).
                        let follows_number =
                            after.is_some_and(|a| parse_num(&parts[a].text).is_some());
                        let bare_ok = if had_conn {
                            ends_amount(&parts, s, &sub_num)
                        } else {
                            !follows_number
                        };
                        if bare_ok {
                            frac = Some(minor);
                            consumed_end = s;
                            trailing = sub_num.trailing.clone();
                            sub_invisible = sub_num.invisible.clone();
                        }
                    }
                }
            }
        }
        out.push_str(&render(base, cur, &num.int, frac.as_deref()));
        // Format characters ride after the amount they followed (#1021).
        out.push_str(&num.invisible);
        out.push_str(&sub_invisible);
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

// ---------------------------------------------------------------------------
// Before the walk
// ---------------------------------------------------------------------------

/// Feminine numerals the reverse tables do not hold, in front of a feminine
/// currency: pt "duas libras", "uma libra", "duzentas rupias", es
/// "doscientas libras". num2words renders the masculine ("dois libras",
/// #266), but speech agrees the number with the noun.
fn masculine_numeral(lang: &str, word: &str) -> Option<String> {
    match (lang, word) {
        ("pt", "duas") => Some("dois".into()),
        ("pt", "uma") => Some("um".into()),
        ("pt" | "es" | "gl", w) if w.ends_with("entas") && w.len() > 6 => {
            Some(format!("{}entos", &w[..w.len() - 5]))
        }
        _ => None,
    }
}

/// The words the sentence walker treats as article-or-one (en "one", fr
/// "un"/"une", de "ein"...); "a"/"an" stay articles in running text.
fn walker_article(lang: &str, word: &str) -> bool {
    word != "a" && word != "an" && article_one(lang, word)
}

/// Currency names whose everyday reading dominates after an article: fr
/// "un livre" (a book), "une livre" (a pound weight), "un franc succès"; es
/// "una corona", "un real desafío", "una libra de carne"; en "one real
/// problem", "no one won"; de "eine Krone", nl "een kroon", "een pond". The
/// article stays a word before them; a figure before them still folds.
fn article_homograph(lang: &str, name: &str) -> bool {
    matches!(
        (lang, name),
        ("en", "real" | "won" | "rand" | "p")
            | ("fr", "livre" | "livres" | "franc" | "real" | "couronne")
            | ("es" | "gl" | "ca" | "it", "corona" | "real" | "libra" | "lliura" | "franco")
            | ("pt", "coroa" | "real" | "libra")
            | ("de", "krone" | "pfund" | "real")
            | ("nl", "kroon" | "pond" | "real")
    )
}

/// After the unit name at word `v`: an optional connector, one to four number
/// words, then a subunit name ("dollar | and twenty cents", "livre | vingt
/// pence"). What makes "a dollar" or "une livre" an amount, not an article
/// before a word.
fn subunit_follows(lang: &str, conns: &[&str], keys: &[String], named: &[Option<Match>], v: usize) -> bool {
    let mut x = v + 1;
    if keys.get(x).is_some_and(|k| conns.contains(&k.as_str())) {
        x += 1;
    }
    let first = x;
    while x < keys.len() && x - first < 4 && crate::token_may_be_number(lang, &keys[x]) {
        x += 1;
    }
    x > first && named.get(x).is_some_and(|m| m.as_ref().is_some_and(|m| m.word.sub))
}

/// The article-one of a language, which counts 1 in front of a currency name.
fn article_one(lang: &str, word: &str) -> bool {
    matches!(
        (lang, word),
        ("en", "one" | "a" | "an")
            | ("fr", "un" | "une")
            | ("es" | "gl" | "ca" | "it", "un" | "una" | "uno")
            | ("pt", "um" | "uma")
            | ("de", "ein" | "eine" | "einen" | "einem" | "einer")
            | ("nl" | "af", "een")
    )
}

/// Prepare a sentence for the walk of a `currency=True` call: the pieces of
/// the sentence in order, each with whether to walk it (`false`: keep it
/// verbatim).
///
/// * fr "douze dollars et trente-quatre **cents**": num2words writes the US
///   cent as "cents", which is also the plural of cent (100); walked whole,
///   "trente-quatre cents" is 3400. A subunit name that follows "<unit>
///   <connector> <number words>" is a piece of its own, kept as a word, so the
///   number before it stays 34 and the fold reads "12 $ et 34 cents" (#49).
/// * pt/es feminine numerals in front of a currency name are given their
///   masculine form, which the tables hold ("duas libras" -> "dois libras").
/// * the article-one in front of a currency name is the amount 1: "one yen",
///   "un rouble", "um iene" (#103) — the walker only counts it before its own
///   short list of units. Not before a name whose ordinary reading dominates
///   ([`article_homograph`]): "un livre de poche", "one real problem". Nor
///   en "a"/"an", an article in running text — unless a subunit after the
///   unit says it is money: "a dollar and twenty cents", "une livre et vingt
///   pence" ([`subunit_follows`]).
pub fn pieces_for_walk(sentence: &str, lang: &str) -> Vec<(String, bool)> {
    pieces_for_walk_with(sentence, lang, false)
}

/// [`pieces_for_walk`] with only the subunit cut, no currency rewrites: the
/// plain walk keeps fr "dollars et cinquante cents" as 50 cents, not 5000
/// (#868).
pub fn subunit_pieces(sentence: &str, lang: &str) -> Vec<(String, bool)> {
    // Only a subunit the walk reads as a multiplier needs the cut: fr "cent",
    // "cents" (100). Elsewhere (en, or no such word in the text) the scan is
    // skipped (#1001).
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    let mults = multiplier_subunits(base);
    if mults.is_empty() || !crate::normalize(sentence).split_whitespace().any(|w| mults.iter().any(|m| m == w)) {
        return vec![(sentence.to_string(), true)];
    }
    pieces_for_walk_with(sentence, lang, true)
}

/// The subunit names of `base` that are also a multiplier word (>= 100),
/// built once per language.
fn multiplier_subunits(base: &str) -> std::sync::Arc<Vec<String>> {
    static CACHE: std::sync::OnceLock<std::sync::RwLock<std::collections::HashMap<String, std::sync::Arc<Vec<String>>>>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.read().ok().and_then(|m| m.get(base).cloned()) {
        return v;
    }
    let v: std::sync::Arc<Vec<String>> = std::sync::Arc::new(
        hand_lexicon(base)
            .iter()
            .filter(|(_, w)| w.sub)
            .map(|(n, _)| crate::normalize(n))
            .filter(|n| matches!(crate::lookup_norm(base, n, false, &[]), Ok(Some(v)) if v >= 100))
            .collect(),
    );
    if let Ok(mut m) = cache.write() {
        m.insert(base.to_string(), v.clone());
    }
    v
}

fn pieces_for_walk_with(sentence: &str, lang: &str, subunits_only: bool) -> Vec<(String, bool)> {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    if hand_lexicon(base).is_empty() {
        return vec![(sentence.to_string(), true)];
    }
    let mut parts = split_parts(sentence);
    let conns = connectors(base);
    let words: Vec<usize> = (0..parts.len()).filter(|&j| !parts[j].blank).collect();
    let keys: Vec<String> = words.iter().map(|&j| word_key(&parts[j].text).0).collect();
    let named: Vec<Option<Match>> = words.iter().map(|&j| match_currency(base, &parts, j)).collect();
    let is_unit = |w: usize| named[w].as_ref().is_some_and(|m| !m.word.sub);
    // Does a unit name end exactly on word `w`?
    let unit_ends_at = |w: usize| {
        (0..=w.min(2)).any(|len| {
            named[w - len]
                .as_ref()
                .is_some_and(|m| !m.word.sub && m.end == words[w])
        })
    };
    let mut kept: Vec<usize> = Vec::new(); // parts kept verbatim
    let mut renames: Vec<(usize, String)> = Vec::new();
    for w in 0..words.len() {
        // A subunit name after "<unit> <connector> <one to four words>".
        if named[w].as_ref().is_some_and(|m| m.word.sub) {
            // The connector right after the unit name, not one inside the
            // count: fr_BE "dollars et vingt et un cents" (#1010).
            let conn = (1..=4usize)
                .take_while(|&back| back + 2 <= w)
                .find(|&back| conns.contains(&keys[w - back - 1].as_str()) && unit_ends_at(w - back - 2));
            // Only a subunit name the walker would read as a number (fr
            // "cents") needs this. "<2-9> cents" is also a real hundreds
            // multiple ("mille euros et deux cents personnes" is 200 people),
            // so there it must end the clause; "trente-quatre cents" (34 x
            // 100) is not French for a number, so it never is (#49).
            let numeric_sub = if subunits_only {
                // fr "centime", ca "cèntim" are never multiplied (#1001).
                matches!(crate::lookup_norm(base, &keys[w], false, &[]), Ok(Some(v)) if v >= 100)
            } else {
                crate::token_may_be_number(base, &keys[w])
            };
            let ends_clause = w + 1 == words.len()
                || matches!(word_key(&parts[words[w]].text).1.as_str(), "." | "!" | "?" | ",");
            // The words between the connector and the subunit are a plain
            // number, unpunctuated, naming no currency, an amount of 1-99
            // cents (#1002, #1003): not "vingt et un. trois cents", "cinq et
            // deux cents", "soixante cents et deux cents" (#1017).
            let ok = conn.is_some_and(|back| {
                let window = &words[w - back..w];
                let unpunct = window.iter().all(|&j| word_key(&parts[j].text).1.is_empty())
                    && word_key(&parts[words[w - back - 1]].text).1.is_empty();
                let unnamed = (w - back..w).all(|v| named[v].is_none());
                // A count in figures: "12 dollars et 50 cents". A figure is
                // never multiplied by "cents", so it needs no clause end.
                let figure = (back == 1 && !keys[w - 1].is_empty() && keys[w - 1].bytes().all(|b| b.is_ascii_digit()))
                    .then(|| keys[w - 1].parse::<i64>().ok())
                    .flatten();
                // The regional table: fr_CH "nonante-neuf", fr_BE "septante".
                let all_numbers = figure.is_some()
                    || keys[w - back..w].iter().all(|k| crate::token_may_be_number(lang, k) || conns.contains(&k.as_str()));
                // Zero too: "douze dollars et zéro cent" is the renderer's
                // spelling of 12.00 (#1019).
                let v = figure.unwrap_or_else(|| match crate::lookup_norm(lang, &keys[w - back..w].join(" "), false, &[]) {
                    Ok(Some(v)) => v,
                    _ => -1,
                });
                // "<2-9> cents" is also a hundreds multiple; the plain walk
                // also takes 11-19 for one ("douze cents personnes" is 1200
                // people), so there it must end the clause. "dix cents" is no
                // 1000 ("mille"), so 10 is no hundreds multiple.
                let hundreds = figure.is_none() && if subunits_only { (2..=19).contains(&v) && v != 10 } else { (1..10).contains(&v) };
                // currency=True keeps, as written, a count it cannot read as
                // one integer but that is one amount: a decimal ("deux virgule
                // cinq cents", "2,5 cents"), native digits, a long figure.
                // Never across a connector: "cinq et deux cents" (#1017, #1022).
                // The raw text too: a symbol ("Ⅻ") may fold in the key.
                let numberish = |v: usize| {
                    let k = &keys[v];
                    parts[words[v]].text.chars().any(char::is_numeric)
                        || crate::token_may_be_number(lang, k)
                        || matches!(k.as_str(), "virgule" | "point")
                };
                // Also a count of 100 or more ("cent cinquante cents", "mille
                // cents"), unless its last word is the 2-9 that multiplies
                // "cents" ("mille deux cents personnes" is 1200), and a symbol
                // the table does not read ("Ⅻ cents") (#1025).
                let last_multiplies = matches!(crate::lookup_norm(lang, &keys[w - 1], false, &[]), Ok(Some(2..=9)));
                // Never when the subunit word extends the count into one
                // number: "deux mille cent personnes" is 2100 (#1029).
                // Only the singular: "mille cents" is no 1100.
                let extends = keys[w] == "cent"
                    && crate::words2num(&format!("{} {}", keys[w - back..w].join(" "), keys[w]), lang, "cardinal").is_ok();
                let unread = !extends && (v == -1 || !all_numbers || (v >= 100 && !last_multiplies));
                let opaque = !subunits_only
                    && unread
                    && (w - back..w).all(numberish)
                    && !keys[w - back..w].iter().any(|k| conns.contains(&k.as_str()));
                // "cent" in the count is the number, not the subunit name.
                let unnamed_count = (w - back..w).all(|v| named[v].is_none() || crate::token_may_be_number(lang, &keys[v]));
                unit_ends_at(w - back - 2)
                    && unpunct
                    && ((opaque && unnamed_count)
                        || (unnamed && all_numbers && (figure.is_some() || (0..=99).contains(&v)) && (!hundreds || ends_clause)))
            });
            if numeric_sub && ok {
                kept.push(words[w]);
            }
        }
        // The article-one right before a currency unit name, opening the
        // amount ("vinte e uma libras" is 21, the "uma" is not an article).
        let starts_unit = |v: usize| {
            named[v].as_ref().is_some_and(|m| !m.word.sub) && !article_homograph(base, &keys[v])
        };
        let in_compound = w > 0 && {
            let p = keys[w - 1].as_str();
            conns.contains(&p) || crate::token_may_be_number(base, p) || masculine_numeral(base, p).is_some()
        };
        // "a dollar and twenty cents", "une livre et vingt pence": the subunit
        // makes it money, whatever the article or the unit's other reading.
        let confirmed = |v: usize| is_unit(v) && subunit_follows(base, conns, &keys, &named, v);
        let article_unit = w + 1 < words.len()
            && ((walker_article(base, &keys[w]) && starts_unit(w + 1))
                || (article_one(base, &keys[w]) && confirmed(w + 1)));
        if !in_compound && word_key(&parts[words[w]].text).1.is_empty() && article_unit {
            renames.push((words[w], "1".to_string()));
        }
        // A feminine numeral whose number run ends on a currency unit. A lone
        // article-one right before the unit ("uma libra") is the article
        // path's call above, not a numeral to rewrite.
        let lone_article =
            !in_compound && walker_article(base, &keys[w]) && w + 1 < words.len() && is_unit(w + 1);
        if let Some(masc) = masculine_numeral(base, &keys[w]).filter(|_| !lone_article) {
            let numberish = |v: usize| {
                masculine_numeral(base, &keys[v]).is_some()
                    || conns.contains(&keys[v].as_str())
                    || crate::token_may_be_number(base, &keys[v])
            };
            let end = (w + 1..words.len()).find(|&v| !numberish(v));
            if end.is_some_and(is_unit) {
                let trailing = word_key(&parts[words[w]].text).1;
                renames.push((words[w], format!("{}{}", masc, trailing)));
            }
        }
    }
    for (j, text) in renames.into_iter().filter(|_| !subunits_only) {
        parts[j].text = text;
    }
    let mut pieces = Vec::new();
    let mut cur = String::new();
    for (j, p) in parts.iter().enumerate() {
        if kept.contains(&j) {
            if !cur.is_empty() {
                pieces.push((std::mem::take(&mut cur), true));
            }
            pieces.push((p.text.clone(), false));
        } else {
            cur.push_str(&p.text);
        }
    }
    if !cur.is_empty() {
        pieces.push((cur, true));
    }
    pieces
}

// ---------------------------------------------------------------------------
// to="currency"
// ---------------------------------------------------------------------------

/// The longest lexicon name at `toks[i..]` (up to three words): what it names
/// and how many words it takes.
fn match_tokens(lex: &[(String, Word)], toks: &[&str], i: usize) -> Option<(Word, usize)> {
    (1..=3.min(toks.len() - i)).rev().find_map(|take| {
        let phrase = toks[i..i + take].join(" ");
        lex.iter().find(|(n, _)| *n == phrase).map(|(_, w)| (*w, take))
    })
}

/// `words2num(text, lang, to="currency")`: the amount of an amount spoken in
/// words — "twelve euros, thirty-four cents" -> 12.34, "douze euros et
/// trente-quatre centimes" -> 12.34, "zwei Euro" -> 2, "ninety nine cents" ->
/// 0.99 — read as `<number> <unit> [<connector>] [<number> [<subunit>]]` or
/// `<number> <subunit>` (#189).
///
/// `lang` is the resolved language key; its lexicon is the fold's (hand names
/// plus everything num2words writes for the currencies in [`ALL`]), and its
/// connectors are the fold's plus the renderer's own separator. `cardinal`
/// reads a run of number words (the language's `to_cardinal`).
///
/// Returns `None` when the text names no currency at all — the caller then
/// reads it as a plain cardinal — and `Some(Err)` when it names one but is not
/// an amount ("twelve euros and change", a subunit of 100 or more, a decimal
/// with a subunit).
pub fn parse_amount(
    text: &str,
    lang: &str,
    cardinal: &dyn Fn(&str) -> Option<bigdecimal::BigDecimal>,
) -> Option<Result<bigdecimal::BigDecimal, String>> {
    use bigdecimal::BigDecimal;
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    let lex = amount_lexicon(lang);
    let norm = crate::normalize(text);
    // The renderer's own separator word: fr " et", ca " amb" (en's "," is gone
    // after normalize).
    let sep = num2words2_core::get_lang_by_key(lang)
        .map(|r| crate::normalize(r.default_separator()))
        .unwrap_or_default();
    // he writes its "and" glued: num2words puts it on the unit ("דולריםו",
    // #264), the language on the subunit amount ("ושלושים"). Both are split
    // off a currency name / the word after one.
    let mut owned: Vec<String> = Vec::new();
    let mut after_unit = false;
    for t in norm.split_whitespace() {
        // A one-letter conjunction (he "ו") is the only kind written glued.
        let glued = sep.chars().count() == 1;
        if glued && after_unit && t.len() > sep.len() && t.starts_with(sep.as_str()) {
            owned.push(sep.clone());
            owned.push(t[sep.len()..].to_string());
            after_unit = false;
            continue;
        }
        let named = |w: &str| lex.iter().any(|(n, x)| n == w && !x.sub);
        if let Some(stem) = t.strip_suffix(sep.as_str()).filter(|s| glued && !s.is_empty() && named(s)) {
            owned.push(stem.to_string());
            owned.push(sep.clone());
            after_unit = false;
            continue;
        }
        after_unit = named(t);
        owned.push(t.to_string());
    }
    let toks: Vec<&str> = owned.iter().map(String::as_str).collect();
    // The first currency name that is not a number word: fr "cent" is both
    // the US cent and 100 ("cent dollars").
    let first = (0..toks.len()).find_map(|i| {
        match_tokens(lex, &toks, i)
            .filter(|_| cardinal(toks[i]).is_none())
            .map(|m| (i, m))
    })?;
    let fail = || Some(Err(format!("cannot parse {} as an amount", crate::w2n_sentence::py_repr_str(text))));
    let (i, (word, len)) = first;
    let number = |ts: &[&str]| -> Option<BigDecimal> {
        if ts.is_empty() {
            return None;
        }
        // "un dólar", "eine Rupie", "um real": the article-one in front of
        // the currency is the amount, though the tables only hold "uno",
        // "eins", "um" in other forms.
        if let [one] = ts {
            if article_one(base, one) {
                return Some(BigDecimal::from(1));
            }
        }
        let canon: Vec<String> = ts
            .iter()
            .map(|t| masculine_numeral(base, t).unwrap_or_else(|| t.to_string()))
            .collect();
        cardinal(&canon.join(" "))
    };
    let whole = |d: &BigDecimal| d.is_integer() && *d >= BigDecimal::from(0);
    let Some(head) = number(&toks[..i]) else {
        return fail();
    };
    let rest = &toks[i + len..];
    let minor = |cur: Currency, n: &BigDecimal| -> Option<BigDecimal> {
        let scale = BigDecimal::from(10u64.pow(u32::from(cur.decimals)));
        (cur.decimals > 0 && whole(n) && *n < scale).then(|| (n / scale).with_scale(i64::from(cur.decimals)))
    };
    if word.sub {
        // "ninety nine cents", "one hundred fifty cents" (1.50).
        if !rest.is_empty() || !whole(&head) || word.cur.decimals == 0 {
            return fail();
        }
        let scale = BigDecimal::from(10u64.pow(u32::from(word.cur.decimals)));
        return Some(Ok((head / scale).with_scale(i64::from(word.cur.decimals))));
    }
    if rest.is_empty() {
        return Some(Ok(head));
    }
    let conns = connectors(base);
    let rest = match rest.first() {
        Some(c) if conns.contains(c) || (!sep.is_empty() && *c == sep) => &rest[1..],
        _ => rest,
    };
    if !whole(&head) || rest.is_empty() {
        return fail();
    }
    // "<number> <subunit>" to the end, or a bare "<number>" ("two euros fifty").
    let sub_at = (1..rest.len()).find_map(|j| {
        match_tokens(lex, rest, j).filter(|(w, l)| w.sub && j + l == rest.len()).map(|_| j)
    });
    let n = match sub_at {
        Some(j) => number(&rest[..j]),
        None => number(rest),
    };
    match n.and_then(|n| minor(word.cur, &n)) {
        Some(f) => Some(Ok(head + f)),
        None => fail(),
    }
}

#[cfg(test)]
mod tests {
    use crate::w2n_lang_en::W2nValue;

    fn fold(text: &str, lang: &str) -> String {
        crate::w2n_sentence::words2num_sentence_opts(text, lang, "cardinal", false, true).unwrap()
    }
    fn amount(text: &str, lang: &str) -> String {
        match crate::words2num(text, lang, "currency").unwrap() {
            W2nValue::Dec(d) => d.to_string(),
            W2nValue::Int(i) => format!("int {i}"),
            W2nValue::Float(f) => format!("float {f}"),
        }
    }

    /// #45: a bare subunit of 100 or more carries into the unit.
    #[test]
    fn bare_subunit_of_100_or_more() {
        assert_eq!(fold("one hundred fifty cents", "en"), "$1.50");
        assert_eq!(fold("one hundred and fifty cents", "en"), "$1.50");
        assert_eq!(fold("one thousand cents", "en"), "$10.00");
        assert_eq!(fold("cent cinquante centimes", "fr"), "1,50 €");
        assert_eq!(fold("ciento cincuenta centavos", "es"), "$1.50");
        assert_eq!(fold("hundertfünfzig Cent", "de"), "1,50 €");
        // Not a subunit of the unit before it: left as said.
        assert_eq!(fold("twelve dollars and 100 cents", "en"), "$12 and 100 cents");
    }

    /// #46: a spoken fraction longer than the minor unit is never truncated.
    #[test]
    fn long_fraction_stays_unfolded() {
        assert_eq!(fold("three point five yen", "en"), "3.5 yen");
        assert_eq!(fold("twelve point three four five euros", "en"), "12.345 euros");
        assert_eq!(fold("deux virgule cinq euros", "fr"), "2,50 €");
    }

    /// #47: num2words' English "<unit>, <n> <subunit>".
    #[test]
    fn comma_before_subunit() {
        assert_eq!(fold("twelve euros, thirty-four cents", "en"), "€12.34");
        assert_eq!(fold("twelve pounds, thirty-four pence", "en"), "£12.34");
        assert_eq!(fold("I paid twenty dollars, fifty cents", "en"), "I paid $20.50");
        assert_eq!(fold("five dollars, please", "en"), "$5, please");
    }

    /// #48: a subunit after "<number> <unknown word> <connector>" is not
    /// given a currency nobody said.
    #[test]
    fn subunit_after_connector_is_not_guessed() {
        assert_eq!(fold("five apples and fifty cents", "en"), "5 apples and 50 cents");
        assert_eq!(fold("doce francos con treinta y cuatro céntimos", "es"), "CHF 12.34");
        assert_eq!(fold("dotze reals amb trenta-quatre centaus", "ca"), "12,34 R$");
        assert_eq!(fold("dotze francs amb trenta-quatre cèntims", "ca"), "CHF 12.34");
        assert_eq!(fold("duas libras e cinquenta pence", "pt"), "2,50 £");
    }

    /// #49: fr "cents" after "<unit> et <number>" is the US cent.
    #[test]
    fn fr_cents_subunit() {
        assert_eq!(fold("douze dollars et trente-quatre cents", "fr"), "12,34 $");
        assert_eq!(fold("deux euros et cinq cents", "fr"), "2,05 €");
        assert_eq!(fold("mille dollars et cinquante cents", "fr"), "1 000,50 $");
        // Still the hundreds where they are hundreds.
        assert_eq!(fold("deux cents euros", "fr"), "200 €");
        assert_eq!(fold("mille euros et deux cents personnes", "fr"), "1 000 € et 200 personnes");
    }

    /// #81: "pounds of" is a weight.
    #[test]
    fn pounds_of_is_weight() {
        assert_eq!(fold("twenty pounds of flour", "en"), "20 pounds of flour");
        assert_eq!(fold("I lost five pounds of weight", "en"), "I lost 5 pounds of weight");
        assert_eq!(fold("twenty pounds", "en"), "£20");
        assert_eq!(fold("twenty quid of change", "en"), "£20 of change");
    }

    /// #103: the article-number counts before any currency the fold knows.
    #[test]
    fn article_one_before_any_currency() {
        assert_eq!(fold("one yen", "en"), "¥1");
        assert_eq!(fold("one rouble", "en"), "₽1");
        assert_eq!(fold("one franc", "en"), "CHF 1");
        assert_eq!(fold("un rouble", "fr"), "1 ₽");
        assert_eq!(fold("un rublo", "es"), "1 ₽");
        assert_eq!(fold("um iene", "pt"), "1 ¥");
        // Not before a name whose ordinary reading dominates.
        assert_eq!(fold("j'ai lu un livre", "fr"), "j'ai lu un livre");
        assert_eq!(fold("une livre de beurre", "fr"), "une livre de beurre");
        assert_eq!(fold("one real problem", "en"), "one real problem");
        assert_eq!(fold("no one won", "en"), "no one won");
        assert_eq!(fold("una corona de flores", "es"), "una corona de flores");
        assert_eq!(fold("uma libra", "pt"), "uma libra");
        assert_eq!(fold("vinte e uma libras", "pt"), "21 £");
        // An article and a homograph count when a subunit makes it money.
        assert_eq!(fold("a dollar and twenty cents", "en"), "$1.20");
        assert_eq!(fold("it costs an euro and fifty cents", "en"), "it costs €1.50");
        assert_eq!(fold("a pound and fifty pence", "en"), "£1.50");
        assert_eq!(fold("une livre et vingt pence", "fr"), "1,20 £");
        assert_eq!(fold("une livre vingt pence", "fr"), "1,20 £");
        // Without one, they stay words.
        assert_eq!(fold("a dollar", "en"), "a dollar");
        assert_eq!(fold("a dollar and two friends", "en"), "a dollar and 2 friends");
        assert_eq!(fold("une livre et deux kilos", "fr"), "une livre et 2 kilos");
        assert_eq!(fold("une livre de beurre et vingt centimes", "fr"), "une livre de beurre et 20 centimes");
        // Plain sentence mode is not touched.
        let plain = |t: &str, l: &str| crate::w2n_sentence::words2num_sentence(t, l, "cardinal", false).unwrap();
        assert_eq!(plain("un livre de poche", "fr"), "un livre de poche");
        assert_eq!(plain("one real problem", "en"), "one real problem");
        assert_eq!(plain("one yen", "en"), "one yen");
    }

    /// #104: every unit and subunit name num2words writes is read.
    #[test]
    fn renderer_currency_names() {
        assert_eq!(fold("doce rublos con treinta y cuatro kopeykas", "es"), "12,34 ₽");
        assert_eq!(fold("dodici rubli e trentaquattro copechi", "it"), "12,34 ₽");
        assert_eq!(fold("doze rublos e trinta e quatro copeques", "pt"), "12,34 ₽");
        assert_eq!(fold("twelve rupees, thirty-four paise", "en"), "₹12.34");
        assert_eq!(fold("zwölf Yuan und vierunddreißig Fen", "de"), "12,34 CN¥");
        assert_eq!(fold("twelve francs, thirty-four rappen", "en"), "CHF 12.34");
        assert_eq!(fold("dos francos", "es"), "CHF 2");
        assert_eq!(fold("dos iens / dos rupies / dos rubles", "ca"), "2 ¥ / 2 ₹ / 2 ₽");
    }

    /// #181: "<unit> <connector> <n>" at the end of the amount.
    #[test]
    fn connector_and_bare_subunit() {
        assert_eq!(fold("two dollars and fifty", "en"), "$2.50");
        assert_eq!(fold("cinco euros con cincuenta", "es"), "5,50 €");
        assert_eq!(fold("cinco euros con cinco", "es"), "5,05 €");
        assert_eq!(fold("two dollars and fifty people", "en"), "$2 and 50 people");
    }

    /// #182: a symbol written after the number is spaced in English.
    #[test]
    fn en_suffix_symbol_spacing() {
        assert_eq!(fold("twenty krone", "en"), "20 kr");
        assert_eq!(fold("two zlotys", "en"), "2 zł");
    }

    /// #189: to="currency" returns the amount, unit plus subunit.
    #[test]
    fn to_currency_amount() {
        assert_eq!(amount("twelve euros, thirty-four cents", "en"), "12.34");
        assert_eq!(amount("twelve euros, fifty cents", "en"), "12.50");
        assert_eq!(amount("twelve dollars and fifty cents", "en"), "12.50");
        assert_eq!(amount("douze euros et trente-quatre centimes", "fr"), "12.34");
        assert_eq!(amount("cent euros", "fr"), "100");
        assert_eq!(amount("zwei Euro", "de"), "2");
        assert_eq!(amount("zwölf Euro", "de"), "12");
        assert_eq!(amount("un dólar", "es"), "1");
        assert_eq!(amount("ninety nine cents", "en"), "0.99");
        assert_eq!(amount("two euros fifty", "en"), "2.50");
        assert_eq!(amount("двенадцать рублей тридцать четыре копейки", "ru"), "12.34");
        // pt feminine numerals (#266).
        assert_eq!(amount("duas libras", "pt"), "2");
        // he: num2words glues "and" to the unit (#264); speech to the amount.
        assert_eq!(amount("שתים עשרה דולריםו שלושים וארבע סנטים", "he"), "12.34");
        assert_eq!(amount("שתים עשרה דולרים ושלושים וארבע סנטים", "he"), "12.34");
        // No currency word: the plain cardinal.
        assert_eq!(amount("forty two", "en"), "int 42");
        // A currency word that is not an amount raises.
        assert!(crate::words2num("twelve euros and change", "en", "currency").is_err());
        assert!(crate::words2num("twelve dollars and 100 cents", "en", "currency").is_err());
    }
}
