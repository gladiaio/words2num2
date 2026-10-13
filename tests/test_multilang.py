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
