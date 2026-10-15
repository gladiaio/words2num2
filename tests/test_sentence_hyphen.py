# -*- coding: utf-8 -*-
"""English hyphens in sentence mode: a compound joins, any other hyphen splits.

A hyphen between two number words makes one number only in a compound, a tens
word and a unit ("twenty-two", "twenty-first") or a number and the scale it
counts ("two-thousand"). Anywhere else it is a boundary: each side reads alone
and the hyphen stays (gladia-enterprise#900).
"""

import pytest

from words2num2 import words2num_sentence


@pytest.mark.parametrize(
    "text,expected",
    [
        ("two thousand-two thousand five", "2000-2005"),
        ("five five five-one two one two", "555-1212"),
        ("twenty-thirty", "20-30"),
        ("page twenty-thirty", "page 20-30"),
        ("ten-twelve", "10-12"),
        ("pages ten-twelve", "pages 10-12"),
        ("three-four", "3-4"),
        ("twenty-five-thirty", "25-30"),
        ("one hundred-five", "100-5"),
    ],
)
def test_hyphen_between_numbers_is_a_boundary(text, expected):
    assert words2num_sentence(text, lang="en") == expected


@pytest.mark.parametrize(
    "text,expected",
    [
        ("twenty-two", "22"),
        ("ninety-nine", "99"),
        ("twenty-first", "21st"),
        ("two-thousand dollars", "2000 dollars"),
        ("forty-two, fifty-five", "42, 55"),
        ("a thirty-two-year-old", "a thirty-two-year-old"),
        ("-17 and a minus", "-17 and a minus"),
    ],
)
def test_compounds_and_signs_are_unchanged(text, expected):
    assert words2num_sentence(text, lang="en") == expected


def test_other_languages_keep_their_hyphenated_compounds():
    assert words2num_sentence("vingt-deux", lang="fr") == "22"
