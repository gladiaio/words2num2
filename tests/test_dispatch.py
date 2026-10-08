# -*- coding: utf-8 -*-
"""Tests for the locale dispatch (now entirely in the Rust core)."""
from decimal import Decimal

import pytest

from words2num2 import Words2NumError, words2num


def test_unknown_lang_raises():
    with pytest.raises(NotImplementedError):
        words2num("forty-two", lang="xx")


def test_dash_normalization():
    assert words2num("forty-two", lang="en") == 42
    # Hyphenated locale forms like en-US fall back to "en".
    assert words2num("forty-two", lang="en-US") == 42


@pytest.mark.parametrize(
    "text,lang,expected",
    [
        # to="currency" returns the amount, unit plus subunit (#189).
        ("twelve euros, thirty-four cents", "en", Decimal("12.34")),
        ("twelve dollars and fifty cents", "en", Decimal("12.50")),
        ("douze euros et trente-quatre centimes", "fr", Decimal("12.34")),
        ("douze euros et cinquante centimes", "fr", Decimal("12.5")),
        ("zwei Euro", "de", Decimal("2")),
        ("zwölf Euro", "de", Decimal("12")),
        ("ninety nine cents", "en", Decimal("0.99")),
        ("doce dólares con treinta y cuatro centavos", "es", Decimal("12.34")),
    ],
)
def test_to_currency_amount(text, lang, expected):
    got = words2num(text, lang=lang, to="currency")
    assert got == expected
    assert isinstance(got, Decimal)


def test_to_currency_without_a_currency_word_is_the_cardinal():
    assert words2num("forty-two", to="currency") == 42


def test_to_currency_that_is_not_an_amount_raises():
    with pytest.raises(Words2NumError):
        words2num("twelve euros and change", to="currency")
