# -*- coding: utf-8 -*-
"""Multi-language smoke tests via the generic num2words2-backed backend.

These tests require ``num2words2`` to be installed.
"""

from decimal import Decimal

import pytest

from words2num2 import Words2NumError, words2num, words2num_sentence

num2words2 = pytest.importorskip("num2words2")


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

# gladiaio/num2words2#134-#142 moved five languages from a Latin
# transliteration to their own script. The reverse tables are built from the
# embedded `num2words2-core`, so whenever the installed `num2words2` and that
# core disagree on the script, the language cannot round-trip — see #14.
# This list may shrink; it must never grow.
_NATIVE_SCRIPT = {
    "ti": (0x1200, 0x137F),  # Ge'ez
    "ky": (0x0400, 0x04FF),  # Cyrillic
    "or": (0x0B00, 0x0B7F),  # Odia
    "ckb": (0x0600, 0x06FF),  # Arabic
    "kok": (0x0900, 0x097F),  # Devanagari
}
# Whether the embedded num2words2-core renders them in their own script.
# Flip in the change that bumps it.
_CORE_RENDERS_NATIVE = True


def _renders_native(lang):
    lo, hi = _NATIVE_SCRIPT[lang]
    return any(lo <= ord(ch) <= hi for ch in num2words2.num2words(1, lang=lang))


KNOWN_ROUNDTRIP_GAPS = {
    lang for lang in _NATIVE_SCRIPT if _renders_native(lang) != _CORE_RENDERS_NATIVE
}


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
                reason="gladiaio/words2num2#14 — the installed num2words2 "
                "and the embedded num2words2-core render this "
                "language in different scripts",
            ),
        )
    return lang


@pytest.mark.slow
@pytest.mark.parametrize(
    "lang", [_roundtrip_param(code) for code in sorted(supported_langs())]
)
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
        "ban",  # was -> ba  (Bashkir)
        "ceb",  # was -> ce  (Chechen)
        "cnh",  # was -> cn  (Chinese)
        "fil",  # was -> fi  (Finnish)
        _roundtrip_param("kok"),  # was -> ko  (Korean)
        "miz",  # was -> mi  (Maori)
        "pap",  # was -> pa  (Punjabi)
        "pli",  # was -> pl  (Polish)
        # Script variants that were collapsed onto their base language.
        "sr_Latn",
        "uz_Cyrl",
    ],
)
def test_codes_are_not_misresolved_to_another_language(code):
    """A known code must be answered by itself, not by a prefix match."""
    assert words2num(num2words2.num2words(42, lang=code), lang=code) == 42


@pytest.mark.parametrize(
    "text,expected",
    [
        # "ciento" is never rendered on its own by num2words (100 is "cien"),
        # so the reverse table had no entry for it and the sentence walker
        # could not open a run on it: "ciento 54 dólares".
        ("ciento cincuenta y cuatro dólares", "154 dólares"),
        ("ciento uno", "101"),
        ("cien", "100"),
        ("doscientos treinta", "230"),
        ("ciento veinte mil", "120000"),
    ],
)
def test_es_ciento_composes_in_sentence(text, expected):
    assert words2num_sentence(text, lang="es") == expected


def test_es_bare_ciento_is_one_hundred():
    assert words2num("ciento", lang="es") == 100


@pytest.mark.parametrize(
    "text,expected",
    [
        # Apocope inside a number ("un" for "uno" before a noun or a scale word):
        # num2words never renders it, so the walker stopped the run before it.
        ("cincuenta y un centavos", "51 centavos"),
        ("un mil treinta y cuatro dólares", "1034 dólares"),
        ("veintiún dólares", "21 dólares"),
        ("treinta y una llamadas", "31 llamadas"),
        # On its own it is an article.
        ("un momento por favor", "un momento por favor"),
        ("una llamada", "una llamada"),
        ("el 2 y cinco", "el 2 y 5"),
    ],
)
def test_es_apocope_inside_a_number(text, expected):
    assert words2num_sentence(text, lang="es") == expected


@pytest.mark.parametrize(
    "text,lang,expected",
    [
        # Numbers in figures follow the language's separators (#60).
        ("1.234", "de", 1234),
        ("1.234", "es", 1234),
        ("2,5", "de", Decimal("2.5")),
        ("1 234,5", "fr", Decimal("1234.5")),
        ("3.14", "fr", Decimal("3.14")),
    ],
)
def test_digit_literal_locale_separators(text, lang, expected):
    got = words2num(text, lang=lang)
    assert got == expected
    assert type(got) is type(expected)


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # Leading zeros / a fraction of any size (#52).
        ("fr", "trois virgule zéro vingt-cinq", "3,025"),
        ("fr", "trois virgule cent vingt-cinq", "3,125"),
        # The separator word derived from the renderer (#107).
        ("sv", "tre komma ett fyra", "3,14"),
        ("cs", "tři čárka jedna čtyři", "3,14"),
        ("hi", "तीन दशमलव एक चार", "3.14"),
        ("ko", "삼 점 일 사", "3.14"),
        ("en_IN", "three point one four", "3.14"),
        # Whole-word + denominator (#108).
        ("ru", "три целых четырнадцать сотых", "3,14"),
        ("hu", "három egész tizennégy század", "3,14"),
        ("fa", "سه و چهارده صدم", "3.14"),
        # Native spelling where the renderer writes "point" (#267).
        ("gl", "tres coma un catro", "3,14"),
    ],
)
def test_spoken_decimals_in_sentences(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # words2num reads a decimal in every language (#191).
        ("fr", "trois virgule un quatre", Decimal("3.14")),
        ("es", "tres punto uno cuatro", Decimal("3.14")),
        ("de", "drei Komma eins vier", Decimal("3.14")),
        ("ru", "три целых пять сотых", Decimal("3.05")),
        ("ja", "三点一四", Decimal("3.14")),
        ("tr", "üçvirgülondört", Decimal("3.14")),
    ],
)
def test_words2num_spoken_decimals(lang, text, expected):
    got = words2num(text, lang=lang)
    assert got == expected
    assert isinstance(got, Decimal)


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # A digit after a bare scale word is the next place down (#875).
        ("th", "เงินเดือนประมาณหมื่นห้า", "เงินเดือนประมาณ15000"),
        ("th", "บ้านหลังนี้ราคาล้านสอง", "บ้านหลังนี้ราคา1200000"),
        ("km", "ម៉ឺនប្រាំ រៀល", "15000 រៀល"),
        # 2005 as written, 2500 as spoken: no one number, so words stay.
        ("th", "ราคาสองพันห้า", "ราคาสองพันห้า"),
        ("lo", "ລາຄາ ສອງພັນ ຫ້າ ກີບ", "ລາຄາ ສອງພັນ ຫ້າ ກີບ"),
        ("th", "ราคาสองพันห้าร้อยบาท", "ราคา2500บาท"),
    ],
)
def test_elided_place_after_scale_word(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


def test_elided_place_cardinal_keeps_renderer_spelling():
    assert words2num("หมื่นห้า", lang="th") == 15000
    assert words2num("ໝື່ນຫ້າ", lang="lo") == 15000
    assert words2num("สองพันห้า", lang="th") == 2005


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # my "နှစ်" after a number is the year counter, no 2 (#876).
        ("my", "အသက် နှစ်ဆယ့်တစ် နှစ်", "အသက် 21 နှစ်"),
        ("my", "ငါး နှစ် ကြာ", "5 နှစ် ကြာ"),
        ("my", "တစ်သောင်း နှစ်", "10002"),
        ("my", "ငါး။ နှစ် ယောက် လာတယ်", "5။ 2 ယောက် လာတယ်"),
        ("my", "အဆင့် သုံး၊ နှစ် နဲ့ တစ်", "အဆင့် 3၊ 2 နဲ့ 1"),
        # Before a counter or currency it is the two (#1015).
        ("my", "ငါးရာ နှစ် ကျပ်", "502 ကျပ်"),
        ("my", "တစ်ရာ နှစ် ယောက်", "102 ယောက်"),
        # A digit after the point is no year (#1018).
        ("my", "သုည ဒသမ ငါး နှစ်", "0.52"),
        # Years then another count; a figure before (#1016).
        ("my", "သုံး နှစ် လေး လ", "3 နှစ် 4 လ"),
        ("my", "5 နှစ်", "5 နှစ်"),
        # Spaced groups add only below the lowest place filled.
        ("ko", "칠만 이천팔백삼십팔 삼", "72838 3"),
        ("hu", "ezerháromszáznegyvenhárom kettő", "1343 2"),
    ],
)
def test_year_counter_and_spaced_groups_issue_876(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("my", "တစ်ရာ။ ငါး ယောက်", "100။ 5 ယောက်"),
        ("my", "တစ်ရာ၊ ငါး ယောက်", "100၊ 5 ယောက်"),
        ("ur", "x ایک سو۔ پانچ y", "x 100۔ 5 y"),
        ("zh", "五，十二", "5，12"),
        ("ja", "千：十二", "1000：12"),
        ("zh", "二十，五", "20，五"),
        ("zh", "好，一样", "好，一样"),
        ("th", "ยี่สิบ，ห้า", "20，ห้า"),
    ],
)
def test_script_mark_ends_run_issue_1023(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("fr", "il reste ,deux cents euros", "il reste ,200 euros"),
        ("hi", "x ।पाँच सौ y", "x ।500 y"),
        ("ur", "x ،دو سو y", "x ،200 y"),
        # A figure, and zh / ja / th, are untouched (#1048).
        ("en", "x .5 y", "x .5 y"),
        ("fr", "x ,5 y", "x ,5 y"),
        ("zh", "二十 ,五", "20 ,五"),
    ],
)
def test_leading_clause_mark_issue_1041(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("en", "x )two thousand twenty four y", "x )two thousand twenty four y"),
        ("fr", "x )deux cents y", "x )deux cents y"),
        ("es", "x )dos mil veinticuatro y", "x )dos mil veinticuatro y"),
        (
            "hr",
            "x ]dvije tisuće dvadeset četiri y",
            "x ]dvije tisuće dvadeset četiri y",
        ),
        # Two numbers stay two; the open bracket still reads (control).
        # Spoken years and digit strings too (#1119).
        ("en", "x ]nineteen eighty four y", "x ]nineteen eighty four y"),
        ("en", "x )oh five y", "x )oh five y"),
        ("en", ")twenty one twenty one", ")twenty one 21"),
        ("en", "x (two thousand twenty four) y", "x (2024) y"),
    ],
)
def test_closer_led_number_stays_whole_issue_1114(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("zh", "x )第二十一 y", "x )第21 y"),
        ("zh", "比赛结果 ，第三名", "比赛结果 ，第3名"),
        ("zh", "x ，约二十 y", "x ，约20 y"),
        ("ja", "x 、第三 y", "x 、第3 y"),
        ("th", "x )ที่ยี่สิบเอ็ด y", "x )ที่21 y"),
        # A spaced format character inside the number (#1130); a mark-led word
        # that holds the number (ceb "ika-usa", #1143).
        ("ceb", "x )ika-usa libo y", "x )ika-usa libo y"),
        # The walk may add a hyphen after the prefix (#1151).
        ("id", "x )kedua puluh satu y", "x )kedua puluh satu y"),
        ("he", "x )ועשרים וחמש y", "x )ועשרים וחמש y"),
        ("en", "x )two hundred \u200b five y", "x )two hundred \u200b five y"),
        ("en_AERO", "x fife fife fife y", "x 555 y"),
        # The compat closers too (#1127).
        ("en", "x ）two hundred \u200b five y", "x ）two hundred \u200b five y"),
        # A dash lead keeps the number whole too; no look-ahead over a mark.
        ("fr", "x −deux mille vingt-quatre y", "x −deux mille vingt-quatre y"),
        ("en", "x -double zero seven y", "x -double zero seven y"),
        ("en", "x )one two. three four y", "x )one 2. 3 4 y"),
    ],
)
def test_mark_led_prefix_still_reads_issue_1122(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("en", 'x "twenty" one y', 'x "20" one y'),
        ("en", "x 'twenty' one y", "x '20' one y"),
        ("en", 'x twenty" three y', 'x 20" 3 y'),
        ("en", 'x twenty\u200b" three y', 'x 20\u200b" 3 y'),
        ("en", 'he said "two" thousand people', 'he said "2" thousand people'),
        ("fi", 'x "kaksikymmentä" yksi y', 'x "20" 1 y'),
        # Controls: a quoted number reads whole, a bracket ends the run too.
        ("en", 'x "twenty one" y', 'x "21" y'),
        ("en", "x 'nineteen eighty four' y", "x '1984' y"),
        ("en", "x (twenty) one y", "x (20) one y"),
        # An ordinal after the quote reads on its own, as after ")" (#1133).
        ("az", 'x "iyirmi" birinci y', 'x "20" 1-ci y'),
        ("et", 'x "kakskümmend" esimene y', 'x "20" 1. y'),
        ("bg", 'x сто" първи y', 'x 100" 1-ви y'),
        # Nor as a fraction across the quote (#1149); after a combining mark
        # the quote ends the word too (#1150).
        ("bg", 'x сто" четвърти y', 'x 100" 4-ти y'),
        ("pt", 'x vinte" quarto y', 'x 20" 4º y'),
        ("en", 'x three" quarters y', 'x 3" quarters y'),
        ("ml", 'x ഇരുപത്" ഒന്ന് y', 'x 20" 1 y'),
        ("bo", 'x གཅིག་བརྒྱ་" གསུམ་ y', 'x 100" 3 y'),
        ("bo", 'x གཅིག་བརྒྱ༌" གསུམ་ y', 'x 100" 3 y'),
        ("sa", "x एकम् शतम्' त्रीणि y", "x 100' 3 y"),
        ("en", "x two. half y", "x 2. half y"),
    ],
)
def test_closing_quote_ends_run_issue_1120(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


def test_closer_led_ordinal_stays_whole_issue_1114():
    # was "x )twenty 1 y"
    assert words2num_sentence("x )twenty first y", to="ordinal") == "x )twenty first y"


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("zh", "二十．五", "20．五"),
        ("en", "x 5．5 y", "x 5．5 y"),
        ("ja", "x 五十︒ 二 y", "x 50︒ 2 y"),
        ("en", "x fifty｣ two y", "x 50｣ 2 y"),
        ("en", "x ﹕fifty y", "x ﹕50 y"),
        # ｀ splits off like ASCII "`" (#1113).
        ("en", "x ｀twenty five y", "x ｀25 y"),
        # A compat closer or ＿ keeps the whole number as written (#1112).
        ("en", "x ）two thousand twenty four y", "x ）two thousand twenty four y"),
        ("ru", "x _две тысячи двадцать четыре y", "x _две тысячи двадцать четыре y"),
        ("el", "x πενήντα; δύο y", "x 50; 2 y"),
        ("uk", "x п＇ять y", "x 5 y"),
    ],
)
def test_folded_marks_issue_1088(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("el", "x ;πενήντα y", "x ;50 y"),
        ("en", "x fifty ﹑two y", "x 50 ﹑2 y"),
        ("en", "x fifty］ two y", "x 50］ 2 y"),
        ("en", "x fifty⁾ two y", "x 50⁾ 2 y"),
        ("en", "x fifty︙ two y", "x 50︙ 2 y"),
        ("en", "x fifty﹙ apples", "x 50﹙ apples"),
        ("en", "x forty… two y", "x 40… 2 y"),
        ("en", "x ］fifty y", "x ］fifty y"),
        ("en", "x ＿fifty y", "x ＿fifty y"),
        ("en", "x fifty﹍ two y", "x 50﹍ 2 y"),
        ("en", "x fifty｀ two y", "x 50｀ 2 y"),
    ],
)
def test_marks_by_nfkc_fold_issue_1096(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "text, expected",
    [
        ("x ‼fifty dollars y", "x ‼$50 y"),
        ("x five dollars fifty︕ y", "x $5.50︕ y"),
        ("x fifty dollars︕ y", "x $50︕ y"),
        ("x ．5 dollars", "x ．5 dollars"),
    ],
)
def test_currency_marks_by_nfkc_fold_issue_1099(text, expected):
    assert words2num_sentence(text, currency=True) == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        # After a number, ending its clause, the article-one counts (#911).
        ("en", "three, two, one, go", "3, 2, 1, go"),
        ("en", "the score was two one", "the score was 2 1"),
        ("fr", "trois, deux, un, partez", "3, 2, 1, partez"),
        ("es", "tres, dos, uno", "3, 2, 1"),
        ("fr", "deux, un instant", "2, un instant"),
        ("en", "I saw two, one of them ran", "I saw 2, one of them ran"),
        # Speaker labels (#912).
        ("en", "Speaker one: hello", "Speaker 1: hello"),
        ("fr", "Locuteur un : bonjour", "Locuteur 1 : bonjour"),
        ("es", "Hablante uno: hola", "Hablante 1: hola"),
        ("it", "Parlante uno: ciao", "Parlante 1: ciao"),
        # Not a pronoun after a person noun, nor after an ordinal (#1175,
        # #1176); a sentence mark after the item is fine (#1177).
        ("en", "he was a guest, one of hundreds", "he was a guest, one of hundreds"),
        ("en", "give each person one", "give each person one"),
        ("en", "the hundredth one, not the first", "the 100th one, not the first"),
        ("fr", "dans la famille, un", "dans la famille, un"),
        ("en", "three, two, one!", "3, 2, 1!"),
        ("fr", "trois, deux, un !", "3, 2, 1 !"),
        # de "ein" is also a verb particle (#1174 audit).
        ("de", "Er schläft um zwei ein", "Er schläft um 2 ein"),
    ],
)
def test_article_one_after_a_number_or_speaker_label(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang, text, expected",
    [
        # Georgian as it is written: the vigesimal tens joined by "და",
        # spaced too, and the bound hundreds and thousand (#989).
        ("ka", "ოცდა ათი", 30),
        ("ka", "ოცდაერთი", 21),
        ("ka", "სამოცდაცხრამეტი", 79),
        ("ka", "ოთხმოცდათორმეტი", 92),
        ("ka", "ას ოცდაერთი", 121),
        ("ka", "ათას ოცდაერთი", 1021),
        ("ka", "ორი ათას ხუთასი", 2500),
        ("ka", "მინუს ოცდაერთი", -21),
        ("ka", "ოცდაერთი.", 21),
        # Edges as the table path reads them (#1198 audit).
        ("ka", "-ოცდაერთი", -21),
        ("ka", "minus ოცდაერთი", -21),
        ("ka", "(ოცდაერთი)", 21),
        ("ka", "ოცდაერთი…", 21),
        ("ka", "„ოცდაერთი“", 21),
        ("ka", "–ოცდაერთი", -21),
        ("ka", "MINUS ოცდაერთი", -21),
        ("ka", "（ოცდაერთი）", 21),
        ("my", "တစ်ဆယ့် တစ်.", 11),
        ("my", "-တစ်ဆယ့် တစ်", -11),
        ("my", "minus တစ်ဆယ့် တစ်", -11),
        # Burmese spaced inside one number (#989).
        ("my", "တစ်ဆယ့် တစ်", 11),
        ("my", "တစ်ဆယ့် နှစ်", 12),
    ],
)
def test_written_compounds_read(lang, text, expected):
    assert words2num(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang, text",
    [
        ("ka", "ოცდა"),
        ("ka", "ერთი ერთი"),
        ("ka", "ოცდა ოცი"),
        ("my", "တစ် နှစ်"),
        ("my", "ငါး ငါး"),
        # Separate figures are not glued (#1199).
        ("my", "1 2"),
        ("my", "၁ ၂"),
        ("my", "12 34"),
        ("my", "2 3.5"),
        ("my_MM", "1 2"),
        ("my", "-1 2"),
        ("my", "1 2."),
        ("ka", "--ოცდაერთი"),
        ("ka", "+ოცდაერთი"),
        ("ka", "ოცდაერთი%"),
        ("my", "တစ်ဆယ့် တစ်%"),
    ],
)
def test_written_compounds_reject_non_numbers(lang, text):
    with pytest.raises(Words2NumError):
        words2num(text, lang=lang)


@pytest.mark.parametrize(
    "text, expected",
    [
        # es split teens 16..19, RAE-accepted (#866).
        ("tenía diez y ocho años", "tenía 18 años"),
        ("diez y nueve mil", "19000"),
        ("diez y seis, diez y siete", "16, 17"),
        # A clock time and 13..15 stay.
        ("a las diez y seis", "a las 10:06"),
        ("diez y tres", "10 y 3"),
        # A list after "<number>,", "las … minutos", a glued symbol (#1213
        # audit).
        ("los números cinco, diez y seis", "los números 5, 10 y 6"),
        ("a las diez y seis minutos", "a las 10 y 6 minutos"),
        ("diez y seis€", "10 y seis€"),
        ("¿diez y seis?", "¿16?"),
        # Only an opener leads; a duration and "Sí," read the teen (#1213
        # audit).
        ("#diez y seis", "#diez y 6"),
        ("esperé diez y seis minutos", "esperé 16 minutos"),
        ("Sí, diez y seis", "Sí, 16"),
        # A mark on "diez" is a boundary or would be lost (#1213 audit).
        ("Fueron diez, y seis se quedaron", "Fueron 10, y 6 se quedaron"),
        ("…diez y ocho años", "…10 y 8 años"),
    ],
)
def test_es_split_teens(text, expected):
    assert words2num_sentence(text, lang="es") == expected


def test_es_split_teens_modes():
    # Not rewritten where nothing reads the teen (#1213 audit).
    assert (
        words2num_sentence("tenía diez y ocho años", lang="es", to="ordinal")
        == "tenía diez y ocho años"
    )
    assert (
        words2num_sentence(
            "diez y seis dólares con diez y nueve centavos", lang="es_MX", currency=True
        )
        == "$16.19"
    )
    assert (
        words2num_sentence(
            "el diez y seis de mayo de dos mil diez y nueve", lang="es", date=True
        )
        == "el 16 de mayo de 2019"
    )
    assert (
        words2num_sentence("a las diez y seis", lang="es", to="currency")
        == "a las 10 y 6"
    )


def test_es_split_teens_cardinal():
    assert words2num("diez y seis", lang="es") == 16
    assert words2num("diez y nueve", lang="es") == 19
    assert words2num("mil novecientos diez y ocho", lang="es", to="year") == 1918


@pytest.mark.parametrize(
    "lang, text, expected",
    [
        # "un poco" / "um pouco" after the tens is the quantifier (#861).
        ("es_MX", "pesaba ochenta y un poco más", "pesaba 80 y un poco más"),
        # pt "um" is also the number and "pouco" also the adverb (#1210 audit).
        ("pt_BR", "em dois mil e vinte e um pouco mudou", "em 2021 pouco mudou"),
        ("es", "treinta y una poco", "31 poco"),
        # Everything else keeps the compound: nouns, verbs, pronouns, units,
        # currencies, years (#1210 audit).
        ("es_AR", "llegaron veinte y una señora", "llegaron 21 señora"),
        ("pt_BR", "trinta e um passaram na prova", "31 passaram na prova"),
        (
            "pt",
            "o ano de dois mil e vinte e um foi difícil",
            "o ano de 2021 foi difícil",
        ),
        ("es", "de los cincuenta, treinta y una aprobaron", "de los 50, 31 aprobaron"),
        ("pt", "cem libras e cinquenta e um pence", "100 libras e 51 pence"),
        ("es", "treinta y un km", "31 km"),
        ("es", "cuarenta y un euro", "41 euro"),
        ("es_MX", "llegaron treinta y una personas", "llegaron 31 personas"),
        ("es", "cuarenta y un mil", "41000"),
    ],
)
def test_tens_and_article(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang, text, expected",
    [
        # Whole millions or more "and a half" (#859).
        ("es_AR", "dos millones y medio de personas", "2500000 de personas"),
        (
            "pt_BR",
            "um pacote de um bilhão e meio de reais.",
            "um pacote de 1500000000 de reais.",
        ),
        ("fr", "un million et demi de personnes", "1500000 de personnes"),
        ("it", "un milione e mezzo di persone", "1500000 di persone"),
        # Below a million, or a time, it stays.
        ("es", "mil y medio", "1000 y medio"),
        ("es", "las dos y medio", "las 2 y medio"),
        # Figures as written are never added to (#1208); a half that belongs
        # to the next noun stays (#1209).
        ("es", "1000000 y medio", "1000000 y medio"),
        ("es", "un millón y medio kilo", "1000000 y medio kilo"),
        ("es", "un millón y medio millón", "1000000 y medio millón"),
        ("es", "dos millones y medio ambiente", "2000000 y medio ambiente"),
        (
            "es",
            "dos millones y medio de comunicación",
            "2000000 y medio de comunicación",
        ),
        ("pt_BR", "um milhão e meio ambiente", "1000000 e meio ambiente"),
        ("es", "DOS MILLONES Y MEDIO", "2500000"),
        ("fr", "un million et demi d'euros", "1500000 d'euros"),
        ("fr", "trois millions et demi d\u2019habitants", "3500000 d\u2019habitants"),
        ("pt", "um bilião e meio", "1500000000000"),
        ("fr", "un million et demi !", "1500000 !"),
        ("es", "dos millones y medio  ambiente", "2000000 y medio  ambiente"),
        (
            "es",
            "dos millones y medio  de comunicación",
            "2000000 y medio  de comunicación",
        ),
        ("pt_BR", "um trilhão e meio", "1500000000000"),
        ("it", "due milioni e mezzo i bambini", "2500000 i bambini"),
        ("es", "dos millones y medio\t ambiente", "2000000 y medio\t ambiente"),
        ("es", "dos millones y medio\u00a0 kilo", "2000000 y medio\u00a0 kilo"),
    ],
)
def test_scale_and_a_half_sentence(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang, text, expected",
    [
        ("es", "un millón y medio", 1500000),
        ("es", "dos millones y medio", 2500000),
        ("pt_BR", "um bilhão e meio", 1500000000),
        ("fr", "un million et demi", 1500000),
    ],
)
def test_scale_and_a_half_cardinal(lang, text, expected):
    assert words2num(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang, text, expected",
    [
        # th official "หนึ่ง" after the hundreds (#879), my bare ten (#877).
        ("th", "สองร้อยหนึ่ง", 201),
        ("th", "หนึ่งร้อยหนึ่ง", 101),
        ("th", "สามพันสองร้อยหนึ่ง", 3201),
        ("my", "ဆယ်", 10),
        ("my", "ဆယ့်ငါး", 15),
        ("my", "ဆယ်သိန်း", 1000000),
    ],
)
def test_th_my_written_forms(lang, text, expected):
    assert words2num(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang, text",
    [
        # Not the official forms, or no number (#1214 audit).
        ("th", "เอ็ด"),
        ("th", "สิบหนึ่ง"),
        ("th", "ร้อยหนึ่ง"),
        ("my", "ဆယ်တန်း"),
    ],
)
def test_th_my_written_forms_reject(lang, text):
    with pytest.raises(Words2NumError):
        words2num(text, lang=lang)


