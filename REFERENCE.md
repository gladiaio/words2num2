# words2num2 — API Reference

Complete reference for every public symbol in `words2num2`. For an
overview, see [README.rst](README.rst).

## Table of contents

- [Top-level functions](#top-level-functions)
  - [`words2num`](#words2numtext-lang--en-to--cardinal-kwargs)
  - [`words2num_sentence`](#words2num_sentencesentence-lang--en-to--cardinal-kwargs)
  - [`auto_parse`](#auto_parsetext-lang--en-prefer--none-thousands_sep--none-decimal_sep--none)
  - [`auto_parse_sentence`](#auto_parse_sentencetext--expandfalse)
  - [`parse_number_string`](#parse_number_strings-thousands_sep--none-decimal_sep--none-lang--none)
- [Data classes](#data-classes)
  - [`Quantity`](#quantity)
  - [`UnitInfo`](#unitinfo)
  - [`CurrencyInfo`](#currencyinfo)
- [Registries](#registries)
  - [`CONVERTER_CLASSES`](#converter_classes)
  - [`UNITS`](#units)
  - [`CURRENCIES`](#currencies)
  - [`NUMBER_FORMAT_DEFAULTS`](#number_format_defaults)
- [Exceptions](#exceptions)
  - [`Words2NumError`](#words2numerror)
- [Per-language converters](#per-language-converters)
- [Pluralization](#pluralization)
- [Conversion types](#conversion-types)

---

## Top-level functions

### `words2num(text, lang="en", to="cardinal", **kwargs)`

Parse `text` (a number written in words) into a numeric value.

**Parameters**

| Name | Type | Default | Description |
|---|---|---|---|
| `text` | `str` | — | The words to parse (e.g. `"forty-two"`). |
| `lang` | `str` | `"en"` | Locale code. Same set as `num2words2`. Hyphens (`en-US`) are normalized to underscores; unknown subtags fall back to the base language. |
| `to` | `str` | `"cardinal"` | One of `"cardinal"`, `"ordinal"`, `"ordinal_num"`, `"year"`, `"currency"`. |

**Returns**

`int`, `float`, or `decimal.Decimal` depending on the input form.

**Raises**

- `NotImplementedError` — `lang` is not registered in `CONVERTER_CLASSES`.
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
```

---

### `words2num_sentence(sentence, lang="en", to="cardinal", **kwargs)`

Walk a sentence and replace every word-number with its numeric form.
Non-number tokens pass through unchanged. Punctuation is preserved.

**Aliases:** `convert_sentence`, `sentence_to_words`.

**Parameters** — same as `words2num`.

**Returns** — `str`.

**Examples**

```python
>>> words2num_sentence("I bought twenty-three apples and fourteen pears.")
'I bought 23 apples and 14 pears.'
>>> words2num_sentence("She is twenty-one years old.")
'She is 21 years old.'
>>> words2num_sentence("In nineteen ninety nine, two thousand people came.", to="year")
'In 1999, 2000 people came.'
```

---

### `auto_parse(text, lang="en", prefer=None, thousands_sep=None, decimal_sep=None)`

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

### `auto_parse_sentence(text, lang="en", prefer=None, thousands_sep=None, decimal_sep=None, expand=False)`

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

### `parse_number_string(s, thousands_sep=None, decimal_sep=None, lang=None)`

Parse a digit-form numeric string with configurable separators.

**Resolution order:**

1. If `thousands_sep` and/or `decimal_sep` are given, use them.
2. Otherwise if `lang` matches a known locale, use its defaults from
   [`NUMBER_FORMAT_DEFAULTS`](#number_format_defaults).
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

### `UnitInfo`

Per-unit metadata in the [`UNITS`](#units) registry.

| Field | Type | Description |
|---|---|---|
| `short` | `str` | Canonical short form (e.g. `"cm"`). |
| `long` | `str` | Canonical long form (e.g. `"centimeter"`). |
| `kind` | `str` | `"length"`, `"mass"`, `"temperature"`, `"time"`, `"volume"`, or `"percent"`. |
| `confidence` | `float` | Default confidence reported when the token is matched (lower for ambiguous tokens). |

### `CurrencyInfo`

Per-currency metadata in the [`CURRENCIES`](#currencies) registry.

| Field | Type | Description |
|---|---|---|
| `code` | `str` | ISO 4217 code (`"USD"`, `"EUR"`, …). |
| `symbol` | `str` | Symbol (`"$"`, `"€"`, …). |
| `long` | `str` | Singular long form (`"dollar"`, `"euro"`, …). |

---

## Registries

### `CONVERTER_CLASSES`

`Dict[str, Words2Num_Base]`. Maps a locale code to a per-language
converter instance. Same keys as `num2words2.CONVERTER_CLASSES`.

```python
>>> from words2num2 import CONVERTER_CLASSES
>>> len(CONVERTER_CLASSES)
120
>>> "fr" in CONVERTER_CLASSES
True
>>> CONVERTER_CLASSES["fr"].to_cardinal("quarante-deux")
42
```

### `UNITS`

`Dict[str, UnitInfo]`. Maps a unit token (and its case-insensitive or
plural alias) to its `UnitInfo`. Used by `auto_parse` to resolve the
unit suffix.

Categories included:

- **Length:** `mm`, `cm`, `dm`, `m`, `km`, `in`, `ft`, `yd`, `mi`, `nm`, `µm`/`um`
- **Mass:** `mg`, `g`, `kg`, `t`, `lb`/`lbs`, `oz`
- **Temperature:** `°`, `°C`, `°F`, `K`, `C`, `F`
- **Time:** `ms`, `s`, `sec`, `min`, `h`/`hr`/`hrs`, `d`
- **Volume:** `ml`, `cl`, `dl`, `l`/`L`, `gal`
- **Percent:** `%`

Ambiguous tokens (`m`, `g`, `t`, `K`, `s`, `h`, `d`, `in`) have a
`confidence` < 1.0; callers can override with `prefer={...}`.

### `CURRENCIES`

`Dict[str, CurrencyInfo]`. Maps a currency symbol (`$`, `€`, …) **or**
ISO code (`USD`, `EUR`, …) to its `CurrencyInfo`.

Symbols recognized: `$`, `€`, `£`, `¥`, `₹`, `₽`, `₩`, `₺`.
Codes recognized: `USD`, `EUR`, `GBP`, `JPY`, `CHF`, `CAD`, `AUD`,
`CNY`, `INR`, `BRL`, `MXN`, `RUB`, `KRW`.

### `NUMBER_FORMAT_DEFAULTS`

`Dict[str, Dict[str, str]]`. Per-locale defaults for the
`thousands` and `decimal` separators used by `parse_number_string`
when no explicit override is given.

Excerpt:

| Locale | thousands | decimal | Notes |
|---|---|---|---|
| `en`, `en_GB`, `en_IN`, `zh*`, `ja`, `ko`, `th` | `,` | `.` | |
| `fr`, `fr_BE`, `fr_DZ` | ` ` (NBSP) | `,` | |
| `fr_CH` | `'` | `.` | Swiss apostrophe |
| `de`, `es*`, `it`, `pt*`, `nl`, `ro`, `hr`, `sl`, `sr`, `tr`, `el`, `da`, `is`, `fo` | `.` | `,` | |
| `ru`, `uk`, `be`, `bg`, `pl`, `cs`, `sk`, `hu`, `sv`, `no`, `nn`, `fi`, `et`, `lt`, `lv` | ` ` | `,` | |
| `_default` | `,` | `.` | Fallback. |

Full table: `words2num2/formats.py`.

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

## Per-language converters

Every locale module (`words2num2/lang_XX.py`) defines a class
`Words2Num_XX` inheriting from `Words2Num_Base`. The class is
instantiated once and stored in `CONVERTER_CLASSES`.

The base class provides a generic implementation that derives a
`{normalized_words: integer}` lookup table by calling `num2words2`
forward across an integer range (`-1..10000` by default, configurable
via `LOOKUP_RANGE`). The first `to_cardinal` / `to_ordinal` call
materializes the table lazily.

To add a hand-written grammar parser for a locale, override
`to_cardinal` / `to_ordinal` in the subclass:

```python
# words2num2/lang_FR.py
from .base import Words2Num_Base

class Words2Num_FR(Words2Num_Base):
    LANG = "fr"

    def to_cardinal(self, text):
        # Custom French grammar that handles soixante-dix (70),
        # quatre-vingts (80), conjunctions like "et", etc.
        ...
```

`words2num2/lang_EN.py` is a complete reference implementation.

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

The `to=` parameter on `words2num` accepts:

| Value | Meaning | Example |
|---|---|---|
| `"cardinal"` | Standard count. | `"forty-two"` → `42` |
| `"ordinal"` | Position. | `"twenty-first"` → `21` |
| `"ordinal_num"` | Digit-form ordinal. | `"21st"` → `21` |
| `"year"` | Year-pair shorthand. | `"nineteen ninety nine"` → `1999` |
| `"currency"` | Currency expression. | `"twelve dollars and fifty cents"` → varies by locale |

Unsupported values raise `NotImplementedError`.

---

## See also

- [README.rst](README.rst) — overview, install, quickstart.
- [CHANGELOG.md](CHANGELOG.md) — version history.
- [LOCAL_TESTING.md](LOCAL_TESTING.md) — running the test suite locally.
- [CONTRIBUTING.md](CONTRIBUTING.md) — adding hand-written parsers.
- [num2words2](https://github.com/gladiaio/num2words2) — the forward direction.
