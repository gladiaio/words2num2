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


# #80: a lone homograph of a number word stays a word (maintainer policy: it
# converts only next to another number word). de "Acht" capitalised is the noun.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("it", "tu sei bravo", "tu sei bravo"),
        ("fr", "quoi de neuf ?", "quoi de neuf ?"),
        ("fr", "c'est tout neuf", "c'est tout neuf"),
        ("de", "in Acht nehmen", "in Acht nehmen"),
        ("nl", "acht geven", "acht geven"),
        ("fr", "neuf euros", "neuf euros"),
        # Next to another number word it is the number.
        ("it", "sei e trenta", "6 e 30"),
        ("fr", "dix neuf", "19"),
        ("fr", "neuf cents", "900"),
        ("de", "um acht Uhr", "um 8 Uhr"),
        ("de", "Acht Leute kamen.", "8 Leute kamen."),
    ],
)
def test_80_homographs(lang, text, expected):
    assert s(text, lang) == expected


# #82: the percent idiom holding the hundred word stays words.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("el", "πενήντα τοις εκατό", "50 τοις εκατό"),
        ("el", "εκατό τοις εκατό", "100 τοις εκατό"),
        ("vi", "hai mươi phần trăm", "20 phần trăm"),
        ("fr", "cinquante pour cent", "50 pour cent"),
    ],
)
def test_82_percent_idiom(lang, text, expected):
    assert s(text, lang) == expected


# #83: the indefinite article that is the word for one stays a word, unless a
# unit follows.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("tr", "Bir gün bir adam geldi.", "Bir gün bir adam geldi."),
        ("tr", "bir şey değil", "bir şey değil"),
        ("hu", "Egy nap egy ember jött.", "Egy nap egy ember jött."),
        ("sv", "ett par dagar", "ett par dagar"),
        ("no", "En dag kom en mann med et barn.", "En dag kom en mann med et barn."),
        ("da", "En dag kom en mand med et barn.", "En dag kom en mand med et barn."),
        ("el", "και ένα παιδί", "και ένα παιδί"),
        ("fa", "یک روز یک مرد آمد.", "یک روز یک مرد آمد."),
        ("hi", "एक दिन एक आदमी आया।", "एक दिन एक आदमी आया।"),
        ("vi", "có một người đàn ông đến", "có một người đàn ông đến"),
        ("tr", "bir lira", "1 lira"),
        ("hu", "egy óra", "1 óra"),
        ("tr", "bir iki üç", "123"),
    ],
)
def test_83_article_one(lang, text, expected):
    assert s(text, lang) == expected


# #183: the article-one in front of a scale word opens the number.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("no", "ett tusen kroner", "1000 kroner"),
        ("sv", "en miljon kronor", "1000000 kronor"),
        ("ro", "o mie de lei", "1000 de lei"),
        ("ro", "un milion de lei", "1000000 de lei"),
        ("sv", "en bok", "en bok"),
        ("ro", "o carte", "o carte"),
    ],
)
def test_183_article_before_scale(lang, text, expected):
    assert s(text, lang) == expected


# #74: case-declined cardinals are read as their nominative.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("ru", "Со скоростью сто двадцати километров в час.", "Со скоростью 120 километров в час."),
        ("ru", "Около трёх тысяч человек.", "Около 3000 человек."),
        ("ru", "двадцати трёх тысяч", "23000"),
        ("uk", "близько трьох тисяч людей", "близько 3000 людей"),
        ("uk", "п'яти тисяч", "5000"),
        ("pl", "około trzech tysięcy osób", "około 3000 osób"),
        ("ar", "دفعت ثلاثة وعشرين يورو", "دفعت 23 يورو"),
        ("ar", "ولد في عام ألف وتسعمائة وتسعة وتسعين.", "ولد في عام 1999."),
        ("ar", "عشرين", "20"),
        ("ar", "اثنين", "2"),
    ],
)
def test_74_declined_cardinals(lang, text, expected):
    assert s(text, lang) == expected