@pytest.mark.parametrize(
    "lang, text, expected",
    [
        ("th", "ห้องสองร้อยหนึ่ง", "ห้อง201"),
        ("th", "รถเมล์สายห้าร้อยหนึ่ง", "รถเมล์สาย501"),
        ("my", "နံနက် ဆယ် နာရီ သုံးဆယ် မိနစ်", "နံနက် 10 နာရီ 30 မိနစ်"),
        ("my", "လူ ဆယ့်တစ် ယောက်", "လူ 11 ယောက်"),
    ],
)
def test_th_my_written_forms_sentence(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "text, expected",
    [
        # lo billion "ຕື້" (#878).
        ("ຫ້າຕື້", 5000000000),
        ("ໜຶ່ງຕື້", 1000000000),
        ("ສອງຕື້ສາມ", 2000000003),
    ],
)
def test_lo_billion(text, expected):
    assert words2num(text, lang="lo") == expected


def test_lo_billion_elided_place():
    # "ຕື້ສອງ" is 1.2 billion, as "ລ້ານສອງ" is 1.2 million; "ສອງຕື້ຫ້າ" (2.5
    # billion or 2 000 000 005) stays, as "ສອງລ້ານຫ້າ" does (#1215 audit).
    assert words2num("ຕື້ສອງ", lang="lo") == 1200000000
    assert words2num_sentence("ສອງຕື້ຫ້າ ກີບ", lang="lo") == "ສອງຕື້ຫ້າ ກີບ"


