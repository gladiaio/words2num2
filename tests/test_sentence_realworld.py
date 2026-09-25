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


# ---------------------------------------------------------------------------
# Ordinals. Plain cardinal mode reads them too and writes a rank in figures
# ("15th", "1er", "2e", "3º", "2."). "first"/"second" and their translations
# are only a rank next to a month (or "of the month"): "first of all", "wait
# a second", "la première fois" stay in words. A fraction ("a fifth of", "two
# thirds") stays in words too.
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "text,expected",
    [
        ("due on the fifteenth", "due on the 15th"),
        ("the twenty first of march", "the 21st of march"),
        ("march twenty first", "march 21st"),
        ("on june fifteenth two thousand twenty four", "on june 15th 2024"),
        ("the twenty second of april", "the 22nd of april"),
        ("on the thirty first", "on the 31st"),
        ("my birthday is october third", "my birthday is october 3rd"),
        ("the fourth of july", "the 4th of july"),
        ("third quarter results", "3rd quarter results"),
        ("one hundred and fifth", "105th"),
        ("he came third", "he came 3rd"),
        ("the eleventh", "the 11th"),
        ("the twelfth of never", "the 12th of never"),
        ("the hundredth customer", "the 100th customer"),
        ("twenty-first century", "21st century"),
        ("Fifteenth of June.", "15th of June."),
        # first / second: a date, or just words.
        ("the second of may", "the 2nd of may"),
        ("september the first", "september the 1st"),
        ("the first of the month", "the 1st of the month"),
        ("first of all", "first of all"),
        ("at first i thought", "at first i thought"),
        ("your first name", "your first name"),
        ("the first time", "the first time"),
        ("wait a second", "wait a second"),
        ("the second floor", "the second floor"),
        # Fractions.
        ("two thirds of the amount", "two thirds of the amount"),
        ("three quarters", "three quarters"),
        ("a fifth of it", "a fifth of it"),
        # Digits are literals, never an ordinal.
        ("the 2nd", "the 2nd"),
    ],
)
def test_en_ordinals(text, expected):
    assert words2num_sentence(text, lang="en") == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("fr", "le premier avril", "le 1er avril"),
        ("fr", "la première de mars", "la 1re de mars"),
        ("fr", "le premier du mois", "le 1er du mois"),
        ("fr", "le vingt-troisième jour", "le 23e jour"),
        ("fr", "au deuxième étage", "au 2e étage"),
        ("fr", "au troisième trimestre", "au 3e trimestre"),
        ("fr", "le quinzième", "le 15e"),
        ("fr", "la première fois", "la première fois"),
        ("fr", "premier ministre", "premier ministre"),
        ("fr", "une seconde", "une seconde"),
        ("fr", "un dixième", "1 dixième"),  # "un" itself: see the article tests
        ("es", "el primero de mayo", "el 1º de mayo"),
        ("es", "el primero de cada mes", "el 1º de cada mes"),
        ("es", "el tercer piso", "el 3º piso"),
        ("es", "la vigésima", "la 20ª"),
        ("es", "el quinto", "el 5º"),
        ("es", "la primera vez", "la primera vez"),
        ("es", "las primeras", "las primeras"),
        ("es", "un segundo", "un segundo"),
        ("es", "el 2 y cinco", "el 2 y 5"),
        ("de", "am zweiten april", "am 2. april"),
        ("de", "am ersten mai", "am 1. mai"),
        ("de", "der dritte", "der 3."),
        ("de", "zum ersten mal", "zum ersten mal"),
        ("it", "il primo maggio", "il 1º maggio"),
        ("it", "il terzo piano", "il 3º piano"),
        ("it", "la prima volta", "la prima volta"),
        ("pt", "primeiro de maio", "1º de maio"),
        ("pt", "o terceiro andar", "o 3º andar"),
        ("pt", "a primeira vez", "a primeira vez"),
        ("nl", "de tweede verdieping", "de 2e verdieping"),
        ("nl", "vijftiende", "15e"),
        ("nl", "de eerste keer", "de eerste keer"),
    ],
)
def test_ordinals_other_languages(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected
