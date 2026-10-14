//! num2words2 -> words2num2 round-trip, every advertised language
//! (gladiaio/words2num2#19).
//!
//! Renders with the same `num2words2-core` the reverse tables are built from
//! and parses back through `words2num`, the entry point the Python API calls.
//! No external dependency, so it runs wherever `cargo test` does — including
//! the moment a `num2words2-core` bump changes a language's rendering.
//!
//! Two ratchets, both strict: a language/value pair that starts working must
//! be deleted from its list (the test fails until it is), and one that starts
//! failing fails the test. The lists can only shrink.
//!
//! * `KNOWN_PARSE_FAILURES` — `words2num` raises. Loud, recoverable.
//! * `KNOWN_WRONG_VALUES` — `words2num` returns a *different number*. Silent,
//!   and the one that matters for a transcription pipeline.

use std::collections::BTreeMap;

use num_bigint::BigInt;
use words2num2_core::w2n_lang_en::W2nValue;

/// Inside the reverse table (<= 10000), then composed by `parse_scaled`.
const SAMPLE: &[i64] = &[
    0, 1, 7, 11, 15, 21, 42, 99, 100, 101, 999, 1000, 1001, 1234, 10000, 12345, 100000, 1000000,
    1234567,
];

#[rustfmt::skip]
const KNOWN_PARSE_FAILURES: &[(&str, &[i64])] = &[
    ("bm", &[12345,100000,1234567]),
    ("hmn", &[1000000,1234567]),
    ("sn", &[1234567]),
];

/// ha spells 10000 and 100000 exactly like 1010 ("dubu goma") and 1100 ("dubu
/// ɗari"); the table holds the smaller one, so no parser can tell them apart.
/// A renderer fix in num2words2, see `KNOWN_TABLE_COLLISIONS`.
#[rustfmt::skip]
const KNOWN_WRONG_VALUES: &[(&str, &[i64])] = &[
    ("ha", &[10000,100000]),
];

/// Values in 0..=10000 the renderer spells exactly like a smaller number, so
/// first-write-wins in the reverse table can only return the smaller one:
/// ha 10000 / 1010 ("dubu goma"). Counts, per language.
const KNOWN_TABLE_COLLISIONS: &[(&str, usize)] = &[("ha", 1)];

#[derive(Default, Debug, PartialEq)]
struct Outcome {
    fails: Vec<i64>,
    wrong: Vec<i64>,
}

fn roundtrip(lang: &str, values: impl IntoIterator<Item = i64>) -> (Outcome, Vec<String>) {
    let renderer = num2words2_core::get_lang_by_key(lang).expect("advertised language renders");
    let mut out = Outcome::default();
    let mut detail = Vec::new();
    for v in values {
        let Ok(words) = renderer.to_cardinal(&BigInt::from(v)) else {
            continue;
        };
        match words2num2_core::words2num(&words, lang, "cardinal") {
            Ok(W2nValue::Int(n)) if n == BigInt::from(v) => {}
            Ok(got) => {
                out.wrong.push(v);
                detail.push(format!("{v} -> {words:?} -> {got:?}"));
            }
            Err(_) => out.fails.push(v),
        }
    }
    (out, detail)
}

fn known(list: &[(&str, &[i64])]) -> BTreeMap<String, Vec<i64>> {
    list.iter()
        .map(|(l, v)| (l.to_string(), v.to_vec()))
        .collect()
}

#[test]
fn every_language_round_trips_except_the_known_gaps() {
    let fails = known(KNOWN_PARSE_FAILURES);
    let wrong = known(KNOWN_WRONG_VALUES);
    let mut langs = words2num2_core::supported_langs();
    langs.sort();

    let mut problems = Vec::new();
    for lang in &langs {
        let (got, detail) = roundtrip(lang, SAMPLE.iter().copied());
        let expected = Outcome {
            fails: fails.get(*lang).cloned().unwrap_or_default(),
            wrong: wrong.get(*lang).cloned().unwrap_or_default(),
        };
        if got != expected {
            problems.push(format!(
                "{lang}: raises {:?} (known {:?}), wrong {:?} (known {:?}) {detail:?}",
                got.fails, expected.fails, got.wrong, expected.wrong
            ));
        }
    }
    for lang in fails.keys().chain(wrong.keys()) {
        if !langs.contains(&lang.as_str()) {
            problems.push(format!(
                "{lang}: listed as a known gap but no longer advertised"
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "round-trip ratchet moved. Fixed? Delete the entry. Regressed? Fix it.\n{}",
        problems.join("\n")
    );
}

/// Every value in the reverse-table range, every language: nothing may parse
/// to a different number. ~1.7M renders, so it is opt-in:
/// `cargo test --release -p words2num2-core -- --ignored`.
#[test]
#[ignore]
fn table_range_never_parses_to_another_number() {
    let allowed: BTreeMap<&str, usize> = KNOWN_TABLE_COLLISIONS.iter().copied().collect();
    let mut problems = Vec::new();
    for lang in words2num2_core::supported_langs() {
        let (got, detail) = roundtrip(lang, 0..=10000);
        let want = allowed.get(lang).copied().unwrap_or(0);
        if got.wrong.len() != want {
            let shown: Vec<_> = detail.iter().take(3).collect();
            problems.push(format!(
                "{lang}: {} wrong (known {want}) {shown:?}",
                got.wrong.len()
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