# #75: tr spaced hundreds in front of a scale word.
@pytest.mark.parametrize(
    "text,expected",
    [
        ("Ev üç yüz bin lira.", "Ev 300000 lira."),
        ("üç yüz bin beş yüz", "300500"),
        ("iki yüz elli bin", "250000"),
        ("üç yüz milyon", "300000000"),
        ("üç yüz bin", "300000"),
    ],
)
def test_75_tr_spaced_hundreds(text, expected):
    assert s(text, "tr") == expected


# #77: vi "không trăm" (zero hundreds), "linh", "tư".
@pytest.mark.parametrize(
    "text,expected",
    [
        ("hai nghìn không trăm linh bốn", "2004"),
        ("hai nghìn không trăm hai mươi", "2020"),
        ("ba mươi tư", "34"),
        ("hai nghìn lẻ bốn", "2004"),
    ],
)
def test_77_vi_spoken_forms(text, expected):
    assert s(text, "vi") == expected


# #78: standard spellings beside the renderer's.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("el", "Γεννήθηκε το χίλια εννιακόσια ενενήντα εννιά.", "Γεννήθηκε το 1999."),
        ("el", "δεκαοχτώ", "18"),
        ("hi", "मेरा नंबर नौ आठ सात छह पाँच है।", "मेरा नंबर 98765 है।"),
        ("hi", "पांच", "5"),
        ("da", "tres kroner", "60 kroner"),
        ("da", "toogtres", "62"),
        ("bn", "তার বয়স পঁয়ত্রিশ বছর।", "তার বয়স 35 বছর।"),
    ],
)
def test_78_spelling_variants(lang, text, expected):
    assert s(text, lang) == expected


# #179: cs inverted unit-a-tens.
@pytest.mark.parametrize(
    "text,expected",
    [("pětadvacet korun", "25 korun"), ("dvaadvacet", "22"), ("jednadvacet", "21"),
     ("dvacet pět korun", "25 korun")],
)
def test_179_cs_inverted(text, expected):
    assert s(text, "cs") == expected


# #180: ro contracted teens.
@pytest.mark.parametrize(
    "text,expected",
    [("șaișpe", "16"), ("paișpe", "14"), ("doișpe lei", "12 lei"), ("unșpe", "11"),
     ("cinșpe", "15")],
)
def test_180_ro_contracted_teens(text, expected):
    assert s(text, "ro") == expected


# #73: a case ending / classifier glued to the last number word is cut off
# and re-emitted after the figures; a suffixed homograph stays a word.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("tr", "on ikide", "12'de"),
        ("tr", "otuz beşte", "35'te"),
        ("tr", "Bin dokuz yüz doksan dokuzda doğdu.", "1999'da doğdu."),
        ("tr", "Saat üçte buluşalım.", "Saat 3'te buluşalım."),
        ("hu", "Háromkor találkozunk.", "3-kor találkozunk."),
        ("hu", "Ötöt kérek.", "5-öt kérek."),
        ("hu", "hármat kérek", "3-at kérek"),
        ("bn", "আমরা বারোটা ডিম কিনেছি।", "আমরা 12টা ডিম কিনেছি।"),
        # Ordinary words that end like a suffixed number.
        ("tr", "yüzde elli", "yüzde 50"),
        ("tr", "ona verdim", "ona verdim"),
        ("tr", "onda kaldı", "onda kaldı"),
        ("tr", "masanın altında", "masanın altında"),
        ("tr", "biri geldi", "biri geldi"),
        ("hu", "egykor", "egykor"),
        ("hu", "a héten", "a héten"),
        ("bn", "একটা বই", "একটা বই"),
    ],
)
def test_73_case_suffixes(lang, text, expected):
    assert s(text, lang) == expected


# #74 (fi): Finnish case forms, ending re-emitted after a colon.
@pytest.mark.parametrize(
    "text,expected",
    [
        ("Tulen viidellä bussilla.", "Tulen 5:llä bussilla."),
        ("kahdessa paikassa", "2:ssa paikassa"),
        ("kolmen", "3:n"),
    ],
)
def test_74_fi_case_forms(text, expected):
    assert s(text, "fi") == expected


# #76: ar / he proclitic preposition glued to a number.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("he", "נולד באלף תשע מאות תשעים ותשע", "נולד ב-1999"),
        ("he", "נפגש בשלוש", "נפגש ב-3"),
        ("he", "לחמש דקות", "ל-5 דקות"),
        ("ar", "البيت بثلاثمائة ألف دولار", "البيت ب300000 دولار"),
        ("ar", "لخمسة أيام", "ل5 أيام"),
        # Not a proclitic + number.
        ("ar", "لست هنا", "لست هنا"),
        ("he", "המאה העשרים", "המאה העשרים"),
    ],
)
def test_76_proclitics(lang, text, expected):
    assert s(text, lang) == expected


# #184: fr / it / ca elided clitic before a number word.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("fr", "le train d'onze heures", "le train d'11 heures"),
        ("it", "dell'otto per cento", "dell'8 per cento"),
        ("it", "dall'undici al venti", "dall'11 al 20"),
        ("fr", "jusqu'à onze", "jusqu'à 11"),
        ("ca", "d'onze", "d'11"),
        ("fr", "l'un d'entre eux", "l'un d'entre eux"),
        ("it", "un'altra", "un'altra"),
    ],
)
def test_184_elided_clitic(lang, text, expected):
    assert s(text, lang) == expected


# #177: numbers inside unspaced zh / ja / th text convert in place; words that
# merely contain a numeral character stay.
@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("ja", "りんごを三個買いました。", "りんごを3個買いました。"),
        ("ja", "家は三百万円です。", "家は3000000円です。"),
        ("zh", "他住在一百一十二号。", "他住在112号。"),
        ("zh", "我买了三个苹果。", "我买了3个苹果。"),
        ("th", "ฉันจ่ายยี่สิบสามบาท", "ฉันจ่าย23บาท"),
        ("zh", "我买了 三 个苹果", "我买了 3 个苹果"),
        ("zh", "两个苹果", "2个苹果"),
        ("zh", "十二月", "12月"),
        ("zh", "十分钟", "10分钟"),
        # Ordinary words.
        ("zh", "我们一起去", "我们一起去"),
        ("zh", "十分好", "十分好"),
        ("zh", "千万不要", "千万不要"),
        ("zh", "万一下雨", "万一下雨"),
        ("zh", "我有一个问题", "我有一个问题"),
        ("zh", "四川菜", "四川菜"),
        ("ja", "九州に行く", "九州に行く"),
        ("th", "สามารถ", "สามารถ"),
        ("th", "สามีของฉัน", "สามีของฉัน"),
        ("th", "เก้าอี้", "เก้าอี้"),
        ("th", "แสนดี", "แสนดี"),
        ("th", "สองสามวัน", "สองสามวัน"),
    ],
)
def test_177_unspaced_scripts(lang, text, expected):
    assert s(text, lang) == expected


# #178: native Korean numerals; the attributive forms need a counter.
@pytest.mark.parametrize(
    "text,expected",
    [
        ("사과 세 개를 샀어요.", "사과 3 개를 샀어요."),
        ("세 시에 만나요.", "3 시에 만나요."),
        ("그녀는 서른다섯 살이에요.", "그녀는 35 살이에요."),
        ("열두 개", "12 개"),
        ("하나 둘 셋", "1 2 3"),
        ("스무 살", "20 살"),
        ("삼십오 세", "35 세"),
        ("네, 알겠습니다", "네, 알겠습니다"),
        ("한 번 해 보세요", "한 번 해 보세요"),
        ("열이 나요", "열이 나요"),
        ("세상", "세상"),
    ],
)
def test_178_ko_native_numerals(text, expected):
    assert s(text, "ko") == expected


# #73 (ko): a Sino number glued to its counter.
@pytest.mark.parametrize(
    "text,expected",
    [
        ("집은 삼만원이에요", "집은 30000원이에요"),
        ("오십만원", "500000원"),
        ("일월", "1월"),
        ("이번 주", "이번 주"),
        ("만일", "만일"),
        ("천만에요", "천만에요"),
        ("구원", "구원"),
        ("사명", "사명"),
        ("일부", "일부"),
    ],
)
def test_73_ko_glued_counter(text, expected):
    assert s(text, "ko") == expected