def test_lo_billion_sentence():
    assert (
        words2num_sentence("ງົບປະມານ ຫ້າຕື້ ກີບ", lang="lo")
        == "ງົບປະມານ 5000000000 ກີບ"
    )


@pytest.mark.parametrize(
    "text, expected",
    [
        # es "con" as the decimal comma of a whole number (#864, #1226).
        ("tres con cinco", Decimal("3.5")),
        ("treinta y tres con cinco", Decimal("33.5")),
        ("mil quinientos con veinte", Decimal("1500.20")),
        ("menos tres con cinco", Decimal("-3.5")),
        ("tres con cinco.", Decimal("3.5")),
        ("tres  con cinco", Decimal("3.5")),
        # The split teens read before "con" too (#1223 audit A).
        ("diez y seis con cinco", Decimal("16.5")),
        ("diez y nueve con cincuenta", Decimal("19.50")),
    ],
)
def test_es_con_decimal_cardinal(text, expected):
    assert words2num(text, lang="es") == expected


@pytest.mark.parametrize(
    "text, expected",
    [
        # In a sentence "con" is as often "with": never the decimal (#1224,
        # #1225, #1223 audit).
        (
            "hay dos yogures: uno con cinco por ciento de grasa",
            "hay 2 yogures: uno con 5 por ciento de grasa",
        ),
        ("tres coma cinco con dos por ciento", "3,5 con 2 por ciento"),
        ("tres con cinco por ciento", "3 con 5 por ciento"),
        ("café con leche", "café con leche"),
    ],
)
def test_es_con_not_decimal_in_sentence(text, expected):
    assert words2num_sentence(text, lang="es") == expected


