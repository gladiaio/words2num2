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
