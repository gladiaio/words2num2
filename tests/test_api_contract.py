# -*- coding: utf-8 -*-
"""The public call contract: language codes, ``to=`` modes, keyword
arguments and the command-line entry point."""

import subprocess
import sys

import pytest

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
