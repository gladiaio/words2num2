# -*- coding: utf-8 -*-
"""Multi-language smoke tests via the generic num2words2-backed backend.

These tests require ``num2words2`` to be installed.
"""
import pytest

num2words2 = pytest.importorskip("num2words2")

from words2num2 import words2num


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("fr", "quarante-deux", 42),
        ("es", "cuarenta y dos", 42),
        ("de", "zweiundvierzig", 42),
        ("it", "quarantadue", 42),
        ("pt", "quarenta e dois", 42),
        ("nl", "tweeenveertig", 42),
        ("ru", "сорок два", 42),
        ("pl", "czterdziesci dwa", 42),
    ],
)
def test_multilang_42(lang, text, expected):
    assert words2num(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,n",
    [
        ("fr", 1),
        ("fr", 100),
        ("fr", 999),
        ("es", 1),
        ("es", 500),
        ("de", 1),
        ("de", 73),
        ("it", 1),
        ("it", 200),
        ("pt", 1),
        ("nl", 1),
    ],
)
def test_multilang_roundtrip(lang, n):
    """Forward num2words → back to int via words2num — must roundtrip."""
    words = num2words2.num2words(n, lang=lang)
    assert words2num(words, lang=lang) == n


# ---------------------------------------------------------------------------
# The accepted language set is derived from the renderer, so it cannot drift
# from the advertised one. Before that, `supported_langs()` listed 172 codes
# while the resolver knew 120: 28 raised NotImplementedError and 8 more were
# silently routed to a different language by the two-character prefix
# fallback. See gladiaio/words2num2#18.
# ---------------------------------------------------------------------------

from words2num2 import supported_langs  # noqa: E402

# `ti` renders in Ge'ez from num2words2 >= the release carrying
# gladiaio/num2words2#134, but the reverse tables are built from the pinned
# `num2words2-core` dependency, which still renders the old Latin
# transliteration. It round-trips again once that pin is bumped — see #14.
# This list may shrink; it must never grow.
KNOWN_ROUNDTRIP_GAPS = {"ti"}


def _roundtrip_param(lang):
    """One param per language, strict-xfail for the known gaps.

    strict=True makes this a ratchet in both directions: a gap language that
    starts working reports XPASS and fails the run, which is the prompt to
    delete it from KNOWN_ROUNDTRIP_GAPS. The list can only shrink.
    """
    if lang in KNOWN_ROUNDTRIP_GAPS:
        return pytest.param(
            lang,
            marks=pytest.mark.xfail(
                strict=True,
                reason="gladiaio/words2num2#14 — reverse tables come from the "
                       "pinned num2words2-core, which still renders this "
                       "language in Latin transliteration",
            ),
        )
    return lang


@pytest.mark.slow
@pytest.mark.parametrize("lang", [_roundtrip_param(code) for code in sorted(supported_langs())])
def test_roundtrip_every_language(lang):
    """num2words2 -> words2num2 is the identity for every advertised language.

    This is simultaneously the registry test: a language that `supported_langs()`
    advertises but the resolver rejects raises NotImplementedError here.
    """
    for value in (0, 1, 11, 42, 100, 1000):
        words = num2words2.num2words(value, lang=lang)
        assert words2num(words, lang=lang) == value, f"{lang}: {value} -> {words!r}"


@pytest.mark.parametrize(
    "code",
    [
        # Each of these used to be swallowed by the two-character prefix
        # fallback and answered by an unrelated language.
        "ban",   # was -> ba  (Bashkir)
        "ceb",   # was -> ce  (Chechen)
        "cnh",   # was -> cn  (Chinese)
        "fil",   # was -> fi  (Finnish)
        "kok",   # was -> ko  (Korean)
        "miz",   # was -> mi  (Maori)
        "pap",   # was -> pa  (Punjabi)
        "pli",   # was -> pl  (Polish)
        # Script variants that were collapsed onto their base language.
        "sr_Latn",
        "uz_Cyrl",
    ],
)
def test_codes_are_not_misresolved_to_another_language(code):
    """A known code must be answered by itself, not by a prefix match."""
    assert words2num(num2words2.num2words(42, lang=code), lang=code) == 42
