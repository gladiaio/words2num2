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
# Sentence mode on bank IVR transcripts: a spoken year is a year, a string of
# single digits is a digit string. Neither is summed.
# ---------------------------------------------------------------------------

@pytest.mark.parametrize(
    "sentence,expected",
    [
        ("twenty twenty five", "2025"),
        ("nineteen ninety nine", "1999"),
        ("december one twenty twenty five", "december 1 2025"),
        ("in twenty twenty", "in 2020"),
        ("twenty oh five", "2005"),
        ("twenty nineteen", "2019"),
        # Already a valid cardinal: unchanged.
        ("two thousand twenty five", "2025"),
        ("twenty hundred", "2000"),
        ("twenty five people", "25 people"),
        ("twenty first", "21"),
    ],
)
def test_sentence_reads_spoken_years(sentence, expected):
    assert words2num_sentence(sentence) == expected


@pytest.mark.parametrize(
    "sentence,expected",
    [
        ("your account ending in four five six seven", "your account ending in 4567"),
        ("zero six one two", "0612"),
        ("one two three", "123"),
        ("one oh one", "101"),
        ("seven eight", "78"),
        # A lone digit word, and tens + unit, stay cardinals.
        ("press one", "press 1"),
        ("twenty five", "25"),
        ("one hundred twenty three", "123"),
        # unit + tens is two numbers, not 25 and not 520.
        ("five twenty", "5 20"),
        # "oh" does not open a digit run.
        ("oh seven", "0 7"),
    ],
)
def test_sentence_digit_runs_concatenate(sentence, expected):
    assert words2num_sentence(sentence) == expected


@pytest.mark.parametrize(
    "text", ["twenty twenty five", "four five six seven", "one two three"]
)
def test_year_and_digit_runs_are_not_one_cardinal(text):
    # "twenty twenty five" must never become 45, nor "one two three" 6.
    with pytest.raises(Words2NumError):
        words2num(text)


@pytest.mark.parametrize(
    "text,expected",
    [
        ("twenty twenty five", 2025),
        ("twenty oh five", 2005),
        ("twenty hundred", 2000),
        ("two thousand twenty five", 2025),
    ],
)
def test_explicit_year_mode_unchanged(text, expected):
    assert words2num(text, to="year") == expected
