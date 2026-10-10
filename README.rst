words2num2 — words to numbers
=============================

.. image:: https://img.shields.io/pypi/v/words2num2.svg
   :target: https://pypi.python.org/pypi/words2num2
   :alt: PyPI version

.. image:: https://img.shields.io/pypi/pyversions/words2num2.svg
   :target: https://pypi.python.org/pypi/words2num2
   :alt: Python versions

.. image:: https://img.shields.io/pypi/dm/words2num2.svg
   :target: https://pypi.python.org/pypi/words2num2
   :alt: Downloads / month

.. image:: https://img.shields.io/pypi/status/words2num2.svg
   :target: https://pypi.python.org/pypi/words2num2
   :alt: Package status

.. image:: https://img.shields.io/aur/version/python-words2num2.svg
   :target: https://aur.archlinux.org/packages/python-words2num2
   :alt: AUR version

.. image:: https://github.com/gladiaio/words2num2/workflows/CI/badge.svg
   :target: https://github.com/gladiaio/words2num2/actions/workflows/ci.yml
   :alt: CI

.. image:: https://github.com/gladiaio/words2num2/workflows/Lint/badge.svg
   :target: https://github.com/gladiaio/words2num2/actions/workflows/lint.yml
   :alt: Lint

.. image:: https://github.com/gladiaio/words2num2/workflows/CodeQL/badge.svg
   :target: https://github.com/gladiaio/words2num2/actions/workflows/codeql-analysis.yml
   :alt: CodeQL

.. image:: https://github.com/gladiaio/words2num2/workflows/E2E%20Tests/badge.svg
   :target: https://github.com/gladiaio/words2num2/actions/workflows/e2e-tests.yml
   :alt: E2E Tests

.. image:: https://coveralls.io/repos/github/gladiaio/words2num2/badge.svg?branch=main
   :target: https://coveralls.io/github/gladiaio/words2num2?branch=main
   :alt: Coverage

.. image:: https://img.shields.io/github/v/release/gladiaio/words2num2.svg
   :target: https://github.com/gladiaio/words2num2/releases
   :alt: Latest release

.. image:: https://img.shields.io/github/last-commit/gladiaio/words2num2.svg
   :target: https://github.com/gladiaio/words2num2/commits/main
   :alt: Last commit

.. image:: https://img.shields.io/github/issues/gladiaio/words2num2.svg
   :target: https://github.com/gladiaio/words2num2/issues
   :alt: Issues

.. image:: https://img.shields.io/badge/license-LGPL--2.1-blue.svg
   :target: https://github.com/gladiaio/words2num2/blob/main/COPYING
   :alt: License

The inverse of `num2words2 <https://github.com/gladiaio/num2words2>`_.

``words2num2`` parses spoken-form numbers — ``"forty-two"``,
``"trois cent quatre"``, ``"二十三"`` — and returns numeric values.
It mirrors num2words2's locale list (**100+ languages, 172 language
keys**) and adds a free-text *auto-parse* mode that handles
currencies, units, configurable thousands/decimal separators, and
ASR/LLM-style mixed text.

It is **Rust-powered with a thin Python binder**. The whole parsing engine —
the English grammar, the generic reverse-lookup backend, the sentence walker,
number-format parsing and auto-parse — runs in a compiled Rust core
(PyO3/``abi3``); Python only shapes arguments and results. The core embeds the
num2words2 conversion engine natively, so ``words2num2`` is **self-contained
and has no runtime dependencies**, while running several times faster than the
former pure-Python implementation. Output is unchanged — validated against a
frozen corpus of ~11,000 round-trip cases.

The project is hosted on GitHub_, and the full documentation is available
in the Wiki_. Contributions are welcome.

.. _GitHub: https://github.com/gladiaio/words2num2
.. _Wiki: https://github.com/gladiaio/words2num2/wiki
.. _GitHub Releases: https://github.com/gladiaio/words2num2/releases

Why this library
----------------

Existing inverse libraries are usually English-only, lack a sentence
mode, and don't compose with the locale defaults you already use for
the forward direction. ``words2num2``:

* Accepts the **same locale codes** as ``num2words2`` so the two
  libraries are drop-in inverses of each other.
* Has a hand-written grammar parser for **English** and a generic
  reverse-lookup backend that auto-derives ``{words → number}``
  tables from ``num2words2`` for **every other locale** out of the box.
* Walks free text via ``words2num_sentence`` / ``auto_parse_sentence``
  — useful when post-processing ASR transcripts, LLM output, or
  user-typed forms that mix words and digits.
* Handles currency symbols (``$ € £ ¥ ₹ ₽ ₩ ₺``), ISO codes
  (``USD/EUR/...``), scale shortcuts (``$5m`` → 5,000,000), units
  (length / mass / temperature / time / area / volume / percent), and
  CLDR-style number formats per locale.
* Pluralizes long-form units in expand mode (``5 dollars`` /
  ``1 dollar``, ``5 feet`` / ``1 foot``, ``5 yen`` / ``1 yen``).

Performance
-----------

The parsing engine is compiled Rust (PyO3/``abi3``), so there is no
Python-level tokenising or table walking on the hot path. Typical native
throughput (Apple M-series, nanoseconds per call, after warmup):

.. list-table::
   :header-rows: 1
   :widths: 60 20

   * - Operation
     - ns/call
   * - English cardinal (``"eight thousand seven hundred sixty-five"``)
     - ~5,900
   * - English ordinal (``"forty-second"``)
     - ~4,100
   * - French cardinal (``"trois cent quatre"``)
     - ~1,900
   * - Russian cardinal (``"сорок два"``)
     - ~1,700
   * - ``parse_number_string("1.234.567,89", lang="de")``
     - ~240
   * - ``auto_parse("$12,345.00")``
     - ~990

For the 100+ generic locales, the first call in a given language builds the
``{words → number}`` reverse table once (~35 ms) and caches it in the core;
every call after that is a native lookup. Output is byte-for-byte identical to
the former pure-Python implementation, validated against a frozen corpus of
~11,000 round-trip cases and the full test suite.

Installation
------------

**pip** (Python 3.10+)::

    pip install words2num2

**cargo** (Rust) — the compiled core is published on crates.io as
`words2num2-core <https://crates.io/crates/words2num2-core>`_::

    cargo add words2num2-core

That's it — the wheel bundles the Python binder and the compiled Rust
extension, and ``words2num2`` has **no runtime dependencies** (the num2words2
conversion engine is compiled in, so you do *not* need the num2words2 package).

Prebuilt wheels are published for:

* **Linux** — x86_64 and aarch64 (manylinux2014)
* **macOS** — Apple Silicon (arm64) and Intel (x86_64)
* **Windows** — x86_64

The wheels are CPython ``abi3`` wheels: one per platform serves every CPython
3.10+. **PyPy** (3.11+) is supported from source only: it cannot load ``abi3``
wheels, so pip builds the source distribution, and CI runs the full test suite
on PyPy that way. On any other platform (or an older release without a
matching wheel), pip also falls back to the source distribution and builds the
extension locally — this needs a **stable Rust toolchain**
(`rustup <https://rustup.rs>`_); nothing else.

**Arch Linux / Manjaro (AUR):**

.. code-block:: bash

    # With an AUR helper
    yay -S python-words2num2
    paru -S python-words2num2

    # Or manually
    git clone https://aur.archlinux.org/python-words2num2.git
    cd python-words2num2
    makepkg -si

**From source** (needs a stable Rust toolchain and `maturin
<https://www.maturin.rs>`_)::

    git clone https://github.com/gladiaio/words2num2
    cd words2num2
    pip install -e .          # builds the Rust extension via maturin
    # or, to produce a wheel:  maturin build --release

Wheels for every version are also attached to the `GitHub Releases`_ page.

Quickstart
----------

.. code-block:: python

    >>> from words2num2 import words2num, words2num_sentence
    >>> words2num("forty-two")
    42
    >>> words2num("one thousand two hundred thirty-four")
    1234
    >>> words2num("minus seven")
    -7
    >>> words2num("three point one four")
    Decimal('3.14')
    >>> words2num("nineteen ninety nine", to="year")
    1999
    >>> words2num("twenty-first", to="ordinal")
    21
    >>> words2num("quarante-deux", lang="fr")
    42
    >>> words2num("zweiundvierzig", lang="de")
    42
    >>> words2num("сорок два", lang="ru")
    42

    >>> words2num_sentence("I bought twenty-three apples and fourteen pears.")
    'I bought 23 apples and 14 pears.'

Sentence mode
-------------

``words2num_sentence`` is built for transcripts (ASR, LLM output): it reads
what the speaker meant and leaves everything else alone.

