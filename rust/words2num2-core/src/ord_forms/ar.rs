//! Arabic. num2words2 writes the definite masculine ordinal (الأول, الثالث,
//! الحادي عشر). Speech also uses the feminine in -ة and الأولى (#152), the
//! indefinite form without ال (ثالث, أول) (#152), and الحادي — not the
//! renderer's الأول — for the unit 1 of every compound (الحادي والعشرون,
//! الحادي بعد المائة) (#249). From 1000 the renderer returns the cardinal
//! (ألف), so the definite الألف is read through the bare form (#245).
//!
//! Every literal here is in normalized form: hamza and harakat are stripped
//! (أول -> اول), ة and ى are letters and stay.

fn masculine(tok: &str) -> String {
    match tok {
        "الاولى" => "الاول".to_string(),
        "اولى" => "اول".to_string(),
        // الثالثة -> الثالث, الحادية -> الحادي, عشرة -> عشر; but مائة (100)
        // is the word itself, not a feminine.
        t if t.ends_with("ماية") || t.ends_with("مائة") => t.to_string(),
        t => match t.strip_suffix('ة') {
            Some(stem) if stem.chars().count() >= 2 => stem.to_string(),
            _ => t.to_string(),
        },
    }
}

/// The definite form: ال on the first word, و + ال on a coordinated one
/// (ثالث وعشرون -> الثالث والعشرون).
fn definite(toks: &[String]) -> Vec<String> {
    toks.iter()
        .enumerate()
        .map(|(i, t)| {
            if t.starts_with("ال") || t.starts_with("وال") {
                t.clone()
            } else if i == 0 {
                format!("ال{t}")
            } else if let Some(rest) = t.strip_prefix('و').filter(|r| r.chars().count() >= 2) {
                format!("وال{rest}")
            } else {
                t.clone()
            }
        })
        .collect()
}

/// الحادي as the unit of a compound (anything but الحادي عشر) is the
/// renderer's الأول.
fn hadi(toks: &[String]) -> Vec<String> {
    let mut out = toks.to_vec();
    if out.len() >= 2 && out[0] == "الحادي" && !matches!(out[1].as_str(), "عشر" | "عشرة")
    {
        out[0] = "الاول".to_string();
    }
    out
}

pub fn variants(norm: &str) -> Vec<String> {
    let toks: Vec<String> = norm.split(' ').map(masculine).collect();
    let def = definite(&toks);
    let mut out = vec![
        toks.join(" "),
        def.join(" "),
        hadi(&toks).join(" "),
        hadi(&def).join(" "),
    ];
    // الالف -> الف: the renderer's cardinal-shaped ordinals from 1000 on.
    if let Some(rest) = norm.strip_prefix("ال") {
        out.push(rest.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn literals_are_normalized() {
        for w in ["الاولى", "اولى", "الاول", "الحادي", "عشر", "عشرة"] {
            assert_eq!(crate::normalize(w), w);
        }
    }

    #[test]
    fn arabic_feminine_and_indefinite_ordinals_issue_152() {
        for (input, want) in [
            ("الأولى", 1),
            ("الثانية", 2),
            ("الثالثة", 3),
            ("الخامسة", 5),
            ("أول", 1),
            ("ثاني", 2),
            ("ثالث", 3),
            ("الحادية عشرة", 11),
        ] {
            assert_eq!(
                crate::lookup("ar", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }

    #[test]
    fn arabic_hadi_in_compounds_issue_249() {
        for (input, want) in [
            ("الحادي والعشرون", 21),
            ("الحادية والعشرون", 21),
            ("الحادي بعد المائة", 101),
            ("الأول والعشرون", 21),
            ("الحادي عشر", 11),
        ] {
            assert_eq!(
                crate::lookup("ar", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }

    #[test]
    fn arabic_definite_thousand_issue_245() {
        assert_eq!(crate::lookup("ar", "الألف", true, &[]), Ok(Some(1000)));
    }
}
