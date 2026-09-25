# -*- coding: utf-8 -*-
"""Real-life ASR finals through ``words2num_sentence``.

Call-center, IVR and finance transcripts: the walker sees whole sentences,
not isolated numbers, so what matters is that a number is read as the
speaker meant it and that everything else is left alone.
"""

import pytest

from words2num2 import words2num_sentence


@pytest.mark.parametrize(
    "text,expected",
    [
        # num2words writes "deux cents" / "quatre-vingts"; on their own "cents"
        # and "vingts" are not number words, so the walker stopped the run
        # before them: "2 cents euros", "4 vingts".
        ("deux cents euros", "200 euros"),
        ("quatre vingts", "80"),
        ("cinq cents personnes", "500 personnes"),
        # Speech drops the plural -s just as freely.
        ("mille deux cent", "1200"),
        ("mille deux cents", "1200"),
        ("deux million", "2000000"),
        # Two scale words: "un million" then "deux cent mille".
        ("un million deux cent mille", "1200000"),
        ("trois millions cinq cent mille euros", "3500000 euros"),
        # Feminine agreement inside a number.
        ("cinquante et une personnes", "51 personnes"),
        ("vingt et une heures", "21 heures"),
        ("une personne", "une personne"),
        # "pour cent" is the percent sign, not the number 100.
        ("vingt pour cent", "20 pour cent"),
        ("un pour cent", "1 pour cent"),
        ("cent pour cent", "100 pour cent"),
    ],
)
def test_fr_compounds(text, expected):
    assert words2num_sentence(text, lang="fr") == expected


@pytest.mark.parametrize(
    "text,expected",
    [
        # num2words glues "veintiuno"; spoken with the connector it is still 21.
        ("veinte y uno", "21"),
        ("treinta y cinco", "35"),
        ("diez por ciento", "10 por ciento"),
        ("ciento por ciento", "100 por ciento"),
    ],
)
def test_es_compounds(text, expected):
    assert words2num_sentence(text, lang="es") == expected


@pytest.mark.parametrize(
    "text,expected",
    [
        # "a" opens a run in front of a scale word.
        ("a hundred dollars", "100 dollars"),
        ("a thousand dollars", "1000 dollars"),
        ("a million", "1000000"),
        ("a hundred and one dogs", "101 dogs"),
        ("an hour", "an hour"),
        # A run never ends on a connector — the connector is a word again.
        ("one and a half hours", "one and a half hours"),
        ("five point", "5 point"),
        ("one hundred and", "100 and"),
        ("point five", "0.5"),
        ("five point five", "5.5"),
        # A scale word after a decimal stays a word.
        ("two point five million", "2.5 million"),
        ("two point five million dollars", "2.5 million dollars"),
        ("one point five percent", "1.5 percent"),
        ("twenty per cent", "20 per cent"),
    ],
)
def test_en_compounds(text, expected):
    assert words2num_sentence(text, lang="en") == expected


