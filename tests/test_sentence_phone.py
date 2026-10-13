# -*- coding: utf-8 -*-
"""Phone numbers in sentence mode (``words2num_sentence(..., phone=True)``).

Each dictated phone number is written the way its country writes it. A spoken
country code decides the country; otherwise ``region``, the region of
``lang``, or the regions where the language is spoken, the main one first.
"""

import pytest

from words2num2 import words2num_sentence


def phone(text, lang="en", region=None):
    return words2num_sentence(text, lang=lang, phone=True, region=region)


FR_MOBILE = "zéro six douze trente-quatre cinquante-six soixante-dix-huit"


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("fr", "mon numéro est " + FR_MOBILE, "mon numéro est 06 12 34 56 78"),
        (
            "fr",
            "zéro six, douze, trente-quatre, cinquante-six, soixante-dix-huit.",
            "06 12 34 56 78.",
        ),
        (
            "en",
            "call me at two one two four five six seven eight nine oh",
            "call me at (212) 456-7890",
        ),
        (
            "en_GB",
            "oh seven four double oh one two three four five six",
            "07400 123456",
        ),
        ("de", "null drei null eins zwei drei vier fünf sechs", "030 123456"),
        ("es", "seis uno dos tres cuatro cinco seis siete ocho", "612 34 56 78"),
        (
            "fr",
            "plus trente-trois six douze trente-quatre cinquante-six "
            "soixante-dix-huit",
            "+33 6 12 34 56 78",
        ),
        (
            "en",
            "plus forty four seven four double oh one two three four five six",
            "+44 7400 123456",
        ),
    ],
)
def test_phone(lang, text, expected):
    assert phone(text, lang) == expected


def test_region():
    assert phone(FR_MOBILE, "fr", region="CH") == "061 234 56 78"
    assert (
        phone("oh seven four double oh one two three four five six", region="GB")
        == "07400 123456"
    )
    # A region the number does not fit: the digits stay.
    assert phone("zero six one two three four five six seven eight", region="US") == (
        "0612345678"
    )


@pytest.mark.parametrize(
    "lang,text",
    [
        ("en", "it was nineteen eighty four twenty twenty six"),
        ("en", "I have five apples and three pears"),
        ("en", "press one two three"),
        ("en", "five five five two three four five six seven eight"),
        ("fr", "douze personnes"),
    ],
)
def test_not_a_phone(lang, text):
    assert phone(text, lang) == words2num_sentence(text, lang=lang)


def test_off_by_default():
    assert words2num_sentence(FR_MOBILE, lang="fr") == "06 12 34 56 78"
    assert (
        words2num_sentence("two one two four five six seven eight nine oh")
        == "2124567890"
    )


@pytest.mark.parametrize(
    "text, lang, kwargs, expected",
    [
        # Groups from the trunk prefix still join.
        ("appelez le 06, 12, 34, 56, 78", "fr", {}, "appelez le 06 12 34 56 78"),
        # Commas splitting it as its plan writes it, or between digits
        # dictated one by one; "+" across a list (#1179 audit).
        (
            "call two one two, five five five, one two three four",
            "en",
            {},
            "call (212) 555-1234",
        ),
        (
            "llama al seis uno dos, tres cuatro, cinco seis, siete ocho",
            "es",
            {},
            "llama al 612 34 56 78",
        ),
        (
            "llama al nueve uno, uno dos tres, cuatro cinco, seis siete",
            "es",
            {},
            "llama al 911 23 45 67",
        ),
        ("ext 2, +1, 212, 555, 1234", "en", {}, "ext 2, +1 212-555-1234"),
        ("ruf 030 12 2345 an", "de", {}, "ruf 030 122345 an"),
        # A number with groups of three (#1182); a 0-prefixed number holding
        # "dd mm yyyy" is read whole (#1181).
        (
            "ligue para nove um dois, três quatro cinco, seis sete oito",
            "pt",
            {},
            "ligue para 912 345 678",
        ),
        ("appelez le 06 12 2024 56", "fr", {"date": True}, "appelez le 06 12 20 24 56"),
        # A date and a figure after it; a phone after a list figure (#1179
        # audit).
        ("on 12 05 1984 12", "en", {"date": True}, "on 12/5/1984 12"),
        (
            "né le 23 10 2026, 300 personnes",
            "fr",
            {"date": True},
            "né le 23/10/2026, 300 personnes",
        ),
        ("call 2, 212 555 1234", "en", {}, "call 2, (212) 555-1234"),
        ("212 555 1234, 415 867 5309", "en", {}, "(212) 555-1234, (415) 867-5309"),
        # A date is read first, or left alone (#940).
        (
            "né le vingt-trois dix deux mille vingt-six",
            "fr",
            {"date": True},
            "né le 23/10/2026",
        ),
        ("né le vingt-trois dix deux mille vingt-six", "fr", {}, "né le 23 10 2026"),
        ("on 02 20 1984", "en", {"date": True}, "on 2/20/1984"),
    ],
)
def test_phone_leaves_dates_and_list_items(text, lang, kwargs, expected):
    assert words2num_sentence(text, lang=lang, phone=True, **kwargs) == expected
