# -*- coding: utf-8 -*-
"""The currency fold of sentence mode (``words2num_sentence(..., currency=True)``).

An amount spoken as number + currency word (+ connector + subunit) is written
the way the language writes that currency. Off by default: the output is the
plain sentence-mode one.
"""

import pytest

from words2num2 import words2num_sentence


def fold(text, lang):
    return words2num_sentence(text, lang=lang, currency=True)


@pytest.mark.parametrize(
    "text,expected",
    [
        ("one thousand three hundred fifty five dollars and twenty eight cents", "$1,355.28"),
        ("your balance is one hundred and fifty four dollars and ninety two cents.", "your balance is $154.92."),
        ("twelve dollars", "$12"),
        ("one dollar", "$1"),
        ("five dollars, please", "$5, please"),
        ("ninety nine cents", "$0.99"),
        ("fifty bucks", "$50"),
        ("a hundred pounds", "£100"),
        ("twenty quid", "£20"),
        ("three pounds fifty", "£3.50"),
        ("twenty five pence", "£0.25"),
        ("two euros fifty", "€2.50"),
        ("one thousand euros and five cents", "€1,000.05"),
        ("ten thousand yen", "¥10,000"),
        ("one hundred canadian dollars", "CA$100"),
        ("ten swiss francs", "CHF 10"),
        ("two hundred reais", "R$200"),
        ("fifty rupees", "₹50"),
        # Not an amount.
        ("a couple of dollars", "a couple of dollars"),
        ("the dollars", "the dollars"),
        ("two point five million dollars", "2.5 million dollars"),
        ("dollars and cents", "dollars and cents"),
    ],
)
def test_en(text, expected):
    assert fold(text, "en") == expected


@pytest.mark.parametrize(
    "text,expected",
    [
        ("quarante-trois dollars et vingt centimes", "43,20 $"),
        ("deux cents euros", "200 €"),
        ("mille deux cents euros cinquante", "1 200,50 €"),
        ("trois mille quatre cent cinquante euros", "3 450 €"),
        ("cent mille euros", "100 000 €"),
        ("un solde de quarante-trois euros et vingt-deux centimes.", "un solde de 43,22 €."),
        ("deux euros cinquante", "2,50 €"),
        ("deux virgule cinq euros", "2,50 €"),
        ("vingt centimes", "0,20 €"),
        ("un euro", "1 €"),
        ("douze dollars, s'il vous plaît", "12 $, s'il vous plaît"),
        ("cent livres", "100 £"),
        ("cinq livres sterling", "5 £"),
        ("dix francs suisses", "10 CHF"),
        ("cinq dollars canadiens", "5 $ CA"),
        ("dix yens", "10 ¥"),
        ("cent francs cfa", "100 FCFA"),
        # Not an amount.
        ("des euros", "des euros"),
        ("quelques dollars", "quelques dollars"),
        ("les dollars", "les dollars"),
    ],
)
def test_fr(text, expected):
    assert fold(text, "fr") == expected


@pytest.mark.parametrize(
    "text,expected",
    [
        ("ciento cincuenta y cuatro dólares con noventa y dos centavos", "$154.92"),
        ("quinientos euros con veinte céntimos", "500,20 €"),
        ("doce dólares", "$12"),
        ("un dólar", "$1"),
        ("noventa y nueve centavos", "$0.99"),
        ("mil pesos", "$1,000"),
        ("cien pesos mexicanos", "$100"),
        ("dos euros cincuenta", "2,50 €"),
        ("veinte libras", "20 £"),
        ("unos dólares", "unos dólares"),
    ],
)
def test_es(text, expected):
    assert fold(text, "es") == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("de", "dreiundzwanzig euro fünfzig", "23,50 €"),
        ("de", "dreiundzwanzig euro und fünfzig cent", "23,50 €"),
        ("de", "hundert dollar", "100 $"),
        ("de", "tausend euro", "1.000 €"),
        ("de", "zehn schweizer franken", "CHF 10"),
        ("de", "ein euro", "1 €"),
        ("de", "fünfzig cent", "0,50 €"),
        ("it", "venti euro e cinquanta centesimi", "20,50 €"),
        ("it", "due euro e un centesimo", "2,01 €"),
        ("it", "dieci dollari", "10 $"),
        ("it", "cento sterline", "100 £"),
        ("it", "un euro", "1 €"),
        ("pt", "duzentos reais e cinquenta centavos", "R$ 200,50"),
        ("pt", "dez euros", "10 €"),
        ("pt", "cinquenta dólares", "US$ 50"),
        ("pt", "um real", "R$ 1"),
        ("pt", "vinte centavos", "R$ 0,20"),
        ("nl", "twintig euro vijftig", "€ 20,50"),
        ("nl", "honderd dollar", "$ 100"),
        ("nl", "duizend euro", "€ 1.000"),
        ("nl", "tien pond", "£ 10"),
        ("nl", "een euro", "€ 1"),
    ],
)
def test_other_languages(lang, text, expected):
    assert fold(text, lang) == expected


@pytest.mark.parametrize(
    "lang,text",
    [
        ("en", "one thousand three hundred fifty five dollars and twenty eight cents"),
        ("fr", "quarante-trois dollars et vingt centimes"),
        ("es", "ciento cincuenta y cuatro dólares con noventa y dos centavos"),
        ("it", "venti euro e cinquanta centesimi"),
    ],
)
def test_off_by_default(lang, text):
    plain = words2num_sentence(text, lang=lang)
    assert "$" not in plain and "€" not in plain
    assert words2num_sentence(text, lang=lang, currency=False) == plain


def test_it_centesimi_is_the_cent_after_a_number_even_without_the_fold():
    assert words2num_sentence("cinquanta centesimi", lang="it") == "50 centesimi"
    assert words2num_sentence("il centesimo cliente", lang="it") == "il 100º cliente"


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # A transcript's punctuation puts a comma where the speaker paused: the
        # subunit that follows (connector + number + subunit word) is the same amount.
        ("fr", "le solde est de zéro dollar, et cinquante-cinq centimes.", "le solde est de 0,55 $."),
        ("fr", "deux euros, et cinquante centimes", "2,50 €"),
        ("en", "zero dollars, and fifty five cents", "$0.55"),
        ("es", "dos dólares, con cincuenta centavos", "$2.50"),
        ("de", "zwei euro, und fünfzig cent", "2,50 €"),
        ("it", "due euro, e cinquanta centesimi", "2,50 €"),
        ("pt", "dois reais, e cinquenta centavos", "R$ 2,50"),
        ("nl", "twee euro, en vijftig cent", "€ 2,50"),
        # Without a spoken subunit the comma still ends the amount.
        ("en", "five dollars, and fifty people came", "$5, and 50 people came"),
        ("fr", "deux euros, cinquante", "2 €, 50"),
        ("en", "twelve dollars, and", "$12, and"),
    ],
)
def test_comma_before_the_subunit(lang, text, expected):
    assert fold(text, lang) == expected
