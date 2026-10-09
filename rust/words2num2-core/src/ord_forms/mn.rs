//! Mongolian. num2words2 writes the ordinal suffix as its own word ("нэг
//! дүгээр", "хоёр дугаар"); standard orthography glues it to the number
//! ("нэгдүгээр анги", "хорин нэгдүгээр") (#248).

use super::suffix::map_last;

pub fn variants(norm: &str) -> Vec<String> {
    map_last(norm, |t| {
        ["дүгээр", "дугаар"].iter().find_map(|suf| {
            t.strip_suffix(suf)
                .filter(|stem| !stem.is_empty())
                .map(|stem| format!("{stem} {suf}"))
        })
    })
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn mongolian_glued_ordinal_suffix_issue_248() {
        for (input, want) in [("нэгдүгээр", 1), ("хоёрдугаар", 2), ("хорин нэгдүгээр", 21)]
        {
            assert_eq!(
                crate::lookup("mn", input, true, &[]),
                Ok(Some(want)),
                "{input}"
            );
        }
    }
}
