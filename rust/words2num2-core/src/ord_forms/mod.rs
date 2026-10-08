//! Spoken ordinal forms the renderer does not write.
//!
//! num2words2 renders one ordinal per number — usually the masculine, the
//! nominative, the definite form — so the reverse table holds only that
//! spelling. Speech agrees the ordinal with its noun (hi "पहली बार"), adds a
//! case suffix (tr "ikinciye"), or uses an equally standard second form
//! (ja "第一", fa "دومین"). Each language file maps such an input to the
//! renderer's canonical spelling; the caller then probes the ordinal table
//! with it, so a variant can never invent a number the table does not hold.

mod ar;
mod bn;
mod fa;
mod gu;
mod hi;
mod hy;
mod ja;
mod ko;
mod mn;
mod mr;
mod ms;
mod ne;
mod pa;
mod suffix;
mod tr;
mod ur;
mod vi;

/// Candidate spellings, in the renderer's canonical ordinal form, for the
/// normalized ordinal `norm` of language `lang`. Empty when the language has
/// no such rule or the input carries none of its markers.
pub fn variants(lang: &str, norm: &str) -> Vec<String> {
    let base = lang.split(&['_', '-'][..]).next().unwrap_or(lang);
    let raw = match base {
        "ar" => ar::variants(norm),
        "bn" => bn::variants(norm),
        "fa" => fa::variants(norm),
        "gu" => gu::variants(norm),
        "hi" => hi::variants(norm),
        "hy" => hy::variants(norm),
        "ja" | "jp" => ja::variants(norm),
        "ko" => ko::variants(norm),
        "mn" => mn::variants(norm),
        "mr" => mr::variants(norm),
        "ms" => ms::variants(norm),
        "ne" => ne::variants(norm),
        "pa" => pa::variants(norm),
        "tr" => tr::variants(norm),
        "ur" => ur::variants(norm),
        "vi" => vi::variants(norm),
        _ => return Vec::new(),
    };
    // The table keys are normalized: so is every candidate, whatever a file
    // wrote, and the input itself is not a variant of itself.
    let mut out: Vec<String> = Vec::new();
    for v in raw {
        let v = crate::normalize(&v);
        if !v.is_empty() && v != norm && !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_language_has_no_variants() {
        assert!(variants("xx", "abc").is_empty());
        assert!(variants("en", "firsts").is_empty());
    }

    #[test]
    fn candidates_are_normalized_and_exclude_the_input() {
        for (lang, input) in [("ar", "الاولى"), ("hi", "पहली"), ("tr", "ikinciye")] {
            let norm = crate::normalize(input);
            for v in variants(lang, &norm) {
                assert_eq!(crate::normalize(&v), v, "{lang} {input}");
                assert_ne!(v, norm);
            }
        }
    }
}
