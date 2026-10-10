# -*- coding: utf-8 -*-
"""Tests for auto_parse, auto_parse_sentence, parse_number_string."""

from decimal import Decimal

import pytest

from words2num2 import (
    Quantity,
    auto_parse,
    auto_parse_sentence,
    parse_number_string,
)
from words2num2.base import Words2NumError

# ---------------------------------------------------------------------------
# Currency
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "text,value,unit,kwargs",
    [
        ("$12.50", Decimal("12.50"), "USD", {}),
        ("$12,345.00", Decimal("12345.00"), "USD", {}),
        ("€42", 42, "EUR", {}),
        (
            "€12,50",
            Decimal("12.50"),
            "EUR",
            {"lang": "de"},
        ),  # comma decimal in DE locale
        ("£1,000.99", Decimal("1000.99"), "GBP", {}),
        ("¥500", 500, "JPY", {}),
        ("USD 100", 100, "USD", {}),
        ("100 USD", 100, "USD", {}),
        ("12.50 €", Decimal("12.50"), "EUR", {}),
    ],
)
def test_currency(text, value, unit, kwargs):
    q = auto_parse(text, **kwargs)
    assert q.value == value
    assert q.unit == unit
    assert q.kind == "currency"


@pytest.mark.parametrize(
    "text,value",
    [
        ("$5k", 5_000),
        ("$5m", 5_000_000),
        ("$5b", 5_000_000_000),
        ("$5bn", 5_000_000_000),
        ("$1.5b", 1_500_000_000),
        ("$2.5t", 2_500_000_000_000),
    ],
)
def test_currency_scale_shortcuts(text, value):
    q = auto_parse(text)
    assert q.value == value
    assert q.kind == "currency"


# ---------------------------------------------------------------------------
# Units
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "text,value,unit,kind",
    [
        ("5cm", 5, "cm", "length"),
        ("5 cm", 5, "cm", "length"),
        ("3.5 km", Decimal("3.5"), "km", "length"),
        ("100mm", 100, "mm", "length"),
        ("42%", 42, "%", "percent"),
        ("70 %", 70, "%", "percent"),
        ("20°C", 20, "°C", "temperature"),
        ("-40°F", -40, "°F", "temperature"),
        ("5kg", 5, "kg", "mass"),
        ("250ml", 250, "ml", "volume"),
        ("3L", 3, "L", "volume"),
        ("30min", 30, "min", "time"),
        ("100ms", 100, "ms", "time"),
    ],
)
def test_unit_digit_form(text, value, unit, kind):
    q = auto_parse(text)
    assert q.value == value
    assert q.unit == unit
    assert q.kind == kind


@pytest.mark.parametrize(
    "text,value,unit",
    [
        ("forty-two kg", 42, "kg"),
        ("twenty-three cm", 23, "cm"),
        ("five centimeters", 5, "cm"),
        ("ten kilometers", 10, "km"),
        ("twelve dollars", 12, "USD"),
        ("twenty-three percent", 23, "%"),
    ],
)
def test_unit_word_form(text, value, unit):
    q = auto_parse(text)
    assert q.value == value
    assert q.unit == unit


def test_unit_long_expansion():
    q = auto_parse("5cm")
    assert q.unit_long == "centimeter"
    q = auto_parse("$12.50")
    assert q.unit_long == "dollar"


def test_prefer_overrides_ambiguous_unit():
    q = auto_parse("5m", prefer={"m": "mile"})
    assert q.value == 5
    assert q.unit_long == "mile"
    assert q.unit == "mi"  # the whole unit, not just its long form (#203)


# ---------------------------------------------------------------------------
# Number-string parsing with separators
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "text,kwargs,expected",
    [
        ("12,345.67", {}, Decimal("12345.67")),
        ("12.345,67", {"lang": "de"}, Decimal("12345.67")),
        ("1 234,56", {"lang": "fr"}, Decimal("1234.56")),
        ("12'345.67", {"thousands_sep": "'", "decimal_sep": "."}, Decimal("12345.67")),
        ("1_234.56", {"thousands_sep": "_"}, Decimal("1234.56")),
        ("1,234,567.89", {}, Decimal("1234567.89")),
        ("1.234.567,89", {"lang": "de"}, Decimal("1234567.89")),
        ("-12,345.67", {}, Decimal("-12345.67")),
        ("+1.5", {}, Decimal("1.5")),
        ("0.5", {}, Decimal("0.5")),
    ],
)
def test_parse_number_string(text, kwargs, expected):
    assert parse_number_string(text, **kwargs) == expected


def test_parse_number_string_invalid():
    with pytest.raises(Words2NumError):
        parse_number_string("")
    with pytest.raises(Words2NumError):
        parse_number_string("not a number")


# ---------------------------------------------------------------------------
# Sentence mode
# ---------------------------------------------------------------------------


def test_sentence_currency_and_unit():
    out = auto_parse_sentence("The package weighs 5kg and costs $12.50.")
    assert "5 kg" in out
    assert "12.5 USD" in out


def test_sentence_preserves_punctuation():
    out = auto_parse_sentence("Cost: $12.50, weight: 30kg.")
    assert "12.5 USD" in out
    assert "30 kg" in out
    assert ", " in out  # comma+space preserved between
    assert out.endswith(".")


def test_sentence_percent_no_space():
    out = auto_parse_sentence("Humidity 70%.")
    assert "70%" in out


@pytest.mark.parametrize(
    "value,singular,expected",
    [
        (1, "dollar", "dollar"),
        (0, "dollar", "dollars"),
        (5, "dollar", "dollars"),
        (1.5, "dollar", "dollars"),
        (-1, "dollar", "dollar"),
        (-5, "dollar", "dollars"),
        (5, "foot", "feet"),
        (1, "foot", "foot"),
        (5, "inch", "inches"),
        (1, "inch", "inch"),
        (5, "kilogram", "kilograms"),
        (5, "yen", "yen"),
        (1, "yen", "yen"),
        (5, "yuan", "yuan"),
        (5, "won", "won"),
        (5, "kelvin", "kelvin"),
        (5, "percent", "percent"),
        (5, "degree celsius", "degrees celsius"),
        (5, "pound sterling", "pounds sterling"),
        (5, "Swiss franc", "Swiss francs"),
    ],
)
def test_pluralize(value, singular, expected):
    from words2num2.converters.auto import pluralize

    assert pluralize(singular, value) == expected


def test_sentence_expand_pluralizes():
    out = auto_parse_sentence("Pay $12.50 for 5kg.", expand=True)
    assert "dollars" in out
    assert "kilograms" in out

    out = auto_parse_sentence("Pay $1.00 for 1kg.", expand=True)
    assert "dollar" in out and "dollars" not in out
    assert "kilogram" in out and "kilograms" not in out

    out = auto_parse_sentence("5 ft and 1 ft.", expand=True)
    assert "5 feet" in out
    assert "1 foot" in out

    out = auto_parse_sentence("Pay 100 yen.", expand=True)
    assert "100 yen" in out


def test_sentence_expand_mode():
    out = auto_parse_sentence("Pay $12.50 for 5kg.", expand=True)
    assert "dollar" in out
    assert "kilogram" in out


def test_sentence_locale_separators_de():
    out = auto_parse_sentence("Total: 1.234,56 €.", lang="de")
    assert "1234.56" in out
    assert "EUR" in out


def test_sentence_locale_separators_fr():
    out = auto_parse_sentence("Total : 1 234,56 €.", lang="fr")
    assert "1234.56" in out
    assert "EUR" in out


# ---------------------------------------------------------------------------
# Quantity dataclass
# ---------------------------------------------------------------------------


def test_quantity_repr_with_unit():
    q = Quantity(value=5, unit="kg", kind="mass")
    assert "kg" in repr(q)
    assert "mass" in repr(q)


def test_quantity_repr_plain():
    q = Quantity(value=42)
    assert "42" in repr(q)


# ---------------------------------------------------------------------------
# October 2026 audit
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "text,kwargs,expected",
    [
        # A dot followed by 1, 2 or 4+ digits is a decimal in any locale (#53).
        ("3.14", {"lang": "fr"}, Decimal("3.14")),
        ("0.5", {"lang": "es"}, Decimal("0.5")),
        ("1.5", {"lang": "de"}, Decimal("1.5")),
        ("12.5", {"lang": "ru"}, Decimal("12.5")),
        ("1,5", {"lang": "en"}, Decimal("1.5")),
        ("1.234", {"lang": "fr"}, 1234),  # a thousands group in fr
        # Hyphenated / unknown languages (#54).
        ("1.234,56", {"lang": "pt-BR"}, Decimal("1234.56")),
        ("1.234,5", {"lang": "id"}, Decimal("1234.5")),
        ("1,5", {"lang": "id"}, Decimal("1.5")),
        # A leading group of 4+ digits is not grouped (#55).
        ("1234.567", {}, Decimal("1234.567")),
        ("1234,567", {}, Decimal("1234.567")),
        # Swiss typographic apostrophe (#109).
        ("1\u2019234.5", {"lang": "de_CH"}, Decimal("1234.5")),
        ("1\u2019234,5", {"lang": "fr_CH"}, Decimal("1234.5")),
        # Exact (#71).
        ("12345678901234567.89", {}, Decimal("12345678901234567.89")),
    ],
)
def test_parse_number_string_audit(text, kwargs, expected):
    got = parse_number_string(text, **kwargs)
    assert got == expected
    assert type(got) is type(expected)


