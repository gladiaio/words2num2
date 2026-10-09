"""Inflected ordinals: gender / case / number / article forms the renderer
never writes, and the figures a sentence writes for them."""

import pytest

from words2num2 import words2num, words2num_sentence


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # #126: es 11th-29th spoken spaced, rendered glued.
        ("es", "vigésimo primero", 21),
        ("es", "décimo primero", 11),
        ("es", "vigésima primera", 21),
        # #137 ru, #140 pl, #145 bg: only the last (ru, bg) or every (pl) element inflects.
        ("ru", "двадцать первая", 21),
        ("pl", "dwudziesta pierwsza", 21),
        ("bg", "двадесет и първа", 21),
        # #172 fr plural.
        ("fr", "deuxièmes", 2),
        ("fr", "premières", 1),
        # #244: the grammatical compound, not the renderer's cardinal + suffix.
        ("cs", "dvacátý první", 21),
        ("hr", "dvadeset prvi", 21),
        ("el", "δέκατος όγδοος", 18),
        ("gl", "primeiro", 1),
    ],
)
def test_inflected_ordinals(lang, text, expected):
    assert words2num(text, lang=lang, to="ordinal") == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # #145 bg, #149 ca, #150 el: the figure agrees with the spoken form.
        ("bg", "първа глава", "1-ва глава"),
        ("bg", "първата глава", "1-вата глава"),
        ("ca", "la cinquena planta", "la 5a planta"),
        ("ca", "el cinquè pis", "el 5è pis"),
        ("el", "η πρώτη μέρα", "η 1η μέρα"),
        ("el", "ο πρώτος όροφος", "ο 1ος όροφος"),
        ("el", "η εικοστή πρώτη μέρα", "η 21η μέρα"),
        # #172 fr plural.
        ("fr", "les deuxièmes places", "les 2es places"),
        ("fr", "les premières places", "les 1res places"),
        # #185 it feminine plural.
        ("it", "le prime volte", "le 1ª volte"),
        ("it", "le seconde volte", "le 2ª volte"),
        ("it", "le terze classi", "le 3ª classi"),
        ("it", "la terza classe", "la 3ª classe"),
        # Homographs stay words: a fraction, a weekday, "other", "seconds", "first(ly)".
        ("fr", "trois cinquièmes", "trois cinquièmes"),
        ("fr", "quelques secondes", "quelques secondes"),
        ("ca", "tres quarts", "tres quarts"),
        ("el", "την Τρίτη", "την Τρίτη"),
        ("bg", "две стотни", "две стотни"),
        ("da", "noget andet", "noget andet"),
        ("sw", "siku ya kwanza", "siku ya kwanza"),
        ("sq", "para", "para"),
    ],
)
def test_sentence_ordinal_figures_agree(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected
