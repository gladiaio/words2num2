# -*- coding: utf-8 -*-
"""The date pass of sentence mode (``words2num_sentence(..., date=True)``).

A day + month (+ year) is written the way the language writes a date. Off by
default: the output is the plain sentence-mode one.
"""

import pytest

from words2num2 import words2num_sentence


def dates(text, lang):
    return words2num_sentence(text, lang=lang, date=True)


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # en (US): month first, comma before the year, no ordinal suffix.
        ("en", "on march twelfth twenty twenty six", "on March 12, 2026"),
        ("en", "the first of march", "March 1"),
        ("en", "on the twenty first of june nineteen ninety.", "on June 21, 1990."),
        ("en", "december one twenty twenty five", "December 1, 2025"),
        ("en", "march the twelfth", "March 12"),
        ("en", "may the fourth", "May 4"),
        ("en", "june twelve", "June 12"),
        ("en", "in march twenty twenty six", "in March 2026"),
        ("en", "see you march twelfth, ok", "see you March 12, ok"),
        # en (GB, AU...): day first.
        ("en_GB", "the twelfth of march twenty twenty six", "12 March 2026"),
        ("en_AU", "on march twelfth", "on 12 March"),
        # fr: "1er", lower-case month.
        ("fr", "le premier avril deux mille vingt-six", "le 1er avril 2026"),
        ("fr", "le douze Mars", "le 12 mars"),
        ("fr", "le vingt et un juin", "le 21 juin"),
        # de: "12.", capitalised month.
        ("de", "am zwölften märz zweitausendsechsundzwanzig", "am 12. März 2026"),
        ("de", "am ersten Mai", "am 1. Mai"),
        # es: no ordinal mark; pt keeps "1º".
        ("es", "el primero de mayo", "el 1 de mayo"),
        ("es", "el doce de marzo de dos mil veintiséis", "el 12 de marzo de 2026"),
        ("pt", "doze de março de dois mil e vinte e seis", "12 de março de 2026"),
        ("it", "il primo maggio", "il 1º maggio"),
        ("it", "il dodici marzo duemilaventisei", "il 12 marzo 2026"),
        ("nl", "twaalf maart tweeduizend zesentwintig", "12 maart 2026"),
    ],
)
def test_date_written_form(lang, text, expected):
    assert dates(text, lang) == expected


@pytest.mark.parametrize(
    "lang,text",
    [
        # Month names that are everyday words need more than a bare number.
        ("en", "I may two things"),
        ("en", "they march twelve miles"),
        ("en", "march twelve"),
        ("en", "you may go"),
        ("en", "twelve people"),
        ("en", "page twelve of the march report"),
        ("fr", "douze personnes"),
        # A language with no date style passes through.
        ("pl", "dwunastego marca"),
    ],
)
def test_no_date_no_change(lang, text):
    assert dates(text, lang) == words2num_sentence(text, lang=lang)


def test_off_by_default():
    assert words2num_sentence("march twelfth") == "march 12th"


def test_date_and_currency_together():
    out = words2num_sentence(
        "on march twelfth I paid ten dollars and fifty cents", currency=True, date=True
    )
    assert out == "on March 12 I paid $10.50"


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # All in numbers: the region's order and separator.
        ("en", "born on three twelve twenty twenty six", "born on 3/12/2026"),
        ("en_GB", "three twelve twenty twenty six", "03/12/2026"),
        ("en", "oh three slash twelve slash twenty twenty six.", "3/12/2026."),
        ("fr", "né le douze trois deux mille vingt-six", "né le 12/03/2026"),
        ("de", "am zwölften dritten zweitausendsechsundzwanzig", "am 12.03.2026"),
        ("es", "el doce del tres de dos mil veintiséis", "el 12/03/2026"),
        ("en", "two twenty nine twenty twenty four", "2/29/2024"),
        # Not a calendar date, no year: left as it is.
        ("en", "thirteen twelve twenty twenty six", "13 12 2026"),
        ("en", "two twenty nine twenty twenty five", "2 29 2025"),
        ("en", "three twelve", "3 12"),
        # Three bare numbers inside a sentence, no date cue: quantities (#1031).
        ("en", "we need three twelve two thousand people", "we need 3 12 2000 people"),
        ("en", "we sold three twelve two thousand and six", "we sold 3 12 2006"),
        ("es", "tres de doce dos mil personas", "3 de 12 2000 personas"),
        (
            "de",
            "wir brauchen drei zwölf zweitausend leute",
            "wir brauchen 3 12 2000 leute",
        ),
        (
            "fr",
            "il faut trois douze deux mille personnes",
            "il faut 3 12 2000 personnes",
        ),
        ("en", "on three twelve twenty twenty six", "on 3/12/2026"),
        (
            "de",
            "wir treffen uns zwölften dritten zweitausendsechsundzwanzig",
            "wir treffen uns 12.03.2026",
        ),
        (
            "en",
            "from three twelve twenty twenty six to four one twenty twenty seven",
            "from 3/12/2026 to 4/1/2027",
        ),
        (
            "es",
            "del doce tres dos mil veintiséis al quince tres dos mil veintiséis",
            "del 12/03/2026 al 15/03/2026",
        ),
        ("en", "Date: three twelve twenty twenty six", "Date: 3/12/2026"),
        (
            "en",
            "Meeting moved. three twelve twenty twenty six.",
            "Meeting moved. 3/12/2026.",
        ),
        ("en", "three twelve twenty twenty six at three thirty", "3/12/2026 at 3:30"),
        ("de", "bis zum 13. 3. 2026", "bis zum 13.03.2026"),
        (
            "nl",
            "van 12 3 2026 tot en met 13 3 2026",
            "van 12-03-2026 tot en met 13-03-2026",
        ),
        ("pt", "de 12 3 2026 a 13 3 2026", "de 12/03/2026 a 13/03/2026"),
        # A round year with a word after it is a quantity, even after a cue.
        ("en", "by three twelve two thousand people", "by 3 12 2000 people"),
        ("de", "ab drei zwölf zweitausend Euro", "ab 3 12 2000 Euro"),
        ("en", "on three twelve twenty twenty six I paid", "on 3/12/2026 I paid"),
        # "Alone" needs a sentence end or a real clock time (#1057, #1058).
        ("en", "three twelve two thousand at most", "3 12 2000 at most"),
        ("en", "3 12 2000, 4 5 1999, 7 8 2001", "3 12 2000, 4 5 1999, 7 8 2001"),
        ("fr", "Résultat : 3 12 2000.", "Résultat : 3 12 2000."),
        ("en", "three twelve twenty twenty six at noon", "3/12/2026 at noon"),
        # A date word always gives a date; pt "de" alone only opens a range.
        (
            "en",
            "born on three twelve two thousand in Paris",
            "born on 3/12/2000 in Paris",
        ),
        ("en", "DOB: 3 12 2000, male", "DOB: 3/12/2000, male"),
        ("en", "from 3 12 2000 to 4 12 2000", "from 3/12/2000 to 4/12/2000"),
        ("pt", "um grupo de três doze dois mil e dez", "um grupo de 3 12 2010"),
        # A range word needs a second date after it (#1051 N1).
        (
            "pt",
            "precisamos de três doze dois mil a cada mês",
            "precisamos de 3 12 2000 a cada mês",
        ),
        (
            "en",
            "from three twelve two thousand to five thousand people",
            "from 3 12 2000 to 5000 people",
        ),
        (
            "nl",
            "van 12 3 2000 tot en met 13 3 2000",
            "van 12-03-2000 tot en met 13-03-2000",
        ),
        ("it", "il dodici tre duemilaventisei", "il 12/03/2026"),
        ("de", "vom 12 3 2000 zum 13 3 2000", "vom 12.03.2000 zum 13.03.2000"),
        # A bare article is no range link (#1076).
        (
            "en",
            "on March fifth twenty twenty six the three twelve two thousand people voted",
            "on March 5, 2026 the 3 12 2000 people voted",
        ),
        (
            "en",
            "on 3 12 2026 the 4 5 2000 votes were counted",
            "on 3/12/2026 the 4 5 2000 votes were counted",
        ),
        # Two-word range links (#1066).
        (
            "de",
            "ab 12 3 2000 bis zum 13. März 2000 in Berlin",
            "ab 12.03.2000 bis zum 13. März 2000 in Berlin",
        ),
        ("pt", "de 12 3 2026 até o 13 3 2026", "de 12/03/2026 até o 13/03/2026"),
        (
            "en",
            "from 3 12 2000 to the 4 12 2000 inclusive",
            "from 3/12/2000 to the 4/12/2000 inclusive",
        ),
        (
            "es",
            "del 12 3 2000 hasta el 13 3 2000 en Madrid",
            "del 12/03/2000 hasta el 13/03/2000 en Madrid",
        ),
        (
            "it",
            "dal 12 3 2000 fino al 13 3 2000 a Roma",
            "dal 12/03/2000 fino al 13/03/2000 a Roma",
        ),
        # The second end in any date form (#1051 N4).
        (
            "en",
            "from three twelve two thousand to four slash twelve slash two thousand",
            "from 3/12/2000 to 4/12/2000",
        ),
        (
            "en",
            "from three twelve two thousand to december fourth two thousand",
            "from 3/12/2000 to December 4, 2000",
        ),
        (
            "es",
            "del tres doce dos mil al cuatro de doce de dos mil",
            "del 03/12/2000 al 04/12/2000",
        ),
        ("en", "the deadline is 3/12/2026 and", "the deadline is 3/12/2026 and"),
    ],
)
def test_numeric_date(lang, text, expected):
    assert dates(text, lang) == expected


def test_region_overrides_the_language_code():
    text = "three twelve twenty twenty six"
    assert words2num_sentence(text, date=True, region="GB") == "03/12/2026"
    assert (
        words2num_sentence("on march twelfth", date=True, region="AU") == "on 12 March"
    )
    assert (
        words2num_sentence("on march twelfth", lang="en_GB", date=True, region="US")
        == "on March 12"
    )


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("en", "on march twelfth twenty twenty six", "on 2026-03-12"),
        ("en", "three twelve twenty twenty six", "2026-03-12"),
        ("fr", "le premier avril deux mille vingt-six", "le 2026-04-01"),
        ("es", "el doce de setiembre de dos mil veintiséis", "el 2026-09-12"),
        ("en", "the first of march", "March 1"),  # no year: written
    ],
)
def test_iso(lang, text, expected):
    assert words2num_sentence(text, lang=lang, date=True, date_format="iso") == expected


def test_unknown_date_format():
    with pytest.raises(NotImplementedError):
        words2num_sentence("march twelfth", date=True, date_format="roman")


@pytest.mark.parametrize(
    "lang, text, expected",
    [
        # An opener glued before the date goes back before it (#1106).
        ("en", "x ［fifth of may y", "x ［May 5 y"),
        ("en", "x (the fifth of may) y", "x (May 5) y"),
        ("en", "x (may fifth) y", "x (May 5) y"),
        ("de", "x (am zwölften März) y", "x (am 12. März) y"),
        # An opener glued after the month goes after the date, not inside.
        ("en", "x the fifth of may( y", "x May 5( y"),
        # A closer after the month or the day goes after the date (#1196).
        ("en", "x [fifth of may] y", "x [May 5] y"),
        ("en", "x “fifth of may” y", "x “May 5” y"),
        ("en", "x 「fifth of may」 y", "x 「May 5」 y"),
        ("en", "x [may fifth] y", "x [May 5] y"),
        ("en", "x fifth of may] y", "x May 5] y"),
    ],
)
def test_opener_around_date_is_kept(lang, text, expected):
    assert words2num_sentence(text, lang=lang, date=True) == expected