def test_parse_number_string_rejects_bad_grouping():
    # "1.5.2026" is a date, not 152026 (#55).
    with pytest.raises(Words2NumError):
        parse_number_string("1.5.2026")


@pytest.mark.parametrize(
    "call",
    [
        lambda: parse_number_string("1.5", thousands_sep=".", decimal_sep="."),
        lambda: parse_number_string("1,5", thousands_sep=",", decimal_sep=","),
        lambda: auto_parse("$1,5", thousands_sep=",", decimal_sep=","),
        lambda: auto_parse_sentence("pay $1,5 now", thousands_sep=",", decimal_sep=","),
    ],
)
def test_identical_separators_raise_value_error(call):
    # #188: Words2NumError is a ValueError.
    with pytest.raises(ValueError):
        call()


def test_scale_suffix_is_exact():
    # #71
    assert auto_parse("$4.1bn").value == 4_100_000_000
    assert auto_parse("$1.005k").value == 1005
    assert auto_parse_sentence("cost $4.1bn total") == "cost 4100000000 USD total"
    assert auto_parse_sentence("$12345678901234567.89") == "12345678901234567.89 USD"


def test_scale_suffix_spellings_never_key_error():
    # #194
    assert auto_parse("$5Mn").value == 5_000_000
    assert auto_parse("$5TN").value == 5_000_000_000_000
    with pytest.raises(Words2NumError):
        auto_parse("$5kn")
    assert auto_parse_sentence("it cost $5kn today") == "it cost $5kn today"


def test_prefer_swaps_unit_and_kind():
    # #203
    q = auto_parse("5m", prefer={"m": "minute"})
    assert (q.unit, q.unit_long, q.kind) == ("min", "minute", "time")
    assert auto_parse_sentence("ran 5m", prefer={"m": "minute"}) == "ran 5 min"
    # Not a reading of "m": ignored.
    q = auto_parse("5m", prefer={"m": "kg"})
    assert (q.unit, q.unit_long, q.kind) == ("m", "meter", "length")


@pytest.mark.parametrize(
    "call",
    [
        lambda: auto_parse_sentence("forty two kg", lang="xx"),
        lambda: auto_parse("$5", lang="xx"),
        lambda: auto_parse("forty", lang="xx"),
    ],
)
def test_unknown_lang_raises_not_implemented(call):
    # #204
    with pytest.raises(NotImplementedError):
        call()


# ---------------------------------------------------------------------------
# Regional separators reach every entry point (#423)
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("es_MX", "1,234", 1234),
        ("es-MX", "1,234", 1234),
        ("es_US", "1,234", 1234),
        ("es_419", "1,234", 1234),
        ("de_CH", "1.234", Decimal("1.234")),
        ("de_CH", "1'234", 1234),
        ("es", "1.234", 1234),
        ("km", "1,234", 1234),
        ("hi", "12,34,567.5", Decimal("1234567.5")),
    ],
)
def test_regional_separators_in_words2num_and_auto_parse(lang, text, expected):
    from words2num2 import words2num

    assert words2num(text, lang=lang) == expected
    assert auto_parse(text, lang=lang).value == expected


def test_explicit_separators_never_fall_back_to_a_float():
    with pytest.raises(Words2NumError):
        auto_parse("1.5", lang="en", decimal_sep=",")


def test_lowercase_region_in_parse_number_string():
    assert parse_number_string("1,234", lang="es-mx") == 1234
    assert parse_number_string("12,34,567", lang="en-in") == 1234567


def test_comma_after_sign_word_is_punctuation():
    from words2num2 import words2num

    assert words2num("moins, 1.234", lang="fr") == -1234
    assert words2num("minus, 1.234", lang="de") == -1234


def test_auto_parse_sign_word_keeps_separators_and_region():
    assert auto_parse("moins 1.234", lang="fr").value == -1234
    assert auto_parse("moins 1.234", lang="fr", decimal_sep=".").value == Decimal(
        "-1.234"
    )
    assert auto_parse("menos 1.234", lang="es_MX").value == Decimal("-1.234")
    assert auto_parse("menos 1,234", lang="es_MX").value == -1234
