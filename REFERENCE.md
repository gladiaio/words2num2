# words2num2 — API Reference

Complete reference for every public symbol in `words2num2`. For an
overview, see [README.rst](README.rst).

## Table of contents

- [Top-level functions](#top-level-functions)
  - [`words2num`](#words2num)
  - [`words2num_sentence`](#words2num_sentence)
  - [`auto_parse`](#auto_parse)
  - [`auto_parse_sentence`](#auto_parse_sentence)
  - [`parse_number_string`](#parse_number_string)
  - [`normalize`](#normalize)
  - [`supported_langs`](#supported_langs)
- [Data classes](#data-classes)
  - [`Quantity`](#quantity)
- [Constants](#constants)
  - [`CONVERTER_TYPES`](#converter_types)
- [Exceptions](#exceptions)
  - [`Words2NumError`](#words2numerror)
- [Language codes](#language-codes)
- [How a language is parsed](#how-a-language-is-parsed)
- [Pluralization](#pluralization)
- [Conversion types](#conversion-types)

---

## Top-level functions

### `words2num`

`words2num(text, lang="en", to="cardinal")`

Parse `text` (a number written in words) into a numeric value.

**Parameters**

| Name | Type | Default | Description |
|---|---|---|---|
| `text` | `str` | — | The words to parse (e.g. `"forty-two"`). |
| `lang` | `str` | `"en"` | Locale code, case-insensitive; see [Language codes](#language-codes). |
| `to` | `str` | `"cardinal"` | One of [`CONVERTER_TYPES`](#converter_types): `"cardinal"`, `"ordinal"`, `"ordinal_num"`, `"year"`, `"currency"`, `"digits"`. See [Conversion types](#conversion-types). |

There are no other keyword arguments: an unknown one is a `TypeError`
naming it.

**Returns**

`int`, `float`, or `decimal.Decimal` depending on the input form.

**Raises**

- `NotImplementedError` — `lang` is not a supported language, or `to` is
  not a conversion type.
- `Words2NumError` — `text` cannot be parsed.

**Examples**

```python
>>> words2num("forty-two")
42
>>> words2num("one thousand two hundred")
1200
>>> words2num("three point one four")
Decimal('3.14')
>>> words2num("twenty-first", to="ordinal")
21
>>> words2num("nineteen ninety nine", to="year")
1999
>>> words2num("quarante-deux", lang="fr")
42
>>> words2num("two seven five", to="digits")
275
>>> words2num("one", lang="EN-us")
1
```

---

### `words2num_sentence`

`words2num_sentence(sentence, lang="en", to="cardinal", currency=False)`

Walk a sentence and replace every word-number with its numeric form.
Non-number tokens pass through unchanged. Punctuation is preserved.

**Aliases:** `convert_sentence`, `sentence_to_words`.

**Parameters** — `lang` and `to` as for `words2num`, checked before the
walk (an unsupported value raises `NotImplementedError`, it does not
return the sentence unchanged). `currency=True` writes an amount spoken as
number + currency word (+ subunit) the way the language writes it
(`"twelve dollars and fifty cents"` → `"$12.50"`).

**Returns** — `str`.

**Examples**

```python
>>> words2num_sentence("I bought twenty-three apples and fourteen pears.")
'I bought 23 apples and 14 pears.'
>>> words2num_sentence("She is twenty-one years old.")
'She is 21 years old.'
>>> words2num_sentence("In nineteen ninety nine, two thousand people came.", to="year")
'In 1999, 2000 people came.'
>>> words2num_sentence("my code is zero seven")
'my code is 07'
```

---

### `auto_parse`

`auto_parse(text, lang="en", prefer=None, thousands_sep=None, decimal_sep=None)`

Parse a single quantity expression — a number plus an optional unit
or currency — and return a structured [`Quantity`](#quantity).

**Resolution order:**

1. Currency-prefix form (`$12.50`, `€42`, `$5m`, `USD 100`).
2. Currency-suffix form (`12.50 €`, `100 USD`).
3. Number + unit-suffix in digit form (`5cm`, `20°C`, `42%`, `3.5 km`).
4. Word-form number + unit (`forty-two kg`, `twenty-three percent`).
5. Pure number (digit form via `parse_number_string`, then word form
   via `words2num`).

**Parameters**

| Name | Type | Default | Description |
|---|---|---|---|
| `text` | `str` | — | Quantity expression. |
| `lang` | `str` | `"en"` | Locale for word-form fallback and locale-default separators. |
| `prefer` | `Optional[Dict[str, str]]` | `None` | Disambiguation hints for unit tokens. Example: `{"m": "mile"}`. |
| `thousands_sep` | `Optional[str]` | `None` | Override locale default. |
| `decimal_sep` | `Optional[str]` | `None` | Override locale default. |

**Returns** — [`Quantity`](#quantity).

**Raises** — `Words2NumError` if the input cannot be parsed.

**Examples**

```python
>>> auto_parse("$12,345.00")
Quantity(value=12345.0, unit='USD', kind='currency', confidence=1.0)

>>> auto_parse("$5m").value
5000000

>>> auto_parse("5cm")
Quantity(value=5, unit='cm', kind='length', confidence=1.0)

>>> auto_parse("forty-two kg").value
42

>>> auto_parse("12,50 €", lang="de").value
12.5

>>> auto_parse("5m", prefer={"m": "mile"}).unit_long
'mile'
```

---

### `auto_parse_sentence`

`auto_parse_sentence(text, lang="en", prefer=None, thousands_sep=None, decimal_sep=None, expand=False)`

Walk free text and replace every quantity in place. Punctuation
between matches is preserved.

**Parameters**

| Name | Type | Default | Description |
|---|---|---|---|
| `text` | `str` | — | Sentence or paragraph. |
| `lang` | `str` | `"en"` | Locale for word-form fallback and separators. |
| `prefer` | `Optional[Dict[str, str]]` | `None` | Disambiguation hints. |
| `thousands_sep` | `Optional[str]` | `None` | Override locale default. |
| `decimal_sep` | `Optional[str]` | `None` | Override locale default. |
| `expand` | `bool` | `False` | If `True`, render the long unit form with English plural rules (`12.5 dollars`); otherwise render the canonical short form (`12.5 USD`). |

**Returns** — `str`.

**Examples**

```python
>>> auto_parse_sentence("Pay $12.50 for 5kg at -5°C.")
'Pay 12.5 USD for 5 kg at -5 °C.'

>>> auto_parse_sentence("Pay $12.50 for 5kg.", expand=True)
'Pay 12.5 dollars for 5 kilograms.'

>>> auto_parse_sentence("Pay $1.00 for 1kg.", expand=True)
'Pay 1 dollar for 1 kilogram.'

>>> auto_parse_sentence("Total: 1.234,56 €.", lang="de")
'Total: 1234.56 EUR.'
```

In short form, percent and bare degree are tightly glued
(`70%`, `20°`); other units are space-separated (`5 kg`).

---

### `parse_number_string`

`parse_number_string(s, thousands_sep=None, decimal_sep=None, lang=None)`

Parse a digit-form numeric string with configurable separators.

**Resolution order:**

1. If `thousands_sep` and/or `decimal_sep` are given, use them.
2. Otherwise if `lang` matches a known locale, use its default
   separators (see [the table below](#locale-separator-defaults)).
3. Otherwise auto-detect from the string itself.

**Auto-detect heuristic:**

- If both `.` and `,` appear, the rightmost one is the decimal.
- If one separator appears multiple times, it is thousands.
- If one separator appears once with exactly 3 trailing digits, it
  is thousands; otherwise it is decimal.
- Spaces (incl. NBSP/NNBSP/THIN_SPACE), apostrophe, and underscore
  are always thousands.

**Parameters**

| Name | Type | Default | Description |
|---|---|---|---|
| `s` | `str` | — | Digit-form numeric string. May start with `+` or `-`. |
| `thousands_sep` | `Optional[str]` | `None` | Explicit thousands separator. |
| `decimal_sep` | `Optional[str]` | `None` | Explicit decimal separator. |
| `lang` | `Optional[str]` | `None` | Locale for default separators. |

**Returns** — `int` if the string represents an integer, `float` otherwise.

**Raises** — `Words2NumError`.

**Examples**

```python
>>> parse_number_string("12,345.67")
12345.67
>>> parse_number_string("12.345,67", lang="de")
12345.67
>>> parse_number_string("1 234,56", lang="fr")
1234.56
>>> parse_number_string("12'345.67", thousands_sep="'", decimal_sep=".")
12345.67
>>> parse_number_string("1_234.56", thousands_sep="_")
1234.56
>>> parse_number_string("-12,345.67")
-12345.67
```

---

### `normalize`

`normalize(text)`

Normalize `text` the way the core does before a lookup: NFKD, lowercase,
strip diacritics, hyphens between words become spaces, collapse
whitespace. Returns `str`.

```python
>>> normalize("Forty-Two")
'forty two'
>>> normalize("Trente-Deux")
'trente deux'
```

---

### `supported_langs`

`supported_langs()`

Return the list of language keys `words2num` accepts (the same set the
embedded num2words2 renderer serves).

```python
>>> "fr" in supported_langs()
True
>>> "pt_BR" in supported_langs()
True
```

---

## Data classes

### `Quantity`

Returned by `auto_parse` and used by `auto_parse_sentence` internally.

| Field | Type | Description |
|---|---|---|
| `value` | `int \| float \| Decimal` | Numeric value. |
| `unit` | `Optional[str]` | Canonical short unit (`"kg"`, `"USD"`, `"%"`, `"°C"`, …). |
| `unit_long` | `Optional[str]` | Canonical long unit (`"kilogram"`, `"dollar"`, `"percent"`, …). |
| `kind` | `Optional[str]` | One of `"currency"`, `"length"`, `"mass"`, `"temperature"`, `"time"`, `"volume"`, `"percent"`, or `None`. |
| `confidence` | `float` | In `[0, 1]`. Drops below 1.0 for ambiguous unit tokens (`m`, `g`, `t`, `K`, `s`, `h`, `d`). |
| `raw` | `str` | The original input. |

`Quantity` has a custom `__repr__` for readable debug output.

---

## Constants

### `CONVERTER_TYPES`

Tuple of the accepted `to=` values (see [Conversion types](#conversion-types)).

```python
>>> CONVERTER_TYPES
('cardinal', 'ordinal', 'ordinal_num', 'year', 'currency', 'digits')
```

### Locale separator defaults

`parse_number_string` and `auto_parse` use per-locale default separators
when none is given. The table lives in the core
(`rust/words2num2-core/src/w2n_formats.rs`); it is not exported. Excerpt:

| Locale | thousands | decimal | Notes |
|---|---|---|---|
| `en`, `en_GB`, `en_IN`, `zh*`, `ja`, `ko`, `th` | `,` | `.` | |
| `fr`, `fr_BE`, `fr_DZ` | ` ` (NBSP) | `,` | |
| `fr_CH` | `'` | `.` | Swiss apostrophe |
| `de`, `es*`, `it`, `pt*`, `nl`, `ro`, `hr`, `sl`, `sr`, `tr`, `el`, `da`, `is`, `fo` | `.` | `,` | |
| `ru`, `uk`, `be`, `bg`, `pl`, `cs`, `sk`, `hu`, `sv`, `no`, `nn`, `fi`, `et`, `lt`, `lv` | ` ` | `,` | |
| anything else | `,` | `.` | Fallback. |

Units recognized by `auto_parse` (also internal): length (`mm` `cm` `m`
`km` `in` `ft` `yd` `mi` …), mass (`mg` `g` `kg` `t` `lb` `oz`),
temperature (`°C` `°F` `K`), time (`ms` `s` `min` `h` `d`), volume (`ml`
`l` `gal`), and `%`; currencies by symbol (`$` `€` `£` `¥` `₹` `₽` `₩` `₺`)
or ISO code (`USD`, `EUR`, `GBP`, …). Ambiguous tokens (`m`, `g`, `t`,
`K`, `s`, `h`, `d`, `in`) get a `confidence` below 1.0; override them with
`prefer={...}`.

---

## Exceptions

### `Words2NumError`

Subclass of `ValueError`. Raised whenever input cannot be parsed —
empty strings, unrecognized tokens, mismatched separators, malformed
digit groups, etc.

```python
>>> from words2num2 import words2num
>>> from words2num2.base import Words2NumError
>>> try:
...     words2num("forty zoot")
... except Words2NumError as exc:
...     print("could not parse:", exc)
could not parse: unrecognized token 'zoot' in 'forty zoot'
```

---

## Language codes

`lang` is matched against [`supported_langs()`](#supported_langs):

1. an exact key (`"en"`, `"sr_Latn"`, `"en_Aero_FAA"`);
2. `-` and `_` are interchangeable (`"pt-BR"`);
3. case is ignored (`"EN"`, `"pt_br"`), and ISO 639-2 three-letter codes
   name their language (`"eng"`, `"fra"`, `"ger"`);
4. unknown trailing subtags fall back to the base language
   (`"en-US"` → `en`, `"sr-Latn-RS"` → `sr_Latn`).

Anything else raises `NotImplementedError`, including language names
(`"swedish"`): there is no prefix guessing, which used to read
`"swedish"` as Swahili (`sw`).

```python
>>> words2num("trois", lang="FR")
3
>>> words2num("um", lang="pt-br")
1
>>> words2num("one", lang="swedish")
Traceback (most recent call last):
  ...
NotImplementedError: language 'swedish' is not supported
```

---

## How a language is parsed

All parsing happens in the compiled Rust core (`rust/words2num2-core`);
the `words2num2` Python package is a thin binder over it. English has a
hand-written grammar (`w2n_lang_en.rs`). Every other language is parsed
by a reverse lookup table built lazily, on first use, by rendering
`-1..10000` with the embedded num2words2 renderer (`num2words2-core`) and
inverting it; larger numbers are composed from the language's scale words
(thousand, million, …) derived the same way. There are no per-language
Python modules to override: to improve a language, change the core (see
[CONTRIBUTING.md](CONTRIBUTING.md)).

---

## Pluralization

`words2num2.converters.auto.pluralize(long_form, value)` applies English
plural rules to a long-form unit name. Invoked automatically by
`auto_parse_sentence` when `expand=True`.

**Rules:**

- `value == ±1` → singular.
- Uncountable units stay invariant: `yen`, `yuan`, `won`, `kelvin`, `percent`.
- Irregular forms: `foot → feet`, `inch → inches`, `pound sterling → pounds sterling`,
  `degree celsius → degrees celsius`, `degree fahrenheit → degrees fahrenheit`,
  `Swiss franc → Swiss francs`, etc.
- Regular: `-s`, `-es` after sibilants (`s/x/z/ch/sh`), `-ies` after consonant + `y`.

```python
>>> from words2num2.converters.auto import pluralize
>>> pluralize("dollar", 5)
'dollars'
>>> pluralize("dollar", 1)
'dollar'
>>> pluralize("foot", 5)
'feet'
>>> pluralize("yen", 100)
'yen'
>>> pluralize("inch", 5)
'inches'
```

---

## Conversion types

The `to=` parameter on `words2num` and `words2num_sentence` accepts
(`CONVERTER_TYPES`):

| Value | Meaning | Example |
|---|---|---|
| `"cardinal"` | Standard count. | `"forty-two"` → `42` |
| `"ordinal"` | Position. | `"twenty-first"` → `21` |
| `"ordinal_num"` | Digit-form ordinal. | `"21st"` → `21` |
| `"year"` | Year-pair shorthand. | `"nineteen ninety nine"` → `1999` |
| `"currency"` | Currency amount (unit + subunit). | `"twelve dollars and fifty cents"` |
| `"digits"` | Digit by digit, concatenated. | `"two seven five"` → `275` |

`"digits"` returns an `int`, so leading zeros are dropped
(`"zero seven"` → `7`); [`words2num_sentence`](#words2num_sentence) keeps
them (`"zero seven"` → `"07"`).

Unsupported values raise `NotImplementedError`.

---

## See also

- [README.rst](README.rst) — overview, install, quickstart.
- [CHANGELOG.md](CHANGELOG.md) — version history.
- [LOCAL_TESTING.md](LOCAL_TESTING.md) — running the test suite locally.
- [CONTRIBUTING.md](CONTRIBUTING.md) — adding hand-written parsers.
- [num2words2](https://github.com/gladiaio/num2words2) — the forward direction.
