# -*- coding: utf-8 -*-
"""Real-life ASR finals through ``words2num_sentence``.

Call-center, IVR and finance transcripts: the walker sees whole sentences,
not isolated numbers, so what matters is that a number is read as the
speaker meant it and that everything else is left alone.
"""

import pytest

from words2num2 import words2num_sentence


@pytest.mark.parametrize(
    "text,expected",
    [
        # num2words writes "deux cents" / "quatre-vingts"; on their own "cents"
        # and "vingts" are not number words, so the walker stopped the run
        # before them: "2 cents euros", "4 vingts".
        ("deux cents euros", "200 euros"),
        ("quatre vingts", "80"),
        ("cinq cents personnes", "500 personnes"),
        # Speech drops the plural -s just as freely.
        ("mille deux cent", "1200"),
        ("mille deux cents", "1200"),
        ("deux million", "2000000"),
        # Two scale words: "un million" then "deux cent mille".
        ("un million deux cent mille", "1200000"),
        ("trois millions cinq cent mille euros", "3500000 euros"),
        # Feminine agreement inside a number.
        ("cinquante et une personnes", "51 personnes"),
        ("vingt et une heures", "21 heures"),
        ("une personne", "une personne"),
        # "pour cent" is the percent sign, not the number 100.
        ("vingt pour cent", "20 pour cent"),
        ("un pour cent", "1 pour cent"),
        ("cent pour cent", "100 pour cent"),
    ],
)
def test_fr_compounds(text, expected):
    assert words2num_sentence(text, lang="fr") == expected


@pytest.mark.parametrize(
    "text,expected",
    [
        # num2words glues "veintiuno"; spoken with the connector it is still 21.
        ("veinte y uno", "21"),
        ("treinta y cinco", "35"),
        ("diez por ciento", "10 por ciento"),
        ("ciento por ciento", "100 por ciento"),
    ],
)
def test_es_compounds(text, expected):
    assert words2num_sentence(text, lang="es") == expected


@pytest.mark.parametrize(
    "text,expected",
    [
        # "a" opens a run in front of a scale word.
        ("a hundred dollars", "100 dollars"),
        ("a thousand dollars", "1000 dollars"),
        ("a million", "1000000"),
        ("a hundred and one dogs", "101 dogs"),
        ("an hour", "an hour"),
        # A run never ends on a connector — the connector is a word again.
        ("one and a half hours", "1 and a half hours"),
        ("five point", "5 point"),
        ("one hundred and", "100 and"),
        ("point five", "0.5"),
        ("five point five", "5.5"),
        # A scale word after a decimal stays a word.
        ("two point five million", "2.5 million"),
        ("two point five million dollars", "2.5 million dollars"),
        ("one point five percent", "1.5 percent"),
        ("twenty per cent", "20 per cent"),
    ],
)
def test_en_compounds(text, expected):
    assert words2num_sentence(text, lang="en") == expected
