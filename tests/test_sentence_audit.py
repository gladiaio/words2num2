# -*- coding: utf-8 -*-
"""Sentence walker fixes from the October 2026 audit (label ``audit-2026-10``).

Each block names the issue it pins. Negative cases (text that must stay as
words) matter as much as the conversions: a raise or a word left alone is
always better than a wrong number.
"""

import time

import pytest

from words2num2 import auto_parse_sentence, words2num_sentence


def s(text, lang="en"):
    return words2num_sentence(text, lang=lang)


# #51: two digit words are a range ("two or three"), not one number.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("fr", "j attends deux trois minutes", "j attends 2 3 minutes"),
        ("en", "I waited two three minutes", "I waited 2 3 minutes"),
        ("es", "espere dos tres minutos", "espere 2 3 minutos"),
        ("sv", "jag har två tre katter", "jag har 2 3 katter"),
        # A dictated string is still read: three digits, or a leading zero.
        ("en", "one two three", "123"),
        ("en", "zero six", "06"),
        ("fr", "deux trois quatre", "234"),
        ("es", "cero uno", "01"),
    ],
)
def test_51_two_digit_words_are_a_range(lang, text, expected):
    assert s(text, lang) == expected


# #68: a token in figures never joins a word run.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("en", "born twenty 1990", "born 20 1990"),
        ("en", "nineteen 90", "19 90"),
        ("en", "fifty 50", "50 50"),
        ("en", "two 000", "2 000"),
        ("en", "five 100", "5 100"),
        ("fr", "cinq 5", "5 5"),
        ("es", "dos 2", "2 2"),
    ],
)
def test_68_digit_tokens_stay_literals(lang, text, expected):
    assert s(text, lang) == expected


# #79: "oh" is the interjection unless it sits next to another digit word.
@pytest.mark.parametrize(
    "text,expected",
    [
        ("oh no", "oh no"),
        ("Oh, really", "Oh, really"),
        ("I said oh", "I said oh"),
        ("room oh seven", "room 07"),
        ("one oh one", "101"),
        ("twenty oh five", "2005"),
    ],
)
def test_79_oh_is_an_interjection(text, expected):
    assert s(text) == expected


# #186: long runs of number words are linear, not cubic.
@pytest.mark.parametrize(
    "lang,word",
    [("de", "zweitausend"), ("it", "duemila"), ("fr", "deux mille"), ("es", "mil"),
     ("en", "twenty first"), ("en", "nineteen ninety")],
)
def test_186_long_number_runs_finish_fast(lang, word):
    text = " ".join([word] * 2000)
    start = time.perf_counter()
    s(text, lang)
    # Cubic growth took 16 s for 200 de tokens; linear is well under a second.
    assert time.perf_counter() - start < 10


# #202: no exponent notation in rewritten text.
def test_202_positional_notation():
    assert s("zero point zero zero zero zero zero zero one") == "0.0000001"
    assert auto_parse_sentence("0.0000001 kg") == "0.0000001 kg"


# #69: punctuation around a number is kept and never blocks it.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("en", "forty two)", "42)"),
        ("en", "(forty two", "(42"),
        ("en", "(forty two)", "(42)"),
        ("en", 'he said "forty" twice', 'he said "40" twice'),
        ("en", '"forty-two"', '"42"'),
        ("en", "forty…", "40…"),
        ("fr", "quarante…", "40…"),
        ("en", "“forty two”", "“42”"),
        ("fr", "«vingt-deux»", "«22»"),
        ("es", "¿veinte?", "¿20?"),
        ("en", "(see above)", "(see above)"),
    ],
)
def test_69_punctuation_is_kept(lang, text, expected):
    assert s(text, lang) == expected


# #65: the connectors the renderer writes between number words keep a run
# whole, and a run never ends on (and so never swallows) one.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("is", "har fjörutíu og tveir kindur", "har 42 kindur"),
        ("no", "fem hundre og femtifem", "555"),
        ("fa", "چهل و دو", "42"),
        ("sq", "dyzet e dy", "42"),
        ("bg", "четиридесет и два лева", "42 лева"),
        ("ha", "qqx ɗari biyar hamsin da biyar zzx", "qqx 555 zzx"),
        ("ha", "qqx miliyan dubu ɗari biyu talatin da huɗu ɗari biyar sittin da bakwai zzx",
         "qqx 1234567 zzx"),
        ("lus", "qqx pakhat nuai pahnih za leh sawmthum leh pali sang panga za leh sawmruk "
                "leh pasarih zzx", "qqx 1234567 zzx"),
        ("vi", "hai nghìn lẻ bốn", "2004"),
        ("ro", "douăzeci de lei", "20 de lei"),
        ("ro", "douăzeci de mii de lei", "20000 de lei"),
        # "and" between two numbers is a list, not a sum.
        ("sw", "mbili na tatu", "2 na 3"),
        ("no", "fem og seks", "5 og 6"),
        ("fa", "پنج و شش", "5 و 6"),
        ("bg", "пет и шест", "5 и 6"),
    ],
)
def test_65_renderer_connectors(lang, text, expected):
    assert s(text, lang) == expected


# #67: the comma num2words2 writes after a scale word is inside the number.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("en", "hello twelve thousand, three hundred and forty-five world", "hello 12345 world"),
        ("en", "one million, two hundred and thirty-four thousand, five hundred and sixty-seven",
         "1234567"),
        ("pt_BR", "doze mil, trezentos e quarenta e cinco", "12345"),
        ("en", "two thousand, three hundred people", "2300 people"),
        # Not a lower group after the comma: two numbers.
        ("en", "in two thousand, five people came", "in 2000, 5 people came"),
        ("en", "twelve thousand, hello", "12000, hello"),
        ("en", "one, two, three", "1, 2, 3"),
    ],
)
def test_67_comma_inside_a_cardinal(lang, text, expected):
    assert s(text, lang) == expected
