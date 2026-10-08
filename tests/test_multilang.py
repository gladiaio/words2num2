# -*- coding: utf-8 -*-
"""Multi-language smoke tests via the generic num2words2-backed backend.

These tests require ``num2words2`` to be installed.
"""

from decimal import Decimal

import pytest

num2words2 = pytest.importorskip("num2words2")

from words2num2 import words2num, words2num_sentence


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("fr", "quarante-deux", 42),
        ("es", "cuarenta y dos", 42),
        ("de", "zweiundvierzig", 42),
        ("it", "quarantadue", 42),
        ("pt", "quarenta e dois", 42),
        ("nl", "tweeenveertig", 42),
        ("ru", "сорок два", 42),
        ("pl", "czterdziesci dwa", 42),
    ],
)
def test_multilang_42(lang, text, expected):
    assert words2num(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,n",
    [
        ("fr", 1),
        ("fr", 100),
        ("fr", 999),
        ("es", 1),
        ("es", 500),
        ("de", 1),
        ("de", 73),
        ("it", 1),
        ("it", 200),
        ("pt", 1),
        ("nl", 1),
    ],
)
def test_multilang_roundtrip(lang, n):
    """Forward num2words → back to int via words2num — must roundtrip."""
    words = num2words2.num2words(n, lang=lang)
    assert words2num(words, lang=lang) == n


# ---------------------------------------------------------------------------
# The accepted language set is derived from the renderer, so it cannot drift
# from the advertised one. Before that, `supported_langs()` listed 172 codes
# while the resolver knew 120: 28 raised NotImplementedError and 8 more were
# silently routed to a different language by the two-character prefix
# fallback. See gladiaio/words2num2#18.
# ---------------------------------------------------------------------------

from words2num2 import supported_langs  # noqa: E402

# gladiaio/num2words2#134-#142 moved five languages from a Latin
# transliteration to their own script. The reverse tables are built from the
# embedded `num2words2-core`, so whenever the installed `num2words2` and that
# core disagree on the script, the language cannot round-trip — see #14.
# This list may shrink; it must never grow.
_NATIVE_SCRIPT = {
    "ti": (0x1200, 0x137F),   # Ge'ez
    "ky": (0x0400, 0x04FF),   # Cyrillic
    "or": (0x0B00, 0x0B7F),   # Odia
    "ckb": (0x0600, 0x06FF),  # Arabic
    "kok": (0x0900, 0x097F),  # Devanagari
}
# Whether the embedded num2words2-core renders them in their own script.
# Flip in the change that bumps it.
_CORE_RENDERS_NATIVE = True


def _renders_native(lang):
    lo, hi = _NATIVE_SCRIPT[lang]
    return any(lo <= ord(ch) <= hi for ch in num2words2.num2words(1, lang=lang))


KNOWN_ROUNDTRIP_GAPS = {
    lang for lang in _NATIVE_SCRIPT if _renders_native(lang) != _CORE_RENDERS_NATIVE
}


def _roundtrip_param(lang):
    """One param per language, strict-xfail for the known gaps.

    strict=True makes this a ratchet in both directions: a gap language that
    starts working reports XPASS and fails the run, which is the prompt to
    delete it from KNOWN_ROUNDTRIP_GAPS. The list can only shrink.
    """
    if lang in KNOWN_ROUNDTRIP_GAPS:
        return pytest.param(
            lang,
            marks=pytest.mark.xfail(
                strict=True,
                reason="gladiaio/words2num2#14 — the installed num2words2 "
                       "and the embedded num2words2-core render this "
                       "language in different scripts",
            ),
        )
    return lang


@pytest.mark.slow
@pytest.mark.parametrize("lang", [_roundtrip_param(code) for code in sorted(supported_langs())])
def test_roundtrip_every_language(lang):
    """num2words2 -> words2num2 is the identity for every advertised language.

    This is simultaneously the registry test: a language that `supported_langs()`
    advertises but the resolver rejects raises NotImplementedError here.
    """
    for value in (0, 1, 11, 42, 100, 1000):
        words = num2words2.num2words(value, lang=lang)
        assert words2num(words, lang=lang) == value, f"{lang}: {value} -> {words!r}"


