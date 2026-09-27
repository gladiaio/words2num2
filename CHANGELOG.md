# Changelog

All notable changes to `words2num2` are documented here. This project
follows [Semantic Versioning](https://semver.org/) and uses
[Keep a Changelog](https://keepachangelog.com/) style.

## [Unreleased]

Sentence mode (`words2num_sentence`) was audited on ~300 real-life ASR finals
(call-center, IVR, finance; en / fr / es, plus de / it / pt / nl and a sweep
of 20 other locales): 59 % correct before, 96 % after. Every change is
covered in `tests/test_sentence_realworld.py`.

### Added

- **Currency fold in sentence mode**, `words2num_sentence(..., currency=True)`
  (off by default): an amount spoken as number + currency word (+ connector
  + subunit) is written the way the language writes that currency — en
  `"$1,355.28"`, `"$0.99"`, `"£20"`, `"¥10,000"`; fr `"43,20 $"`,
  `"1 200,50 €"`, `"5 $ CA"`; es `"$154.92"` (dollars and pesos in the
  Latin-American form) / `"500,20 €"`; de `"23,50 €"`; it `"20,50 €"`; pt
  `"R$ 200,50"`; nl `"€ 20,50"`. 23 currencies, names and subunits in en /
  fr / es / de / it / pt / nl / ca. A currency word without a number stays a
  word.
- it `"cinquanta centesimi"` is the cent, not the hundredth (was `"50 100º"`).
- de `"tausend Euro"` -> `"1000 Euro"` (the bare thousand morpheme).
- **Ordinals in sentence mode**, written in the language's figures: en
  `21st` / `22nd` / `3rd` / `15th`, fr `1er` / `1re` / `2e`, es-pt-it `3º` /
  `20ª`, de `2.`, nl `15e`, num2words2's `ordinal_num` elsewhere when it
  carries a marker (tr `1'inci`; ru / pl keep the words). A compound is
  always a rank (`"twenty first"`, `"vingt-troisième"`); a lone `first` /
  `second` and its translations only next to a month or "of the month"
  (`"first of all"`, `"wait a second"`, `"la première fois"` stay words);
  fractions stay words (`"a fifth of"`, `"two thirds"`, `"deux tiers"`).
  The ordinal table also matches the inflected forms speech uses (fr
  `première`, es `primer(a)` / `tercer(a)`, de `zweiten`, it `prima`).
- **Digit strings in every language** (were English-only): fr `"sept cinq
  zéro un zéro"` -> `"75010"`, `"zéro six douze …"` -> `"06 12 …"`, es
  `"seis uno nueve ocho cero"` -> `"61980"`; en `"double zero seven"` ->
  `"007"`, `"triple five"` -> `"555"`.
- **Spoken decimal separators**: fr `virgule`, es/ca `coma`, pt `vírgula`,
  it `virgola`, de/nl `komma`, ro `virgulă`, pl `przecinek` -> `","`;
  es/pt/it `punto` / `ponto` -> `"."`: `"trois virgule cinq"` -> `"3,5"`,
  `"deux virgule cinquante"` -> `"2,50"`, `"dos coma cero cinco"` ->
  `"2,05"`.
- **Glued thousands** of de / nl / af / it / sv / da: `"dreiundzwanzigtausend"`
  -> `23000`, `"tweehonderdduizend"` -> `200000`, `"ventitremila"` ->
  `23000`, `"hunderttausend"` -> `100000`; de `"eine Million"`, it `"un
  milione"` open a number.
- fr counts hundreds by the dozen: `"douze cents"` -> `1200`.
- es tens joined with the spoken connector: `"veinte y uno"` -> `21`.

### Fixed

- **The article / pronoun 1.** `"no one answered"` -> `"no 1 answered"`,
  `"un solde de 43 euros"` -> `"1 solde …"`, `"um momento"` -> `"1
  momento"`: a lone `one` / `un` / `une` / `uno` / `um` / `een` / `ein` now
  stays a word unless something counts it — a unit, a currency or a time
  word (`"one dollar"`, `"une heure"`, `"one o'clock"`), the percent phrase,
  a label word before it (`"press one"`, `"number one"`, `"tapez un"`,
  `"marque uno"`), a month, a counted list (`"one, two, three"`). `"one
  second"` stays a pause. es `"un dólar"` now reads `"1 dólar"`.
- **Compounds the walker broke apart.** A token that is not a number word on
  its own is tried as the continuation of the run before the run closes:
  `"deux cents euros"` -> `"200 euros"` (was `"2 cents euros"`), `"quatre
  vingts"` -> `"80"`; fr `"mille deux cent"` / `"deux million"` (plural -s
  dropped or added) match the table; fr `"cinquante et une personnes"` ->
  `"51 personnes"`; ar / id / he compounds that stopped at the connector
  now compose.
- **Connectors are never eaten**: `"one and a half hours"` was `"1 half
  hours"`, `"five point"` was `"5"`, `"one hundred and"` was `"100"`.
- **Percent phrases**: `"vingt pour cent"` -> `"20 pour 100"`, `"diez por
  ciento"` -> `"10 por 100"` — the second word is the percent sign.
- **`"a"` before a scale word** opens the number: `"a hundred dollars"` ->
  `"100 dollars"` (was `"a 100 dollars"`); not after `half` / `quarter`.
- **Quantities stay words**: `"a couple of thousand"`, `"hundreds of"`,
  `"des millions"`, `"millones de personas"`, `"half a million"`; a scale
  word after a decimal stays a word (`"2.5 million"`, `"2,5 millions"`).
- **Decades**: `"in the nineteen nineties"` (was `"in the 19 nineties"`).
- **Year pairs only for Germanic languages**: es `"a las catorce treinta"`
  is a time of day, not 1430.
- `"ten seconds"` is time, not a fraction; `"one k"` counts.
- A lone punctuation part never extends a run: `"quarante-deux ?"` keeps
  its space.
- vi cardinals were rendered as ordinals (its ordinal table holds every
  cardinal).

### Changed

- `"twenty first"` in sentence mode is `"21st"` (was `"21"`).

## [0.3.3] — 2026-09-24

### Fixed

- **Spoken years in sentence mode.** `words2num_sentence` summed
  `"twenty twenty five"` to `"45"` and `"nineteen ninety nine"` to `"118"`
  (or, since the fix below, split them into `"20 25"` / `"19 99"`). A run
  that is not one cardinal but reads as a year — "nineteen"/"twenty" +
  "oh" + digit / a teen / a tens word (+ unit) — now gives the year:
  `"2025"`, `"1999"`, `"december one twenty twenty five"` ->
  `"december 1 2025"`, `"twenty oh five"` -> `"2005"`. `"two thousand
  twenty five"` is still `"2025"`, `"twenty five people"` still
  `"25 people"`; `to="year"` is unchanged.
- **Digit strings in sentence mode.** Account numbers, ZIP codes and phone
  numbers read one digit at a time were summed: `"your account ending in
  four five six seven"` -> `"... 22"`. Two or more consecutive single-digit
  words (with "oh"/"o" as 0 inside the run) now concatenate: `"... 4567"`,
  `"zero six one two"` -> `"0612"`, `"one two three"` -> `"123"`. A lone
  digit word stays a cardinal (`"press one"` -> `"press 1"`), as does
  tens + unit (`"twenty five"` -> `"25"`); `"five twenty"` -> `"5 20"`.
- **A unit may not follow a unit** (nor a teen follow a tens word) in an
  English cardinal, the counterpart of the tens rule below: `"one two
  three"` is three digits, not 6, and `words2num("one two three")` raises.
- **Spanish "ciento".** num2words never renders `"ciento"` on its own (100 is
  `"cien"`), so the reverse table had no entry for it and the sentence
  walker could not open a run on it: `"ciento cincuenta y cuatro dólares"`
  -> `"ciento 54 dólares"`. The bare hundred prefix now reads as 100 and
  the run grows into the table hit: `"154 dólares"`, `"ciento uno"` ->
  `"101"` (`"cien"` and `"doscientos treinta"` were already fine).

- **A tens word no longer composes with a preceding unit.** English reads a
  tens word with a *following* unit ("sixty three" = 63), never the reverse,
  so `"three sixty"` is two numbers in sequence rather than 3 + 60.
  `words2num_sentence` now returns `"3 60"` where it returned `"63"`, and
  likewise `"three sixty five"` -> `"3 65"`, `"nineteen eighty four"` ->
  `"19 84"`, `"twenty twenty"` -> `"20 20"`. `words2num` raises for these,
  since a single number was asked for and two were given.

  `"hundred"` and the scale words close the sub-hundred slot, so
  `"one hundred sixty"` (160), `"three hundred sixty"` (360) and
  `"two thousand sixty"` (2060) are unaffected — as is the pair reading on
  `to="year"`, where `"nineteen eighty four"` is still 1984. Resolves #17.

### Changed

- The two sentence-mode expectations recorded with the tens rule above,
  `"nineteen eighty four"` -> `"19 84"` and `"twenty twenty"` -> `"20 20"`,
  are superseded by the year reading: `"1984"`, `"2020"`.

## [0.2.3] — 2026-05-02

### Fixed
- `aur-publish.yml` workflow now follows HTTP redirects when polling
  for the freshly-published PyPI sdist. Without `curl -L` the legacy
  `files.pythonhosted.org` URL returns a 302 and the wait loop never
  satisfied, so the AUR job timed out before computing the sha256.

### Verified
- End-to-end release pipeline: tag push → build → test → GitHub
  Release → PyPI Trusted Publishing → AUR push, all green.

## [0.2.2] — 2026-05-01

### Changed
- **Bumped minimum Python to 3.10.** `requires-python = ">=3.10"`.
  Python 3.8 and 3.9 are no longer supported.
- CI matrix now runs on Python 3.10, 3.11, 3.12, 3.13, 3.14, and 3.15
  (with `allow-prereleases: true` so 3.15 alpha is exercised). The
  `Build and Release` and `Publish to PyPI (manual)` workflows use
  Python 3.13.
- Trove classifiers in `pyproject.toml` and `setup.py` updated to
  reflect 3.10–3.15.

### Added
- Comprehensive documentation:
  - `REFERENCE.md` — full API reference with parameters, return types,
    examples for every public symbol.
  - `LOCAL_TESTING.md` — repo setup, test invocations, smoke-test
    recipe, release pre-flight, troubleshooting.
  - Expanded `CONTRIBUTING.md` — where-to-look table, hand-written
    parser guide, adding units/currencies, release checklist.
  - Updated `README.rst` with full feature tour, badges, configurable
    number formats, and the auto-parse mode.
- **Arch Linux / AUR package** — `packaging/aur/python-words2num2/`
  with `PKGBUILD`, `.SRCINFO`, and a maintenance README. Installable
  via `yay -S python-words2num2` (after first AUR push).
- **Eight additional GitHub Actions workflows mirroring num2words2:**
  - `codeql-analysis.yml` — weekly Python security scan.
  - `e2e-tests.yml` — full pytest run on Linux/macOS/Windows × Python
    3.10–3.14 (+ PyPy 3.10).
  - `scheduled-test.yml` — nightly cross-platform test matrix.
  - `pr-size.yml` — auto-labels PRs `size/XS`–`size/XXL`.
  - `manual-release.yml` — `workflow_dispatch` to cut a release
    without touching git locally.
  - `manual-publish.yml` — `workflow_dispatch` PyPI publish (TestPyPI
    optional) using `PYPI_API_TOKEN` secret.
  - `python-publish.yml` — auto-publish on CI success when the
    detected version isn't already on PyPI.
  - `aur-publish.yml` — pushes the matching `PKGBUILD` to the AUR on
    new tags. Needs `AUR_SSH_PRIVATE_KEY` repo secret.
- Expanded README badges: PyPI version, Python versions, downloads,
  status, AUR version, CI, Lint, CodeQL, E2E Tests, Coveralls coverage,
  latest release, last commit, issues, license.

## [0.2.1] — 2026-05-01

### Added
- `pluralize(long_form, value)` helper in `words2num2.converters.auto`.
- Plural rules applied automatically by `auto_parse_sentence(..., expand=True)`.
  - Irregular forms: `foot → feet`, `inch → inches`, `pound sterling →
    pounds sterling`, `degree celsius/fahrenheit`, multi-word
    currencies (`Swiss francs`, etc.).
  - Uncountable units stay invariant: `yen`, `yuan`, `won`, `kelvin`,
    `percent`.
  - Regular `-s`/`-es`/`-ies` for everything else.
- 21 new pluralization tests.

### Fixed
- Sentence-mode regex no longer eats trailing whitespace before non-unit
  words (`Pay $12.50 for 5kg.` now keeps the space before "for").

## [0.2.0] — 2026-05-01

### Added
- **Auto-parse mode**: `auto_parse(text, ...)` and
  `auto_parse_sentence(text, ...)` — extract numeric values plus their
  unit from free text.
- `Quantity` dataclass with `value`, `unit`, `unit_long`, `kind`,
  `confidence`, `raw`.
- `parse_number_string(s, thousands_sep=None, decimal_sep=None,
  lang=None)` with caller-overridable separators, per-locale CLDR-style
  defaults for 50+ locales, and an auto-detect heuristic.
- Currency support: prefix and suffix forms for `$ € £ ¥ ₹ ₽ ₩ ₺` plus
  ISO codes (`USD`, `EUR`, `GBP`, `JPY`, `CHF`, `CAD`, `AUD`, `CNY`,
  `INR`, `BRL`, `MXN`, `RUB`, `KRW`).
- Currency scale shortcuts: `$5k`, `$5m`, `$5b`, `$5bn`, `$2.5t`.
- Unit support:
  - Length — `mm`, `cm`, `dm`, `m`, `km`, `in`, `ft`, `yd`, `mi`, `nm`, `µm`.
  - Mass — `mg`, `g`, `kg`, `t`, `lb`/`lbs`, `oz`.
  - Temperature — `°`, `°C`, `°F`, `K`, `C`, `F`.
  - Time — `ms`, `s`/`sec`, `min`, `h`/`hr`/`hrs`, `d`.
  - Volume — `ml`, `cl`, `dl`, `l`/`L`, `gal`.
  - Percent — `%`.
- Word-form unit aliases (English): `forty-two kilograms`,
  `twenty-three percent`, etc.
- Disambiguation hints: `prefer={"m": "mile", "g": "giga"}` for
  ambiguous unit tokens.
- `expand=True` mode for `auto_parse_sentence` — renders the long unit
  form (`12.5 dollar` instead of `12.5 USD`).
- 55 new tests covering currency, units, separators, and sentence mode.
- New public exports: `auto_parse`, `auto_parse_sentence`, `Quantity`,
  `UNITS`, `CURRENCIES`, `parse_number_string`,
  `NUMBER_FORMAT_DEFAULTS`.

## [0.1.1] — 2026-05-01

### Added
- Mirror of `num2words2`'s CI/CD as four GitHub Actions workflows:
  - `ci.yml` — Python 3.8–3.13 matrix, pytest, coverage.
  - `lint.yml` — black, flake8, isort, mypy.
  - `release.yml` — auto-build, GitHub Release, PyPI Trusted Publishing
    on tag push.
  - `publish-pypi.yml` — manual `workflow_dispatch` fallback using
    `PYPI_API_TOKEN` / `TEST_PYPI_API_TOKEN` repo secrets.

### Fixed
- Dependency spec `num2words2 >= 0.1.0.dev0` so pre-release versions
  from PyPI satisfy the requirement.

## [0.1.0] — 2026-05-01

### Added
- Initial release. Mirrors `num2words2`'s package layout with 120
  dispatch entries (~100 distinct languages, 14 regional variants, 2
  aliases).
- Hand-written English grammar parser (`lang_EN.py`) — cardinals,
  ordinals, decimals, negatives, scale words to *centillion*, year
  mode, "and" connectors, hyphenation.
- Generic backend (`Words2Num_Base`) that derives a `{words → number}`
  lookup table by calling `num2words2` forward across the integer
  range `-1..10000`. Provides correctness for that window for every
  locale that `num2words2` supports.
- 117 stub language modules subclassing the generic backend.
- `words2num(text, lang, to)` and `words2num_sentence(text, ...)`
  walking running text and replacing word-numbers in place.
- CLI: `words2num2 "forty-two"`.
- `Words2NumError` exception type.
- 59 tests.

[0.2.3]: https://github.com/gladiaio/words2num2/releases/tag/v0.2.3
[0.2.2]: https://github.com/gladiaio/words2num2/releases/tag/v0.2.2
[0.2.1]: https://github.com/gladiaio/words2num2/releases/tag/v0.2.1
[0.2.0]: https://github.com/gladiaio/words2num2/releases/tag/v0.2.0
[0.1.1]: https://github.com/gladiaio/words2num2/releases/tag/v0.1.1
[0.1.0]: https://github.com/gladiaio/words2num2/releases/tag/v0.1.0
