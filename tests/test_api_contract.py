# -*- coding: utf-8 -*-
"""The public call contract: language codes, ``to=`` modes, keyword
arguments and the command-line entry point."""

import subprocess
import sys

import pytest

import words2num2
from words2num2 import (
    CONVERTER_TYPES,
    Words2NumError,
    words2num,
    words2num_sentence,
)
from words2num2.__main__ import main


# --- #192: language codes are case-insensitive ------------------------------
@pytest.mark.parametrize(
    "text, lang, expected",
    [
        ("one", "EN", 1),
        ("un", "FR", 1),
        ("one", "En", 1),
        ("one", "EN-US", 1),
        ("one", "en_us", 1),
        ("um", "pt_br", 1),
        ("one", "eng", 1),
        ("trois", "fra", 3),
    ],
)
def test_lang_code_case_and_form_issue_192(text, lang, expected):
    assert words2num(text, lang=lang) == expected


def test_sentence_lang_code_case_issue_192():
    assert words2num_sentence("il a vingt ans", lang="FR") == "il a 20 ans"


# --- #197: no "first two characters" guess ----------------------------------
@pytest.mark.parametrize(
    "text, lang",
    [
        ("moja", "swedish"),  # used to resolve to sw (Swahili)
        ("uno", "estonian"),  # used to resolve to es (Spanish)
        ("бір", "kazakh"),  # used to resolve to ka (Georgian)
        ("one", "enx"),  # used to resolve to en
        ("one", "english"),
    ],
)
def test_lang_names_and_prefixes_are_rejected_issue_197(text, lang):
    with pytest.raises(NotImplementedError, match="is not supported"):
        words2num(text, lang=lang)
    with pytest.raises(NotImplementedError, match="is not supported"):
        words2num_sentence(text, lang=lang)


# --- #199 / #201: keyword arguments and unknown `to` ------------------------
def test_words2num_unknown_kwarg_is_named_issue_199():
    with pytest.raises(TypeError, match="precision"):
        words2num("one", precision=2)


def test_sentence_unknown_kwarg_raises_issue_201():
    with pytest.raises(TypeError, match="foo"):
        words2num_sentence("twenty five", foo=1)


@pytest.mark.parametrize(
    "text, lang, to",
    [("twenty five", "en", "bogus"), ("vingt cinq", "fr", "cardnal")],
)
def test_sentence_unknown_to_raises_issue_201(text, lang, to):
    with pytest.raises(NotImplementedError, match="unsupported"):
        words2num_sentence(text, lang=lang, to=to)
    with pytest.raises(NotImplementedError, match="unsupported"):
        words2num(text, lang=lang, to=to)


def test_converter_types_are_all_accepted():
    assert "digits" in CONVERTER_TYPES
    for to in CONVERTER_TYPES:
        words2num_sentence("twenty five", to=to)


# --- #196: to="digits" ------------------------------------------------------
def test_digits_mode_issue_196():
    assert words2num("two seven five", to="digits") == 275
    # An int: the leading zero is dropped. Sentence mode keeps it.
    assert words2num("zero seven", to="digits") == 7
    assert words2num_sentence("my code is zero seven") == "my code is 07"


# --- #200: the CLI ----------------------------------------------------------
def test_cli_space_and_equals_forms_issue_200(capsys):
    assert main(["--lang", "fr", "trois"]) == 0
    assert capsys.readouterr().out.strip() == "3"
    assert main(["--lang=fr", "--to=ordinal", "troisième"]) == 0
    assert capsys.readouterr().out.strip() == "3"
    assert main(["forty", "two"]) == 0
    assert capsys.readouterr().out.strip() == "42"


@pytest.mark.parametrize(
    "argv, message",
    [
        (["forty", "zoot"], "unrecognized token 'zoot'"),
        (["--lang", "swedish", "en"], "is not supported"),
        (["--to", "bogus", "one"], "unsupported"),
    ],
)
def test_cli_reports_errors_without_traceback_issue_200(capsys, argv, message):
    assert main(argv) == 1
    captured = capsys.readouterr()
    assert captured.out == ""
    assert message in captured.err
    assert "Traceback" not in captured.err


def test_cli_module_entry_point_issue_200():
    ok = subprocess.run(
        [sys.executable, "-m", "words2num2", "--lang", "fr", "trois"],
        capture_output=True,
        text=True,
    )
    assert (ok.returncode, ok.stdout.strip()) == (0, "3")
    bad = subprocess.run(
        [sys.executable, "-m", "words2num2", "forty zoot"],
        capture_output=True,
        text=True,
    )
    assert bad.returncode == 1
    assert "Traceback" not in bad.stderr
    assert "zoot" in bad.stderr