@pytest.mark.parametrize(
    "code",
    [
        # Each of these used to be swallowed by the two-character prefix
        # fallback and answered by an unrelated language.
        "ban",   # was -> ba  (Bashkir)
        "ceb",   # was -> ce  (Chechen)
        "cnh",   # was -> cn  (Chinese)
        "fil",   # was -> fi  (Finnish)
        _roundtrip_param("kok"),   # was -> ko  (Korean)
        "miz",   # was -> mi  (Maori)
        "pap",   # was -> pa  (Punjabi)
        "pli",   # was -> pl  (Polish)
        # Script variants that were collapsed onto their base language.
        "sr_Latn",
        "uz_Cyrl",
    ],
)
def test_codes_are_not_misresolved_to_another_language(code):
    """A known code must be answered by itself, not by a prefix match."""
    assert words2num(num2words2.num2words(42, lang=code), lang=code) == 42


@pytest.mark.parametrize(
    "text,expected",
    [
        # "ciento" is never rendered on its own by num2words (100 is "cien"),
        # so the reverse table had no entry for it and the sentence walker
        # could not open a run on it: "ciento 54 dólares".
        ("ciento cincuenta y cuatro dólares", "154 dólares"),
        ("ciento uno", "101"),
        ("cien", "100"),
        ("doscientos treinta", "230"),
        ("ciento veinte mil", "120000"),
    ],
)
def test_es_ciento_composes_in_sentence(text, expected):
    assert words2num_sentence(text, lang="es") == expected


def test_es_bare_ciento_is_one_hundred():
    assert words2num("ciento", lang="es") == 100


@pytest.mark.parametrize(
    "text,expected",
    [
        # Apocope inside a number ("un" for "uno" before a noun or a scale word):
        # num2words never renders it, so the walker stopped the run before it.
        ("cincuenta y un centavos", "51 centavos"),
        ("un mil treinta y cuatro dólares", "1034 dólares"),
        ("veintiún dólares", "21 dólares"),
        ("treinta y una llamadas", "31 llamadas"),
        # On its own it is an article.
        ("un momento por favor", "un momento por favor"),
        ("una llamada", "una llamada"),
        ("el 2 y cinco", "el 2 y 5"),
    ],
)
def test_es_apocope_inside_a_number(text, expected):
    assert words2num_sentence(text, lang="es") == expected


@pytest.mark.parametrize(
    "text,lang,expected",
    [
        # Numbers in figures follow the language's separators (#60).
        ("1.234", "de", 1234),
        ("1.234", "es", 1234),
        ("2,5", "de", Decimal("2.5")),
        ("1 234,5", "fr", Decimal("1234.5")),
        ("3.14", "fr", Decimal("3.14")),
    ],
)
def test_digit_literal_locale_separators(text, lang, expected):
    got = words2num(text, lang=lang)
    assert got == expected
    assert type(got) is type(expected)


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # Leading zeros / a fraction of any size (#52).
        ("fr", "trois virgule zéro vingt-cinq", "3,025"),
        ("fr", "trois virgule cent vingt-cinq", "3,125"),
        # The separator word derived from the renderer (#107).
        ("sv", "tre komma ett fyra", "3,14"),
        ("cs", "tři čárka jedna čtyři", "3,14"),
        ("hi", "तीन दशमलव एक चार", "3.14"),
        ("ko", "삼 점 일 사", "3.14"),
        ("en_IN", "three point one four", "3.14"),
        # Whole-word + denominator (#108).
        ("ru", "три целых четырнадцать сотых", "3,14"),
        ("hu", "három egész tizennégy század", "3,14"),
        ("fa", "سه و چهارده صدم", "3.14"),
        # Native spelling where the renderer writes "point" (#267).
        ("gl", "tres coma un catro", "3,14"),
    ],
)
def test_spoken_decimals_in_sentences(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # words2num reads a decimal in every language (#191).
        ("fr", "trois virgule un quatre", Decimal("3.14")),
        ("es", "tres punto uno cuatro", Decimal("3.14")),
        ("de", "drei Komma eins vier", Decimal("3.14")),
        ("ru", "три целых пять сотых", Decimal("3.05")),
        ("ja", "三点一四", Decimal("3.14")),
        ("tr", "üçvirgülondört", Decimal("3.14")),
    ],
)
def test_words2num_spoken_decimals(lang, text, expected):
    got = words2num(text, lang=lang)
    assert got == expected
    assert isinstance(got, Decimal)
