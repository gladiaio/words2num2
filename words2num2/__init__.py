# -*- coding: utf-8 -*-
# Copyright (c) 2026, Jean-Louis Queguiner. All Rights Reserved.
#
# This library is free software; you can redistribute it and/or
# modify it under the terms of the GNU Lesser General Public
# License as published by the Free Software Foundation; either
# version 2.1 of the License, or (at your option) any later version.
"""words2num2 — a thin Python binder over the Rust conversion core.

The inverse of num2words2: parse spoken-form numbers ("forty-two", "trois
cent quatre", ...) back into numeric values across the same 100+ locales.

Every conversion is served by the compiled ``_rust`` extension. This module
only surfaces the public names and passes each call straight through — the
locale resolution, the English grammar, the reverse-table lookup for the other
119 locales, and the sentence/auto-parse walkers all live in the core. There is
no pure-Python conversion or dispatch: an input the core declines raises rather
than silently diverging.

>>> from words2num2 import words2num
>>> words2num("forty-two")
42
>>> words2num("one thousand two hundred")
1200
>>> words2num("three point one four")
Decimal('3.14')
>>> words2num("trente-deux", lang="fr")
32
"""
from __future__ import unicode_literals

# The compiled core is mandatory: this package is a binder over it. A source
# checkout must build the extension (maturin develop) — an ImportError here is
# the honest signal that it has not been built.
from . import _rust as _RUST  # noqa: E402

# Exception + result types the core references by name (see
# rust/words2num2-py/src/lib.rs: `words2num_error` imports
# `words2num2.base.Words2NumError`, `quantity_to_py` imports
# `words2num2.converters.auto.Quantity`). Re-exported so the historic public
# import surface keeps working.
from .base import Words2NumError  # noqa: E402
from .converters.auto import Quantity  # noqa: E402

try:
    from ._version import __version__, __version_tuple__
except ImportError:
    # Package is not installed, provide defaults
    __version__ = "unknown"
    __version_tuple__ = (0, 0, 0, "unknown", 0)


__all__ = [
    "words2num",
    "words2num_sentence",
    "convert_sentence",
    "sentence_to_words",
    "auto_parse",
    "auto_parse_sentence",
    "parse_number_string",
    "normalize",
    "supported_langs",
    "Quantity",
    "Words2NumError",
]


def words2num(text, lang="en", to="cardinal", **kwargs):
    """Parse ``text`` (a number written in words) into a numeric value.

    Args:
        text: The words to parse, e.g. ``"forty-two"``.
        lang: Language code (default ``"en"``).
        to: Conversion type — ``cardinal``, ``ordinal``, ``ordinal_num``,
            ``year``, or ``currency``.

    Returns:
        ``int``, ``float`` or ``Decimal`` depending on the input. ``to="currency"``
        returns the amount as a ``Decimal``, unit plus subunit
        (``"twelve euros, thirty-four cents"`` -> ``Decimal("12.34")``); text
        that names no currency is read as a plain cardinal. A number written
        in figures is read with the language's separators (``"1,234"`` ->
        1234 in en, ``"2,5"`` -> ``Decimal("2.5")`` in de).

    Raises:
        NotImplementedError: if ``lang`` or ``to`` is not supported.
        Words2NumError: if ``text`` cannot be parsed.
    """
    return _RUST.words2num(text, lang, to, kwargs or None)


def words2num_sentence(sentence, lang="en", to="cardinal", currency=False, **kwargs):
    """Convert every word-number in ``sentence`` to numeric form.

    Walks the sentence, matching the longest run of number tokens at each
    position; non-number tokens pass through. The walk is a full port in the
    core; this is a thin binder over it.

    With ``currency=True`` an amount spoken as number + currency word
    (+ subunit) is written the way the language writes that currency:
    ``"twelve dollars and fifty cents"`` -> ``"$12.50"``, fr ``"quarante-trois
    dollars et vingt centimes"`` -> ``"43,20 $"``, de ``"zwanzig euro fünfzig"``
    -> ``"20,50 €"``. Off by default: the output is unchanged.
    """
    return _RUST.words2num_sentence(sentence, lang, to, kwargs or None, currency)


# Aliases (parity with num2words2)
convert_sentence = words2num_sentence
sentence_to_words = words2num_sentence


def auto_parse(text, lang="en", prefer=None, thousands_sep=None,
               decimal_sep=None):
    """Parse a single quantity expression, e.g. ``"$12,345.00"`` or ``"5cm"``.

    Returns a :class:`Quantity`; raises ``Words2NumError`` if unparseable or
    if ``thousands_sep == decimal_sep``, and ``NotImplementedError`` for an
    unsupported ``lang``. A digit-form decimal is an exact ``Decimal`` and a
    scale suffix shifts its point (``"$4.1bn"`` -> ``4100000000``).

    ``prefer`` disambiguates ambiguous unit tokens (``{"m": "mile"}``,
    ``{"m": "minute"}``): the preferred unit replaces the whole reading
    (``unit``, ``unit_long``, ``kind``). A target that is not a reading of the
    token (``{"m": "kg"}``) is ignored.
    """
    return _RUST.auto_parse(text, lang, prefer, thousands_sep, decimal_sep)


def auto_parse_sentence(text, lang="en", prefer=None, thousands_sep=None,
                        decimal_sep=None, expand=False):
    """Walk free text and rewrite every quantity expression in place.

    Each match becomes ``"<value> <unit>"``; with ``expand=True`` units use
    their long form. Returns the rewritten string.
    """
    return _RUST.auto_parse_sentence(
        text, lang, prefer, thousands_sep, decimal_sep, expand)


def parse_number_string(s, thousands_sep=None, decimal_sep=None, lang=None):
    """Parse a numeric string with configurable / per-locale separators.

    Resolution order (all in the core): explicit separators (which must
    differ), then the ``lang`` locale's defaults, then auto-detection from the
    string itself. A dot or comma followed by 1, 2 or 4+ digits is a decimal in
    any locale (``"3.14"`` with ``lang="fr"`` is 3.14); ``"1.234"`` stays a
    thousands group where the locale writes decimals with a comma.

    Returns an ``int``, or an exact ``Decimal`` for a decimal.
    """
    return _RUST.parse_number_string(s, thousands_sep, decimal_sep, lang)


def normalize(text):
    """Normalize ``text`` the way the core does before a reverse lookup
    (NFKD, lowercase, strip diacritics, collapse whitespace, drop hyphens)."""
    return _RUST.normalize(text)


def supported_langs():
    """Return the locale codes the reverse-table backend can serve."""
    return _RUST.supported_langs()
