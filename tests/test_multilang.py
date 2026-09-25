# -*- coding: utf-8 -*-
"""Multi-language smoke tests via the generic num2words2-backed backend.

These tests require ``num2words2`` to be installed.
"""

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
