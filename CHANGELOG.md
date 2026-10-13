# Changelog

All notable changes to `words2num2` are documented here. This project
follows [Semantic Versioning](https://semver.org/) and uses
[Keep a Changelog](https://keepachangelog.com/) style.

## [Unreleased]

### Fixed

- es/pt/fr/it/ca: whole millions or more "and a half" read as one number ("dos millones y medio de personas" is "2500000 de personas", was "2000000 y medio de personas"; pt_BR "um bilhão e meio" 1500000000; `words2num("un millón y medio")` is 1500000, was an error) (#859).
- `date=True` (en): "may"/"march" after a count and before a verb-like word, or after a subject before a figure, are verbs, and "of" takes an ordinal day ("all three may come" is "all 3 may come", was "all May 3 come"; "they march two thousand miles" "they march 2000 miles"; "chapter twelve of march" "chapter 12 of march") (#907); a cardinal day after the month before a counted noun ("people", "children", "trucks" …) counts it ("in june twelve people died" is "in june 12 people died", was "in 12 June people died" in en_GB) (#931).
- Sentence mode, clock times: es "entre las dos y tres" is a range ("entre las 2 y 3", was "entre las 2:03") (#937); "y tres cuartos" / "trois quarts" are 45 minutes ("a las 2:45", was "a las 2:03 cuartos"; fr "à 3h45", was "à 3h03 quarts") (#934); a connector after the minutes is no longer dropped ("las 2:10 y las 3", was "las 2:10 las 3"; de "um 5:12 Uhr und 6 Uhr", was "um 5:12 Uhr 6 Uhr").
- Georgian and Burmese cardinals in their written forms: ka "ოცდაერთი" is 21, "ოცდა ათი" 30, "ას ოცდაერთი" 121, "სამოცდაცხრამეტი" 79 (all raised: the table only knows num2words2's "ოცი ერთი" spelling); my "တစ်ဆယ့် တစ်" (spaced) is 11, was an error (#989). A sign and brackets, quotes or sentence marks around them read as they do around the table forms ("-ოცდაერთი" -21, "(ოცდაერთი)" 21, "တစ်ဆယ့် တစ်." 11); "%", "#", "$" do not (#1198 audit).
- Sentence mode, clock times: minutes before any currency name of the language or a few more counted nouns are a count, not a time ("las dos y cinco pesos" with `currency=True` is "las 2 y $5", was "las 2:05 pesos"; fr "huit heures trois kilos" "8 heures 3 kilos"; de "um neun Uhr zwanzig Kinder" "um 9 Uhr 20 Kinder"; es "las dos y cinco niños" "las 2 y 5 niños"). Names that are mostly other words ("won", "real", "pound the table", es "peso la harina"), names ("Penny", "Franco") and a spaced "p m" keep the time (#1194).
- Sentence mode, `currency=True` and `date=True`: an opening bracket or quote glued to an amount or a date no longer blocks the fold or gets lost ("(fifty dollars)" is "($50)", was "(50 dollars)"; "«cinquante euros»" "«50 €»"; "x ［fifth of may y" "x ［May 5 y"; "fifty dollars( " "$50(", was "$50"; "the fifth of may(" "May 5(", was "May( 5"); a closing bracket or quote after the month or day goes after the date ("fifth of may]" "May 5]", was "May] 5"). A "the" or a connector with a mark after it is no longer swallowed with its mark ("the) fifth of may" is "the) May 5", was "May 5"; "five dollars and) fifty cents" "$5 and) 50 cents", was "$5.50") (#1104, #1105, #1106, #1196).
- Sentence mode, clock times: a time is no longer read out of a longer number ("at nine fifteen thousand" is "at 9 15000", was "at 9:15 thousand"; fr "cent vingt heures quinze" "120 heures 15", was "100 20h15"; es "las dos menos treinta y uno" "las 2 menos 31") (#928, #935), nor when its minutes count something after it ("sold at nine fifty each" is "sold at 9 50 each", was "at 9:50 each"; fr "huit heures trois fois" "8 heures 3 fois"; de "um neun Uhr zwanzig Leute" "um 9 Uhr 20 Leute"; es "las dos y cinco personas" "las 2 y 5 personas") (#904, #933). A number after the minutes stays its own count ("um neun Uhr zwanzig zwei Leute" is "um 9:20 Uhr 2 Leute"), and after a cue fr keeps the hour ("à neuf heures quinze fois deux" is "à 9h 15 fois 2").
- `phone=True`: a day, a month and a year opening a run of figures are left to the date pass, also with a figure after them: fr "né le vingt-trois dix deux mille vingt-six" with `date=True` is "né le 23/10/2026" (was "né le 23 10 20 26"), "on 12 05 1984 12" "on 12/5/1984 12"; a number dialled from its trunk "0" that holds them is still read whole ("appelez le 06 12 2024 56"), and a zero-padded area code is no day ("ruf 030 12 2024 an") (#940). A run that crosses commas and is no phone number is read again up to its first comma, and a piece that follows another figure with a comma or nothing after it is a phone only grouped the way its plan writes it or dictated as one figure: "call 2, 212 555 1234" is "call 2, (212) 555-1234", while "In 2018, 2019 312000 cars" and "7, 21 24 35 47 59" stay as they are. A "0"-led day-month-year with a figure after it that is a valid national number is still read as a phone ("am 02 04 1984 12") (#940, #1189).
- Sentence mode: the article-one counts right after a number when it ends its clause ("three, two, one, go" is "3, 2, 1, go", was "3, 2, one, go"; "the score was two one" "the score was 2 1"; fr "trois, deux, un, partez", es, pt, it, nl, tr, ca, ro "unu" likewise; not de "ein", a verb particle as well, nor ro "o"), but not before a noun ("deux, un instant", "two, one of them" stay words) (#911); and as a speaker label before its colon ("Speaker one: hello" is "Speaker 1: hello"; fr "Locuteur", es "Hablante", pt "Locutor", it "Parlante", "Oratore"; "a guest, one of hundreds" keeps the pronoun) (#912). The word before must be a cardinal ("the hundredth one" keeps "one"), and a sentence mark after the item is fine ("three, two, one!" is "3, 2, 1!").
- `to="year"` sentences: "N hundred" and "N oh M" read as one year ("nine hundred" is 900, was "9 100"; "one oh one dalmatians" "101 dalmatians"; "the year five hundred and twenty" "the year 520") (#903); a decade word joins only a spoken century of 16 to 20 (10 to 21 after "the", "early", "mid", "late": "the fourteen nineties" is "the 1490s") with no ordinal and no mark between: "forty twenties" is "40 twenties" (was "4020s"), "I was nineteen, nineties kids" "I was 19, nineties kids" (was "1990s"), while "the nineteen nineties" is still "the 1990s" (#905, #927).
- `currency=True`: the article-one before a unit counts 1 only when a subunit amount of 0 to 99 follows the whole unit name: "a US dollar and twenty cents" is "$1.20" (was "a US dollar and 20 cents"), fr "une livre sterling et vingt pence" "1,20 £"; "a dollar and two hundred cents" stays "a dollar and 200 cents" (was "$1 and 200 cents"). A zero subunit counts ("a dollar zero cents" is "$1.00"), and so do the regional tens (fr_CH "septante-deux pence"); a sentence mark after the unit ends it ("a pound sterling. And twenty pence" stays words, as does "a dollar; twenty cents" → "a dollar; $0.20", was "$1; $0.20"). Not after a share right before it: "half a dollar and twenty cents" is "half a dollar and 20 cents" (was "half $1.20"), also "twice", "double", "three quarters of", "a fifth of"; a mark in between keeps the amount ("half, a dollar and twenty cents" is "half, $1.20"). A currency with no minor unit is never confirmed: "a yen and twenty cents" stays "a yen and 20 cents" (was "¥1 and 20 cents") (#906, #929, #930).
- `currency=True`: a closer-led amount keeps its subunit count as written after a fullwidth or ideographic pause comma too ("，", "、", "､", "﹐", "﹑", "︐", "︑", "،", "၊", "՝"), as after ",": "x )twelve dollars， fifty cents y" is unchanged (was "x )twelve dollars， $0.50 y").
- Sentence mode: a format character after a glued minus in another form (fullwidth "－", small "﹣", superscript "⁻", subscript "₋", the hyphens "‐" "‑", the figure dash "‒", "﹘") is kept after the figure's sign, as after "-": "x \u200e－\u200etwenty five y" is "x \u200e-\u200e25 y" (was "x \u200e-25 y"), "x ⁻\u200etwenty five y" "x -\u200e25 y".
- `currency=True`: a mark glued before the article-one, a one-word count or a feminine numeral of an amount is no longer lost: "x )a dollar and fifty cents y" stays as written (was "x $1.50 y", the bracket lost), "x )one dollar y" (was "x $1 y"), "x (one dollar and fifty cents y" is "x (1 dollar and 50 cents y" as "(five" reads, pt "x (duas libras y" "x (2 libras y" (was "x 2 £ y"). After a glued ", ; : ! ?" the mark is kept and the amount still folds: "x ,a dollar and fifty cents y" is "x ,$1.50 y".
- Sentence mode: a fraction word after a number that ends in a mark is no fraction: bg 'x сто" четвърти y' is 'x 100" 4-ти y' (was 'x сто" 4-ти y'), pt 'x vinte" quarto y' is 'x 20" 4º y', 'x three" quarters y' is 'x 3" quarters y', 'x two. half y' is 'x 2. half y' (#1149). A closing quote after a combining mark (ml virama, sa, ta, km) or a tsheg (་ ༌) ends the number: ml 'x ഇരുപത്" ഒന്ന് y' is 'x 20" 1 y' (was 'x 21 y', the quote lost) (#1150). A format character before a glued closer or clause mark ("\u200b)five", "\u200e។ប្រាំ") leaves the word as written, as the mark alone does: "x twenty \u200b)five y" is "x 20 \u200b)five y" (was "x 25 y") (#1147, #1148).
- `currency=True`: after an amount left as written because a closing bracket opens it, the subunit count stays as written too, when the words read as one amount without the bracket: "x )twenty five dollars and fifty cents y" is unchanged (was "x )twenty five dollars and 50 cents y"), fr "x )vingt euros cinquante y" (was "x )vingt euros 50 y"); "x )five dollars and fifty people y" still reads "50" (#1123).
- `currency=True`: a mark followed by a format character after the unit is kept: "x five dollars，\u200e y" is "x $5，\u200e y" (was "x $5 y", the comma and the mark lost), also "," "." "!" ")" and ZWSP / WJ / BOM / SHY; a pause comma before the subunit still joins it, and the format character goes after the amount: "twelve dollars,\u200b fifty cents" is "$12.50\u200b". A sentence mark behind a format character ends the clause before a subunit count, in default mode too: fr "x un dollar et un.\u200b cent y" is "x 1 dollar et un.\u200b 100 y", as the text without the format character reads (#1154, #1155).
- Sentence mode: the 0.4.1 closer-led rule (#1114) no longer leaves a number as written after a mark-led prefix that is no number word: zh "x )第二十一 y" is "x )第21 y" again (0.4.1: unchanged), also "，约二十", ja "、第三", th ")ที่ยี่สิบเอ็ด" (#1122). A mark-led word that holds the number still keeps it whole: ceb "x )ika-usa libo y", id "x )kedua puluh satu y", he "x )ועשרים וחמש y" (#1143, #1151). A spaced format character inside the number no longer splits it: "x )two hundred \u200b five y" stays as written (0.4.1: "x )two hundred \u200b 5 y") (#1130).
- Sentence mode: a format character before a number word is kept ("x \u200bfive y" is "x \u200b5 y", was "x 5 y"), and the bidi marks LRM, RLM and ALM count as format characters, so a number next to one reads ("twenty.\u200e One" is "20.\u200e One", was left as written; ar "عشرون،\u200f واحد" "20،\u200f 1"); inside a number they are dropped like ZWSP ("ninety \u200enine" is 99). A currency, date or phone fold reads through a leading format character as before ("twelve dollars \u200bfifty" is "$12.50\u200b"). An ordinal word with one attached stays an ordinal ("x \u200bthird y" is "x \u200b3rd y", was "x 3 y"), and, in the aero spelling alphabets, a token of format characters alone between two digit words ends a dictation ("fife \u200b seven" is "5 \u200b 7", was 57) (#1000, #1089, #1090, #1091, #1092).
- Sentence mode: a quoted number followed by a space and a format character alone keeps its closing quote: 'x "five" \u200b y' is 'x "5" \u200b y' (was 'x "5\u200b y'), also with LRM / RLM / ALM. In ar / he a format character before the conjunction of a number word ("\u200eوخمسة") no longer splits the number: "خمسمائة \u200eوخمسة وخمسون" is 555 (was "505 و50"). A format character before a scale word no longer ends the number at the comma after it: "twelve \u200bthousand, three hundred and forty-five" is 12345 (was "12000, 345"). A format character before a leading bracket, quote or comma rides with the mark and ends the number before it: "x forty \u200b(two) y" is "x 40 \u200b(2) y" (was "x 42) y"); after a glued sign it is kept after the sign: "x -\u200etwenty five y" is "x -\u200e25 y". The same between two leading marks ("x '\u200b'twenty five y" is "x '\u200b'25 y", was "x '25 y"), before a sign ("x \u200e-\u200efive y" is "x \u200e-\u200e5 y") and before a zh / ja / th mark ("四十 \u200b.七" is "40 \u200b.七", was 47). With `date=True` or `phone=True` the format characters next to a converted number are dropped, LRM / RLM / ALM now included, as ZWSP already was (#1020) (#1115, #1116, #1117, #1121, #1125, #1126, #1129, #1138, #1139, #1140).
- Sentence mode and `currency=True`: a compatibility form of a punctuation mark (fullwidth, small, vertical, superscript, Greek ";") is now read by what it folds to under NFKC (a terminal mark, a closing or opening bracket or quote, or "_" / "`"), at either end of a word: "x fifty］ two y" is "x 50］ 2 y", "x fifty⁾ two y" "x 50⁾ 2 y" (were 52), "x ;πενήντα y" (with the Greek question mark U+037E) "x ;50 y" (the Greek question mark was lost), and with `currency=True` "x ‼fifty dollars y" is "x ‼$50 y", "x five dollars fifty︕ y" "x $5.50︕ y". A closer or "＿" glued before a word leaves it as written, like ")" and "_" ("x ］fifty y" stays, was "x 50 y" with the mark lost). The whole number after such a closer stays as written, no split value ("x ）two thousand twenty four y", was "x ）two 1024 y" on the branch). A fullwidth "｀" before a word is cut off like "`" ("x ｀twenty five y" is "x ｀25 y"). ASCII "_" before a word now leaves it as written too: "x fifty _two y" is "x 50 _two y" (was "x 52 y", the "_" lost). A form of "." before a figure stays a decimal point ("x ．5 dollars" is not folded) (#1094, #1095, #1096, #1099, #1112, #1113).
- Sentence mode: after a number word that closes a straight quote, an ordinal word reads on its own, as after a closing bracket: az 'x "iyirmi" birinci y' is 'x "20" 1-ci y' (was left as written since #1120), et 'x "20" 1. y', bg 'x 100" 1-ви y' (#1133).

## [0.4.1] — 2026-10-12

### Fixed

- Sentence mode: a straight closing quote (`"` or `'`) after a number word ends the number, as a closing bracket does, and stays in the output: 'x "twenty" one y' is 'x "20" one y' (was 'x "21 y', the quote lost), 'x twenty" three y' 'x 20" 3 y' (was 'x 23 y'), 'he said "two" thousand people' 'he said "2" thousand people' (was 'he said "2000 people'). A quoted number still reads whole: 'x "twenty one" y' is 'x "21" y' (#1120).
- Sentence mode: a closing bracket glued before a number word (")two", "]deux", "}zwei") keeps that word as written, and now the rest of its number stays as written with it instead of being read as a smaller number: "x )two thousand twenty four y" is unchanged (was "x )two 1024 y"), fr "x )deux cents y" unchanged (was "x )deux 100 y"), also with `currency=True` ("he paid )twenty five dollars", was "he paid )twenty $5") and `date=True` ("x )twenty first of may y", was "x )twenty May 1 y"). Spoken years and digit strings too: "x ]nineteen eighty four y" (was "x ]nineteen 84 y"), "x )oh five y". Two numbers after the mark stay two: ")twenty one twenty one" is ")twenty one 21" (#1114, #1119).
- Sentence mode: ‼ ‥ ⁇ ⁈ ⁉, halfwidth ｡ ､, small-form ﹐ ﹒ ﹔ ﹖ ﹗, ․, hy ։ ՝, am ፧, and the NFKC-folded ． ; ︐–︖ ︰ ﹑ ﹕ end a number and stay in the output: "x fifty‼ two y" is "x 50‼ 2 y" (was "x 52 y", the mark lost); the closing ＂ ＇ ｣ ﹚ ﹜ ﹞ （ after a number are kept, and the sentence marks glued before a number word are cut off and kept like ",": "x ‼fifty y" is "x ‼50 y" (#1087, #1088; a closer glued before a word is left as written, see #1094 above).
- `currency=True`: more marks are kept after a folded amount: `}`, `‥ ‼ ⁇ ⁈ ⁉`, halfwidth `｡ ､ ｣`, small-form `﹒ ﹐ ﹔ ﹖ ﹗`, `“ ‘ ＂ ＇ 〗 〙 〛 ․`, my `။ ၊`, km `។ ៕`, hy `։ ՝`, am `፧`, th `๚`; a comma-like `､ ၊ ՝ ، ﹐` before a subunit joins the amount like ","; a sentence mark or quote before a subunit ends the amount, as "." and "”" do: "twelve dollars‼ fifty cents" is "$12‼ $0.50" (was "$12.50") (#1077).
- fr, ca: a one-word subunit count after "<unit> et" (ca "i", "amb") is the amount, not the article "un" or the adjective "neuf": "un dollar et un cent" is "1 dollar et 1 cent" (was "1 dollar et un cent"), "deux dollars et neuf cents" "2 dollars et 9 cents"; with `currency=True` "2,09 $", "un euro et un centime" "1,01 €", ca "dos euros amb un cèntim" "2,01 €". With `currency=True` "un cent" is never 100 either: "douze dollars et un cent chacun" is "12,01 $ chacun" (was "12 $ et 100 chacun") (#1063). Only in `cardinal`, `year` and `currency` modes; `ordinal`, `ordinal_num` and `digits` leave the count a word (#1065). A count with a mark before or in it ("(un cent)", "vingt( cents") stays as written (#1075). Not yet: a unit word that is itself a homograph ("un real amb un centau" gives "un real amb 1 centau") (#1009).
- `date=True`: an article alone ("the", de "dem" / "zum", es "el", pt "o", it "il" / "fino", fr "le") is no range link: "on 3 12 2026 the 4 5 2000 votes were counted" keeps "4 5 2000" as numbers; "to the", "bis zum", "hasta el" … still join a range (#1076).
- `currency=True`: a punctuation mark after a folded amount (fullwidth, CJK, script, closing quotes and brackets) is kept: "x cinq euros， y" is "x 5 €， y" (was "x 5 € y"), "x five dollars。" "x $5。", "x five dollars” y" "x $5” y"; a fullwidth "，" / "、" before a subunit is a pause like ",", so the amount still joins (#1067, #1071).
- `date=True`: a range joined by two words (de "bis zum", pt "até o", en "to the", es "hasta el", it "fino al") folds its first end too: "ab 12 3 2000 bis zum 13. März 2000" is "ab 12.03.2000 bis zum 13. März 2000" (#1066).
- `date=True`: three numbers with no separator are a date only after a date cue ("on", "born", fr "le", de "am", es "el" ...) or as the whole text: "we need three twelve two thousand people" stays "we need 3 12 2000 people" (was "3/12/2000 people"), "on three twelve twenty twenty six" is still "on 3/12/2026". Also a cue with ":" or "," after it ("Date: …"), the second end of a range ("from … to …", es "del … al …"), a date alone in its sentence or before a clock time ("… at three thirty"), and the de ordinal form ("zwölften dritten …", "12. 3. 2026"), which counts as a separator. es / pt need "de" on both sides like the other separators: "tres de doce dos mil" is no date. After a preposition cue ("by", "from", de "ab", fr "au" ...) a round year ("two thousand") with a word after it stays a quantity ("by three twelve two thousand people"); after a date word ("on", "born", "DOB:", fr "le", de "am") a date is always a date ("born on three twelve two thousand in Paris" is "born on 3/12/2000 in Paris"); pt "de" alone only opens a range ("de 12 3 2026 a 13 3 2026"), and de "dem" / "zum" count only behind a real cue ("ab dem", "bis zum"). A range word counts only when a second date follows it. Behaviour change from 0.4.0: a numeric date inside a sentence with none of these ("we met three twelve twenty twenty six") is no longer folded (#1031).
- `currency=True`: an amount after a glued 。 or 、 folds like one after ，: "x 。five dollars y" is "x 。$5 y" (#1059).
- my: "<digit> နှစ် <digit> <counter>" is years and another count, not a dictated digit string: "သုံး နှစ် လေး လ" is "3 နှစ် 4 လ" (was "324 လ"), "ကိုး နှစ် နှစ် ယောက်" "9 နှစ် 2 ယောက်"; a longer dictation ("ရှစ် လေး နှစ် တစ်", a phone number after a number word or figure) stays one digit string. Not with a leading zero ("သုည နှစ် သုံး လ" stays 023), before a currency or a unit ("သုံး နှစ် လေး ကျပ်" stays 324 kyat), or when a number goes on after the last နှစ်. Trade-off: a four-digit "<d> နှစ် <d> နှစ်" (a PIN such as 5252) with nothing number-like after it reads as years. A figure before နှစ် is a number too, with "," or "." in it but no leading zero: "5 နှစ်", "2,024 နှစ်" stay as written (was "5 2", "2,024 2"); after it a second နှစ် is 2 ("5 နှစ် နှစ်" is "5 နှစ် 2"), and a symbol ("²") is no number before the years shape (#1016).
- Sentence mode: a clause mark glued before a number word (",deux cents", "...deux cents", hi "।पाँच सौ", ur "،دو سو") is cut off and kept, and the number reads whole after it: "il reste ,deux cents euros" is "il reste ,200 euros" (was "il reste 200 euros", the comma lost), hi "x ।पाँच सौ y" "x ।500 y" (was "x ।पाँच 100 y"), and two numbers either side of it stay two: "twenty ,five" is "20 ,5" (was 25). Also ur ۔, hy ։, the fullwidth ，．！？；： and 。、, but not in zh / ja / th, where a lone numeral after a mark stays as written. With `currency=True` the amount still folds after the mark: "x ,five dollars y" is "x ,$5 y" (#1041, #1044, #1047, #1048, #1049). Two consequences: a kept "." or "," before a converted number can read like a decimal ("x .five y" is "x .5 y", fr "il reste ,cinq euros" "il reste ,5 euros"), and currency, decimal and year joins no longer cross such a glued mark ("twelve dollars ,fifty cents" with `currency=True` is "$12 ,$0.50", the subunit read as its own amount as with a spaced comma; "two point .five" is "2 point .5", "nineteen ,ninety" as a year "19 ,90"). In zh / ja / th a fullwidth "．" between two numbers is now kept and ends the first: "十万．一百" is "100000．100" (was 100100).
- A sign word with a mark, then a space, before a figure reads with the locale's separators again: fr "moins: 1.234" is -1234 (was -1.234 in 0.4.0), en "minus: 1,234" -1234 (was an error in 0.4.0) (#1032).
- Sentence mode: in zh / ja / th a run never crosses a glued mark into the piece after it, nor opens on such a piece, so the two numbers are not merged there and the mark stays: "二十，五" is "20，五" (was 25), "一千，五" "1000，五" (was 10005), th "ยี่สิบ，ห้า" "20，ห้า"; a lone numeral after the mark stays as written, as in a list ("二十，五，六") (#1028).
- fr / fr_BE / fr_CH / fr_CA, `currency=True`: after "<unit> et", a subunit count of 100 or more in words (up to four words) keeps "cents" as written instead of multiplying it: "douze dollars et cent cinquante cents" is "12 $ et 150 cents" (was "12 $ et 150 100"), and so does a lone symbol ("Ⅻ cents"). A count ending on the 2-9 that multiplies "cents" is still hundreds ("mille deux cents personnes" is 1200), and a singular "cent" that continues the number stays in it ("deux mille cent personnes" is 2100). Longer counts are still multiplied, as before (#1025, #1029).

## [0.4.0] — 2026-10-11

**Breaking:** numerical symbols (`½` `①` `⑩` `²` `₃` `Ⅻ` `㊀` …) are no longer read as numbers or folded by default, in every entry point including `normalize()` (#958, #960, #971); pass `convert_numerical_symbols=True` to get the old behaviour. From this release on, a release is cut automatically every day at noon (Paris) when `main` has unreleased changes (`scheduled-release.yml`).

The October 2026 audit (gladiaio/words2num2#40–#268) rendered every advertised language with
num2words2-core in every mode, parsed it back, and filed each distinct defect. This release fixes
169 of them; the rest are num2words2 renderer defects (fixed upstream, mitigated here where possible)
or partly fixed, and stay open. On a 1.4M-value cardinal sweep, failures drop from 422774 to 20716
with no value that parsed before parsing differently now.

### Added

- `words2num_sentence(..., date=True)`: a day + month (+ year) is written the way the language writes a date: "on march twelfth twenty twenty six" → "on March 12, 2026", `en_GB` → "12 March 2026", de "am zwölften märz" → "am 12. März", es "el primero de mayo" → "el 1 de mayo" (en, fr, de, es, pt, it, nl). Off by default.
- `date=True` also writes a date spoken all in numbers in figures, in the region's order ("three twelve twenty twenty six" → "3/12/2026" in en, "03/12/2026" in en_GB, "12.03.2026" in de); `region=` overrides the region of `lang`, and `date_format="iso"` writes every date with a year as "2026-03-12".
- `words2num_sentence(..., phone=True)`: each phone number is written the way its country writes it ("06 12 34 56 78", "(212) 456-7890", "+33 6 12 34 56 78"), from Google libphonenumber's numbering plans compiled in (`phonenumber` crate, Apache-2.0; no runtime dependency). The country comes from a spoken country code, else `region`, the region of `lang`, or the language's regions.
- French, German and Spanish sentence mode write clock times in figures: fr "à neuf heures quinze" → "à 9h15", "et quart" / "et demie" / "moins le quart" → "9h15" / "9h30" / "8h45"; de "um neun Uhr fünfzehn" → "um 9:15 Uhr", "um halb zehn" → "um 9:30"; es "a las nueve y cuarto" → "a las 9:15". An hour alone is a time only after a time cue ("à neuf heures" → "à 9h"); a duration stays ("trois heures de route" → "3 heures de route").

### Changed (behaviour callers will notice)

- `auto_parse_sentence` writes a decimal with the mark the same call reads as the decimal (the caller's `decimal_sep`, else "." when only `thousands_sep` is given, else the locale's), so its output reads back as the same number: de "3,125 kg" stays "3,125 kg" (was "3.125 kg", which de reads as 3125, so a second pass multiplied the value by 1000), and de "3.5 kg" becomes "3,5 kg". Locales with a decimal point are unchanged (#867).
- fr_CH: the decimal mark is a comma, as in CLDR (the point was its currency mark). A single separator before exactly three digits now reads the other way: "1,234" is 1.234 (was 1234) and "1.234" is 1234 (was 1.234); "1,000" is 1.000 and "1.000" 1000. Other point decimals ("12.50", "1'234.5", "3.14") read as before. de_CH, it_CH and de_LI keep the point (#951).
- th, lo, km: a digit after a bare scale word from a thousand up is the next place down, as prices and salaries are spoken: "หมื่นห้า" is 15 000 (was 10 005), "ล้านสอง" 1 200 000, lo "ໝື່ນຫ້າ" and km "ម៉ឺនប្រាំ" 15 000. With a multiplier ("สองพันห้า", lo "ສອງພັນ ຫ້າ", km "ពីរពាន់ ប្រាំ") num2words2 writes the same words for the digit as units (2005), so `words2num` keeps that reading, but in sentence mode the span stays words rather than a number that may be 1 000x off. This also leaves the renderer's own spellings of such values as words in sentences, about 5 000 per language in a dense grid (km/lo 1001, th "ราคาหนึ่งพันห้า" 1005, "ห้าหมื่นห้า" 50 005, ...), a year included ("ปีสองพันห้า" stays words: unspaced text gives no sure year label, as "ต่อปี" is "per year"). `words2num` gives no signal of the ambiguity: "พันห้า" is 1500 but "หนึ่งพันห้า" 1005. my is not affected (#875).
- Numerical symbols — superscript, subscript, circled and parenthesized digits ("²", "₂", "①", "⑩", "⒈") and vulgar fractions ("½") — are no longer read as numbers by default. `words2num` and `auto_parse` raise on them, alone or glued to a figure (`words2num("①")` was 1, `"1①"` was 11, `"19⑨⑨"` 1999 in year mode, `"¹⁰"` 10). `words2num_sentence` and `auto_parse_sentence` leave them as written ("the ⑤ million" was "the 5000000", "see note ³ below" was "see note 3 below"). The new keyword `convert_numerical_symbols=True`, on all four functions and as `--convert-numerical-symbols` in the CLI, brings back the earlier reading: a run of them alone in `words2num`/`auto_parse`, and the English sentence walker; `auto_parse_sentence` never read them. A symbol glued to a figure raises in either mode. The keyword is spelled `convert_numerical_symbols` (the issues wrote `convert_numeric_symbols`, which is not accepted). For Rust callers, `words2num2_core::words2num` and `auto_parse` refuse them too; `words2num2_core::with_numerical_symbols(true, || ...)` opts in for the current thread, and `is_compat_digit` is the test (#869, #949, #955, #958). `normalize()` keeps them as written too (`normalize("x ① y")` was `"x 1 y"`, `normalize("½")` `"1⁄2"`, `normalize("Ⅻ")` `"xii"`); its new `convert_numerical_symbols=True` folds them as before, and the lookup tables' internal normalization is unchanged (#972).
- ha: where num2words2 writes "<scale> <10-999>" for both the product and the sum, for a million and above (it drops the "da" a sum takes), the product is read, as a Hausa reader does: "miliyan talatin" is 30 000 000, "miliyan ɗari biyar" 500 000 000. These raised since #348. The cost is the renderer's own spelling of such sums: 1 000 580 ("miliyan ɗari biyar tamanin") now reads 580 000 000, 990 of the 1 000 values 10^6..10^6+999 (and the same at 10^9); write the sum with "da" to read it ("miliyan da talatin" is 1 000 030, "miliyan da biyu" 1 000 002). The thousands are unchanged in every mode ("dubu goma" reads 1 010; its ordinal and "miliyan dubu ɗari" still raise). In sentence mode the words after a scale word that cannot be read (ha "tiriliyan", tet "miliaun") stay words with it, instead of leaving a wrong fragment ("miliaun 2 500000"). sw reads more of its renderer's spellings too (#882).
- `auto_parse_sentence` no longer expands a unit written as a single capital letter (other than "L"; "°C" and "°F" still read), "in", or an "s" glued to a whole number ending in 0, in running text — the shapes of "Room 5 C", "1 in 3" and "the 1990s". So "it is 70 F today", "it is 300 K", "it is -5 C", "a 12in pipe", "a 5 in screen", "a 30s timeout" and "timeout=30s" stay as written; write "°F", "kelvin", "inches", "30 s", or pass `prefer` (e.g. `prefer={"in": "inch"}`) to read them (#392). Currency scales are not affected ("$5M", "€3K").
- Documented that `words2num(..., to="digits")` returns an `int`, so leading zeros are dropped ("zero zero seven" → 7; use `words2num_sentence` to keep "007"), and that digit runs of any length are read (#195, #215).

- `parse_number_string`, `auto_parse` and `auto_parse_sentence` return an exact `Decimal` for a number with a decimal part, never a binary `float`; scale suffixes shift the decimal point, so `"$4.1bn"` is `4100000000` and `"$12345678901234567.89"` keeps every digit (#71).
- `words2num("3.14")` returns `Decimal("3.14")`, the same type as `words2num("three point one four")` (#71).

- Language codes are case-insensitive (`"EN"`, `"pt_br"`, `"EN-US"`), unknown trailing subtags fall back one at a time (`"sr-Latn-RS"` -> `sr_Latn`), and ISO 639-2 codes are accepted (`"eng"`, `"fra"`, `"ger"`) (#192).
- **Breaking:** the "first two characters" language fallback is gone. Language names and unknown codes such as `"swedish"`, `"estonian"`, `"kazakh"` or `"enx"` now raise `NotImplementedError`; before, they resolved to an unrelated language (Swahili, Spanish, Georgian, English) (#197).
- `words2num` and `words2num_sentence` no longer advertise `**kwargs`: an unknown keyword is a `TypeError` that names it (#199, #201).
- The CLI uses argparse: `--lang fr` works as well as `--lang=fr`, and an unparseable input or unsupported `--lang`/`--to` prints `words2num2: error: ...` on stderr and exits 1 instead of printing a traceback (#200).
- PyPy (3.11+) is supported from source only (releases ship CPython abi3 wheels, which PyPy cannot load); the PyPy CI job now gates (#212).

### Fixed

- Sentence mode: a script's own sentence or clause mark ends a number as "." and "," do, and stays in the output: my "တစ်ရာ။ ငါး ယောက်" is "100။ 5 ယောက်" (was "105 ယောက်"), likewise ၊, ur ۔, km ។ ៕, am ። ፣ ፤, and the fullwidth ，！？；： glued between two zh/ja/th numbers: "五，十二" is "5，12" (#1023, #1026).
- Sentence mode: an invisible format character (ZWSP, ZWNJ, ZWJ, word joiner, BOM, soft hyphen) right after a punctuation mark no longer hides the mark: "one hundred.\u200b Five people" is "100.\u200b 5 people" (was "105 people"), fr "vingt.\u200b Trois amis" "20.\u200b 3 amis" (was "23 amis"), and the mark and the format character are kept in the output. A format character after a closing bracket, quote or script mark ("twenty)\u200b one", ar "عشرون،\u200b واحد", hi "बीस।\u200b एक") is cut off with the mark, as without it ("20)\u200b one", "20،\u200b 1"), never merged into 21, and one before the mark ("twenty\u2060) one") reads as without it (#1004, #1011). A format character with no mark before it is still a mere word-break hint, as inside unspaced th/zh/ja numbers ("二百\u200b五十" is 250) (#870). A format character left after a number word at the end of its run is kept: "x five\u200b y" is "x 5\u200b y" (was "x 5 y"), and one before a closing mark stays with it: "twenty\u2060) one" is "20\u2060) one" (was "20) one") (#1005). With `currency=True` the amount still folds, the character after it: "twelve\u200b dollars" is "$12\u200b", as is a figure written with one, "20\u200b dollars" (#1021); with `date=True` or `phone=True` the value folds as before and the character is not kept (#1020).
- fr / fr_BE / fr_CH: a subunit count written with "et un" ("douze dollars et vingt et un cents", the renderer's spelling of x.21–x.91) reads as cents, not 2100: the connector is the one right after the unit name, not the "et" inside the count. This holds in the plain walk and with `currency=True` (#1010). With `currency=True` the count is likewise one plain number naming no currency and crossing no punctuation, so "soixante cents et deux cents personnes" and "cinq et deux cents personnes" keep 200 people (#1017). A count of zero still reads: "douze dollars et zéro cent" is "12,00 $" (#1019). A count in figures is the amount whatever its clause: "12 dollars et 2 cents personnes" is "12,02 $ personnes". A count that is one amount but no integer keeps its subunit as written: "douze dollars et deux virgule cinq cents" is "12 $ et 2,5 cents" (#1022).
- my: "နှစ်" after a number is the year counter, not 2: "အသက် နှစ်ဆယ့်တစ် နှစ်" is "အသက် 21 နှစ်" (was "21 2"), "နှစ်ထောင့်နှစ်ဆယ့်လေး နှစ်" is "2024 နှစ်" (was 2026). The renderer's spaced "တစ်သောင်း နှစ်" (10002) still reads. "တစ်သောင်း နှစ် ကြာ" stays 10002 (the renderer's spelling wins over "ten thousand years"). Before a counter or currency it is the two: "ငါးရာ နှစ် ကျပ်" is "502 ကျပ်", "တစ်ရာ နှစ် ယောက်" "102 ယောက်" (#1015); after a mark it is the two, never the year ("အဆင့် သုံး၊ နှစ် နဲ့ တစ်" is "အဆင့် 3၊ 2 နဲ့ 1", #1014). A digit after the point "ဒသမ" is no year: "ငါး ဒသမ နှစ်" is 5.2 (#1018). A round number with nothing after the "နှစ်" reads as years in a sentence ("နှစ်ထောင် နှစ်" is "2000 နှစ်") though `words2num` still reads the whole text as 2002. Spaced groups add only below the lowest place filled, in every language that sums spaced groups (my, ko, hu): `words2num` refuses my "2024 ငါး" (was 2029) and hu "ezerötszáz száz" (was 1600); sentences split ko "칠만 이천팔백삼십팔 삼" into "72838 3" (was 72841) and hu "ezerháromszáznegyvenhárom kettő" into "1343 2" (was 1345), as en "two thousand twenty four two" is "2024 2". A ko compound spaced inside ("이천오 십") now reads as its glued spelling (2050, was 2015) (#876).
- Sentence mode without `currency=True` reads a US-cent subunit after "<unit> et" as cents, as `currency=True` already did: fr "douze dollars et cinquante cents" is "12 dollars et 50 cents", not "12 dollars et 5000" (fr, fr_BE, fr_CH, fr_DZ). Only the "<unit> et <number> cents" form is covered; without "et" ("douze dollars cinquante cents") the cents are still read as hundreds (#868).
- English sentence mode (`cardinal` and `year`) writes a clock time: an hour (one..twelve) then its minutes ("fifteen", "forty five", "oh five") after "at", "around", "by", "until", "from"... or before am/pm: "call me at nine fifteen" → "call me at 9:15", "nine thirty am" → "9:30 am". Without a cue it stays two numbers ("five twenty" → "5 20").
- `words2num_sentence(..., to="year")`: a century and a decade word read as one decade, "the nineteen nineties" → "the 1990s" (was "the 19 nineties"); a pair with a single-digit first half is no year in running text (was "at 915"). The single-token `words2num("nine eleven", to="year")` is still 911 (#111).
- `currency=True`: an article before a currency counts as 1 when a subunit follows: en "a dollar and twenty cents" → "$1.20", "an euro and fifty cents" → "€1.50", fr "une livre et vingt pence" → "1,20 £". Without a subunit, en "a"/"an" and the fr homograph "une livre" stay words, as before ("a dollar", "une livre de beurre").
- A tens morpheme the renderer writes glued reads when spoken apart again, as before #621: fi "kolme kymmentä" is 30 ("hän on kolme kymmentä viisi vuotta" → "hän on 35 vuotta"), ro "două zeci" 20 ("două zeci și trei" 23), et "kaks kümmend kolm" 23, hu "huszon kettő" 22, es "veinti tres" 23, be "два ццаць адзін" 21. A morpheme is what a tens word leaves after its leading unit (fi "kolme|kymmentä") or a word from 21 to 99 after its trailing unit 1-9 (es "veinti|tres"), four letters at least, never a teen's piece: so sv "fem ton" (five tonnes), nb "før ti", de "sie ben", it "dici otto", hu "hat van", fr "d eux", vi "ba y" and nl "twee de" stay words (#285, #947, #982, #983). Follow-ups: a unit a suffix morpheme follows ends no run ("viisi kymmentä kuusi kymmentä" is "50 60", not "56 kymmentä"), a morpheme that is an everyday word is none (hy "յոթ անասուն", seven head of livestock, is no 70), and a new tens group ends a run, so long spaced-morpheme texts read at about the pre-#981 speed (es "veinti tres" × 800: 0.2 s → 0.03 s, fi "kolme kymmentä viisi" × 533: 0.26 s → 0.02 s) (#987, #988, #990, #994).
- A sign before a figure reads as "-figure" when it is glued, as zh, ja and th write it ("负1234" is -1234, "マイナス2.75" -2.75, "ติดลบ1234" -1234; they raised), or of several words (zu "ngaphansi kwe 1.5", lg "wansi wa 2.75", hmn "tsis txaus 1.234": a `Decimal` with the locale's grouping, no longer a float). Year mode takes the same path ("ติดลบ1999" is -1999), so "minus 1.234" in year mode reads with the locale's grouping as "-1.234" does (#950).
- Sentence mode crosses the comma num2words2 writes after a scale word whatever the accents: pt_BR "dois milhoes, trezentos mil" (unaccented, as ASR writes it) and NFD text are 2300000, as "dois milhões, trezentos mil" is, no longer "2000000, 300000" (#948).
- `auto_parse` reads a figure before a unit with the caller's `thousands_sep`/`decimal_sep` or not at all, as it reads a bare figure, a currency or a percent: `auto_parse("1.5 km", decimal_sep=",")` and de `auto_parse("12.5 km", thousands_sep=".", decimal_sep=",")` raise instead of giving the float 1.5 / 12.5 the caller ruled out; "1,5 km" with `decimal_sep=","` is `Decimal("1.5")`. The spaced percent and other units did the same ("1.5 %", "2.5 h") and raise now too. This holds for any figure, with the caller's own separator characters ("٣٫٥ km" with `decimal_sep="٫"`), a typographic minus or fullwidth digits too, and without explicit separators a figure before a unit reads as the bare figure does: de "−1.234 kg" is -1234 kg, not -1.234 (a #515 follow-up, #952, #976).
- Sentence mode writes a spoken decimal with the region's decimal mark, whatever the word, so the figure reads back as the same number in that region: es_MX/de_CH/it_CH "tres coma uno cuatro uno" is "3.141" (was "3,141", which the same region reads as 3141), and es/it/pt/gl "tres punto uno cuatro uno" is "3,141" (was "3.141", read back as 3141); es "tres punto cinco" is now "3,5". A language with no format row keeps the word's own mark (af "3,5"). fr_CH writes a comma decimal, as CLDR has it (see Changed) (a #515 follow-up, #951).
- ar: a comma before "و" is punctuation inside a number only after a hundred or a scale word, with a smaller remainder ("ألف، وخمسمائة" is still 1500): "ثلاثة، وخمسون" and "خمسة، وعشرون" are a list again and raise, not 53 and 25, and so do "مائة وثلاثة، وعشرون" (was 123) and "مائة فاصلة خمسة، وخمسون" (was 100.55); signed forms keep their reading ("سالب ألف، وخمسمائة" is -1500) (a #862 regression, #954, #975).
- zh, ja: circled and parenthesized ideographs ("㊀", "㈩", "㊄"), the Hangzhou ten "〸" and kanbun marks ("㆒") are numerical symbols too: `words2num` and `auto_parse` raise on them by default (they read 1, 10, 50; `auto_parse("㊀", lang="zh")` was `Quantity(value=1)`) and sentence mode leaves them as written ("x ㈢ y" was "x 3 y", "三十㈢" was 33); `convert_numerical_symbols=True` reads them alone as before. The Roman numerals ("Ⅻ", "ⅻ") are named as symbols too; they already raised (#963, #965).
- en: a dictation run of digit words stops at a dash on either edge of a word, which is a sign or punctuation, not a hyphen: "it was -four five six degrees" is "-4 5 6", not 456; "call one two three -four" is "123 -4", not 1234. A trailing dash ends the run and stays after the figure: "five five five- one two one two" is "555- 1212", "call one two three-." is "123-." (main dropped the dash). In digits mode a signed zero is no digit: `words2num("-oh three", to="digits")` raises instead of giving 3 (#956).
- ar: a dash or minus ("-", "−", "–", "－", ...; not the superscript "⁻", which no mark reading takes, as "⁻، خمسون" shows) before an ASCII comma and a word decimal keeps the comma as the hundredths mark, as the sign word "سالب" does: "-, خمسون" is -0.50, not -50. As after "-،", only hundredths in the form the renderer writes them ("عشر", "خمس وخمسون") follow the mark, so "-, ثلاثة", "-, واحد", "-, عشرة", "-, مائة", "-, ألف", "-, صفر" and "-, خمسون ," now raise (they gave -3, -1, -10, -100, -1000, 0 and -50), and `to="year"`/`to="ordinal"` raise for "-, خمسون" (#959).
- `auto_parse` reads area and volume units, with the new `kind="area"`: "5 m²", "5 m2", "5 m^2", "10 m³", "2 cm³", km², ft², in² (#896). A glued "2"/"3" is read in `auto_parse` only, and not after "in" ("5 in2" still raises; write "in²"). A case-folded powered unit gets the same lower confidence as other folded units ("5 KM²" is km² at 0.8), and a mega prefix is not folded ("5 Mm²" raises).
- `auto_parse_sentence` reads "m²" and "m^2" as one unit: plain mode writes "5 m^2" as "5 m²", and expand mode writes "a 50 m² flat" as "a 50 square meters flat" (was "a 50 meters² flat") and "5 ft²" as "5 square feet"; "5 in² of film" now expands, and a case-folded one folds as `auto_parse` does ("5 KM² area" is "5 square kilometers area", and so are "5 M²" and "5 IN²", which a bare "M" or "IN" in running text would not be: the power makes the unit plain). A glued digit is left as written in running text ("5 m2 slots", "1 in2 adults") (#896, #953).
- `words2num` and `auto_parse` read a whole figure with its ordinal indicator as that figure, in every language and mode, as de "1." is: es "1º", "1.º", "1.000º", pt "1ª" (NFKD made them "1o"/"1a", which raised). A fraction takes none: es "1,5º", pt "2.5ª" raise. The degree sign is unaffected ("20°" still raises) (#896).
- `auto_parse` reads a sign before a figure as `words2num` does, with the locale's separators: fr `auto_parse("moins 1.234")` is -1234, not -1.234, also with a unit ("moins 1.234 kg"), and ar `auto_parse("-، 50")` is -0.50, not -50. A comma split from the sign is punctuation too ("moins , 1.234"). The caller's `thousands_sep`/`decimal_sep` and region hold on this path (es_MX "menos 1,234" is -1234), and the value is a `Decimal` or an `int` like `words2num`'s, no longer a float; a dash and a comma before the figure read with explicit separators too (`auto_parse("-, 5", decimal_sep=".")` is -5). ar `auto_parse("-، 5")`, `"-، 1.234"` and `"minus ، 5"` raise, as `words2num` does; they gave -5 and -1.234 (#896).
- A comma after a sign before a figure is punctuation, whether the sign is a word or a dash: fr "moins, 1.234" and de "-, 1.234" are -1234, not the float -1.234; fa "منفی، 1,234.5" reads again. Where "،" is the decimal mark (ar), "-، 50" stays -0.50 (#896).
- A superscript or subscript digit glued to a figure is no part of it: `words2num("10²")` and `auto_parse("10²")` raise instead of returning 102 (#869). A sign word or a minus before a signed figure is no double negative: fr "moins -5" and "-(-5)" raise instead of reading 5 (#698). `to="digits"` keeps a leading minus: "-five" raises instead of reading 5 (#697).
- Ambiguous ha/bm spans in sentence mode, follow-ups to #506: a number before a span nothing reads still reads ("mutum biyar miliyan dubu ɗari" is "mutum 5 miliyan dubu ɗari"); a span that reads on its own ("dubu tamanin", 1 080 and 80 000 alike) is no split point and stays words with what precedes it; no split after a scale word (#796). A comma ends the words-span (#797); the number's ordinal word stays words with it, including a span ending in its own ordinal ("ba tan ni sabanan") (#798); two unit words side by side are two numbers ("ba tan ni kelen saba" reads the "saba").
- Arabic "،": the oblique forms of the renderer's hundredths read after the mark (tens "ثلاثة ، خمسين" is 3.50, feminine compounds "خمس وخمسين", "اثنتين", "اثنتي عشرة"), a regression from the #617 fix; a masculine unit there still raises (#881).
- Arabic "،", follow-ups to the #668 fixes: three digits after a glued mark group thousands and raise rather than read a decimal ("12،500" is no 12.5) (#804); a "+" sign is read on glued figures (#805); a one-digit figure after the mark is a list item and raises, only two digits read as hundredths ("ثلاثة ، 5" is no 3.05) (#806); a comma typed glued to a number word, or an ASCII one, before "و" and a number word is inside the number again ("ألف، وخمسمائة" is 1500), but not the renderer's spaced " ، " nor before "واحد" ("مائة ، واحد" still raises) (#807).
- `auto_parse_sentence` leaves a unit shaped like something else as written: a single capital letter other than "L" and not after "°" ("a 5G network", "a 4K TV", "a 3D movie", "Room 5 C"), "in" ("1 in 3 adults", "Born in 1990 in Paris"), an "s" glued to a whole decade ("the 1990s"), and "won" before a number ("Score 2 won 3"); a number glued to a Latin word starts no quantity ("my MP3s", "vitamin B12 5 mg"), a number glued to a word of another script still does ("约5kg"). Lowercase m, g, s, t, d and h still expand (#392). Unit symbols keep their case where it is the SI prefix: "5 Mg", "5 Ms", "5 Mm", "5 Ml", "5 Nm" and "5 NM" are no longer read as milligram, millisecond, millimetre, millilitre or nanometre unless `prefer` names that reading; other case-folded symbols ("5 KG", "5 Gal") still read, at a confidence below 1.0 (#393).
- A negative spoken decimal below one keeps its sign, whatever the sign looks like: a sign word (fr "moins zéro virgule cinq" is -0.5, not 0.5), a multi-word one (lg "wansi wa", zu "ngaphansi kwe"), one glued to the number (zh "负零点五", ja "マイナス零点五") or any dash (#364); renderer `to_cardinal_float` of -0.01..-0.99 and other negatives now reads back in 14,260 more language/value pairs. Only one sign is read, and a long run of signs is refused in one step. A double negative before an apocope raises instead of cancelling out: es "menos menos un" (#298).
- Sentence mode no longer turns ordinary words into numbers: clause punctuation inside a token joins no two words ("twenty,two" stays as written, #283); a word glued onto a number reads no other number (fr "d'eux" is no "deux", vi "ba y tá" is 3, nl "twee de" is no "tweede") and cs/sk/pl numbers before a one-letter word now convert (#285); digits of any script are literals, so "one hundred ٥" is no 105 (#288); pt "é" / it "è" (is) are no connector "e" (and), so "cem é cinquenta" is 100 and 50 (#411); ru "семью" (family) and ru/uk "сорока" (magpie) stay words, except after a preposition or quantity word ("с семью друзьями", "около сорока человек") where they are 7 and 40 (#433).
- Sentence mode crosses a comma after a scale word only after the words the language's renderer writes one after inside a number (en and its variants after every scale, pt_BR, it and lij after "miliardo" and the scales above it): ru "две тысячи, триста" and it "un milione, duecento" are two numbers again, not 2300 and 1000200 (#657, a regression from the #67 fix).
- Figures read with the locale's real separators: Mexican, Central American, US and Caribbean Spanish (`es_MX`, `es_GT`, `es_NI`, `es_HN`, `es_US`, ...) group with a comma and `de_CH`/`it_CH` with an apostrophe (#423); the alias codes `dk`/`cz` read like `da`/`cs`, and id, ca, eu, gl, mk, az, bs, lb, jv, su, km, lo group with a dot (#424); hi, bn, ta, te, kn, ml, gu, pa, or, ne, si, as, ur, mr, my, ms, tl group with a comma, so "5,000" is 5000, not 5.000, and the Indian lakh shape ("12,34,567.5") reads for hi, bn, ta, te, ml, gu, pa, or, ne, as, mr and en_IN (#464). The regional rows reach `words2num`, `auto_parse` and the sentence APIs, not only `parse_number_string`: the caller's region is kept for the format lookup. Spoken decimals in sentence mode now write the locale's decimal mark for the new rows: mk, id, eu, bs and lb write "3,14"; `nb` reads "1.234" as 1234, like `no`.
- A zero head group is no thousands group: "0.500" in de and "0,500" in en are 0.500, not 500, and "0.500.000" raises (#377). With an explicit `thousands_sep` the caller's grouping is trusted: "0,234" is 234.
- `parse_number_string` with explicit separators checks the groups: `"1.5"` with `decimal_sep=","` and `"12,5"` with `thousands_sep=","` raise instead of returning 15 and 125 (#425).
- A typographic minus (U+2212, en dash, ...) before a figure is a sign: de "−1.234" is -1234 and fr "−1,5" is -1.5, in `words2num`, `parse_number_string` and `auto_parse` (#443); a sign word before a figure reads the figure with the locale's separators: de "minus 1.234" is -1234, not -1.234 (#309).
- Arabic "،" decimals, follow-ups to #371: only the hundredths the renderer writes may follow the mark, so a list "ثلاثة ، أربعة" raises instead of reading 3.04 (#617); a mark glued to a word ("مائة، خمسون") or written as an ASCII comma between words ("مائة , خمسون", "مائة,خمسون") is read, and a doubled mark raises (#616); classic num2words' ASCII-comma output reads where both its integer and its hundredths are one word (its spaced "و" in compounds is not read, #662) (#658); figures glued to the mark read it as their decimal point ("٣،٥" is 3.5, as "3,5" is); `to="digits"` keeps reading a dictated "خمسة ، ستة ، سبعة" as 567; a glued minus is read ("-ثلاثة ، خمسون" -> -3.50) (#619); `to="currency"` reads the decimal and `to="year"`/`"ordinal"` raise instead of dropping the mark (#620).
- bo: a compound scale word no longer swallows the scale of the multiplier before it: "གཅིག་འབུམ་གཅིག་ཁྲི་དུང་ཕྱུར" is 11·10^12, not 100001·10^12, and a tsheg before a glued scale word no longer breaks the split; on a 3900-value sweep above 10^4, 81 wrong values and 36 raises are gone (#349).
- ha: "miliyan dubu" is 1 001 000 again (a postfix multiplier of 1000 or more is only taken where the renderer writes the product so) (#348). For millions and above, a spelling the renderer gives two values now raises, both ways round, instead of returning one of them; its ordinal raises too, and sentence mode leaves the whole number in words rather than reading part of it. num2words2 drops the connector before a 10-999 remainder after a scale word, so this covers round values: "miliyan hamsin" is 50 million and also how the renderer writes 1 000 050, so it now raises; so do "miliyan biyu dubu goma" (2 001 010 and 2 010 000) and "miliyan goma" (10^7 and 1 000 010). On main those read one of the two values, silently wrong for the other. The thousands are unchanged: "dubu goma" (1 010 and 10 000) still reads 1 010, as before. bm "ba tan ni kelen" (10001 and 11000) stays in words in sentence mode, with any words of the same number after it.
- Arabic decimals: the renderer's "،" separator is read as the decimal mark with the hundredths after it, in the feminine forms the renderer writes ("ثلاثة ، أربع عشرة" -> 3.14, "مائة ، خمسون" -> 100.50, "، خمسون" -> 0.50, "سالب ، خمسون" -> -0.50, the dual "ألفا ، خمسون" -> 2000.50); before, the mark was dropped and "مائة ، خمسون" read as 150. A trailing "،" stays punctuation. The standard separator "فاصلة" is read too ("ثلاثة فاصلة أربعة عشر" -> 3.14) (#371).
- Cardinals up to 10^18 parse: trillion, long-scale billion/billiard, the Indian kharab/padma/shankh, ja/zh 兆 and 京, ko 조 and 경 ("eine Billion", "dos billones", "one kharab", "一兆"). A power whose multipliers the renderer drops (ro "bilion" for 1, 2 and 5·10^12) is no longer learned, so it raises instead of reading a wrong value (#84, #220).
- Stacked scale words multiply instead of adding: bn "দশ কোটি কোটি" = 10^15, ba "бер миллиард миллиард" = 10^18, eu "mila mila milioi" = 10^12, th "หนึ่งล้านสองแสนล้าน" = 1.2·10^12; the remainder after a scale word must stay below it. A stacked reading is accepted only where num2words2 spells that very number so, and languages that never stack (fr "deux mille deux mille") decline (#40, #217).
- Scale composition never wraps around i64: an overflowing input raises instead of returning a negative or wrapped number (#72).
- Very long inputs (tens of thousands of number groups) are refused up front instead of recursing once per group and overflowing the stack (#187).
- Digit-by-digit numbers (en_AERO) of 19 digits parse; longer runs raise instead of overflowing (#94).
- dv scale words are read from their glued renderings with the special multiplier forms (ދެ = 2, އެއް = 1): "ތިރީސްއެއްހާސް" = 31000 instead of a raise or 30000, "ދެމިލިޔަން" = 2000000 (#41).
- vi "tỷ" (10^9) is a scale word: "một tỷ" = 1000000000. bm "ba" stays out, since every "ba X ni Y" spelling is also another number (#42).
- kok / or / pli: लाख / ଲକ୍ଷ / lakkha is a lakh (10^5), and the renderer's "ten lakh" unit reads 10^6 ("धा लाख", "एक धा लाख", "eka dasa-lakkha"); "एक शंभर धा लाख", which reads 10^8 to the renderer and 110 lakh to a reader, raises. sn "bhiriyoni mbiri" = 2·10^9 (#43, #221).
- A table number glued to a scale is split when more follows: hu "tízezer-száz", fi "kymmenentuhatta sata", sv "tiotusen etthundra", nl "tienduizend honderd" = 10100 (#90, #256).
- ja / zh / ko / bo / th myriad numbers with a multiplier up to 9999 and a multi-scale tail parse: "千二百三十四万五千六百七十八" = 12345678, "일억 이천삼백사십오만 육천칠백팔십구" = 123456789 (#91).
- zh 零 between scale groups: "一万零一" = 10001, "一亿零一万" = 100010000, "一万零一十" = 10010. A lone digit after a scale without 零 ("一万五", spoken for 15000) now raises instead of reading 10005 (#92).
- sv "tjugoettusen" (ett + tusen sharing a t) = 21000, and the renderer's "en miljonett" / "två miljonerett" = 1000001 / 2000001 (#99, #239).
- th final "เอ็ด" after a higher place: "หนึ่งหมื่นเอ็ด" = 10001 (#100).
- sl / lt scale plurals heard only after 3, 4 or 10+: "tri milijarde", "dešimt milijardų" (#102).
- The standard spellings da "en million" and sl "pet milijonov" / "pet milijard" parse beside the forms num2words2 writes (#240, #259).
- Numbers above 10000 with a connector after the scale word now parse in fa, ckb, ku, ki, is, da, nb/no, bm, ha, lg, lij, om, rm, rw, sq, tet, ti, vi, xh, zu, bg and cy ("ده هزار و یک" = 10001, "ein milljón og einn" = 1000001, "miliyan da ɗaya" = 1000001). In sentences, a number run no longer ends on the connector, so "tre og fire" stays "3 og 4". bm writes 10001 and 11000 the same way ("ba tan ni kelen"), so that text is not read as either number (#85).
- "And" written as a prefix of the next word now parses after a scale word: ar "مليون وواحد", he "מיליון ואחת", sn "miriyoni neimwe" (#86, #101).
- A multiplier in a gender, construct or attributive form now parses: ru/be/uk/bg/hr/sr "двадцать одна тысяча", cs "dvě miliardy", el "είκοσι μία χιλιάδες", cy "tair ar hugain mil", is "eitt þúsund", he "שני מיליון" / "אחד עשר אלף", hu "kétmillió", et "sada tuhat", ce, la, mn, rm, lij. These forms are taken from num2words2's own longer renderings (#87).
- ar dual and accusative scale nouns now parse: "مليونان", "ملياران", "ألفاً", "مئتا ألف", and the genitive "ألفين" (#88).
- da "tusinde", and a thousand glued to the words around it, now parse: "tolvtusinde og trehundrede og femogfyrre" = 12345, "ethundrede og ettusindsyvhundrede og fireogtyve" = 101724 (#89).
- The form a scale word takes before a remainder now parses: kn "ಹತ್ತು ಸಾವಿರದ ಒಂದು", te "పది వేయిల ఒకటి", tg "даҳ ҳазору як" (#93, #96, #97).
- rm numbers with the connector glued after the thousand now parse ("dieschmilliedin" = 10001) (#95).
- A bare hundred inside a larger number now parses: nb "hundre tusen", tg "даҳ ҳазор яксад" (#98).
- Feminine numerals now parse on their own: pt "duas", "duzentas", es "doscientas", ru "две", uk "дві", cs "dvě", ca "dues", el "τρεις", is "tvö". "duas libras" becomes "2 £" when currency=True (#105).
- In sentence mode, gendered and combining number words no longer split a number into two: "две тысячи двадцать четыре" gives 2024 instead of "две 1024", and kn/mn 12345 is read as one number. A form that only works inside a longer number stays a word when it stands alone (he "יום שני", "Monday") (#66).
- el "εκατόν" before a remainder now parses: "εκατόν δώδεκα" = 112. num2words2 writes "εκατό δώδεκα", which still parses too (#237).
- Negative numbers parse in every language: the minus word is read off the renderer (fr "moins sept", es "menos cuarenta y dos", zh "负七", xh "ngaphantsi kwe sixhenxe") instead of an English-only list (#123).
- The sign applies to the whole number: "moins un million mille" is -1001000 (was -999000), am "ሰልቢ አንድ ሺህ …" is -1234 (was -766), and negatives above 10000 ("minus zwölftausend…") no longer raise (#61).
- A minus glued to the number parses: th "ติดลบเจ็ด", tr "eksiyedi" (#124).
- Kannada and Telugu accept the spoken minus words ಋಣ / ಮೈನಸ್ and మైనస్ as well as the renderer's "(-)" (#243).
- Typographic dashes and the minus sign (en dash, non-breaking hyphen, U+2212) work like "-": "twenty–one", "−five" (#114).
- Curly and modifier-letter apostrophes match the ASCII one: uk "п’ять", uz "toʻrt", haw "ʻekahi" (#115).
- Turkish/Azerbaijani upper case with dotless ı parses: "KIRK", "ALTI" (#116).
- German "ss" matches "ß": Swiss "dreissig", "ZWEIUNDDREISSIG" (#117).
- Invisible characters (BOM, zero-width space/joiners, soft hyphen, word joiner) no longer break a number word (#118).
- Malayalam legacy chillus (consonant + virama + ZWJ) match the atomic chillu: "ഒന്‍പത്" (#119).
- Tibetan numbers parse without the final tsheg: "གཅིག" (#120).
- Script-native punctuation and brackets are ignored around a number: "पाँच।", "خمسة،", "五。", "«cinq»", "(five)"; in sentence mode a trailing mark stays after the figure (#121).
- Hindi/Nepali anusvara spellings match the chandrabindu ones ("पांच" = 5), and the Bengali chandrabindu is optional (#122).
- Norwegian ordinal "attende" is 18th again, no longer 8th ("åttende"); "atte" without the ring still reads 8 (#44).
- Ordinals now parse in every gender, case and number, not only the masculine form num2words2 renders: ru "первая", "третьего", "двадцать первая" (#137), uk "перша" (#138), be "першая", "трэцяя" (#139), pl "dwudziesta pierwsza" (#140), cs "druhého" (#141), sk "prvá" (#142), sl "tretja" (#143), hr/sr/bs "prva", "трећег" (#144), bg "първа", "първата", "третия" (#145), lt "pirmoji" (#146), lv "otrā" (#147), ro "a doua", "a douăzeci și una", "întâiul" (#148), ca "primera", "cinquena", "primers" (#149), el "πρώτη", "εικοστή πρώτη" (#150), he "ראשונה", "שלישית", "הראשון" (#151), is "fyrsta", "önnur" (#159), da "andet" (#160), nb "annen"/"annet" (#161), sv "förste"/"andre" (#162), cy "trydedd", "gyntaf" (#163), la "prima", "primum" (#164), eo "unuan", "unuaj" (#165), et "esimese", "kolmanda" (#166), fi "toisen", "kolmannen" (#167), hu "elsőt", "másodikán" (#168), sq "e dyta", "i parë" (#169), mt "ewwel" without the article (#170), sw "ya kwanza", "cha pili" (#171), fr plural "premières", "deuxièmes" (#172).
- es ordinals 11th-29th spoken spaced ("vigésimo primero", "décimo primero", "vigésima primera") parse like the glued form (#126).
- Compound ordinals parse in their grammatical form where num2words2 renders the cardinal + a suffix: cs "dvacátý první", sk "dvadsiaty prvý", hr/sr/bs "dvadeset prvi", lt "dvidešimt pirmas", lv "divdesmit pirmais", el "δέκατος όγδοος", la "vicesimus primus", gl "primeiro", "vixésimo primeiro" (#244); el "εκατοστός εικοστός πρώτος" is 121 (#230).
- be "васьмідзясяты" (80th) and every x80th parse (the renderer writes them as the 70th) (#231); "дзевяцісоты" (900th) with a Cyrillic і parses (#241).
- cy "unfed ar hugain", "ugeinfed", "deugeinfed" (#251) and the round hundreds / 1000th "dau ganfed", "tri chanfed", "milfed" (#252) parse.
- ro "al o sutălea", "al o sută unulea", "al o mielea" parse (#253).
- Sentence mode writes the figure in the spoken gender and number: bg "1-ва глава", ca "la 5a planta", el "η 1η μέρα", fr "les 2es places" (#145, #149, #150, #172), it "le 1ª volte" (#185). An inflected ordinal after a number stays a fraction ("trois cinquièmes", "tres quarts"), and homographs stay words when alone (el "Τρίτη" Tuesday, da "andet" other, fr "secondes", sw "kwanza").
- Ordinals agreed with their noun or carrying a case suffix now parse: ar feminine / indefinite (الأولى, الثالثة, ثالث, أول), hi पहली / पहले / पाँचवीं / पाँचवें, mr पहिली / पहिले / पाचवी, ne पहिली / दोस्री, gu પહેલી / પહેલું / પાંચમી, pa ਪਹਿਲੀ / ਪਹਿਲੇ / ਪੰਜਵੀਂ, ur پہلی / پہلے / پانچویں, tr ikinciye / birinciyi / birincisi / üçüncüsü (#152, #153, #154, #155, #156, #157, #158, #173).
- The other standard ordinal forms parse too: fa دومین / بیست و یکمین and اول / نخست / نخستین, ja 第一 / 第二十一, ko 첫째 / 둘째 / 셋째 and 두번째 (#174, #175, #176).
- Standard spellings the renderer does not produce are accepted: ar الحادي والعشرون / الحادي بعد المائة (#249), mn glued нэгдүгээр / хорин нэгдүгээр (#248), hy glued քսանմեկերորդ (#250), ms kesebelas / kedua puluh satu (#254), vi thứ mười một / thứ nhất / thứ tư, bn প্রথম / একুশতম and ar الألف in ordinal mode (#245).
- In sentence mode a Hindi / Gujarati / Turkish ordinal keeps its agreement in figures: "पहली बार" is "१ली बार", "ikinciye" is "2'nciye" (#153).
- `to="ordinal"` no longer maps et "üheksas" (9th) and ms "kesepuluh" (10th) to -1: the ordinal table is no longer built from the renderer's garbage ordinal of -1 (#62).
- `to="ordinal_num"`: a prefix hyphen is not a minus sign ("ke-21", "ika-21", "we-100" are 21 / 21 / 100) (#63); thousands-grouped figures are read whole ("1,000th", "1.000.", "1 000e" are 1000) while a decimal ("2.5th") or figures glued to a word ("covid19") raise (#64); la Roman numerals (XXI, MMXXIV) and the word forms ar / bn render (الأول, একুশতম) parse (#127, #246).
- Ordinals above 10000 parse: "dix mille unième" (10001), "douze mille trois cent quarante-cinquième" (12345), "zwanzigtausendste" (20000), "millonésimo" (1000000) (#125).
- Sentence mode reads a compound ordinal spoken in parts as one: es "vigésimo cuarto piso" is "24º piso", tr "kırk ikinci" is "42'nci", it "venti quattresimo" is "24º", ro "al douăzeci și unulea" is "al 21-lea"; pl "dwadzieścia pierwszy" stays in words instead of becoming "20 pierwszy" (#70).
- English `to="year"`: a tens word plus unit is the year itself ("twenty-two" → 22, not 2002) (#56); "oh"/"o" years read ("nineteen oh-five" → 1905, "ten oh six" → 1006) (#110); 3-digit years as num2words2 renders them read ("nine eleven" → 911, "one twenty-seven" → 127) (#111); the "BC"/"AD" era word is accepted ("forty-four BC" → -44) (#112).
- English stacked scale words multiply: "a thousand million" → 1000000000, "one million million" → 10^12 (#57).
- English no longer sums two numbers said side by side ("one hundred one hundred", "one thousand one million", "12 34" now raise) (#58).
- English reads every scale word num2words2 renders up to centillion ("one unvigintillion" → 10^66) and every scale ordinal ("one vigintillionth", "one centillionth") (#113).
- en_IN, en_NG, en_NE and en_NP fall back to the English grammar where their table has no reading: decimals, year pairs, "a hundred", "double zero seven", scales above the table; en_IN keeps lakh/crore (#193).
- `to="year"` reads the year spelling of every table language's renderer: year words (bn সাল, ba йыл, vi năm, hy թվական, km ឆ្នាំ, zh/ja 年, ...) are stripped (#128), zh digit-by-digit years ("一九〇五年" → 1905) (#129), ja imperial eras ("明治三十八年" → 1905) (#130), ta "ஆயிரத்து" (#131), fi glued years (#132), af "negentien nege-en-negentig" (#133), nb/no "nitten hundre og fem" (#134), am "አሥራ ዘጠኝ መቶ አምስት" (#135), dv "ނަވާރަ ސަތޭކަ ފަހެއް" (#136), and lij years carrying the renderer's stray ". urtea" suffix (#247).
- `to="digits"` reads Wolof hyphenated digits ("juróom-ñaar" is 7, not 5 2) (#50), the two-word lus/miz zero "a awmlo" (#106), and English "double"/"triple" (#190).
- `words2num_sentence(..., to="digits")` writes a digit run as its digit string, keeping leading zeros and "double"/"triple" ("call double zero seven" → "call 007") (#190, #195).

- Two digit words in a row are a range, not one number: "deux trois minutes" → "2 3 minutes", not "23". A dictated string still needs three digits or a leading zero ("zero six" → "06", "one two three" → "123") (#51).
- A token already written in figures never joins a word run: "nineteen 90" → "19 90", "cinq 5" → "5 5" (#68).
- English "oh" is the interjection on its own ("oh no", "I said oh" are left alone) and 0 only next to another digit word ("room oh seven" → "room 07") (#79).
- Long texts made of number words are processed in linear time (2000 tokens in well under a second instead of minutes) (#186).
- Rewritten text never uses exponent notation: "zero point zero zero zero zero zero zero one" → "0.0000001", and `auto_parse_sentence("0.0000001 kg")` keeps "0.0000001 kg" (#202).
- Punctuation around a number is kept and no longer blocks it: "(forty two)" → "(42)", "\"forty\"" → "\"40\"", "forty…" → "40…", "«vingt-deux»" → "«22»", "¿veinte?" → "¿20?" (#69).
- Cardinals joined by a connector convert whole in ~37 more languages, with the connectors taken from num2words2's own spelling (is "fjörutíu og tveir" → 42, no "fem hundre og femtifem" → 555, fa "چهل و دو", sq, bg, ha, lus, vi "lẻ" ...); ro "douăzeci de lei" keeps its "de" ("20 de lei"); "and" between two separate numbers stays a list ("fem og seks" → "5 og 6") (#65).
- The comma num2words2 writes after a scale word stays inside the number: "twelve thousand, three hundred and forty-five" → "12345"; "in two thousand, five people came" still gives "2000, 5" (#67).
- A lone homograph of a number word stays a word (it "tu sei bravo", fr "quoi de neuf", nl "acht geven", de "in Acht nehmen"); next to another number word it is the number ("sei e trenta" → "6 e 30") (#80).
- el "τοις εκατό" and vi "phần trăm" (percent) keep their hundred word (#82).
- The indefinite article that is the word for one stays a word in tr, hu, sv, da, no, el, fa, hi, bn, vi ("Bir gün bir adam geldi."), unless a unit follows ("bir lira" → "1 lira") (#83).
- The article-one in front of a scale word opens the number: sv "en miljon kronor" → "1000000 kronor", no "ett tusen" → "1000", ro "o mie de lei" → "1000 de lei" (#183).
- Case-declined cardinals are read: ru "около трёх тысяч" → "около 3000", "сто двадцати" → "120", uk "трьох тисяч", pl "trzech tysięcy", ar "ثلاثة وعشرين" → 23, "عشرين" → 20, fi "viidellä" → "5:llä" (#74).
- tr spaced hundreds before a scale word: "üç yüz bin" → 300000, "üç yüz bin beş yüz" → 300500 (also in `words2num`) (#75).
- vi "hai nghìn không trăm linh bốn" → 2004, "không trăm hai mươi" → 2020, "ba mươi tư" → 34 (#77).
- Common spellings beside num2words2's: el "εννιά", "δεκαοχτώ", hi "छह", "पांच", da "tres" / "halvtres", bn "পঁয়ত্রিশ" (#78), cs inverted "pětadvacet" → 25 (#179), ro contracted teens "unșpe" ... "nouășpe" (#180). Also accepted by `words2num`.
- A case ending, classifier or counter glued to a number is kept after the figures: tr "on ikide" → "12'de", "Saat üçte" → "Saat 3'te", hu "Háromkor" → "3-kor", bn "বারোটা" → "12টা", ko "삼만원이에요" → "30000원이에요"; words that only look suffixed stay ("yüzde elli" → "yüzde 50", "ona", "egykor") (#73).
- A proclitic preposition glued to a number is kept before it: he "באלף תשע מאות תשעים ותשע" → "ב-1999", ar "بثلاثمائة ألف" → "ب300000" (#76); so is an elided clitic: fr "d'onze heures" → "d'11 heures", it "dell'otto" → "dell'8" (#184).
- Numbers inside unspaced Chinese, Japanese and Thai text convert in place: "我买了三个苹果。" → "我买了3个苹果。", "家は三百万円です。" → "家は3000000円です。", "ฉันจ่ายยี่สิบสามบาท" → "ฉันจ่าย23บาท"; words that contain a numeral character stay (一起, 十分, 四川, สามารถ, เก้าอี้) (#177).
- Native Korean numerals are read: "사과 세 개" → "사과 3 개", "서른다섯 살" → "35 살", "하나 둘 셋" → "1 2 3"; "네" (yes) and the age counter in "삼십오 세" are left alone (#178).

- `parse_number_string(..., lang=...)` reads a dot followed by 1, 2 or 4+ digits as a decimal in every locale (`"3.14"` with `lang="fr"` is 3.14, `"1,5"` with `lang="en"` is 1.5); `"1.234"` stays a thousands group where the locale writes decimals with a comma (#53).
- A hyphenated locale (`"pt-BR"`) uses its own separators, and a language without a separator table auto-detects instead of applying English rules (#54).
- Auto-detection no longer reads `"1234.567"` as 1234567 or `"1.5.2026"` as 152026: a leading group of four or more digits is a decimal, and repeated separators must group thousands in threes (#55).
- `words2num` reads numbers written in figures with the language's separators: en `"1,234"` is 1234 (not 235), de `"1.234"` is 1234 and de `"2,5"` is 2.5 (#59, #60).
- The Swiss typographic apostrophe (`1’234.5`) is accepted as a thousands separator (#109).
- `thousands_sep` equal to `decimal_sep` (or a digit as a separator) raises `ValueError` instead of silently deleting the decimal point (#188).
- `auto_parse` accepts `Mn`/`BN`/`TN`-style scale suffixes and raises `Words2NumError`, never `KeyError`, for an unknown one; `auto_parse_sentence` leaves such a span untouched instead of aborting (#194).
- `auto_parse(..., prefer=...)` now swaps the whole unit (`unit`, `unit_long`, `kind`): `"5m"` with `{"m": "minute"}` is `5 min`, a time; a target that is not a reading of the token is ignored (#203).
- `auto_parse` and `auto_parse_sentence` raise `NotImplementedError` for an unsupported language, like `words2num` (#204).
- `words2num(text, to="currency")` parses a spoken amount and returns it as a `Decimal`, unit plus subunit: `"twelve euros, thirty-four cents"` is `Decimal("12.34")`, de `"zwei Euro"` is `Decimal("2")`; text without a currency word is read as a plain cardinal (#189).
- Currency fold (`words2num_sentence(..., currency=True)`): a bare subunit of 100 or more carries into the unit (`"one hundred fifty cents"` -> `$1.50`) (#45); a spoken fraction longer than the currency's minor unit is left as said instead of truncated (#46); num2words' English `"twelve euros, thirty-four cents"` folds to `€12.34` (#47); a subunit after an amount that was not folded is no longer given a currency nobody named (#48); fr `"douze dollars et trente-quatre cents"` is `12,34 $`, not `12 $ et 3400` (#49).
- Currency fold: `"twenty pounds of flour"` stays a weight (#81); `"one yen"`, `"un rouble"`, `"um iene"` fold like any other amount (#103); every unit and subunit name num2words writes is recognised (`paise`, `kopeykas`, `copechi`, `Fen`, `rappen`, es `francos`, ca `iens`/`rupies`) (#104); `"two dollars and fifty"` and `"cinco euros con cincuenta"` fold at the end of an amount (#181); English writes `20 kr` and `2 zł` with a space (#182).
- Spoken decimals are read in every language the way num2words writes them, derived from its own output: separator words (sv `"komma"`, cs `"čárka"`, hi `"दशमलव"`, ko `"점"`, en_IN `"point"`), whole-word + denominator forms (ru `"три целых четырнадцать сотых"`, hu `"három egész tizennégy század"`, fa, sk, kz, mn), and separators glued into one word (ja `"三点一四"`, tr `"üçvirgülondört"`, `words2num` only) (#107, #108).
- A decimal fraction may start with zeros or be a number up to 999: fr `"trois virgule zéro vingt-cinq"` is `3,025`, `"trois virgule cent vingt-cinq"` is `3,125` (#52).
- `words2num(..., to="cardinal")` reads spoken decimals in every language, not only English: fr `"trois virgule un quatre"` is `Decimal("3.14")` (#191).
- The standard decimal word is accepted where num2words writes English "point": gl `"coma"`, mk `"запирка"`, nn/fo/lb `"komma"`, oc `"virgula"`, mt `"punt"` (#267).
- `to="currency"` reads pt feminine numerals before a feminine currency (`"duas libras"`) and Hebrew amounts with the conjunction glued to either the unit or the subunit, working around num2words renderings (#264, #266).
- `words2num_sentence` raises `NotImplementedError` for an unknown `to` (e.g. `"cardnal"`) instead of returning the sentence unchanged (#201).
- Rust API: `is_scale_word_of`, `scale_words` and `token_may_be_number` no longer cache an entry for every unsupported language string passed in (unbounded memory) (#216).
- Releases: `release.yml` refuses a tag that disagrees with `pyproject.toml` or `_version.py`, and installs and tests every native wheel and the sdist before publishing (#205, #206).
- `auto-release.yml` decides on PyPI, not the tag, so a release that failed after tagging is retried; it now also dispatches the AUR publish (#207, #208).
- AUR package builds again: maturin + Rust make-dependencies, no stale `docopt`/`num2words2` dependencies, and a `check()` that runs the test suite (now shipped in the sdist) and can fail (#209).
- E2E tests run on Rust-only changes, and E2E/Scheduled install `num2words2` so the multilingual round trip is no longer skipped (#210, #211).
- The Lint workflow fails on black, isort, flake8 and mypy violations, and the code passes them; the package ships `py.typed` and `_rust.pyi` type stubs (#213).

### Documentation

- `to="digits"` is documented: digit-by-digit, returns an `int` (leading zeros dropped; sentence mode keeps them) (#196).
- README and REFERENCE.md no longer document names that do not exist since the Rust port (`CONVERTER_CLASSES`, `UNITS`, `CURRENCIES`, `NUMBER_FORMAT_DEFAULTS`, `lang_XX.py`); every REFERENCE.md example is now run as a test (#198).
- Setup docs updated for the Rust package: Python >= 3.10, Rust toolchain for source builds, no runtime dependencies, abi3 wheels, Intel macOS wheels, `normalize` docstring (#214).

## [0.3.5] — 2026-10-08

### Fixed

- **Numbers above 10000 no longer parse to a different number** (#36). A scale
  word's magnitude is now read off num2words2's own rendering instead of being
  assumed from the probed power:
  - lakh / crore are 10^5 / 10^7, not 10^6 / 10^9 — as bn en_IN en_NE en_NP gu
    hi kn ml mr ne pa si ta te ur (hi "दस लाख" was 10000000, en_IN "one
    lakh" 1000000);
  - a multiplier is never a scale word (eu "bat", ha "biyu", en_AERO "zero");
  - multipliers after the scale word: ha / ff / sw "dubu biyu" = 2000, eu
    "milioi bat" = 1000000 (was 10^12);
  - scales glued to their multiplier: km / lo / th / my myriads and
    hundred-thousands, zh / ja / ko 万 / 億, bo, dv;
  - a scale glued to its multiplier inside a longer number: hu
    "egymillió-egy" = 1000001, sv "tolvtusen trehundrafyrtiofem" = 12345, nl, fi;
  - en_AERO and its aliases read digit by digit ("wun zero zero zero zero
    zero" = 100000, was 4000).
  On the round-trip ratchet (`rust/words2num2-core/tests/roundtrip.rs`),
  wrong values go from 72 to 2 and raises from 153 to 62. The 2 left are ha
  10000 / 100000, which num2words2 renders exactly like 1010 / 1100.
  Where the multiplier may follow the scale word (ha ff sw sn bm tet eu) a
  composed reading is kept only if num2words2 renders it the same way, so an
  ambiguous spelling raises rather than returning a wrong number. When the
  renderer spells two values alike (sw / ff / sn 10001 and 11000), the one
  that spelling actually denotes is returned (11000).

- **Indic, Tibetan and Thaana numbers no longer parse to a different
  number.** `normalize()` stripped every combining mark, which erased the
  vowel signs of these scripts — they are letters, not accents. hi `"पचासी"`
  (85) normalized to `"पचास"` (50) and parsed as **50**; bo `"དགུ་"` (9) as 6;
  bn `"একাশি"` (81) as 21; dv 8000 as 1000. Across the 0..=10000 tables that
  was **5310 silently wrong values in 7 languages** (bn bo dv gu hi kn pa).
  The 101 that remain (or 70 vs 17, ha 10000 vs 1010) are the renderer
  spelling two numbers the same way.
  Only optional diacritics are stripped now: the generic combining blocks
  (é, ñ, й), Arabic harakat, Hebrew points and the Indic nukta (so hi
  `"सडसठ"` still reads as 67). Behaviour change for
  `normalize()`: Brahmic, Tibetan, Thaana, Thai and Lao marks are kept.

### Changed

- **Breaking for five languages: Tigrinya, Kyrgyz, Odia, Central Kurdish
  and Konkani parse their own script, and no longer parse a Latin
  transliteration** (#14). ti `"ሓደ"`, ky `"кырк эки"`, or `"ଚାଳିଶ"`, ckb
  `"یەک هەزار"`, kok `"एक शंभर"` now parse; `"ḥade"`, `"kırk eki"`,
  `"calīśa"`, `"yek hezar"`, `"ek xambhar"` — spellings no native writer
  uses — raise `Words2NumError`. The reverse tables follow the renderer,
  which moved to the native scripts in gladiaio/num2words2#134–#142. Odia
  70 (`"ସତୁରୀ"`) and 17 are distinct again; before, 70 / 170 / … parsed as
  17 / 117 / ….
- `num2words2-core` dependency floor raised to **0.1.3** (was `"0.1"`,
  which the lockfile resolved to 0.1.0), and the unlinked copy at
  `rust/num2words2-core/` is gone (#15). It was a workspace member nothing
  depended on: compiled on every build, and editing it changed nothing.

## [0.3.4] — 2026-10-05

Sentence mode (`words2num_sentence`) was audited on ~300 real-life ASR finals
(call-center, IVR, finance; en / fr / es, plus de / it / pt / nl and a sweep
of 20 other locales): 59 % correct before, 96 % after. Every change is
covered in `tests/test_sentence_realworld.py`.

### Added

- **Currency fold in sentence mode**, `words2num_sentence(..., currency=True)`
  (off by default): an amount spoken as number + currency word (+ connector
  + subunit) is written the way the language writes that currency — en
  `"$1,355.28"`, `"$0.99"`, `"£20"`, `"¥10,000"`; fr `"43,20 $"`,
  `"1 200,50 €"`, `"5 $ CA"`; es `"$154.92"` (dollars and pesos in the
  Latin-American form) / `"500,20 €"`; de `"23,50 €"`; it `"20,50 €"`; pt
  `"R$ 200,50"`; nl `"€ 20,50"`. 23 currencies, names and subunits in en /
  fr / es / de / it / pt / nl / ca. A currency word without a number stays a
  word.
- it `"cinquanta centesimi"` is the cent, not the hundredth (was `"50 100º"`).
- de `"tausend Euro"` -> `"1000 Euro"` (the bare thousand morpheme).
- **Ordinals in sentence mode**, written in the language's figures: en
  `21st` / `22nd` / `3rd` / `15th`, fr `1er` / `1re` / `2e`, es-pt-it `3º` /
  `20ª`, de `2.`, nl `15e`, num2words2's `ordinal_num` elsewhere when it
  carries a marker (tr `1'inci`; ru / pl keep the words). A compound is
  always a rank (`"twenty first"`, `"vingt-troisième"`); a lone `first` /
  `second` and its translations only next to a month or "of the month"
  (`"first of all"`, `"wait a second"`, `"la première fois"` stay words);
  fractions stay words (`"a fifth of"`, `"two thirds"`, `"deux tiers"`).
  The ordinal table also matches the inflected forms speech uses (fr
  `première`, es `primer(a)` / `tercer(a)`, de `zweiten`, it `prima`).
- **Digit strings in every language** (were English-only): fr `"sept cinq
  zéro un zéro"` -> `"75010"`, `"zéro six douze …"` -> `"06 12 …"`, es
  `"seis uno nueve ocho cero"` -> `"61980"`; en `"double zero seven"` ->
  `"007"`, `"triple five"` -> `"555"`.
- **Spoken decimal separators**: fr `virgule`, es/ca `coma`, pt `vírgula`,
  it `virgola`, de/nl `komma`, ro `virgulă`, pl `przecinek` -> `","`;
  es/pt/it `punto` / `ponto` -> `"."`: `"trois virgule cinq"` -> `"3,5"`,
  `"deux virgule cinquante"` -> `"2,50"`, `"dos coma cero cinco"` ->
  `"2,05"`.
- **Glued thousands** of de / nl / af / it / sv / da: `"dreiundzwanzigtausend"`
  -> `23000`, `"tweehonderdduizend"` -> `200000`, `"ventitremila"` ->
  `23000`, `"hunderttausend"` -> `100000`; de `"eine Million"`, it `"un
  milione"` open a number.
- fr counts hundreds by the dozen: `"douze cents"` -> `1200`.
- es tens joined with the spoken connector: `"veinte y uno"` -> `21`.

### Ported from upstream

- **Reject a decimal separator with nothing after it.** Ports
  [revdotcom/words2num#5][w2n5] by [@qmac][qmac]. The separator used to be
  dropped silently, so `words2num("one point")` returned `Decimal('1')`,
  `words2num("two thousand point")` returned `Decimal('2000')` and a bare
  `words2num("point")` returned `Decimal('0')`. All of these now raise
  `Words2NumError` with upstream's message, *"Invalid sequence: no tokens
  following 'point'"*. Legitimate decimals are unaffected — `"point five"` is
  still `Decimal('0.5')`, and `"minus zero point zero"` still keeps its
  signed zero.
- **A decimal separator is now an explicit sentence-run head.** The sentence
  walker decides where a number run may start with
  `is_number_word(token)`, which is `to_cardinal(token).is_ok()` — so
  `"point"` qualified only because `to_cardinal("point")` returned `0`, i.e.
  because of the bug above. `words2num_sentence("point five")` would have
  regressed to `"point 5"`. The separators are named explicitly now, so the
  behaviour is a property of the walker rather than a side effect. The rest
  of the run-internal words are unchanged: a run still may not open with
  `and`, `minus`, `negative`, `a` or `an`.

[w2n5]: https://github.com/revdotcom/words2num/pull/5
[qmac]: https://github.com/qmac

### Fixed

- **Every advertised language is now actually accepted.** `supported_langs()`
  reported 172 codes while the resolver knew 120, because the two were
  separate hand-maintained lists. **28 languages raised
  `NotImplementedError`** despite being advertised and despite their reverse
  tables working (`bm ckb cz dk dv ff hmn jv ki ksw ku ky lg lij lus nb om or
  rm rm_puter rm_surmiran rm_sursilv rm_sutsilv rm_vallader rw ti xh zu`).
  The accepted set is derived from the renderer now, so the two cannot drift
  again. Round-trip coverage goes from 124 to 171 of 172 languages.
- **Eight codes were silently answered by a different language.**
  `resolve_lang` falls back to the first two characters of the code, so an
  unknown key matched something else instead of failing: `ban` (Balinese) was
  served by `ba` (Bashkir), `ceb` (Cebuano) by `ce` (Chechen), `cnh` (Hakha
  Chin) by `cn` (Chinese), `fil` (Filipino) by `fi` (Finnish), `kok`
  (Konkani) by `ko` (Korean), `miz` (Mizo) by `mi` (Māori), `pap`
  (Papiamento) by `pa` (Punjabi), `pli` (Pali) by `pl` (Polish). It failed
  loudly only because those table pairs happened to share no words.
  `sr_Latn` and `uz_Cyrl` were likewise collapsed onto their base language's
  script.
- `converter_for` silently fell back to **English** for any resolved language
  missing from the frozen list — unreachable before only because
  `resolve_lang` rejected those languages first.
- **The article / pronoun 1.** `"no one answered"` -> `"no 1 answered"`,
  `"un solde de 43 euros"` -> `"1 solde …"`, `"um momento"` -> `"1
  momento"`: a lone `one` / `un` / `une` / `uno` / `um` / `een` / `ein` now
  stays a word unless something counts it — a unit, a currency or a time
  word (`"one dollar"`, `"une heure"`, `"one o'clock"`), the percent phrase,
  a label word before it (`"press one"`, `"number one"`, `"tapez un"`,
  `"marque uno"`), a month, a counted list (`"one, two, three"`). `"one
  second"` stays a pause. es `"un dólar"` now reads `"1 dólar"`.
- **Compounds the walker broke apart.** A token that is not a number word on
  its own is tried as the continuation of the run before the run closes:
  `"deux cents euros"` -> `"200 euros"` (was `"2 cents euros"`), `"quatre
  vingts"` -> `"80"`; fr `"mille deux cent"` / `"deux million"` (plural -s
  dropped or added) match the table; fr `"cinquante et une personnes"` ->
  `"51 personnes"`; ar / id / he compounds that stopped at the connector
  now compose.
- **Connectors are never eaten**: `"one and a half hours"` was `"1 half
  hours"`, `"five point"` was `"5"`, `"one hundred and"` was `"100"`.
- **Percent phrases**: `"vingt pour cent"` -> `"20 pour 100"`, `"diez por
  ciento"` -> `"10 por 100"` — the second word is the percent sign.
- **`"a"` before a scale word** opens the number: `"a hundred dollars"` ->
  `"100 dollars"` (was `"a 100 dollars"`); not after `half` / `quarter`.
- **Quantities stay words**: `"a couple of thousand"`, `"hundreds of"`,
  `"des millions"`, `"millones de personas"`, `"half a million"`; a scale
  word after a decimal stays a word (`"2.5 million"`, `"2,5 millions"`).
- **Decades**: `"in the nineteen nineties"` (was `"in the 19 nineties"`).
- **Year pairs only for Germanic languages**: es `"a las catorce treinta"`
  is a time of day, not 1430.
- `"ten seconds"` is time, not a fraction; `"one k"` counts.
- A lone punctuation part never extends a run: `"quarante-deux ?"` keeps
  its space.
- vi cardinals were rendered as ordinals (its ordinal table holds every
  cardinal).

### Changed

- `"twenty first"` in sentence mode is `"21st"` (was `"21"`).

## [0.3.3] — 2026-09-24

### Fixed

- **Spoken years in sentence mode.** `words2num_sentence` summed
  `"twenty twenty five"` to `"45"` and `"nineteen ninety nine"` to `"118"`
  (or, since the fix below, split them into `"20 25"` / `"19 99"`). A run
  that is not one cardinal but reads as a year — "nineteen"/"twenty" +
  "oh" + digit / a teen / a tens word (+ unit) — now gives the year:
  `"2025"`, `"1999"`, `"december one twenty twenty five"` ->
  `"december 1 2025"`, `"twenty oh five"` -> `"2005"`. `"two thousand
  twenty five"` is still `"2025"`, `"twenty five people"` still
  `"25 people"`; `to="year"` is unchanged.
- **Digit strings in sentence mode.** Account numbers, ZIP codes and phone
  numbers read one digit at a time were summed: `"your account ending in
  four five six seven"` -> `"... 22"`. Two or more consecutive single-digit
  words (with "oh"/"o" as 0 inside the run) now concatenate: `"... 4567"`,
  `"zero six one two"` -> `"0612"`, `"one two three"` -> `"123"`. A lone
  digit word stays a cardinal (`"press one"` -> `"press 1"`), as does
  tens + unit (`"twenty five"` -> `"25"`); `"five twenty"` -> `"5 20"`.
- **A unit may not follow a unit** (nor a teen follow a tens word) in an
  English cardinal, the counterpart of the tens rule below: `"one two
  three"` is three digits, not 6, and `words2num("one two three")` raises.
- **Spanish "ciento".** num2words never renders `"ciento"` on its own (100 is
  `"cien"`), so the reverse table had no entry for it and the sentence
  walker could not open a run on it: `"ciento cincuenta y cuatro dólares"`
  -> `"ciento 54 dólares"`. The bare hundred prefix now reads as 100 and
  the run grows into the table hit: `"154 dólares"`, `"ciento uno"` ->
  `"101"` (`"cien"` and `"doscientos treinta"` were already fine).

- **A tens word no longer composes with a preceding unit.** English reads a
  tens word with a *following* unit ("sixty three" = 63), never the reverse,
  so `"three sixty"` is two numbers in sequence rather than 3 + 60.
  `words2num_sentence` now returns `"3 60"` where it returned `"63"`, and
  likewise `"three sixty five"` -> `"3 65"`, `"nineteen eighty four"` ->
  `"19 84"`, `"twenty twenty"` -> `"20 20"`. `words2num` raises for these,
  since a single number was asked for and two were given.

  `"hundred"` and the scale words close the sub-hundred slot, so
  `"one hundred sixty"` (160), `"three hundred sixty"` (360) and
  `"two thousand sixty"` (2060) are unaffected — as is the pair reading on
  `to="year"`, where `"nineteen eighty four"` is still 1984. Resolves #17.

### Changed

- The two sentence-mode expectations recorded with the tens rule above,
  `"nineteen eighty four"` -> `"19 84"` and `"twenty twenty"` -> `"20 20"`,
  are superseded by the year reading: `"1984"`, `"2020"`.

## [0.2.3] — 2026-05-02

### Fixed
- `aur-publish.yml` workflow now follows HTTP redirects when polling
  for the freshly-published PyPI sdist. Without `curl -L` the legacy
  `files.pythonhosted.org` URL returns a 302 and the wait loop never
  satisfied, so the AUR job timed out before computing the sha256.

### Verified
- End-to-end release pipeline: tag push → build → test → GitHub
  Release → PyPI Trusted Publishing → AUR push, all green.

## [0.2.2] — 2026-05-01

### Changed
- **Bumped minimum Python to 3.10.** `requires-python = ">=3.10"`.
  Python 3.8 and 3.9 are no longer supported.
- CI matrix now runs on Python 3.10, 3.11, 3.12, 3.13, 3.14, and 3.15
  (with `allow-prereleases: true` so 3.15 alpha is exercised). The
  `Build and Release` and `Publish to PyPI (manual)` workflows use
  Python 3.13.
- Trove classifiers in `pyproject.toml` and `setup.py` updated to
  reflect 3.10–3.15.

### Added
- Comprehensive documentation:
  - `REFERENCE.md` — full API reference with parameters, return types,
    examples for every public symbol.
  - `LOCAL_TESTING.md` — repo setup, test invocations, smoke-test
    recipe, release pre-flight, troubleshooting.
  - Expanded `CONTRIBUTING.md` — where-to-look table, hand-written
    parser guide, adding units/currencies, release checklist.
  - Updated `README.rst` with full feature tour, badges, configurable
    number formats, and the auto-parse mode.
- **Arch Linux / AUR package** — `packaging/aur/python-words2num2/`
  with `PKGBUILD`, `.SRCINFO`, and a maintenance README. Installable
  via `yay -S python-words2num2` (after first AUR push).
- **Eight additional GitHub Actions workflows mirroring num2words2:**
  - `codeql-analysis.yml` — weekly Python security scan.
  - `e2e-tests.yml` — full pytest run on Linux/macOS/Windows × Python
    3.10–3.14 (+ PyPy 3.10).
  - `scheduled-test.yml` — nightly cross-platform test matrix.
  - `pr-size.yml` — auto-labels PRs `size/XS`–`size/XXL`.
  - `manual-release.yml` — `workflow_dispatch` to cut a release
    without touching git locally.
  - `manual-publish.yml` — `workflow_dispatch` PyPI publish (TestPyPI
    optional) using `PYPI_API_TOKEN` secret.
  - `python-publish.yml` — auto-publish on CI success when the
    detected version isn't already on PyPI.
  - `aur-publish.yml` — pushes the matching `PKGBUILD` to the AUR on
    new tags. Needs `AUR_SSH_PRIVATE_KEY` repo secret.
- Expanded README badges: PyPI version, Python versions, downloads,
  status, AUR version, CI, Lint, CodeQL, E2E Tests, Coveralls coverage,
  latest release, last commit, issues, license.

## [0.2.1] — 2026-05-01

### Added
- `pluralize(long_form, value)` helper in `words2num2.converters.auto`.
- Plural rules applied automatically by `auto_parse_sentence(..., expand=True)`.
  - Irregular forms: `foot → feet`, `inch → inches`, `pound sterling →
    pounds sterling`, `degree celsius/fahrenheit`, multi-word
    currencies (`Swiss francs`, etc.).
  - Uncountable units stay invariant: `yen`, `yuan`, `won`, `kelvin`,
    `percent`.
  - Regular `-s`/`-es`/`-ies` for everything else.
- 21 new pluralization tests.

### Fixed
- Sentence-mode regex no longer eats trailing whitespace before non-unit
  words (`Pay $12.50 for 5kg.` now keeps the space before "for").

## [0.2.0] — 2026-05-01

### Added
- **Auto-parse mode**: `auto_parse(text, ...)` and
  `auto_parse_sentence(text, ...)` — extract numeric values plus their
  unit from free text.
- `Quantity` dataclass with `value`, `unit`, `unit_long`, `kind`,
  `confidence`, `raw`.
- `parse_number_string(s, thousands_sep=None, decimal_sep=None,
  lang=None)` with caller-overridable separators, per-locale CLDR-style
  defaults for 50+ locales, and an auto-detect heuristic.
- Currency support: prefix and suffix forms for `$ € £ ¥ ₹ ₽ ₩ ₺` plus
  ISO codes (`USD`, `EUR`, `GBP`, `JPY`, `CHF`, `CAD`, `AUD`, `CNY`,
  `INR`, `BRL`, `MXN`, `RUB`, `KRW`).
- Currency scale shortcuts: `$5k`, `$5m`, `$5b`, `$5bn`, `$2.5t`.
- Unit support:
  - Length — `mm`, `cm`, `dm`, `m`, `km`, `in`, `ft`, `yd`, `mi`, `nm`, `µm`.
  - Mass — `mg`, `g`, `kg`, `t`, `lb`/`lbs`, `oz`.
  - Temperature — `°`, `°C`, `°F`, `K`, `C`, `F`.
  - Time — `ms`, `s`/`sec`, `min`, `h`/`hr`/`hrs`, `d`.
  - Volume — `ml`, `cl`, `dl`, `l`/`L`, `gal`.
  - Percent — `%`.
- Word-form unit aliases (English): `forty-two kilograms`,
  `twenty-three percent`, etc.
- Disambiguation hints: `prefer={"m": "mile", "g": "giga"}` for
  ambiguous unit tokens.
- `expand=True` mode for `auto_parse_sentence` — renders the long unit
  form (`12.5 dollar` instead of `12.5 USD`).
- 55 new tests covering currency, units, separators, and sentence mode.
- New public exports: `auto_parse`, `auto_parse_sentence`, `Quantity`,
  `UNITS`, `CURRENCIES`, `parse_number_string`,
  `NUMBER_FORMAT_DEFAULTS`.

## [0.1.1] — 2026-05-01

### Added
- Mirror of `num2words2`'s CI/CD as four GitHub Actions workflows:
  - `ci.yml` — Python 3.8–3.13 matrix, pytest, coverage.
  - `lint.yml` — black, flake8, isort, mypy.
  - `release.yml` — auto-build, GitHub Release, PyPI Trusted Publishing
    on tag push.
  - `publish-pypi.yml` — manual `workflow_dispatch` fallback using
    `PYPI_API_TOKEN` / `TEST_PYPI_API_TOKEN` repo secrets.

### Fixed
- Dependency spec `num2words2 >= 0.1.0.dev0` so pre-release versions
  from PyPI satisfy the requirement.

## [0.1.0] — 2026-05-01

### Added
- Initial release. Mirrors `num2words2`'s package layout with 120
  dispatch entries (~100 distinct languages, 14 regional variants, 2
  aliases).
- Hand-written English grammar parser (`lang_EN.py`) — cardinals,
  ordinals, decimals, negatives, scale words to *centillion*, year
  mode, "and" connectors, hyphenation.
- Generic backend (`Words2Num_Base`) that derives a `{words → number}`
  lookup table by calling `num2words2` forward across the integer
  range `-1..10000`. Provides correctness for that window for every
  locale that `num2words2` supports.
- 117 stub language modules subclassing the generic backend.
- `words2num(text, lang, to)` and `words2num_sentence(text, ...)`
  walking running text and replacing word-numbers in place.
- CLI: `words2num2 "forty-two"`.
- `Words2NumError` exception type.
- 59 tests.

[0.2.3]: https://github.com/gladiaio/words2num2/releases/tag/v0.2.3
[0.2.2]: https://github.com/gladiaio/words2num2/releases/tag/v0.2.2
[0.2.1]: https://github.com/gladiaio/words2num2/releases/tag/v0.2.1
[0.2.0]: https://github.com/gladiaio/words2num2/releases/tag/v0.2.0
[0.1.1]: https://github.com/gladiaio/words2num2/releases/tag/v0.1.1
[0.1.0]: https://github.com/gladiaio/words2num2/releases/tag/v0.1.0
