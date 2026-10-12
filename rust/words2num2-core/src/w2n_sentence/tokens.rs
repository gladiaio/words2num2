//! The sentence walker's token pre-pass.
//!
//! [`super::tokenize`] cuts the text on whitespace only. That is not enough to
//! find the number words of real text: punctuation sticks to them ("(forty",
//! "«vingt-deux»", "forty…"), and many languages glue a number to a
//! preposition, a case ending or a counter. This pass cuts such a token into
//! zero-width-joined parts, so the walker sees the number word on its own, and
//! records how each part is glued so the text is rebuilt byte for byte when
//! nothing converts. `parts.concat() == sentence` always holds.

/// How a part is attached to its neighbour, when it was cut out of a
/// whitespace token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Glue {
    /// A whole whitespace token, a space run, or punctuation split off a word.
    Free,
    /// A proclitic or elided article cut off the number word after it (ar
    /// "ب", he "ב", fr "d'", it "dell'"). When that word converts, the
    /// separator is written between them (he "ב-1999", ar "ب300000", fr
    /// "d'11").
    Prefix(&'static str),
    /// A case ending or classifier cut off the number word before it (tr
    /// "de" of "üçte", hu "kor", bn "টা", fi "llä"). When the number
    /// converts, the separator is written between them (tr "3'te", hu
    /// "3-kor", bn "12টা", fi "5:llä").
    Suffix(&'static str),
}

/// Punctuation that may open a word and is kept outside the number: brackets,
/// quotes (straight, curly, guillemets, CJK corner brackets), the Spanish
/// inverted marks.
fn is_leading_punct(c: char) -> bool {
    super::folded_mark(c) == Some(super::Mark::Opening) || matches!(
        c,
        '(' | '[' | '{' | '"' | '\'' | '`' | '«' | '‹' | '“' | '”' | '„' | '‟' | '‘' | '’' | '‚'
            | '‛' | '¿' | '¡' | '（' | '「' | '『' | '【' | '〈' | '《' | '〔' | '…' | '—' | '–'
            | '＂' | '＇' | '｀' | '｢' | '﹙' | '﹛' | '﹝'
    )
}

/// A clause mark glued to the next word (",deux cents", "।पाँच सौ"): cut off
/// and kept in a spaced language, so the number opens after it (#1041,
/// #1044, #1049). Not in zh / ja / th, where a lone numeral after a mark
/// stays as written (#1028, #1048).
fn is_leading_clause_mark(c: char) -> bool {
    super::folded_mark(c) == Some(super::Mark::Terminal) || matches!(
        c,
        ',' | '.' | ';' | ':' | '!' | '?' | '।' | '॥' | '،' | '؛' | '؟' | '۔' | '։' | '።' | '፣' | '፤'
            | '，' | '．' | '！' | '？' | '；' | '：' | '。' | '、'
            | '‼' | '‥' | '⁇' | '⁈' | '⁉' | '｡' | '､' | '﹐' | '﹒' | '﹔' | '﹖' | '﹗' | '․' | '፧'
            | '\u{037E}' | '︐' | '︑' | '︒' | '︓' | '︔' | '︕' | '︖' | '﹕'
    )
}

/// The trailing punctuation the walker has always handled itself: it strips
/// it for the parse and re-appends it ([`super::rstrip_punct`],
/// [`super::trailing_punct`]), and `,` / `.` close a run.
fn is_legacy_trailing(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'')
}

/// Punctuation that may close a word. A trailing run made only of the legacy
/// class stays on the word; one holding anything else ("two)", "forty…",
/// "vingt-deux»", "पाँच।") is cut off whole and kept verbatim.
pub(super) fn is_trailing_punct(c: char) -> bool {
    is_legacy_trailing(c)
        || super::folded_mark(c).is_some()
        || matches!(
            c,
            ')' | ']' | '}' | '%' | '…' | '»' | '«' | '›' | '‹' | '“' | '”' | '‘' | '’' | '„'
                | '）' | '」' | '』' | '】' | '〉' | '》' | '〕' | '、' | '。' | '，' | '！'
                | '？' | '：' | '；' | '।' | '॥' | '،' | '؛' | '؟' | '٪' | '።' | '፣' | '፤'
                | '·' | '—' | '–' | '¡' | '¿' | '‼' | '‥' | '⁇' | '⁈' | '⁉' | '｡' | '､' | '﹐' | '﹒'
                | '﹔' | '﹖' | '﹗' | '․' | '։' | '՝' | '፧' | '．' | '\u{037E}' | '︐' | '︑' | '︒'
                | '︓' | '︔' | '︕' | '︖' | '﹕' | '｣' | '＂' | '＇' | '﹑' | '︰' | '﹚' | '﹜' | '﹞' | '（'
        )
}