def test_words2num_error_is_a_value_error():
    assert issubclass(Words2NumError, ValueError)


@pytest.mark.parametrize("text", ["①", "⑩", "²", "¹⁰", "1①", "10²", "½", "1½", "1⒈"])
def test_numerical_symbols_are_no_number_by_default(text):
    with pytest.raises(Words2NumError):
        words2num2.words2num(text)
    with pytest.raises(Words2NumError):
        words2num2.auto_parse(text)


def test_convert_numerical_symbols_opt_in():
    assert words2num2.words2num("①", convert_numerical_symbols=True) == 1
    assert words2num2.words2num("¹⁰", convert_numerical_symbols=True) == 10
    assert words2num2.auto_parse("⑩", convert_numerical_symbols=True).value == 10
    # Glued to a figure it is no number even when asked.
    for text in ["1①", "10²"]:
        with pytest.raises(Words2NumError):
            words2num2.words2num(text, convert_numerical_symbols=True)
    s = "the ⑤ million y"
    assert words2num2.words2num_sentence(s) == s
    assert words2num2.auto_parse_sentence("x ① y") == "x ① y"
    assert (
        words2num2.words2num_sentence(s, convert_numerical_symbols=True)
        == "the 5000000 y"
    )
    # The flag does not leak into the next call.
    with pytest.raises(Words2NumError):
        words2num2.words2num("①")


@pytest.mark.parametrize(
    "text",
    ["x ① y", "x ⑩ y", "x ⒈ y", "note ³ here", "x ₃ y", "x ⑴ y", "x Ⅻ y", "x ⅓ y"],
)
def test_numerical_symbols_stay_as_written_in_sentences(text):
    # #958: no fold, no strip, in either sentence walker.
    assert words2num2.words2num_sentence(text) == text
    assert words2num2.auto_parse_sentence(text) == text
    assert words2num2.auto_parse_sentence(text, expand=True) == text


def test_convert_numerical_symbols_scope_per_function(capsys):
    # The keyword brings back the earlier reading, no more: the English walker
    # reads symbols, other walkers and auto_parse_sentence never did.
    on = {"convert_numerical_symbols": True}
    assert words2num2.words2num_sentence("x ① y", lang="en_GB", **on) == "x 1 y"
    for lang in ["fr", "de", "en_IN"]:
        assert words2num2.words2num_sentence("x ① y", lang=lang, **on) == "x ① y"
    for text in ["x ⑤ kg y", "the ⑤ million y", "items ① ② ③"]:
        assert words2num2.auto_parse_sentence(text, **on) == text
        assert words2num2.auto_parse_sentence(text, expand=True, **on) == text
    # The CLI refuses them unless asked.
    assert main(["①"]) == 1
    assert main(["--convert-numerical-symbols", "①"]) == 0
    assert capsys.readouterr().out.strip() == "1"


@pytest.mark.parametrize(
    "text", ["x ① y", "note ³ here", "⒈", "㊀", "x Ⅻ y", "½", "5 m²"]
)
def test_normalize_keeps_numerical_symbols(text):
    # #972: the public normalize() converts no symbol unless asked.
    assert words2num2.normalize(text) == text


def test_normalize_folds_symbols_on_request():
    assert words2num2.normalize("x ① y", convert_numerical_symbols=True) == "x 1 y"
    assert words2num2.normalize("Forty-Two ①") == "forty two ①"
    assert words2num2.normalize("x  ① y") == "x  ① y"


@pytest.mark.parametrize(
    "text, lang, expected",
    [
        ("x five​ y", "en", "x 5​ y"),
        ("five­ apples", "en", "5­ apples"),
        ("x fünf⁠ y", "de", "x 5⁠ y"),
    ],
)
def test_bare_format_char_after_number_is_kept_issue_1005(text, lang, expected):
    assert words2num2.words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "text, kwargs, expected",
    [
        ("it costs twelve\u200b dollars", {"currency": True}, "it costs $12\u200b"),
        ("x 20\u200b dollars", {"currency": True}, "x $20\u200b"),
        ("on the fifth\u200b of march", {"date": True}, "on March 5"),
    ],
)
def test_folds_still_read_a_format_char_value_issue_1021(text, kwargs, expected):
    assert words2num2.words2num_sentence(text, **kwargs) == expected


@pytest.mark.parametrize(
    "text, lang, expected",
    [("moins: 1.234", "fr", -1234), ("minus: 1,234", "en", -1234)],
)
def test_sign_word_mark_space_figure_issue_1032(text, lang, expected):
    assert words2num(text, lang=lang) == expected


