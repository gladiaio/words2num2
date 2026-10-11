# -*- coding: utf-8 -*-
"""The date pass of sentence mode (``words2num_sentence(..., date=True)``).

A day + month (+ year) is written the way the language writes a date. Off by
default: the output is the plain sentence-mode one.
"""

import pytest

from words2num2 import words2num_sentence


def dates(text, lang):
    return words2num_sentence(text, lang=lang, date=True)


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # en (US): month first, comma before the year, no ordinal suffix.
        ("en", "on march twelfth twenty twenty six", "on March 12, 2026"),
        ("en", "the first of march", "March 1"),
        ("en", "on the twenty first of june nineteen ninety.", "on June 21, 1990."),
        ("en", "december one twenty twenty five", "December 1, 2025"),
        ("en", "march the twelfth", "March 12"),
        ("en", "may the fourth", "May 4"),
        ("en", "june twelve", "June 12"),
        ("en", "in march twenty twenty six", "in March 2026"),
        ("en", "see you march twelfth, ok", "see you March 12, ok"),
        # en (GB, AU...): day first.
        ("en_GB", "the twelfth of march twenty twenty six", "12 March 2026"),
        ("en_AU", "on march twelfth", "on 12 March"),
        # fr: "1er", lower-case month.
        ("fr", "le premier avril deux mille vingt-six", "le 1er avril 2026"),
        ("fr", "le douze Mars", "le 12 mars"),
        ("fr", "le vingt et un juin", "le 21 juin"),
        # de: "12.", capitalised month.
        ("de", "am zwölften märz zweitausendsechsundzwanzig", "am 12. März 2026"),
        ("de", "am ersten Mai", "am 1. Mai"),
        # es: no ordinal mark; pt keeps "1º".
        ("es", "el primero de mayo", "el 1 de mayo"),
        ("es", "el doce de marzo de dos mil veintiséis", "el 12 de marzo de 2026"),
        ("pt", "doze de março de dois mil e vinte e seis", "12 de março de 2026"),
        ("it", "il primo maggio", "il 1º maggio"),
        ("it", "il dodici marzo duemilaventisei", "il 12 marzo 2026"),
        ("nl", "twaalf maart tweeduizend zesentwintig", "12 maart 2026"),
    ],
)
def test_date_written_form(lang, text, expected):
    assert dates(text, lang) == expected


@pytest.mark.parametrize(
    "lang,text",
    [
        # Month names that are everyday words need more than a bare number.
        ("en", "I may two things"),
        ("en", "they march twelve miles"),
        ("en", "march twelve"),
        ("en", "you may go"),
        ("en", "twelve people"),
        ("en", "page twelve of the march report"),
        ("fr", "douze personnes"),
        # A language with no date style passes through.
        ("pl", "dwunastego marca"),
    ],
)
def test_no_date_no_change(lang, text):
    assert dates(text, lang) == words2num_sentence(text, lang=lang)


def test_off_by_default():
    assert words2num_sentence("march twelfth") == "march 12th"


def test_date_and_currency_together():
    out = words2num_sentence(
        "on march twelfth I paid ten dollars and fifty cents", currency=True, date=True
    )
    assert out == "on March 12 I paid $10.50"


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # All in numbers: the region's order and separator.
        ("en", "born on three twelve twenty twenty six", "born on 3/12/2026"),
        ("en_GB", "three twelve twenty twenty six", "03/12/2026"),
        ("en", "oh three slash twelve slash twenty twenty six.", "3/12/2026."),
        ("fr", "né le douze trois deux mille vingt-six", "né le 12/03/2026"),
        ("de", "am zwölften dritten zweitausendsechsundzwanzig", "am 12.03.2026"),
        ("es", "el doce del tres de dos mil veintiséis", "el 12/03/2026"),
        ("en", "two twenty nine twenty twenty four", "2/29/2024"),
        # Not a calendar date, no year: left as it is.
        ("en", "thirteen twelve twenty twenty six", "13 12 2026"),
        ("en", "two twenty nine twenty twenty five", "2 29 2025"),
        ("en", "three twelve", "3 12"),
    ],
)
def test_numeric_date(lang, text, expected):
    assert dates(text, lang) == expected


def test_region_overrides_the_language_code():
    text = "three twelve twenty twenty six"
    assert words2num_sentence(text, date=True, region="GB") == "03/12/2026"
    assert (
        words2num_sentence("on march twelfth", date=True, region="AU") == "on 12 March"
    )
    assert (
        words2num_sentence("on march twelfth", lang="en_GB", date=True, region="US")
        == "on March 12"
    )


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("en", "on march twelfth twenty twenty six", "on 2026-03-12"),
        ("en", "three twelve twenty twenty six", "2026-03-12"),
        ("fr", "le premier avril deux mille vingt-six", "le 2026-04-01"),
        ("es", "el doce de setiembre de dos mil veintiséis", "el 2026-09-12"),
        ("en", "the first of march", "March 1"),  # no year: written
    ],
)
def test_iso(lang, text, expected):
    assert words2num_sentence(text, lang=lang, date=True, date_format="iso") == expected


def test_unknown_date_format():
    with pytest.raises(NotImplementedError):
        words2num_sentence("march twelfth", date=True, date_format="roman")