/// Cut leading punctuation and a non-legacy trailing punctuation run off a
/// whitespace token: `(forty` -> `(`, `forty`; `forty…` -> `forty`, `…`;
/// `«vingt-deux»` -> `«`, `vingt-deux`, `»`. A token made only of
/// punctuation stays whole.
fn split_punct(tok: &str) -> Vec<&str> {
    split_punct_with(tok, false)
}

fn split_punct_with(tok: &str, clause_lead: bool) -> Vec<&str> {
    let lead_mark = |c: char| is_leading_punct(c) || (clause_lead && is_leading_clause_mark(c));
    // Format characters before or between the marks ride with them:
    // "\u{200e}(two)" cuts "\u{200e}(" like "(", "'\u{200e}'five" cuts
    // "'\u{200e}'" (#1125, #1129, #1138, #1140). The lead ends at its last
    // mark.
    let lead_end = tok
        .char_indices()
        .take_while(|&(_, c)| lead_mark(c) || is_format(c))
        .filter(|&(_, c)| lead_mark(c))
        .last()
        .map_or(0, |(k, c)| k + c.len_utf8());
    let lead_end = if lead_end == 0 { tok.char_indices().find(|&(_, c)| !lead_mark(c)).map_or(tok.len(), |(k, _)| k) } else { lead_end };
    if lead_end == tok.len() {
        return vec![tok];
    }
    let (lead, rest) = tok.split_at(lead_end);
    let mut trail_start = rest.len();
    // Format characters after a mark ride with it: "twenty)\u{200b}" is cut
    // like "twenty)" (#1004). The cut is at the first mark, so one before it
    // stays on the word ("twenty\u{200b})" cuts ")", #1011), and a run of them
    // alone is no mark at all.
    for (k, c) in rest.char_indices().rev() {
        if is_trailing_punct(c) {
            trail_start = k;
        } else if !is_format(c) {
            break;
        }
    }
    let (core, trail) = rest.split_at(trail_start);
    let mut out = Vec::with_capacity(3);
    if !lead.is_empty() {
        out.push(lead);
    }
    if !trail.is_empty() && trail.chars().any(|c| !is_legacy_trailing(c) && !is_format(c)) && !core.is_empty() {
        out.push(core);
        out.push(trail);
    } else {
        out.push(rest);
    }
    out
}

/// Does `part` hold clause punctuation inside it ("twenty,two", "zwanzig!drei",
/// "_twenty_")? Two words written together, never one number (#283).
/// Apostrophes, hyphens, dashes, dots and the Catalan middle dot belong to
/// words.
pub(crate) fn has_interior_punct(part: &str) -> bool {
    let edge = |c: char| is_leading_punct(c) || is_trailing_punct(c);
    let core = part.trim_matches(edge);
    core.chars().any(|c| {
        (c == '_' || is_leading_punct(c) || is_trailing_punct(c))
            && !matches!(c, '\'' | '\u{2019}' | '\u{2018}' | '\u{02BC}' | '\u{02BB}' | '\u{FF07}' | '`' | '-' | '\u{2010}'..='\u{2015}' | '.' | '\u{00B7}')
    })
}

/// Is `part` punctuation split off a word (or a token made only of
/// punctuation)? It never belongs to a number run.
pub(super) fn is_punct_part(part: &str) -> bool {
    !part.is_empty()
        && part.chars().any(|c| !is_format(c))
        && part.chars().all(|c| is_leading_punct(c) || is_trailing_punct(c) || is_leading_clause_mark(c) || is_format(c))
}

/// An invisible format character (ZWSP, ZWNJ, ZWJ, word joiner, BOM, soft
/// hyphen); see `super::is_format_char`.
fn is_format(c: char) -> bool {
    super::is_format_char(c)
}

/// [`super::tokenize`], then each word cut at its punctuation, and for the
/// languages that glue a number to a particle, at that particle.
pub(super) fn split_parts(
    sentence: &str,
    converter: &super::Converter,
    resolved: &str,
) -> (Vec<String>, Vec<Glue>) {
    let mut parts: Vec<String> = Vec::new();
    let mut glue = Vec::new();
    // Was the previous word a number word? A suffixed homograph ("yüzde",
    // percent) is only peeled inside a number.
    let mut prev_number = false;
    for tok in super::tokenize(sentence) {
        if super::py_str_isspace(&tok) {
            parts.push(tok);
            glue.push(Glue::Free);
            continue;
        }
        let clause_lead = !matches!(base(resolved), "zh" | "cn" | "ja" | "jp" | "th");
        for piece in split_punct_with(&tok, clause_lead) {
            if is_punct_part(piece) {
                parts.push(piece.to_string());
                glue.push(Glue::Free);
                continue;
            }
            let pieces = match segment(converter, resolved, piece) {
                Some(segs) => segs,
                None => peel(converter, resolved, piece, prev_number),
            };
            prev_number = peels(resolved)
                && pieces.iter().any(|(p, g)| *g == Glue::Free && number_like(converter, resolved, p));
            for (p, g) in pieces {
                parts.push(p.to_string());
                glue.push(g);
            }
        }
    }
    (parts, glue)
}

fn is_number(converter: &super::Converter, word: &str) -> bool {
    let w = super::rstrip_punct(word).to_lowercase();
    !w.is_empty() && !super::holds_digit(&w) && converter.is_number_word(&w)
}

/// A number word, or (ar / he) one with the conjunction glued on: "وتسعمائة"
/// (and nine hundred) continues a number.
fn number_like(converter: &super::Converter, resolved: &str, word: &str) -> bool {
    if is_number(converter, word) {
        return true;
    }
    // A bidi mark or other format character before the conjunction
    // ("\u{200e}وخمسة") is no part of the word (#1116).
    let word = word.trim_start_matches(super::is_format_char);
    let rest = match base(resolved) {
        "ar" => word.strip_prefix('و'),
        "he" => word.strip_prefix('ו'),
        _ => None,
    };
    rest.is_some_and(|r| is_number(converter, r))
}

/// The languages whose words may carry a proclitic or a case ending.
fn peels(resolved: &str) -> bool {
    matches!(base(resolved), "ar" | "he" | "fr" | "it" | "ca" | "tr" | "hu" | "bn" | "fi")
}

fn base(resolved: &str) -> &str {
    resolved.split(&['_', '-'][..]).next().unwrap_or(resolved)
}

// ---------------------------------------------------------------------------
// Scripts written without spaces (#177) and Korean glued counters (#73)
// ---------------------------------------------------------------------------

/// zh / ja numeral characters: digits, scales, the zero forms and 两 (two,
/// before a measure word).
fn is_cjk_numeral(c: char) -> bool {
    matches!(
        c,
        '零' | '〇' | '一' | '二' | '三' | '四' | '五' | '六' | '七' | '八' | '九' | '十' | '百'
            | '千' | '万' | '萬' | '亿' | '億' | '兆' | '两' | '兩'
    )
}

/// Measure words and units that make a lone numeral character a count: 三个,
/// 三個, 十二月, 五元, 三人, 三つ. Compared on the first characters of what
/// follows the numeral.
const CJK_COUNTERS: &[&str] = &[
    "个", "個", "只", "隻", "本", "张", "張", "件", "位", "名", "人", "岁", "歲", "歳", "才", "年",
    "月", "日", "号", "號", "点", "點", "時", "时", "分", "秒", "元", "块", "塊", "角", "円", "圆",
    "米", "公里", "公斤", "斤", "克", "升", "次", "回", "倍", "天", "小时", "小時", "周", "週", "层",
    "層", "楼", "樓", "階", "条", "條", "头", "頭", "双", "雙", "种", "種", "家", "间", "間", "部",
    "支", "辆", "輛", "台", "臺", "杯", "瓶", "碗", "份", "页", "頁", "章", "节", "節", "度", "%",
    "％", "枚", "匹", "冊", "册", "番", "つ", "ヶ", "ケ", "か月", "カ月", "キロ", "メートル",
    "グラム", "ドル", "ユーロ", "美元", "欧元", "歩", "着", "軒", "足", "粒", "通", "曲", "話", "行",
    "課", "课",
];

/// Numeral strings that are ordinary words: 十分 (very, enough), 千万 (by all means),
/// 万一 (in case), 一一 (one by one); and 一 before 个 / 個 / つ / 天 / 点, the
/// indefinite "a" (一个问题, "a question"; 一点, "a bit"), like the
/// article-one of #83. They are read only before another counter (千万元).
fn cjk_idiom(seg: &str, after: &str) -> bool {
    match seg {
        "十分" | "千万" | "千萬" | "万一" | "萬一" | "一一" | "万万" | "萬萬" => {
            !CJK_COUNTERS.iter().any(|c| after.starts_with(c))
        }
        "一" => ["个", "個", "つ", "天", "点", "點", "下"].iter().any(|c| after.starts_with(c)),
        // 十分 is "very" (zh) / "enough" (ja); 十分钟 is ten minutes.
        "十" => after.starts_with('分') && !["分钟", "分鐘"].iter().any(|c| after.starts_with(c)),
        _ => false,
    }
}

/// Thai numeral morphemes, as written: the digits, ten, the "ยี่" of twenty,
/// the "เอ็ด" of x1, and the scales.
const TH_MORPHEMES: &[&str] = &[
    "ศูนย์", "หนึ่ง", "สอง", "สาม", "สี่", "ห้า", "หก", "เจ็ด", "แปด", "เก้า", "สิบ", "ยี่", "เอ็ด",
    "ร้อย", "พัน", "หมื่น", "แสน", "ล้าน",
];

/// Thai classifiers and units after a single numeral morpheme: สามบาท,
/// ห้าคน.
const TH_CLASSIFIERS: &[&str] = &[
    "บาท", "คน", "ตัว", "อัน", "ชิ้น", "ปี", "เดือน", "วัน", "ชั่วโมง", "นาที", "วินาที", "ครั้ง",
    "กิโล", "เมตร", "ขวด", "แก้ว", "ใบ", "เล่ม", "คัน", "หลัง", "ข้อ", "เท่า", "โมง", "ทุ่ม", "ห้อง",
    "ชั้น", "จาน", "ถ้วย", "ดอลลาร์", "เปอร์เซ็นต์", "สัปดาห์", "ลูก", "แผ่น", "ที่",
];

/// A Thai mark or vowel that belongs to the syllable before it: a numeral
/// morpheme followed by one is part of a longer word (สาม + ารถ is สามารถ,
/// "can"; สาม + ี is สามี, "husband").
fn th_attached(c: char) -> bool {
    matches!(c as u32, 0x0E30..=0x0E3A | 0x0E45 | 0x0E47..=0x0E4E)
}

/// The Thai leading vowels: a consonant after one belongs to its syllable.
fn th_leading_vowel(c: char) -> bool {
    matches!(c as u32, 0x0E40..=0x0E44)
}

/// Korean Sino numeral syllables, glued to a counter in the same word:
/// 삼만원이에요 (it is 30000 won).
fn is_ko_sino(c: char) -> bool {
    matches!(
        c,
        '영' | '공' | '일' | '이' | '삼' | '사' | '오' | '육' | '칠' | '팔' | '구' | '십' | '백'
            | '천' | '만' | '억' | '조'
    )
}

/// Counters a glued Sino number may carry. A one-syllable number only before
/// the unambiguous ones: 구원 is "salvation", 사명 "mission", 일부 "part".
const KO_COUNTERS: &[&str] = &[
    "원", "개월", "개", "명", "년", "월", "일", "시간", "시", "분", "초", "번", "층", "세", "달러",
    "퍼센트", "킬로", "미터", "권", "대", "회", "호", "장", "마리", "병", "잔", "주", "배", "등",
    "점", "인분", "그램", "리터", "센티", "유로", "엔",
];
const KO_COUNTERS_AFTER_ONE_SYLLABLE: &[&str] = &["월", "년", "개", "층", "달러", "퍼센트", "유로"];

/// Cut the numeral substrings out of a zh / ja / th word, or the glued Sino
/// number off a ko word. `None` for other languages, or when nothing is cut.
fn segment<'a>(
    converter: &super::Converter,
    resolved: &str,
    piece: &'a str,
) -> Option<Vec<(&'a str, Glue)>> {
    let lang = base(resolved);
    let parses = |seg: &str| converter.is_number_word(seg);
    match lang {
        "zh" | "cn" | "ja" | "jp" => {
            let chars: Vec<(usize, char)> = piece.char_indices().collect();
            let mut cuts: Vec<(usize, usize)> = Vec::new();
            let mut k = 0;
            while k < chars.len() {
                if !is_cjk_numeral(chars[k].1) {
                    k += 1;
                    continue;
                }
                let start = k;
                while k < chars.len() && is_cjk_numeral(chars[k].1) {
                    k += 1;
                }
                let (b0, b1) = (chars[start].0, chars.get(k).map_or(piece.len(), |c| c.0));
                if b0 == 0 && b1 == piece.len() {
                    return None; // the whole word: the walker reads it as is
                }
                let seg = &piece[b0..b1];
                let after = &piece[b1..];
                let counted = CJK_COUNTERS.iter().any(|c| after.starts_with(c));
                let ordinal = piece[..b0].ends_with('第');
                let ok = !cjk_idiom(seg, after)
                    && (if k - start >= 2 { true } else { counted || ordinal })
                    && parses(seg);
                if ok {
                    cuts.push((b0, b1));
                }
            }
            cut_into(piece, &cuts)
        }
        "th" => {
            let mut cuts: Vec<(usize, usize)> = Vec::new();
            let mut p = 0;
            while p < piece.len() {
                let mut q = p;
                let mut morphemes = 0;
                while let Some(m) = TH_MORPHEMES
                    .iter()
                    .filter(|m| piece[q..].starts_with(*m))
                    .max_by_key(|m| m.len())
                {
                    q += m.len();
                    morphemes += 1;
                }
                if morphemes == 0 {
                    p += piece[p..].chars().next().map_or(1, char::len_utf8);
                    continue;
                }
                let seg = &piece[p..q];
                let after = &piece[q..];
                let glued_before = piece[..p].chars().last().is_some_and(th_leading_vowel);
                let glued_after = after.chars().next().is_some_and(th_attached);
                let counted = TH_CLASSIFIERS.iter().any(|c| after.starts_with(c));
                let whole = p == 0 && q == piece.len();
                let ok = !whole
                    && !glued_before
                    && !glued_after
                    && (morphemes >= 2 || counted)
                    && parses(seg);
                if ok {
                    cuts.push((p, q));
                }
                p = q;
            }
            cut_into(piece, &cuts)
        }
        // 삼만원이에요: the number, then the counter and the particles, cut
        // as a suffix so it is re-emitted unchanged (#73).
        "ko" => {
            let chars: Vec<(usize, char)> = piece.char_indices().collect();
            let n = chars.iter().take_while(|(_, c)| is_ko_sino(*c)).count();
            if n == 0 || n == chars.len() {
                return None;
            }
            let b = chars[n].0;
            let (num, rest) = piece.split_at(b);
            let counters = if n >= 2 { KO_COUNTERS } else { KO_COUNTERS_AFTER_ONE_SYLLABLE };
            // 이 alone is "this" (이번, 이분, 이년): never a glued number.
            let ok = !(n == 1 && num == "이")
                && counters.iter().any(|c| rest.starts_with(c))
                && parses(num);
            ok.then(|| vec![(num, Glue::Free), (rest, Glue::Suffix(""))])
        }
        _ => None,
    }
}

/// `piece` cut at the byte ranges `cuts` (sorted, disjoint), every part free.
fn cut_into<'a>(piece: &'a str, cuts: &[(usize, usize)]) -> Option<Vec<(&'a str, Glue)>> {
    if cuts.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    let mut at = 0;
    for &(b0, b1) in cuts {
        if b0 > at {
            out.push((&piece[at..b0], Glue::Free));
        }
        out.push((&piece[b0..b1], Glue::Free));
        at = b1;
    }
    if at < piece.len() {
        out.push((&piece[at..], Glue::Free));
    }
    Some(out)
}

/// Cut a proclitic or a case ending off `piece` when what remains is a number
/// word; otherwise the piece whole.
fn peel<'a>(
    converter: &super::Converter,
    resolved: &str,
    piece: &'a str,
    prev_number: bool,
) -> Vec<(&'a str, Glue)> {
    let whole = vec![(piece, Glue::Free)];
    let lang = base(resolved);
    if !peels(resolved) {
        return whole;
    }
    let core = super::rstrip_punct(piece);
    if core.is_empty() || super::holds_digit(core) || is_number(converter, core) {
        return whole;
    }
    // Inside a number the glued conjunction belongs to it (ar "ثلاثة
    // وعشرين", he "תשעים ותשע"), so a proclitic is only cut at a run head.
    let in_number_conjunction = prev_number && matches!(lang, "ar" | "he");
    if let Some((k, sep)) = prefix_cut(converter, lang, core).filter(|_| !in_number_conjunction) {
        return vec![(&piece[..k], Glue::Prefix(sep)), (&piece[k..], Glue::Free)];
    }
    if let Some((k, sep)) = suffix_cut(converter, lang, core, prev_number) {
        // Trailing legacy punctuation stays on the suffix: "üçte." -> "üç", "te.".
        return vec![(&piece[..k], Glue::Free), (&piece[k..], Glue::Suffix(sep))];
    }
    whole
}

/// The byte length of a proclitic of `core` and the separator to write after
/// it, when the rest is a number word (#76, #184).
fn prefix_cut(converter: &super::Converter, lang: &str, core: &str) -> Option<(usize, &'static str)> {
    let rest_is_number = |k: usize| {
        let rest = &core[k..];
        let w = rest.to_lowercase();
        // A lone article-number after an elision ("l'un d'entre eux") is cut
        // too: the walker's article rule keeps it in words.
        is_number(converter, rest)
            || converter.apocope(&w).is_some()
            || super::is_article_word(lang, &w)
    };
    match lang {
        // fr "d'onze", "l'un"; it "dell'otto", "dall'undici", "l'otto"; ca "d'onze".
        "fr" | "it" | "ca" => {
            let (k, c) = core.char_indices().find(|&(_, c)| c == '\'' || c == '’')?;
            let head = &core[..k];
            let k = k + c.len_utf8();
            let elided = head.chars().count() <= 6
                && !head.is_empty()
                && head.chars().all(char::is_alphabetic)
                && k < core.len();
            (elided && rest_is_number(k)).then_some((k, ""))
        }
        // One-letter prepositions and the conjunction glued to the word: ar ب ل
        // ك و ف, he ב ל מ כ ש ו (and two of them: "וב", "שב"). Not the
        // article (ar "ال", he "ה"): "המאה" is "the century".
        "ar" | "he" => {
            let letters: &[char] = if lang == "ar" {
                &['ب', 'ل', 'ك', 'و', 'ف']
            } else {
                &['ב', 'ל', 'מ', 'כ', 'ש', 'ו']
            };
            let sep = if lang == "he" { "-" } else { "" };
            let chars: Vec<(usize, char)> = core.char_indices().collect();
            for n in 1..=2usize {
                if chars.len() <= n + 1 || !chars[..n].iter().all(|(_, c)| letters.contains(c)) {
                    break;
                }
                let k = chars[n].0;
                let rest = crate::normalize(&core[k..]);
                // ar "لست" is "I am not", not "ل" + "ست" (six).
                if lang == "ar" && matches!(rest.as_str(), "ست" | "سبع") {
                    continue;
                }
                if is_number(converter, &core[k..]) {
                    return Some((k, sep));
                }
            }
            None
        }
        _ => None,
    }
}

/// Case endings and classifiers, longest first within each language.
fn suffixes(lang: &str) -> &'static [&'static str] {
    match lang {
        // Locative, ablative, dative, accusative, genitive, instrumental, with
        // the buffer letters.
        "tr" => &[
            "nden", "ndan", "den", "dan", "ten", "tan", "nin", "nın", "nun", "nün", "yle", "yla",
            "de", "da", "te", "ta", "ye", "ya", "yi", "yı", "yu", "yü", "in", "ın", "un", "ün",
            "le", "la", "e", "a", "i", "ı", "u", "ü",
        ],
        "hu" => &[
            "szor", "szer", "ször", "kor", "ban", "ben", "ból", "ből", "nál", "nél", "tól", "től",
            "hoz", "hez", "höz", "ról", "ről", "ra", "re", "at", "et", "öt", "ot", "an", "en",
            "ön", "on", "t",
        ],
        "bn" => &["খানা", "খানি", "গুলো", "গুলি", "জন", "টা", "টি", "টো"],
        "fi" => &[
            "ssa", "ssä", "sta", "stä", "lla", "llä", "lta", "ltä", "lle", "ksi", "tta", "ttä",
            "een", "na", "nä", "en", "n", "a", "ä",
        ],
        _ => &[],
    }
}

/// The separator a language writes between figures and a case ending.
fn suffix_sep(lang: &str) -> &'static str {
    match lang {
        "tr" => "'",
        "hu" => "-",
        "fi" => ":",
        _ => "",
    }
}

/// Number words whose suffixed forms are ordinary words, peeled only inside a
/// number: tr "on" ("ona", "onda": to him, in him), "altı" ("altında":
/// under), "yüz" ("yüzde": percent), "bir" ("biri": someone); hu "egy"
/// ("egykor": formerly), "hét" ("héten": in the week, but "hétkor" is at
/// seven); fi "sata" ("sataa": it rains). Normalised.
fn lone_suffix_excluded(lang: &str, stem: &str, suffix: &str) -> bool {
    match (lang, stem) {
        // `normalize` folds the dotless ı to "i" (#116).
        ("tr", "on" | "alti" | "yuz" | "bir") => true,
        ("hu", "egy") => true,
        ("hu", "het") => suffix != "kor",
        ("fi", "sata") => true,
        _ => false,
    }
}

/// The byte offset where a case ending or classifier starts in `core`, when
/// the stem before it is a number word (#73, #74 fi).
fn suffix_cut(
    converter: &super::Converter,
    lang: &str,
    core: &str,
    prev_number: bool,
) -> Option<(usize, &'static str)> {
    for suf in suffixes(lang) {
        let Some(stem) = core.strip_suffix(suf).or_else(|| {
            // Case-insensitive ending ("ÜÇTE" is rare; the stem keeps its case).
            let lower = core.to_lowercase();
            lower.ends_with(suf).then(|| &core[..core.len() - suf.len()]).filter(|_| core.len() == lower.len())
        }) else {
            continue;
        };
        if stem.chars().count() < 2 {
            continue;
        }
        let norm = crate::normalize(stem);
        // The oblique stems (hu "hárm-", fi "viide-") are read through the
        // aliases ([`super::lexicon`]).
        if !is_number(converter, stem) {
            continue;
        }
        if !prev_number && lone_suffix_excluded(lang, &norm, suf) {
            continue;
        }
        return Some((stem.len(), suffix_sep(lang)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punctuation_is_cut_off_and_kept() {
        assert_eq!(split_punct("(forty"), vec!["(", "forty"]);
        assert_eq!(split_punct("two)"), vec!["two", ")"]);
        assert_eq!(split_punct("two)."), vec!["two", ")."]);
        assert_eq!(split_punct("forty…"), vec!["forty", "…"]);
        assert_eq!(split_punct("«vingt-deux»"), vec!["«", "vingt-deux", "»"]);
        assert_eq!(split_punct("¿veinte?"), vec!["¿", "veinte?"]);
        // The legacy trailing class stays on the word.
        assert_eq!(split_punct("forty,"), vec!["forty,"]);
        assert_eq!(split_punct("\"forty\""), vec!["\"", "forty\""]);
        assert_eq!(split_punct("o'clock"), vec!["o'clock"]);
        assert_eq!(split_punct("…"), vec!["…"]);
        for s in ["“forty two”", "(a) b", "x « y »"] {
            let (parts, _) = split_parts(s, &super::super::converter_for("en"), "en");
            assert_eq!(parts.concat(), s);
        }
    }
}