@pytest.mark.parametrize(
    "text, lang, expected",
    [
        ("x \u200bfive y", "en", "x \u200b5 y"),
        ("twenty.\u200e One", "en", "20.\u200e One"),
        ("x five \u200bapples y", "en", "x 5 \u200bapples y"),
        ("ninety \u200enine cents", "en", "99 cents"),
        ("x \u200bthird y", "en", "x \u200b3rd y"),
        ("x twenty-first\u200e y", "en", "x 21st\u200e y"),
        ("x fife \u200b seven", "en_AERO", "x 5 \u200b 7"),
        ("x one hundred \u200b and five y", "en", "x 105 y"),
        ("one hundred\u200bfive", "en", "105"),
        ("x 二十\u200b,一 y", "zh", "x 20\u200b,一 y"),
        ("x three point one \u200b four y", "en", "x 3.14 y"),
        ("x one \u200b two y", "en", "x 1\u200b 2 y"),
        ("x 二十 \u200e二十 y", "zh", "x 20 \u200e20 y"),
        # A spaced format-only token after a quoted number keeps the quote
        # (#1115, #1117).
        ('x "five" \u200b y', "en", 'x "5" \u200b y'),
        ('he said "five" \u200e and left', "en", 'he said "5" \u200e and left'),
        # ar / he: a format character before the conjunction (#1116).
        ("x خمسمائة \u200eوخمسة وخمسون y", "ar", "x 555 y"),
        ("x ألفان \u200fوأربعة وعشرون y", "ar", "x 2024 y"),
        ("x מאה \u200eועשרים ושלוש y", "he", "x 123 y"),
        # A format character before a scale word with its comma (#1121).
        ("x twelve \u200bthousand, three hundred and forty-five y", "en", "x 12345 y"),
        # A format character before a leading mark rides with the mark
        # (#1125, #1129); after a glued sign it stays after the sign (#1126).
        ("x forty \u200e(two) y", "en", "x 40 \u200e(2) y"),
        ("x forty \u200b(two) y", "en", "x 40 \u200b(2) y"),
        ("x vingt \u200f(cinq) y", "fr", "x 20 \u200f(5) y"),
        ("x -\u200etwenty five y", "en", "x -\u200e25 y"),
        # Between two leading marks, before a sign, before a zh mark
        # (#1138, #1139, #1140).
        ("x '\u200e'twenty five y", "en", "x '\u200e'25 y"),
        ("x '\u200b'twenty five y", "en", "x '\u200b'25 y"),
        ("x —\u200e—twenty five y", "en", "x —\u200e—25 y"),
        ("x \u200e-\u200efive y", "en", "x \u200e-\u200e5 y"),
        # The fullwidth and small minus too (#1083 audit).
        ("x \u200e－\u200etwenty five y", "en", "x \u200e-\u200e25 y"),
        ("x ﹣\u200etwenty five y", "en", "x -\u200e25 y"),
        ("x ⁻\u200etwenty five y", "en", "x -\u200e25 y"),
        ("x ‒\u200etwenty five y", "en", "x -\u200e25 y"),
        ("四十 \u200b.七", "zh", "40 \u200b.七"),
    ],
)
def test_leading_format_char_and_bidi_marks_issue_1000(text, lang, expected):
    assert words2num2.words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "text, kwargs, expected",
    [
        ("x \u200bfive dollars", {"currency": True}, "x \u200b$5"),
        ("twelve dollars \u200bfifty", {"currency": True}, "$12.50\u200b"),
        ("on the \u200bfifth of march", {"date": True}, "on March 5"),
        ("x 二十 \u200e二十 y", {"date": True, "lang": "zh"}, "x 20 \u200e20 y"),
        (
            "call zero \u200b six one two three four five six seven eight",
            {"phone": True},
            "call (061) 234 5678",
        ),
        # A spaced format-only token stays part of the fold (#1107, #1109).
        ("it costs twelve \u200b dollars", {"currency": True}, "it costs $12\u200b"),
        (
            "twenty dollars and five \u200b cents please",
            {"currency": True},
            "$20.05\u200b please",
        ),
        (
            "el primero \u200b de enero de dos mil veinte",
            {"lang": "es"},
            "el 1º\u200b de enero de 2020",
        ),
        (
            "el primero \u200b de enero de dos mil veinte",
            {"lang": "es", "date": True},
            "el 1 de enero de 2020",
        ),
        ("hello \u200b world", {"phone": True}, "hello \u200b world"),
        # Two spaced tokens after a quote, in every mode (#1115 audit).
        ('x "five" \u200e \u200e y', {"date": True}, 'x "5" \u200e \u200e y'),
        ('x "five" \u200b \u200b y', {"phone": True}, 'x "5" \u200b \u200b y'),
    ],
)
def test_leading_format_char_keeps_folds_issue_1000(text, kwargs, expected):
    assert words2num2.words2num_sentence(text, **kwargs) == expected