@pytest.mark.parametrize(
    "lang, text, expected",
    [
        # A spelled-out alternative or range shares its scale (#865).
        (
            "es_MX",
            "Llegaron cinco o seis mil personas a la marcha.",
            "Llegaron 5000 o 6000 personas a la marcha.",
        ),
        (
            "es_AR",
            "entre dos y tres millones de personas",
            "entre 2000000 y 3000000 de personas",
        ),
        ("pt_BR", "dois ou três milhões de pessoas", "2000000 ou 3000000 de pessoas"),
        ("en", "five or six thousand people", "5000 or 6000 people"),
        ("fr", "cinq ou six mille personnes", "5000 ou 6000 personnes"),
        # Not a figure as written, a bare "and", or a falling pair.
        ("es", "5 o seis mil", "5 o 6000"),
        ("en", "two and three thousand", "2 and 3000"),
        ("es", "veinte o dos mil", "20 o 2000"),
        # Figures as written are never multiplied, even next to a spelled
        # range (#1217); compound scales (#1218); chains (#1219); gl "ou"
        # (#1220); "one" (#1222); an opener before the first bound.
        (
            "es",
            "Tenía 2 o 3000 pesos y cinco o seis mil",
            "Tenía 2 o 3000 pesos y 5000 o 6000",
        ),
        (
            "en",
            "two and three thousand, between five and six thousand",
            "2 and 3000, between 5000 and 6000",
        ),
        (
            "es",
            "dos o tres mil millones de personas",
            "2000000000 o 3000000000 de personas",
        ),
        ("en", "five or six or seven thousand", "5000 or 6000 or 7000"),
        ("es", "cinco, seis o siete mil", "5, 6 o 7000"),
        ("gl", "dous ou tres mil", "2000 ou 3000"),
        ("en", "one or two thousand people", "1000 or 2000 people"),
        ("de", "ein oder zwei Millionen", "1000000 oder 2000000"),
        ("es", "(cinco o seis mil)", "(5000 o 6000)"),
        # #1227: A ending a longer number is not shared.
        ("en", "one point two or three million people", "1.2 or 3000000 people"),
        ("es", "treinta y cinco o seis mil", "35 o 6000"),
        # #1228 / F4: B must read with the scale and stay below 10x A.
        ("es", "cinco o seiscientos mil", "5 o 600000"),
        ("en", "two or thirty thousand", "2 or 30000"),
        # #1229: comma list led by a digit item.
        ("en", "5, six or seven thousand", "5, 6 or 7000"),
        # A clause comma before A does not make a list (#1216 audit M1).
        (
            "en",
            "Yesterday, five or six thousand people marched.",
            "Yesterday, 5000 or 6000 people marched.",
        ),
        (
            "es",
            "cinco o seis mil personas, siete u ocho mil euros",
            "5000 o 6000 personas, 7000 u 8000 euros",
        ),
        # The walk decides whether A ends a longer number (#1233).
        ("es", "ciento cinco o seis mil", "105 o 6000"),
        ("gl", "cento dous ou tres mil", "102 ou 3000"),
        ("de", "hundert fünf oder sechs Millionen", "105 oder 6000000"),
        ("en", "triple five or six thousand", "555 or 6000"),
        ("en", "two dot five or six million", "2.5 or 6000000"),
        # A number word right before A is no range bound (#1243).
        ("en", "five six or seven thousand", "5 6 or 7000"),
        ("fr", "zéro six douze ou treize mille", "06 12 ou 13000"),
        ("en", "(five) six or seven thousand", "(5) 6 or 7000"),
        # Blanks between the words (#1234).
        ("en", "one hundred  five or six thousand", "105 or 6000"),
        ("en", "one hundred \t five or six thousand", "105 or 6000"),
        ("en", "5,  six or seven thousand", "5,  6 or 7000"),
        # A scale word the walk does not read stays (#1235).
        ("en", "five or six thousand\u2122", "5 or 6 thousand\u2122"),
        ("es", "cinco o seis mil*", "5 o 6 mil*"),
        # A mark or a scale before A ends the previous number (#1236).
        (
            "es",
            "cinco o seis mil. Siete o ocho millones",
            "5000 o 6000. 7000000 o 8000000",
        ),
        (
            "en",
            "We were twenty. Five or six thousand came.",
            "We were 20. 5000 or 6000 came.",
        ),
        (
            "en",
            "five or six thousand, seven or eight million",
            "5000 or 6000, 7000000 or 8000000",
        ),
        # fr feminine article counts as one.
        ("fr", "une ou deux mille personnes", "1000 ou 2000 personnes"),
        # A clause conjunction before A does not join a number.
        (
            "es",
            "Tenía 2 o 3000 pesos y cinco o seis mil",
            "Tenía 2 o 3000 pesos y 5000 o 6000",
        ),
    ],
)
def test_shared_scale_range(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


def test_shared_scale_keeps_input_private_use_chars():
    # #1231: input already holding U+E000 keeps it; no mark is added or lost.
    out = words2num_sentence("icon \ue000 between five and six thousand", lang="en")
    assert out.count("\ue000") == 1
    assert "6000" in out


def test_es_many_con_fails_fast():
    # #1232: one "con" is the decimal comma; more raise without a retry each.
    import time

    start = time.perf_counter()
    with pytest.raises(Words2NumError):
        words2num("tres " + "con " * 4000 + "cinco", lang="es")
    assert time.perf_counter() - start < 2
    with pytest.raises(Words2NumError):
        words2num("tres con cinco con dos", lang="es")
