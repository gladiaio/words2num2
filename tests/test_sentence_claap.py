"""Cases reported by Claap on Gladia's real-time numerals (fr and en).

Ranges said without "to" ("vingt-cinq trente mille euros"), percentages,
a number word before "pour cent" read as cents, and a phone number that
took the pronoun "one" of "one of them".
"""

import pytest

from words2num2 import words2num_sentence


@pytest.mark.parametrize(
    "lang, text, expected",
    [
        ("fr", "vingt-cinq trente mille euros", "25000-30000 euros"),
        ("fr", "quinze vingt millions", "15000000-20000000"),
        ("fr", "un budget de deux trois mille", "un budget de 2000-3000"),
        ("en", "twenty five thirty thousand dollars", "25000-30000 dollars"),
        ("en", "fifteen twenty million", "15000000-20000000"),
    ],
)
def test_range_without_connector_shares_the_scale(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang, text, expected",
    [
        ("fr", "vingt-cinq trente mille euros", "25 000-30 000 €"),
        ("en", "twenty five thirty thousand dollars", "$25,000-$30,000"),
        ("es", "veinticinco treinta mil euros", "25.000-30.000 €"),
    ],
)
def test_range_without_connector_with_currency(lang, text, expected):
    assert words2num_sentence(text, lang=lang, currency=True) == expected


@pytest.mark.parametrize(
    "lang, text",
    [
        # Not rising, or not of one order: two numbers, as before.
        ("fr", "trente vingt mille"),
        ("fr", "deux trente mille"),
        ("en", "nineteen ninety thousand"),
        # Neither next to each other nor round: a time, then a count.
        ("en", "at nine fifteen thousand"),
        ("fr", "douze dix-huit mille"),
        # A range from one is "one to two thousand", never juxtaposed.
        ("en", "one two thousand"),
        # The first bound is the tail of a digit string.
        ("en", "four five six thousand"),
        # A comma is a list, not a range.
        ("fr", "vingt-cinq, trente mille"),
    ],
)
def test_no_range_where_it_would_be_a_guess(lang, text):
    assert "-" not in words2num_sentence(text, lang=lang)


@pytest.mark.parametrize(
    "lang, text, expected",
    [
        ("fr", "quinze pour cent", "15 %"),
        ("fr", "on prend quinze pour cent.", "on prend 15 %."),
        ("en", "fifteen percent", "15%"),
        ("en", "fifteen per cent of them", "15% of them"),
        ("es", "quince por ciento", "15 %"),
        ("de", "fünfzehn Prozent", "15 %"),
        ("it", "quindici per cento", "15 %"),
        ("fr", "deux virgule cinq pour cent", "2,5 %"),
    ],
)
def test_percent(lang, text, expected):
    assert words2num_sentence(text, lang=lang, percent=True) == expected


def test_percent_is_off_by_default():
    assert words2num_sentence("quinze pour cent", lang="fr") == "15 pour cent"
    assert words2num_sentence("fifteen percent", lang="en") == "15 percent"


def test_percent_needs_a_number():
    # "pour cent" with no figure before it gets no sign.
    assert (
        words2num_sentence("pour cent personnes", lang="fr", percent=True)
        == "pour cent personnes"
    )


@pytest.mark.parametrize(
    "lang, text, expected",
    [
        ("fr", "vingt-cinq mille euros quinze pour cent", "25 000 € 15 pour cent"),
        ("fr", "vingt-cinq mille euros, quinze pour cent", "25 000 €, 15 pour cent"),
        ("en", "two thousand dollars fifteen percent", "$2,000 15 percent"),
        # A bare subunit with no percent after it still folds.
        ("fr", "vingt euros cinquante", "20,50 €"),
    ],
)
def test_number_before_percent_is_no_subunit(lang, text, expected):
    assert words2num_sentence(text, lang=lang, currency=True) == expected


def test_number_before_percent_is_no_subunit_with_percent():
    out = words2num_sentence(
        "vingt-cinq mille euros quinze pour cent",
        lang="fr",
        currency=True,
        percent=True,
    )
    assert out == "25 000 € 15 %"


@pytest.mark.parametrize(
    "text, expected",
    [
        (
            "two one two five five zero one two three one of them",
            "212550123 one of them",
        ),
        (
            "call two one two five five five one two three four one of them",
            "call 2125551234 one of them",
        ),
        # "one" ends the run elsewhere: still a digit.
        ("two one two five five zero one two three one", "2125501231"),
        (
            "two one two five five zero one two three one, of course",
            "2125501231, of course",
        ),
    ],
)
def test_digit_run_leaves_the_pronoun_one_of(text, expected):
    assert words2num_sentence(text, lang="en") == expected


def test_phone_leaves_the_pronoun_one_of():
    out = words2num_sentence(
        "call two one two five five five one two three four one of them",
        lang="en",
        phone=True,
    )
    assert out == "call (212) 555-1234 one of them"