.. code-block:: python

    >>> words2num_sentence("due on the fifteenth of march, twenty twenty five")
    'due on the 15th of march, 2025'
    >>> words2num_sentence("call eight five zero eight two oh nine oh nine five")
    'call 8508209095'
    >>> words2num_sentence("one thousand three hundred fifty five dollars and two point five percent")
    '1355 dollars and 2.5 percent'
    >>> words2num_sentence("no one answered, wait a second, one of them owes me a hundred bucks")
    'no one answered, wait a second, one of them owes me 100 bucks'
    >>> words2num_sentence("un solde de deux cents euros et vingt et une personnes", lang="fr")
    'un solde de 200 euros et 21 personnes'
    >>> words2num_sentence("le vingt-troisième jour, trois virgule cinq pour cent", lang="fr")
    'le 23e jour, 3,5 pour cent'
    >>> words2num_sentence("el primero de mayo, cincuenta y un centavos", lang="es")
    'el 1º de mayo, 51 centavos'
    >>> words2num_sentence("am zweiten April, dreiundzwanzigtausend Euro", lang="de")
    'am 2. April, 23000 Euro'

What it reads:

* **Cardinals and compounds** across scale words (``"un million deux cent
  mille"``, ``"dreiundzwanzigtausend"``, ``"ventitremila"``), with the
  spoken connectors (``"and"``, ``"et"``, ``"y"``) and the plural or
  feminine forms speech adds (``"deux cents"``, ``"quatre vingts"``,
  ``"cinquante et une"``).
* **Spoken years** (``"nineteen ninety nine"``, ``"twenty oh five"``, nl
  ``"negentien negenennegentig"``) and **digit strings** read one digit at a
  time in every language (phone numbers, ZIP codes, ``"double zero
  seven"``), leading zeros kept.
* **Decimals** the way num2words writes them in each language, read from its
  own ``to_cardinal_float`` output: a separator word (en ``"point"``, fr
  ``"virgule"``, es ``"coma"`` / ``"punto"``, sv ``"komma"``, hi ``"दशमलव"``,
  ko ``"점"``), or a whole-word and a denominator (ru ``"три целых
  четырнадцать сотых"``, hu ``"három egész tizennégy század"``), written with
  the locale's decimal character (``"3,5"``). The fraction may be digits
  read one by one or one number below 1000 (fr ``"trois virgule zéro
  vingt-cinq"`` -> ``"3,025"``). ``words2num`` reads them too, as a
  ``Decimal``, including separators glued into one word (ja ``"三点一四"``,
  tr ``"üçvirgülondört"``).
* **Ordinals**, written in the language's figures: ``21st``, fr ``1er`` /
  ``2e``, es ``3º`` / ``20ª``, de ``2.``, nl ``15e``.
* **Percent phrases**: ``"vingt pour cent"`` -> ``"20 pour cent"`` (the
  ``cent`` after ``pour`` is not 100).

What it leaves in words, on purpose:

* the number 1 when it is an article or a pronoun (``"one of them"``,
  ``"no one"``, ``"un instant"``, ``"um momento"``), unless something counts
  it: a unit or currency (``"one dollar"``, ``"une heure"``), a label
  (``"press one"``, ``"option one"``, ``"tapez un"``), a month, a list;
* ``"first"`` / ``"second"`` and their translations away from a date
  (``"first of all"``, ``"wait a second"``, ``"la première fois"``);
* fractions (``"two thirds"``, ``"a fifth of"``, ``"deux tiers"``) and
  quantities (``"a couple of thousand"``, ``"hundreds of"``, ``"des
  millions"``, ``"half a million"``);
* a decade (``"the nineteen nineties"``), a scale word after a decimal
  (``"2.5 million"``), a dangling connector (``"five point"``, ``"one and a
  half"``);
* anything that already holds a digit (``"850-820-9095"``, ``"$426"``).

**Currency fold** (``currency=True``, off by default): an amount spoken as
number + currency word (+ connector + subunit) is written the way the language
writes that currency — the currency picks the symbol and its side, the language
the separators (en ``1,355.28``, fr ``1 355,28``, de/es/it/pt/nl ``1.355,28``;
dollars and pesos in Spanish keep the Latin-American ``$1,234.56``).

.. code-block:: python

    >>> words2num_sentence("one thousand three hundred fifty five dollars and twenty eight cents", currency=True)
    '$1,355.28'
    >>> words2num_sentence("ninety nine cents, twenty quid, ten thousand yen", currency=True)
    '$0.99, £20, ¥10,000'
    >>> words2num_sentence("quarante-trois dollars et vingt centimes", lang="fr", currency=True)
    '43,20 $'
    >>> words2num_sentence("mille deux cents euros cinquante", lang="fr", currency=True)
    '1 200,50 €'
    >>> words2num_sentence("ciento cincuenta y cuatro dólares con noventa y dos centavos", lang="es", currency=True)
    '$154.92'
    >>> words2num_sentence("dreiundzwanzig euro und fünfzig cent", lang="de", currency=True)
    '23,50 €'
    >>> words2num_sentence("duzentos reais e cinquenta centavos", lang="pt", currency=True)
    'R$ 200,50'
    >>> words2num_sentence("twintig euro vijftig", lang="nl", currency=True)
    '€ 20,50'

Subunits after a connector (``"and"``, ``"et"``, ``"con"``, ``"und"``, ``"e"``)
or bare (``"two euros fifty"``), or on their own (``"vingt centimes"`` ->
``"0,20 €"``: the language's default currency — USD for en/es, BRL for pt, EUR
elsewhere). No decimals unless a subunit was spoken (``"twelve dollars"`` ->
``"$12"``); JPY and KRW never carry any. A currency word without a number in
front stays a word (``"des euros"``, ``"a couple of dollars"``), and so does an
amount on a scale word (``"2.5 million dollars"``). Currencies: USD, EUR, GBP,
JPY, CNY, CHF, CAD (fr ``5 $ CA``, the OQLF form), AUD, NZD, MXN and the
unqualified peso (``$``), BRL, INR, KRW, RUB, TRY, PLN, SEK/NOK/DKK, XOF, MAD,
ZAR, with their names and subunits in en, fr, es, de, it, pt, nl and ca —
every name num2words writes for them, plus the colloquial ones (``bucks``,
``quid``). A subunit of 100 or more carries (``"one hundred fifty cents"`` ->
``"$1.50"``); a spoken fraction longer than the currency's minor unit, or a
subunit after an amount that was not folded, is left as said rather than
truncated or given a currency nobody named. ``"pounds of"`` is a weight.

``words2num(text, to="currency")`` returns the amount itself as a ``Decimal``,
unit plus subunit, for any language whose num2words currency names it knows::

    >>> words2num("twelve euros, thirty-four cents", to="currency")
    Decimal('12.34')
    >>> words2num("zwei Euro", lang="de", to="currency")
    Decimal('2')

Text with no currency word is read as a plain cardinal; text with one that is
not an amount (``"twelve euros and change"``) raises ``Words2NumError``.

Auto-parse mode
---------------

``auto_parse`` extracts a numeric value plus its unit from any free-text
expression. ``auto_parse_sentence`` walks running text and replaces every
quantity in place. It supports configurable thousands/decimal
separators per locale, currency symbols and ISO codes, scale shortcuts,
SI/imperial units, percent, and disambiguation hints.

.. code-block:: python

    >>> from words2num2 import auto_parse, auto_parse_sentence

    # Currencies
    >>> auto_parse("$12,345.00")
    Quantity(value=Decimal('12345.00'), unit='USD', kind='currency', confidence=1.0)
    >>> auto_parse("$5m").value
    5000000
    >>> auto_parse("12,50 €", lang="de").value
    Decimal('12.50')

    # Units
    >>> auto_parse("5cm")
    Quantity(value=5, unit='cm', kind='length', confidence=1.0)
    >>> auto_parse("20°C").kind
    'temperature'
    >>> auto_parse("forty-two kg").value
    42

    # Configurable separators
    >>> auto_parse("1.234,56", lang="de").value
    Decimal('1234.56')
    >>> auto_parse("1 234,56", lang="fr").value
    Decimal('1234.56')

    # Disambiguation for ambiguous unit tokens
    >>> auto_parse("5m", prefer={"m": "mile"}).unit_long
    'mile'

    # Sentence mode
    >>> auto_parse_sentence("Pay $12.50 for 5kg of apples at -5°C.")
    'Pay 12.5 USD for 5 kg of apples at -5 °C.'

    # Expand mode renders the long unit form, with English plural rules
    >>> auto_parse_sentence("Pay $12.50 for 5kg.", expand=True)
    'Pay 12.5 dollars for 5 kilograms.'
    >>> auto_parse_sentence("Pay $1.00 for 1kg.", expand=True)
    'Pay 1 dollar for 1 kilogram.'
    >>> auto_parse_sentence("5 ft and 1 ft.", expand=True)
    '5 feet and 1 foot.'

Configurable number formats
---------------------------

``parse_number_string`` is the primitive used by ``auto_parse`` for
digit-form numbers. You can call it directly with explicit separators
or rely on per-locale CLDR-style defaults:

.. code-block:: python

    >>> from words2num2 import parse_number_string

    >>> parse_number_string("12,345.67")                              # auto-detect
    Decimal('12345.67')
    >>> parse_number_string("12.345,67", lang="de")                   # German defaults
    Decimal('12345.67')
    >>> parse_number_string("1 234,56", lang="fr")                    # French defaults (NBSP)
    Decimal('1234.56')
    >>> parse_number_string("3.14", lang="fr")                        # a dot decimal anywhere
    Decimal('3.14')
    >>> parse_number_string("12'345.67", thousands_sep="'", decimal_sep=".")  # Swiss
    Decimal('12345.67')
    >>> parse_number_string("1_234.56", thousands_sep="_")            # programmer
    Decimal('1234.56')

A decimal is an exact ``Decimal``; a whole number is an ``int``.

The locale defaults table covers 50+ locales: English/CJK use comma
thousands and period decimal; French uses non-breaking-space + comma;
Swiss French uses apostrophe + period; German/Spanish/Italian/Portuguese/
Dutch/Romanian use period + comma; Russian/Scandinavian/Slavic use space
+ comma. See ``NUMBER_FORMAT_DEFAULTS`` in
``rust/words2num2-core/src/w2n_formats.rs`` for the full table. A locale
is read strictly — its thousands separator only between groups of exactly
three digits — and anything that does not fit is auto-detected.

Auto-detection heuristic (when no override, no locale match, or the locale
reading does not fit):

1. Spaces, NBSP, apostrophes (``'``, ``’``) and underscore are always
   thousands.
2. If both ``.`` and ``,`` appear, the rightmost one is the decimal and the
   other groups thousands in threes.
3. If one separator appears several times, it is thousands — in groups of
   three, or the string is not a number (``"1.5.2026"`` raises).
4. If one separator appears once it is a decimal (``"3.14"``, ``"1,5"``,
   ``"1234.567"``), except 1-3 digits + separator + exactly 3 digits
   (``"1.234"``): there the locale's decimal separator decides, and with no
   locale it is a decimal.

Command line
------------

.. code-block:: bash

    $ words2num2 "forty-two"
    42
    $ words2num2 "trois cent quatre" --lang=fr
    304
    $ words2num2 --lang fr trois cent quatre
    304
    $ words2num2 "twenty-third" --to=ordinal
    23
    $ words2num2 "forty zoot"
    words2num2: error: unrecognized token 'zoot' in 'forty zoot'

An unparseable input or an unsupported ``--lang``/``--to`` prints one line on
stderr and exits with status 1.

Supported locales
-----------------

``words2num2`` accepts every language key of the embedded num2words2
renderer — **172 keys**, as returned by ``supported_langs()``:

  af, am, ar, as, az, ba, ban, be, bg, bm, bn, bo, br, bs, ca, ce, ceb,
  ckb, cn, cnh, cs, cy, cz, da, de, dk, dv, el, en, en_AERO,
  en_Aero_FAA, en_Aero_ICAO, en_Aero_NATO, en_Aero_USN, en_Aero_US_Army,
  en_Aero_US_Navy, en_IN, en_NE, en_NG, en_NP, en_aero_icao,
  en_x_aero_icao, eo, es, es_CO, es_CR, es_GT, es_HN, es_NI, es_VE, et,
  eu, fa, ff, fi, fil, fo, fr, fr_BE, fr_CH, fr_DZ, gl, gu, ha, haw, he,
  hi, hmn, hr, ht, hu, hy, id, is, it, ja, jp, jv, jw, ka, ki, kk, km,
  kn, ko, kok, ksw, ku, ky, kz, la, lb, lg, lij, ln, lo, lt, lus, lv,
  mg, mi, miz, mk, ml, mn, mr, ms, mt, my, nb, ne, nl, nn, no, oc, om,
  or, pa, pap, pl, pli, ps, pt, pt_BR, rm, rm_puter, rm_surmiran,
  rm_sursilv, rm_sutsilv, rm_vallader, ro, ru, rw, sa, sd, si, sk, sl,
  sn, so, sq, sr, sr_Cyrl, sr_Latn, su, sv, sw, ta, te, tet, tg, th, ti,
  tk, tl, tr, tt, uk, ur, uz, uz_Cyrl, uz_cyr, vi, wo, xh, yi, yo, zh,
  zh_CN, zh_HK, zh_TW, zu

Codes are case-insensitive, ``-`` and ``_`` are interchangeable, unknown
region subtags fall back to the base language (``en-US`` → ``en``) and ISO
639-2 codes are accepted (``eng``, ``fra``). Language names (``"swedish"``)
are not, and raise ``NotImplementedError``.

Aliases: ``jp`` → ``ja``, ``cn`` → ``zh_CN``.

Wiki
----
For the full documentation, including installation, API details, CLI
usage, supported locales, sentence conversion, auto-parse behavior, and
migration guidance, please check the Wiki_. Feel free to propose wiki
enhancements.

Conversion types
~~~~~~~~~~~~~~~~

The ``to=`` parameter (``words2num`` and ``words2num_sentence``) accepts
``cardinal``, ``ordinal``, ``ordinal_num``, ``year``, ``currency`` and
``digits`` (listed in ``words2num2.CONVERTER_TYPES``); anything else raises
``NotImplementedError``. ``digits`` reads a number spoken digit by digit and
concatenates it (``"two seven five"`` → ``275``). It returns an ``int``, so
leading zeros are dropped (``"zero seven"`` → ``7``); sentence mode keeps them
(``words2num_sentence("zero seven")`` → ``"07"``).

How it works
------------

All parsing lives in the compiled Rust core (``rust/words2num2-core``,
also published on crates.io); the ``words2num2`` Python package is a thin
binder over it.

* English has a hand-written recursive-descent parser that handles
  cardinals, ordinals, decimals, negatives, scale words to *centillion*,
  year mode, "and" connectors, and hyphenation.
* Every other language is read through a reverse lookup table, built lazily
  on first use by rendering ``-1..10000`` with the embedded num2words2
  renderer (``num2words2-core``) and inverting it. Larger numbers are
  composed from the language's scale words (thousand, million, …), derived
  the same way.

To improve a language, change the core — see ``CONTRIBUTING.md``.

Public API
----------

================================== =================================================
Function / class                   Purpose
================================== =================================================
``words2num(text, lang, to)``      Parse a single word-form number.
``words2num_sentence(text, ...)``  Replace every word-number in running text.
``auto_parse(text, ...)``          Parse a single quantity (number + unit).
``auto_parse_sentence(text, ...)`` Replace every quantity in running text.
``parse_number_string(text, ...)`` Digit-form parser with separators.
``normalize(text)``                The text normalization applied before lookup.
``supported_langs()``              The accepted language keys.
``CONVERTER_TYPES``                The accepted ``to=`` values.
``Quantity``                       Dataclass returned by ``auto_parse``.
``Words2NumError``                 Raised when input cannot be parsed.
================================== =================================================

See `REFERENCE.md` for the full API reference with parameters,
return types, and examples.

Development
-----------

.. code-block:: bash

    git clone https://github.com/gladiaio/words2num2
    cd words2num2
    make install-dev
    make test          # pytest
    make lint          # black --check + isort --check + flake8 + mypy (what CI runs)
    make format        # apply black + isort

Building from a checkout compiles the Rust core, so a stable Rust toolchain
is needed (``make install-dev`` runs ``pip install -e .`` through maturin).

Releasing
~~~~~~~~~

Releases are cut by bumping the version, not by hand-tagging:

1. Bump ``version`` in ``pyproject.toml`` **and** ``words2num2/_version.py``
   (they must agree) and merge to ``main``.
2. ``auto-release.yml`` sees that the version is not on PyPI, tags the commit
   ``vX.Y.Z`` (or reuses the tag if an earlier attempt failed), and dispatches
   ``release.yml``, ``crates.yml`` and ``aur-publish.yml`` on it.
3. ``release.yml`` checks that the tag, ``pyproject.toml`` and ``_version.py``
   agree, builds the wheels and the sdist, installs each native wheel and the
   sdist in a clean environment and runs the test suite against them, and only
   then publishes to PyPI (Trusted Publishing, no token in CI) and creates the
   GitHub Release.

A failed release is retried by re-running ``auto-release.yml`` (Actions →
Auto release → Run workflow): PyPI, not the tag, decides what is released.
Pushing a ``vX.Y.Z`` tag by hand still runs ``release.yml``, under the same
version check.

Changelog
---------

See `CHANGELOG.md`.

License
-------

LGPL-2.1, mirroring ``num2words2``. See `COPYING`.
