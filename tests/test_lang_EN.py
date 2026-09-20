# -*- coding: utf-8 -*-
"""English words-to-number tests."""
from decimal import Decimal

import pytest

from words2num2 import words2num, words2num_sentence
from words2num2.base import Words2NumError


@pytest.mark.parametrize(
    "text,expected",
    [
        ("zero", 0),
        ("one", 1),
        ("nine", 9),
        ("ten", 10),
        ("nineteen", 19),
        ("twenty", 20),
        ("forty-two", 42),
        ("ninety-nine", 99),
        ("one hundred", 100),
        ("one hundred and one", 101),
        ("two hundred fifty", 250),
        ("a hundred", 100),
        ("one thousand", 1000),
        ("one thousand two hundred thirty-four", 1234),
        ("ten thousand", 10_000),
        ("one hundred thousand", 100_000),
        ("one million", 1_000_000),
        ("two million three hundred thousand", 2_300_000),
        ("one billion", 1_000_000_000),
    ],
)
def test_cardinal(text, expected):
    assert words2num(text) == expected


@pytest.mark.parametrize(
    "text,expected",
    [
        ("minus one", -1),
        ("negative seven", -7),
        ("minus one hundred", -100),
    ],
)
def test_negative(text, expected):
    assert words2num(text) == expected


@pytest.mark.parametrize(
    "text,expected",
    [
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("twenty-first", 21),
        ("one hundredth", 100),
        ("one millionth", 1_000_000),
    ],
)
def test_ordinal(text, expected):
    assert words2num(text, to="ordinal") == expected
    assert words2num(text) == expected  # cardinal also accepts ordinal forms


def test_decimal():
    assert words2num("three point one four") == Decimal("3.14")
    assert words2num("zero point five") == Decimal("0.5")


def test_year():
    assert words2num("nineteen ninety nine", to="year") == 1999
    assert words2num("two thousand twenty four", to="year") == 2024


def test_digits_passthrough():
    assert words2num("42") == 42
    assert words2num("-17") == -17
    assert words2num("3.14") == 3.14


def test_unknown_token_raises():
    with pytest.raises(Words2NumError):
        words2num("forty zoot")


def test_empty_input_raises():
    with pytest.raises(Words2NumError):
        words2num("")


def test_sentence_simple():
    assert (
        words2num_sentence("I bought twenty-three apples.")
        == "I bought 23 apples."
    )


def test_sentence_multiple_runs():
    assert (
        words2num_sentence("I bought twenty-three apples and fourteen pears.")
        == "I bought 23 apples and 14 pears."
    )


def test_sentence_preserves_punctuation():
    assert (
        words2num_sentence(
            "In nineteen ninety nine, two thousand people came.", to="year"
        )
        == "In 1999, 2000 people came."
    )


# ---------------------------------------------------------------------------
# A tens word composes with a FOLLOWING unit ("sixty three" = 63), never with
# a preceding one. "three sixty" is two numbers read in sequence, not 3 + 60.
# See gladiaio/words2num2#17.
# ---------------------------------------------------------------------------

@pytest.mark.parametrize(
    "sentence,expected",
    [
        ("three sixty", "3 60"),
        ("three sixty five", "3 65"),
        ("nineteen eighty four", "19 84"),
        ("twenty twenty", "20 20"),
        ("five forty", "5 40"),
    ],
)
def test_unit_then_tens_reads_as_separate_numbers(sentence, expected):
    assert words2num_sentence(sentence) == expected


@pytest.mark.parametrize(
    "text", ["three sixty", "three sixty five", "nineteen eighty four", "twenty twenty"]
)
def test_unit_then_tens_is_not_one_cardinal(text):
    # Asked for a single number, given two.
    with pytest.raises(Words2NumError):
        words2num(text)


@pytest.mark.parametrize(
    "text,expected",
    [
        # tens + unit is the valid order and is untouched.
        ("sixty three", 63),
        ("ninety nine", 99),
        # "hundred" and the scale words close the sub-hundred slot, so a tens
        # word may legitimately follow them.
        ("one hundred sixty", 160),
        ("three hundred sixty", 360),
        ("two thousand sixty", 2060),
        ("one hundred and five thousand", 105000),
    ],
)
def test_valid_compositions_unchanged(text, expected):
    assert words2num(text) == expected


@pytest.mark.parametrize(
    "text,expected", [("nineteen eighty four", 1984), ("twenty twenty", 2020)]
)
def test_year_path_still_reads_pairs(text, expected):
    # The pair reading lives in to="year" and is deliberately left there.
    assert words2num(text, to="year") == expected




# ---------------------------------------------------------------------------
# Dangling decimal separator — ports revdotcom/words2num#5 by @qmac.
#
# The separator used to be dropped silently when nothing followed it, so
# "one point" returned Decimal('1'), "two thousand point" returned
# Decimal('2000'), and a bare "point" returned Decimal('0').
# ---------------------------------------------------------------------------

@pytest.mark.parametrize(
    "text",
    [
        # Added by upstream #5 — these used to return the integer part.
        "one point",
        "two thousand point",
        "point",
        "one hundred point",
        "zero point",
        "dot",
        "three dot",
        # Already rejected before #5; pinned here so the whole upstream
        # negative list lives in one place.
        "one point thousand",
        "one point two point three",
        "one point point two",
        "eleven thousand point two hundred",
    ],
)
def test_dangling_decimal_separator_raises(text):
    with pytest.raises(Words2NumError):
        words2num(text)


@pytest.mark.parametrize(
    "text,expected",
    [
        ("point five", Decimal("0.5")),
        ("dot five", Decimal("0.5")),
        ("zero point five", Decimal("0.5")),
        ("three point one four", Decimal("3.14")),
        ("one point zero", Decimal("1.0")),
        # The sign of zero survives the guard.
        ("minus zero point zero", Decimal("-0.0")),
    ],
)
def test_decimal_still_parses(text, expected):
    got = words2num(text)
    assert got == expected
    # Decimal("-0.0") == Decimal("0.0"), so compare the repr for the sign.
    assert str(got) == str(expected)


@pytest.mark.parametrize(
    "sentence,expected",
    [
        # A decimal separator is a valid run head. This used to work only
        # because "point" alone parsed as 0; it is explicit now.
        ("point five", "0.5"),
        ("dot five", "0.5"),
        ("say point five now", "say 0.5 now"),
        ("the answer is point five.", "the answer is 0.5."),
        ("a dot five gain", "a 0.5 gain"),
        # A dangling separator is left as prose rather than swallowed.
        ("one point", "1 point"),
    ],
)
def test_sentence_decimal_head(sentence, expected):
    assert words2num_sentence(sentence) == expected


def test_run_may_not_start_with_other_includables():
    # Only the decimal separators were promoted to run heads; "and", "minus"
    # and the articles stay run-internal.
    assert words2num_sentence("minus forty two") == "minus 42"
    assert words2num_sentence("and seven") == "and 7"
