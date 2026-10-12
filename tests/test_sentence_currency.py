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
        (
            "one thousand three hundred fifty five dollars and twenty eight cents",
            "$1,355.28",
        ),
        (
            "your balance is one hundred and fifty four dollars and ninety two cents.",
            "your balance is $154.92.",
        ),
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
        (
            "un solde de quarante-trois euros et vingt-deux centimes.",
            "un solde de 43,22 €.",
        ),
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
        (
            "fr",
            "le solde est de zéro dollar, et cinquante-cinq centimes.",
            "le solde est de 0,55 $.",
        ),
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


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # A bare subunit of 100 or more carries into the unit (#45).
        ("en", "one hundred fifty cents", "$1.50"),
        ("fr", "cent cinquante centimes", "1,50 €"),
        ("en", "twelve dollars and 100 cents", "$12 and 100 cents"),
        # A longer spoken fraction is never truncated (#46).
        ("en", "three point five yen", "3.5 yen"),
        ("en", "twelve point three four five euros", "12.345 euros"),
        # num2words' English "<unit>, <n> <subunit>" (#47).
        ("en", "twelve euros, thirty-four cents", "€12.34"),
        ("en", "I paid twenty dollars, fifty cents", "I paid $20.50"),
        # No currency nobody said (#48).
        ("en", "five apples and fifty cents", "5 apples and 50 cents"),
        ("pt", "duas libras e cinquenta pence", "2,50 £"),
        # fr "cents" is the US cent after "<unit> et <n>" (#49).
        ("fr", "douze dollars et trente-quatre cents", "12,34 $"),
        ("fr", "deux euros et cinq cents", "2,05 €"),
        ("fr", "mille euros et deux cents personnes", "1 000 € et 200 personnes"),
        # "pounds of" is a weight (#81).
        ("en", "twenty pounds of flour", "20 pounds of flour"),
        # The article-number before any known currency (#103).
        ("en", "one yen", "¥1"),
        ("fr", "un rouble", "1 ₽"),
        ("pt", "um iene", "1 ¥"),
        # An article or a homograph counts when a subunit makes it money.
        ("en", "a dollar and twenty cents", "$1.20"),
        ("en", "it costs an euro and fifty cents", "it costs €1.50"),
        ("en", "a pound and fifty pence", "£1.50"),
        ("fr", "une livre et vingt pence", "1,20 £"),
        ("en", "a dollar", "a dollar"),
        ("en", "a dollar and two friends", "a dollar and 2 friends"),
        ("fr", "une livre et deux kilos", "une livre et 2 kilos"),
        # Names num2words writes (#104).
        ("en", "twelve rupees, thirty-four paise", "₹12.34"),
        ("es", "doce rublos con treinta y cuatro kopeykas", "12,34 ₽"),
        ("de", "zwölf Yuan und vierunddreißig Fen", "12,34 CN¥"),
        ("es", "dos francos", "CHF 2"),
        # Connector + bare subunit ending the amount (#181).
        ("en", "two dollars and fifty", "$2.50"),
        ("es", "cinco euros con cincuenta", "5,50 €"),
        ("en", "two dollars and fifty people", "$2 and 50 people"),
        # Suffix symbols are spaced in English (#182).
        ("en", "twenty krone", "20 kr"),
        ("en", "two zlotys", "2 zł"),
    ],
)
def test_audit_fold(lang, text, expected):
    assert fold(text, lang) == expected


@pytest.mark.parametrize(
    "lang,text,to,expected",
    [
        # The plain walk reads the US cent as cents too (#868).
        (
            "fr",
            "douze dollars et cinquante cents",
            "cardinal",
            "12 dollars et 50 cents",
        ),
        (
            "fr_BE",
            "mille deux cent trente-quatre dollars et septante-six cents",
            "cardinal",
            "1234 dollars et 76 cents",
        ),
        (
            "fr_CH",
            "zéro dollar et nonante-neuf cents",
            "cardinal",
            "0 dollar et 99 cents",
        ),
        (
            "fr_DZ",
            "douze dollars et cinquante cents",
            "currency",
            "12 dollars et 50 cents",
        ),
        # Guards: hundreds stay hundreds; a non-multiplier subunit is untouched
        # (#1001).
        ("fr", "deux cents personnes", "cardinal", "200 personnes"),
        ("fr", "un euro et un centime", "cardinal", "1 euro et 1 centime"),
        ("ca", "un euro amb un cèntim", "cardinal", "1 euro amb 1 cèntim"),
    ],
)
def test_plain_walk_subunit(lang, text, to, expected):
    assert words2num_sentence(text, lang=lang, to=to) == expected


@pytest.mark.parametrize(
    "text, lang, expected",
    [
        ("douze dollars et vingt et un cents", "fr_BE", "12,21 $"),
        (
            "cinquante dollars et soixante cents et deux cents personnes",
            "fr_BE",
            "50,60 $ et 200 personnes",
        ),
        (
            "dix dollars et cinq et deux cents personnes",
            "fr",
            "10 $ et 5 et 200 personnes",
        ),
        ("douze dollars et zéro cent", "fr", "12,00 $"),
        ("12 dollars et 50 cents", "fr", "12,50 $"),
        ("douze dollars et deux virgule cinq cents", "fr", "12 $ et 2,5 cents"),
        ("douze dollars et deux virgule cinq cent", "fr", "12 $ et 2,5 cent"),
        ("douze dollars et cent cinquante cents", "fr", "12 $ et 150 cents"),
        ("dix dollars et mille deux cents personnes", "fr", "10 $ et 1200 personnes"),
        (
            "vingt euros et mille cent cinquante personnes",
            "fr",
            "20 € et 1150 personnes",
        ),
    ],
)
def test_subunit_count_is_one_plain_number_issue_1017(text, lang, expected):
    assert words2num_sentence(text, lang=lang, currency=True) == expected


@pytest.mark.parametrize(
    "text, currency, expected",
    [
        ("un dollar et un cent", False, "1 dollar et 1 cent"),
        ("deux dollars et neuf cents", False, "2 dollars et 9 cents"),
        ("deux dollars et neuf cents", True, "2,09 $"),
        ("un euro et un centime", True, "1,01 €"),
        ("douze dollars et un cent chacun", True, "12,01 $ chacun"),
    ],
)
def test_one_word_subunit_count_issue_1009(text, currency, expected):
    assert words2num_sentence(text, lang="fr", currency=currency) == expected


@pytest.mark.parametrize("to", ["ordinal", "ordinal_num", "digits"])
@pytest.mark.parametrize("currency", [False, True])
def test_one_word_count_stays_a_word_outside_cardinal_issue_1065(to, currency):
    text = "douze dollars et vingt cents"
    assert words2num_sentence(text, lang="fr", to=to, currency=currency) == text


def test_one_word_count_after_a_glued_mark_issue_1075():
    assert (
        words2num_sentence("un dollar et (un cent)", lang="fr")
        == "1 dollar et (un cent)"
    )


@pytest.mark.parametrize(
    "text, lang, expected",
    [
        ("x cinq euros， y", "fr", "x 5 €， y"),
        ("x five dollars。", "en", "x $5。"),
        ("twelve dollars and fifty cents！", "en", "$12.50！"),
        ("x five dollars, y", "en", "x $5, y"),
        ("x five dollars” y", "en", "x $5” y"),
        ("x cinq euros． y", "fr", "x 5 €． y"),
        ("x five dollars} y", "en", "x $5} y"),
        ("x five dollars‼ y", "en", "x $5‼ y"),
        ("x cinq euros｡ y", "fr", "x 5 €｡ y"),
        ("x fünf Euro“ y", "de", "x 5 €“ y"),
        ("twelve dollars‼ fifty cents", "en", "$12‼ $0.50"),
        ("x cinco euros､ cincuenta céntimos y", "es", "x 5,50 € y"),
        # No symbol or dash split off; a fullwidth comma is a pause.
        ("x cinq euros€ y", "fr", "x 5 euros€ y"),
        ("five dollars 50% off", "en", "$5 50% off"),
        ("x 12 dollars et 50’ cents", "fr", "x 12 $ et 50’ cents"),
        ("x five dollars， fifty cents y", "en", "x $5.50 y"),
    ],
)
def test_fold_keeps_cjk_trailing_mark_issue_1067(text, lang, expected):
    assert words2num_sentence(text, lang=lang, currency=True) == expected