# ---------------------------------------------------------------------------
# Ordinals. Plain cardinal mode reads them too and writes a rank in figures
# ("15th", "1er", "2e", "3º", "2."). "first"/"second" and their translations
# are only a rank next to a month (or "of the month"): "first of all", "wait
# a second", "la première fois" stay in words. A fraction ("a fifth of", "two
# thirds") stays in words too.
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "text,expected",
    [
        ("due on the fifteenth", "due on the 15th"),
        ("the twenty first of march", "the 21st of march"),
        ("march twenty first", "march 21st"),
        ("on june fifteenth two thousand twenty four", "on june 15th 2024"),
        ("the twenty second of april", "the 22nd of april"),
        ("on the thirty first", "on the 31st"),
        ("my birthday is october third", "my birthday is october 3rd"),
        ("the fourth of july", "the 4th of july"),
        ("third quarter results", "3rd quarter results"),
        ("one hundred and fifth", "105th"),
        ("he came third", "he came 3rd"),
        ("the eleventh", "the 11th"),
        ("the twelfth of never", "the 12th of never"),
        ("the hundredth customer", "the 100th customer"),
        ("twenty-first century", "21st century"),
        ("Fifteenth of June.", "15th of June."),
        # first / second: a date, or just words.
        ("the second of may", "the 2nd of may"),
        ("september the first", "september the 1st"),
        ("the first of the month", "the 1st of the month"),
        ("first of all", "first of all"),
        ("at first i thought", "at first i thought"),
        ("your first name", "your first name"),
        ("the first time", "the first time"),
        ("wait a second", "wait a second"),
        ("the second floor", "the second floor"),
        # Fractions.
        ("two thirds of the amount", "two thirds of the amount"),
        ("three quarters", "three quarters"),
        ("a fifth of it", "a fifth of it"),
        # Digits are literals, never an ordinal.
        ("the 2nd", "the 2nd"),
    ],
)
def test_en_ordinals(text, expected):
    assert words2num_sentence(text, lang="en") == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("fr", "le premier avril", "le 1er avril"),
        ("fr", "la première de mars", "la 1re de mars"),
        ("fr", "le premier du mois", "le 1er du mois"),
        ("fr", "le vingt-troisième jour", "le 23e jour"),
        ("fr", "au deuxième étage", "au 2e étage"),
        ("fr", "au troisième trimestre", "au 3e trimestre"),
        ("fr", "le quinzième", "le 15e"),
        ("fr", "la première fois", "la première fois"),
        ("fr", "premier ministre", "premier ministre"),
        ("fr", "une seconde", "une seconde"),
        ("fr", "un dixième", "un dixième"),
        ("es", "el primero de mayo", "el 1º de mayo"),
        ("es", "el primero de cada mes", "el 1º de cada mes"),
        ("es", "el tercer piso", "el 3º piso"),
        ("es", "la vigésima", "la 20ª"),
        ("es", "el quinto", "el 5º"),
        ("es", "la primera vez", "la primera vez"),
        ("es", "las primeras", "las primeras"),
        ("es", "un segundo", "un segundo"),
        ("es", "el 2 y cinco", "el 2 y 5"),
        ("de", "am zweiten april", "am 2. april"),
        ("de", "am ersten mai", "am 1. mai"),
        ("de", "der dritte", "der 3."),
        ("de", "zum ersten mal", "zum ersten mal"),
        ("it", "il primo maggio", "il 1º maggio"),
        ("it", "il terzo piano", "il 3º piano"),
        ("it", "la prima volta", "la prima volta"),
        ("pt", "primeiro de maio", "1º de maio"),
        ("pt", "o terceiro andar", "o 3º andar"),
        ("pt", "a primeira vez", "a primeira vez"),
        ("nl", "de tweede verdieping", "de 2e verdieping"),
        ("nl", "vijftiende", "15e"),
        ("nl", "de eerste keer", "de eerste keer"),
    ],
)
def test_ordinals_other_languages(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


# ---------------------------------------------------------------------------
# "one" / "un" / "une" / "um" / "een" / "ein": the number 1 is also an article
# or a pronoun. On its own it stays a word; in front of a unit, a currency or
# the percent phrase, after a label word ("press", "option", "number") or a
# month, or in a counted list, it is the number.
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "text,expected",
    [
        ("no one answered", "no one answered"),
        ("one of the agents", "one of the agents"),
        ("one more thing", "one more thing"),
        ("i have one", "i have one"),
        ("which one", "which one"),
        ("this one is better", "this one is better"),
        ("one on one", "one on one"),
        ("give me one second", "give me one second"),
        ("one and a half hours", "one and a half hours"),
        ("one third of it", "one third of it"),
        ("One moment please", "One moment please"),
        # A count.
        ("press one", "press 1"),
        ("press one.", "press 1."),
        ("option one", "option 1"),
        ("number one", "number 1"),
        ("chapter one", "chapter 1"),
        ("one dollar", "1 dollar"),
        ("one percent", "1 percent"),
        ("one per cent", "1 per cent"),
        ("one o'clock", "1 o'clock"),
        ("at one a.m.", "at 1 a.m."),
        ("one pm", "1 pm"),
        ("december one twenty twenty five", "december 1 2025"),
        ("one, two, three", "1, 2, 3"),
        # Never lone: a compound, a decimal, a digit string.
        ("one hundred and one", "101"),
        ("one point five", "1.5"),
        ("one two three", "123"),
    ],
)
def test_en_one(text, expected):
    assert words2num_sentence(text, lang="en") == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("fr", "un solde de quarante-trois euros", "un solde de 43 euros"),
        ("fr", "un instant", "un instant"),
        ("fr", "un tiers", "un tiers"),
        ("fr", "une seconde", "une seconde"),
        ("fr", "l'un d'entre eux", "l'un d'entre eux"),
        ("fr", "un à un", "un à un"),
        ("fr", "il y a un euro", "il y a 1 euro"),
        ("fr", "une heure", "1 heure"),
        ("fr", "un pour cent", "1 pour cent"),
        ("fr", "tapez un", "tapez 1"),
        ("fr", "numéro un", "numéro 1"),
        ("fr", "le un janvier", "le 1 janvier"),
        ("fr", "un million", "1000000"),
        ("fr", "cinquante et une personnes", "51 personnes"),
        ("es", "un momento", "un momento"),
        ("es", "una llamada", "una llamada"),
        ("es", "uno de ellos", "uno de ellos"),
        ("es", "marque uno", "marque 1"),
        ("es", "un dólar", "1 dólar"),
        ("es", "una hora", "1 hora"),
        ("es", "uno por ciento", "1 por ciento"),
        ("es", "cincuenta y un centavos", "51 centavos"),
        ("pt", "um momento", "um momento"),
        ("pt", "um euro", "1 euro"),
        ("pt", "um por cento", "1 por cento"),
        ("it", "un attimo", "un attimo"),
        ("it", "uno di loro", "uno di loro"),
        ("it", "un euro", "1 euro"),
        ("nl", "een moment", "een moment"),
        ("nl", "een euro", "1 euro"),
        ("nl", "een procent", "1 procent"),
        ("de", "ein moment", "ein moment"),
        ("de", "ein euro", "1 euro"),
        ("de", "eine stunde", "1 stunde"),
        ("de", "eins", "1"),
    ],
)
def test_article_one_other_languages(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


# ---------------------------------------------------------------------------
# Digit strings (phone numbers, ZIP codes, account numbers) read one digit at
# a time, in every language; "double"/"triple" in English. Decimals spoken
# with the language's separator word ("virgule", "coma", "punto", "komma"),
# written with that separator. Fractions stay in words.
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("en", "double zero seven", "007"),
        ("en", "double oh seven", "007"),
        ("en", "triple five one two three four", "5551234"),
        ("en", "a double espresso", "a double espresso"),
        ("en", "double the amount", "double the amount"),
        ("fr", "sept cinq zéro un zéro", "75010"),
        ("fr", "zéro six douze trente-quatre cinquante-six soixante-dix-huit", "06 12 34 56 78"),
        ("fr", "un deux trois", "123"),
        ("fr", "un deux", "1 2"),
        ("fr", "j'en ai deux, trois", "j'en ai 2, 3"),
        ("es", "el número es seis uno nueve ocho cero", "el número es 61980"),
        ("es", "un dos tres", "123"),
        ("es", "cero uno", "01"),
        ("de", "null sechs", "06"),
        ("de", "eins zwei drei", "123"),
        ("it", "uno due tre", "123"),
        ("pt", "zero um dois", "012"),
        ("nl", "nul zes", "06"),
    ],
)
def test_digit_strings(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("fr", "trois virgule cinq", "3,5"),
        ("fr", "deux virgule cinquante", "2,50"),
        ("fr", "trois virgule zéro cinq", "3,05"),
        ("fr", "un virgule cinq", "1,5"),
        ("fr", "vingt virgule cinq pour cent", "20,5 pour cent"),
        ("fr", "mille deux cents virgule cinq", "1200,5"),
        ("fr", "trois virgule cinq euros", "3,5 euros"),
        ("fr", "trois virgule", "3 virgule"),
        ("fr", "virgule cinq", "virgule 5"),
        ("es", "tres coma cinco", "3,5"),
        ("es", "tres punto cinco", "3.5"),
        ("es", "dos coma cero cinco", "2,05"),
        ("de", "drei komma fünf", "3,5"),
        ("it", "tre virgola cinque", "3,5"),
        ("pt", "três vírgula cinco", "3,5"),
        ("nl", "drie komma vijf", "3,5"),
    ],
)
def test_decimal_words(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("en", "two halves", "two halves"),
        ("fr", "deux tiers des clients", "deux tiers des clients"),
        ("fr", "trois quarts", "trois quarts"),
        ("fr", "deux cinquièmes", "deux cinquièmes"),
        ("fr", "une demi-heure", "une demi-heure"),
        ("es", "tres cuartos", "tres cuartos"),
        ("es", "dos tercios", "dos tercios"),
        ("es", "el cuarto piso", "el 4º piso"),
        ("de", "drei viertel", "drei viertel"),
    ],
)
def test_fractions_stay_words(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected


# ---------------------------------------------------------------------------
# Quantities and compounds: a scale word on its own is a quantity ("a couple of
# thousand", "des millions"), a decade keeps its number in words, the glued
# thousands of de / nl / it, fr "douze cents", the article-one in front of a
# scale word ("eine Million", "un milione"), and a few walker edges.
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "lang,text,expected",
    [
        ("en", "a couple of thousand", "a couple of thousand"),
        ("en", "half a million", "half a million"),
        ("en", "hundreds of customers", "hundreds of customers"),
        ("en", "a hundred and twenty thousand", "120000"),
        ("en", "in the nineteen nineties", "in the nineteen nineties"),
        ("en", "for ten seconds", "for 10 seconds"),
        ("en", "one k", "1 k"),
        ("en", "three and a half percent", "3 and a half percent"),
        ("en", "one point five million dollars", "1.5 million dollars"),
        ("fr", "des millions", "des millions"),
        ("fr", "un demi-million", "un demi-million"),
        ("fr", "deux virgule cinq millions", "2,5 millions"),
        ("fr", "douze cents", "1200"),
        ("fr", "dix-huit cents euros", "1800 euros"),
        ("fr", "mille", "1000"),
        ("fr", "C'est quarante-deux ?", "C'est 42 ?"),
        ("fr", "un million et demi", "1000000 et demi"),
        ("es", "millones de personas", "millones de personas"),
        ("es", "medio millón", "medio millón"),
        ("es", "dos coma cinco millones", "2,5 millones"),
        ("es", "a las catorce treinta", "a las 14 30"),
        ("es", "mil", "1000"),
        ("de", "dreiundzwanzigtausend", "23000"),
        ("de", "zweihunderttausend euro", "200000 euro"),
        ("de", "hunderttausend", "100000"),
        ("de", "eine million zweihunderttausend", "1200000"),
        ("de", "zwei komma fünf millionen", "2,5 millionen"),
        ("de", "neunzehnhundertneunzig", "1990"),
        ("de", "einen moment bitte", "einen moment bitte"),
        ("nl", "tweehonderdduizend", "200000"),
        ("nl", "een miljoen", "1000000"),
        ("nl", "negentien negenennegentig", "1999"),
        ("it", "ventitremila", "23000"),
        ("it", "centomila", "100000"),
        ("it", "un milione e mezzo", "1000000 e mezzo"),
        ("it", "un attimo", "un attimo"),
    ],
)
def test_quantities_and_compounds(lang, text, expected):
    assert words2num_sentence(text, lang=lang) == expected
