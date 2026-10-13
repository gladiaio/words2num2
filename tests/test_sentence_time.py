# -*- coding: utf-8 -*-
"""Clock times in French, German and Spanish sentence mode.

A time phrase is written in figures: fr "9h15", de "9:15 Uhr" / "9:30", es
"las 9:15". An hour alone is a time only after a time cue; a duration stays a
duration.
"""

import pytest

from words2num2 import words2num_sentence


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("fr", "à neuf heures quinze", "à 9h15"),
        ("fr", "rendez-vous à neuf heures.", "rendez-vous à 9h."),
        ("fr", "vers dix-sept heures trente", "vers 17h30"),
        ("fr", "à neuf heures et quart", "à 9h15"),
        ("fr", "à neuf heures et demie", "à 9h30"),
        ("fr", "à dix heures moins le quart", "à 9h45"),
        ("fr", "à dix heures moins cinq", "à 9h55"),
        ("fr", "à une heure", "à 1h"),
        ("fr", "il est vingt-trois heures cinquante-neuf", "il est 23h59"),
        ("fr", "entre neuf heures et dix heures trente", "entre 9h et 10h30"),
        ("de", "um neun Uhr fünfzehn", "um 9:15 Uhr"),
        ("de", "um neun Uhr", "um 9 Uhr"),
        ("de", "um siebzehn Uhr dreißig.", "um 17:30 Uhr."),
        ("de", "um halb zehn", "um 9:30"),
        ("de", "um Viertel nach neun", "um 9:15"),
        ("de", "um Viertel vor zehn", "um 9:45"),
        ("de", "gegen dreiviertel zehn", "gegen 9:45"),
        ("de", "um halb eins", "um 12:30"),
        ("es", "a las nueve y cuarto", "a las 9:15"),
        ("es", "a las nueve y quince", "a las 9:15"),
        ("es", "a las nueve y media", "a las 9:30"),
        ("es", "a las diez menos cuarto", "a las 9:45"),
        ("es", "a la una en punto", "a la 1:00"),
        ("es", "las diecisiete treinta", "las 17:30"),
        ("es", "Las nueve y media, vale", "Las 9:30, vale"),
    ],
)
def test_clock_time(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # Durations and plain counts.
        ("fr", "trois heures de route", "3 heures de route"),
        ("fr", "pendant deux heures", "pendant 2 heures"),
        ("fr", "il a trois heures de retard", "il a 3 heures de retard"),
        ("fr", "à minuit moins le quart", "à minuit moins le quart"),
        ("de", "drei Stunden", "3 Stunden"),
        ("de", "halb so viel", "halb so viel"),
        ("es", "a las nueve", "a las 9"),
        ("es", "las tres casas", "las 3 casas"),
    ],
)
def test_not_a_time(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


def test_only_plain_modes():
    # to="ordinal" keeps its own reading.
    assert ":" not in words2num_sentence(
        "a las nueve y cuarto", lang="es", to="ordinal"
    )


@pytest.mark.parametrize(
    "text, lang, expected",
    [
        # The number goes on after the time (#928, #935).
        ("at nine fifteen thousand", "en", "at 9 15000"),
        ("at nine twenty first", "en", "at 9 21st"),
        ("cent vingt heures quinze", "fr", "120 heures 15"),
        ("pendant trente et une heures quinze", "fr", "pendant 31 heures 15"),
        ("las dos menos treinta y uno", "es", "las 2 menos 31"),
        # The minutes count something (#933, #904).
        (
            "il travaille huit heures trois fois par semaine",
            "fr",
            "il travaille 8 heures 3 fois par semaine",
        ),
        ("um neun Uhr zwanzig Leute kamen", "de", "um 9 Uhr 20 Leute kamen"),
        ("las dos y cinco personas", "es", "las 2 y 5 personas"),
        ("sold at nine fifty each", "en", "sold at 9 50 each"),
        ("rates at five twenty five percent", "en", "rates at 5 25 percent"),
        # Times still read.
        ("at nine fifteen and then lunch", "en", "at 9:15 and then lunch"),
        ("à neuf heures quinze demain", "fr", "à 9h15 demain"),
        ("um neun Uhr fünfzehn", "de", "um 9:15 Uhr"),
        ("a las nueve y cuarto", "es", "a las 9:15"),
        # "por"/"pour" and "each morning" keep the time; ranges keep both
        # hours; more units (#1191-#1194).
        ("a las ocho y diez por la mañana", "es", "a las 8:10 por la mañana"),
        (
            "rendez-vous à huit heures quinze pour le dîner",
            "fr",
            "rendez-vous à 8h15 pour le dîner",
        ),
        ("we start at nine thirty each morning", "en", "we start at 9:30 each morning"),
        ("entre huit et neuf heures et quart", "fr", "entre 8 et 9h15"),
        ("zwischen neun und zehn Uhr dreißig", "de", "zwischen 9 und 10:30 Uhr"),
        ("it weighs about nine fifty grams", "en", "it weighs about 9 50 grams"),
        # A following count is its own number, not part of the minutes; a
        # refused fr span after a cue keeps the hour (#1190 audit).
        ("um neun Uhr zwanzig zwei Leute kamen", "de", "um 9:20 Uhr 2 Leute kamen"),
        (
            "à neuf heures quinze un homme est arrivé",
            "fr",
            "à 9h15 un homme est arrivé",
        ),
        ("a las nueve y diez un hombre llegó", "es", "a las 9:10 un hombre llegó"),
        ("à neuf heures quinze fois deux", "fr", "à 9h 15 fois 2"),
        ("at nine fifteen first", "en", "at 9:15 first"),
        # A scale word that does not combine with the minutes (#1190 audit).
        ("a las nueve y veinte cien personas", "es", "a las 9:20 100 personas"),
    ],
)
def test_time_reader_leaves_longer_numbers_and_counts(text, lang, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "text, lang, kwargs, expected",
    [
        # Minutes before a currency name of the language or a counted noun
        # are an amount or a count, not a clock time (#1194).
        (
            "shares closed at nine fifty pence",
            "en",
            {"currency": True},
            "shares closed at 9 £0.50",
        ),
        ("at nine fifty rupees", "en", {}, "at 9 50 rupees"),
        ("six heures dix livres", "fr", {"currency": True}, "6 heures 10 £"),
        ("huit heures trois kilos", "fr", {}, "8 heures 3 kilos"),
        ("um neun Uhr zwanzig Kinder", "de", {}, "um 9 Uhr 20 Kinder"),
        ("um neun Uhr zwanzig Pfund", "de", {"currency": True}, "um 9 Uhr 20 £"),
        ("las dos y cinco pesos", "es", {"currency": True}, "las 2 y $5"),
        ("las dos y cinco niños", "es", {}, "las 2 y 5 niños"),
        # A name that is mostly another word keeps the time.
        ("at nine fifteen won the race", "en", {}, "at 9:15 won the race"),
        ("a las dos y cinco real", "es", {}, "a las 2:05 real"),
        # A spaced "p m", a capitalised name and the clearest homographs
        # keep the time (#1197 audit).
        ("at nine fifteen p. m.", "en", {}, "at 9:15 p. m."),
        ("at nine fifteen p m", "en", {"currency": True}, "at 9:15 p m"),
        (
            "at nine fifteen Penny called",
            "en",
            {"currency": True},
            "at 9:15 Penny called",
        ),
        (
            "at nine fifteen pound the table",
            "en",
            {"currency": True},
            "at 9:15 pound the table",
        ),
        (
            "a las nueve y diez Franco llamó",
            "es",
            {"currency": True},
            "a las 9:10 Franco llamó",
        ),
        (
            "a las nueve y diez peso la harina",
            "es",
            {"currency": True},
            "a las 9:10 peso la harina",
        ),
        (
            "um neun Uhr zwanzig Rand des Waldes",
            "de",
            {"currency": True},
            "um 9:20 Uhr Rand des Waldes",
        ),
        ("six heures dix francs", "fr", {}, "6 heures 10 francs"),
        # Upper or title case still counts (#1197 audit).
        ("AT NINE FIFTY RUPEES", "en", {"currency": True}, "AT 9 ₹50"),
        ("a las dos y cinco Pesos", "es", {"currency": True}, "a las 2 y $5"),
    ],
)
def test_time_reader_leaves_currency_and_counted_nouns(text, lang, kwargs, expected):
    assert words2num_sentence(text, lang=lang, **kwargs) == expected