@pytest.mark.parametrize(
    "text, lang, expected",
    [
        (
            "x )twenty five dollars and fifty cents y",
            "en",
            "x )twenty five dollars and fifty cents y",
        ),
        ("x )five dollars fifty cents y", "en", "x )five dollars fifty cents y"),
        ("x )vingt euros cinquante y", "fr", "x )vingt euros cinquante y"),
        # Not one amount without the mark either: the tail reads alone.
        ("x )five dollars and fifty people y", "en", "x )five dollars and 50 people y"),
        ("x )five apples and fifty cents y", "en", "x )five apples and 50 cents y"),
        # A mark-led word that is no number does not hold the amount (#1157).
        ("x )we paid fifty dollars y", "en", "x )we paid $50 y"),
        (
            "x )it costs twenty five dollars and fifty cents y",
            "en",
            "x )it costs $25.50 y",
        ),
        ("x )wij betaalden vijftig euro y", "nl", "x )wij betaalden € 50 y"),
        # A fullwidth / ideographic pause comma, as "," (#1156 audit).
        (
            "x )twelve dollars， fifty cents y",
            "en",
            "x )twelve dollars， fifty cents y",
        ),
        (
            "x )douze euros， cinquante centimes y",
            "fr",
            "x )douze euros， cinquante centimes y",
        ),
        ("x )it's fifty dollars y", "en", "x )it's $50 y"),
        ("x )c'est vingt euros y", "fr", "x )c'est 20 € y"),
        (
            "x )twelve dollars,\u200b fifty cents y",
            "en",
            "x )twelve dollars,\u200b fifty cents y",
        ),
    ],
)
def test_closer_led_amount_keeps_its_subunit_issue_1123(text, lang, expected):
    assert words2num_sentence(text, lang=lang, currency=True) == expected


@pytest.mark.parametrize(
    "text, lang, expected",
    [
        ("x five dollars，\u200e y", "en", "x $5，\u200e y"),
        ("x five dollars,\u200b y", "en", "x $5,\u200b y"),
        ("x cinq euros!\ufeff y", "fr", "x 5 €!\ufeff y"),
        # Unchanged: a format character alone, or before the mark.
        ("x five dollars\u200b y", "en", "x $5 y"),
        ("x five dollars\u200b, y", "en", "x $5, y"),
        # The comma pause and the clause end see through the format
        # character (#1154, #1155).
        ("twelve dollars,\u200b fifty cents", "en", "$12.50\u200b"),
        ("cinq euros,\u200f cinquante centimes", "fr", "5,50 €\u200f"),
        ("x douze dollars et un.\u200b cents y", "fr", "x 12 $ et un.\u200b 100 y"),
    ],
)
def test_mark_before_format_char_kept_after_amount(text, lang, expected):
    """#1083 audit: a mark then a format character after the unit was lost."""
    assert words2num_sentence(text, lang=lang, currency=True) == expected


@pytest.mark.parametrize(
    "text, lang, expected",
    [
        ("x )a dollar and fifty cents y", "en", "x )a dollar and fifty cents y"),
        ("x )one dollar y", "en", "x )one dollar y"),
        ("x (one dollar and fifty cents y", "en", "x (1 dollar and 50 cents y"),
        ("x )un dollar y", "fr", "x )un dollar y"),
        ("x (duas libras y", "pt", "x (2 libras y"),
        # A glued clause mark is kept and the article still counts (#1161);
        # a mark the walk reads past keeps the article a word (#1162).
        ("x ,a dollar and fifty cents y", "en", "x ,$1.50 y"),
        ("x ,une livre et vingt pence y", "fr", "x ,1,20 £ y"),
        # Not after a dot: the walk never folds there (#1159 audit).
        ("x ...a dollar and fifty cents y", "en", "x ...a dollar and 50 cents y"),
        # A mark the walk keeps the word behind: the amount stays whole, as
        # "#five dollars and fifty cents" does (#1163).
        ("x #a dollar and fifty cents y", "en", "x #a dollar and fifty cents y"),
        (
            "x \u200b)a dollar and fifty cents y",
            "en",
            "x \u200b)a dollar and fifty cents y",
        ),
        ("x -a dollar and fifty cents y", "en", "x -a dollar and 50 cents y"),
        (
            "x (\u200ba dollar and fifty cents y",
            "en",
            "x (\u200ba dollar and 50 cents y",
        ),
        ("x \u200ba dollar and fifty cents y", "en", "x \u200b$1.50 y"),
        # Unchanged without a mark.
        ("x a dollar and fifty cents y", "en", "x $1.50 y"),
        ("x duas libras y", "pt", "x 2 £ y"),
    ],
)
def test_count_rewrite_keeps_a_leading_mark(text, lang, expected):
    """#1156 audit: the article / count / feminine rewrite dropped the mark."""
    assert words2num_sentence(text, lang=lang, currency=True) == expected
