//! he: num2words2 renders the bare masculine for 1st-10th (ראשון, שני,
//! שלישי) and the definite ה- form above (העשרים). Speech adds the definite
//! article and the feminine / plural: הראשון, ראשונה, שנייה, שלישית,
//! ראשונים.

fn token(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    // The definite article on a 1st-10th ordinal: הראשון, השלישית.
    let bare = t.strip_prefix('ה').filter(|b| b.chars().count() >= 3);
    for w in std::iter::once(t).chain(bare) {
        out.push(w.to_string());
        match w {
            "ראשונה" | "ראשונים" | "ראשונות" => out.push("ראשון".to_string()),
            "שנייה" | "שניה" => out.push("שני".to_string()),
            _ => {}
        }
        // שלישית -> שלישי, רביעית -> רביעי; plural שלישיים -> שלישי. (שניות,
        // שניים are "seconds" and "two": not mapped.)
        if w != "שניות" && w != "שניים" {
            if let Some(s) = w.strip_suffix("ית").or_else(|| w.strip_suffix("יים")) {
                if s.chars().count() >= 2 {
                    out.push(format!("{s}י"));
                }
            }
        }
    }
    out
}

pub fn variants(norm: &str) -> Vec<String> {
    if norm.contains(' ') {
        return Vec::new(); // the compounds (11th and up) are rendered definite already
    }
    token(norm)
}

#[cfg(test)]
mod tests {
    use super::super::endings::ord;

    #[test]
    fn issue_151_inflected_ordinals_parse() {
        assert_eq!(ord("he", "ראשונה"), Some(1), "ראשונה");
        assert_eq!(ord("he", "שנייה"), Some(2), "שנייה");
        assert_eq!(ord("he", "שלישית"), Some(3), "שלישית");
        assert_eq!(ord("he", "רביעית"), Some(4), "רביעית");
        assert_eq!(ord("he", "הראשון"), Some(1), "הראשון");
        assert_eq!(ord("he", "ראשונים"), Some(1), "ראשונים");
        assert_eq!(ord("he", "שניים"), None, "שניים");
    }
}
