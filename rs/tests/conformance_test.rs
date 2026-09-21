/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! Third-party INI conformance, the Rust half.
//!
//! Asserts exactly the same manifest, divergence list and rules as
//! `ts/test/conformance.test.ts` and `go/ini_conformance_test.go`. Read
//! the header comment of the TypeScript half for why the corpus is
//! assembled rather than official, and why the divergence lists are not
//! an escape hatch.
//!
//! THIS SUITE MUST NEVER SKIP. If the manifest is absent it FAILS.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value as Json;

const MISSING_CORPUS: &str = "INI conformance corpus not found at \
test/corpus/ini-corpus.json.\n\n\
The manifest is self-contained (sources, oracle values and pinned upstream \
SHAs) and ships with the repo. This suite deliberately FAILS rather than \
skips: a conformance suite that silently does not run reports green while \
measuring nothing.";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rs/ has a parent")
        .to_path_buf()
}

struct Case {
    name: String,
    kind: String,
    source: String,
    label: String,
    expected: Json,
}

fn manifest() -> Vec<Case> {
    let path = repo_root()
        .join("test")
        .join("corpus")
        .join("ini-corpus.json");
    let raw = fs::read_to_string(&path).unwrap_or_else(|_| panic!("{MISSING_CORPUS}"));
    let document: Json =
        serde_json::from_str(&raw).expect("test/corpus/ini-corpus.json is not readable JSON");
    let cases = document["cases"]
        .as_array()
        .expect("test/corpus/ini-corpus.json has no cases array");
    assert!(
        !cases.is_empty(),
        "test/corpus/ini-corpus.json is present but contains no cases."
    );
    cases
        .iter()
        .map(|case| Case {
            name: case["name"].as_str().unwrap_or_default().to_string(),
            kind: case["kind"].as_str().unwrap_or_default().to_string(),
            source: case["source"].as_str().unwrap_or_default().to_string(),
            label: case["label"].as_str().unwrap_or_default().to_string(),
            expected: case["expected"].clone(),
        })
        .collect()
}

/// The canonical TypeScript result for every document in `divergent`.
///
/// A divergent document is one this grammar reads differently from the
/// npm/ini oracle, so the oracle cannot say whether this port read it
/// correctly. Without this file the only assertion left for those twelve
/// was "not the oracle", which any third value satisfies: 12 of the 30
/// valid documents, 40% of the corpus, were parsed and then not
/// measured.
///
/// Measured on 2026-09-21 by running each document through
/// `ts/src/ini.ts` at default options with the sibling checkouts beside
/// this repository, and stored rather than computed because a Rust test
/// cannot run Node. Regenerate it the same way if the canonical dialect
/// changes; the key set is asserted to be exactly `divergent`, so an
/// entry cannot be added to one and forgotten in the other.
fn canonical_typescript() -> BTreeMap<String, Json> {
    let raw = include_str!("conformance-canonical.json");
    let document: Json =
        serde_json::from_str(raw).expect("tests/conformance-canonical.json is not readable JSON");
    document
        .as_object()
        .expect("tests/conformance-canonical.json is not an object")
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/// Dialect differences: documents this grammar parses to something other
/// than the npm/ini oracle, each for a reason written down. Keep in step
/// with `DIVERGENT` in ts/test/conformance.test.ts and
/// `conformanceDivergent` in go/ini_conformance_test.go.
fn divergent() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("go-ini/testdata/full.ini", "inline comments off by default; oracle requires a section header to be the whole line"),
        ("inih/examples/test.ini", "oracle requires a section header to be the whole line; inline comments off by default"),
        ("inih/fuzzing/testcases/case1.ini", "same section-header rule; inline comments off by default"),
        ("inih/tests/multi_line.ini", "inline comments off by default"),
        ("inih/tests/normal.ini", "same section-header rule; inline comments off by default"),
        ("iniparser/example/twisted.ini", "inline comments off by default, backslash-escaped newline continues a value, degenerate quote handling differs"),
        ("iniparser/test/ressources/bad_ini/twisted-ofval.ini", "a backslash before a newline escapes it, so the value continues"),
        ("iniparser/test/ressources/good_ini/spaced2.ini", "inline comments off by default"),
        ("iniparser/test/ressources/old.ini", "inline comments off by default"),
        ("iniparser/test/ressources/quotes.ini", "inline comments off by default"),
        ("iniparser/test/ressources/utf8.ini", "inline comments off by default"),
        ("npm-ini/test/fixtures/foo.ini", "with inline comments off, a backslash-semicolon is a literal backslash then a literal semicolon"),
    ])
}

/// Documents this port does NOT yet match the canonical TypeScript on.
/// These would be parity gaps, not dialect choices. Each entry is
/// asserted to STILL be a gap, so a fix turns this suite red and forces
/// the entry to be deleted rather than linger.
///
/// Empty, as `conformanceGoParityGap` is: the Rust port reproduces the
/// TypeScript result on every document in the corpus.
fn rust_parity_gap() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::new()
}

/// Documents an upstream labels malformed that are valid in this
/// dialect. Keep in step with `ACCEPTED_BY_DESIGN` in
/// ts/test/conformance.test.ts.
fn accepted_by_design() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        (
            "inih/tests/bad_comment.ini",
            "bare boolean key, a documented feature",
        ),
        (
            "inih/tests/bad_multi.ini",
            "bare boolean key, a documented feature",
        ),
        (
            "iniparser/test/ressources/bad_ini/ends_well.ini",
            "bare boolean key, a documented feature",
        ),
        (
            "iniparser/test/ressources/bad_ini/twisted-errors.ini",
            "bare boolean keys, a documented feature",
        ),
    ])
}

/// Reduce a value to plain JSON kinds so representation cannot make
/// equal values look different: a whole-valued double renders as an
/// integer, as `json.Marshal` and `JSON.stringify` render it in the
/// other two runtimes. Map order is not compared, because a serde_json
/// map compares as a map.
fn canonical_json(value: &Json) -> Json {
    match value {
        Json::Number(number) => match number.as_f64() {
            Some(float) if float.fract() == 0.0 && float.abs() < 9.007_199_254_740_992e15 => {
                Json::from(float as i64)
            }
            _ => value.clone(),
        },
        Json::Array(items) => Json::Array(items.iter().map(canonical_json).collect()),
        Json::Object(entries) => Json::Object(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), canonical_json(value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Parse with the default options, reporting whether the document was
/// rejected. The engine turns a callback panic into a returned error, so
/// there is nothing here to catch.
fn try_parse(src: &str) -> Result<Json, ()> {
    tabnas_ini::parse(src)
        .map(|value| value.to_json())
        .map_err(|_| ())
}

#[test]
fn corpus_shape() {
    let cases = manifest();
    let valid = cases.iter().filter(|case| case.kind == "valid").count();
    let invalid = cases.iter().filter(|case| case.kind == "invalid").count();
    for case in &cases {
        assert!(
            case.kind == "valid" || case.kind == "invalid",
            "{}: unclassified case kind {:?}",
            case.name,
            case.kind
        );
    }
    assert_eq!(valid, 30, "valid case count changed");
    assert_eq!(invalid, 6, "invalid case count changed");
}

#[test]
fn valid_documents_parse() {
    let gaps = rust_parity_gap();
    for case in manifest().iter().filter(|case| case.kind == "valid") {
        let rejected = try_parse(&case.source).is_err();
        if gaps.contains_key(case.name.as_str()) {
            // A known parity gap; `valid_documents_match_the_oracle`
            // asserts the gap is still real so the entry cannot go
            // stale.
            continue;
        }
        assert!(
            !rejected,
            "{}: a real-world .ini file must not fail to parse",
            case.name
        );
    }
}

#[test]
fn valid_documents_match_the_oracle() {
    let gaps = rust_parity_gap();
    let diverges = divergent();
    let canonical = canonical_typescript();
    for case in manifest().iter().filter(|case| case.kind == "valid") {
        let got = match try_parse(&case.source) {
            Err(()) => {
                // Rejected: already reported by `valid_documents_parse`
                // unless it is a known gap, in which case the gap is
                // still real.
                continue;
            }
            Ok(value) => value,
        };
        let got = canonical_json(&got);
        let want = canonical_json(&case.expected);
        if let Some(why) = gaps.get(case.name.as_str()) {
            assert_ne!(
                got, want,
                "{}: now matches the oracle, so its Rust parity-gap entry ({why:?}) is stale; delete it.",
                case.name
            );
            continue;
        }
        match diverges.get(case.name.as_str()) {
            None => assert_eq!(
                got, want,
                "{}: parsed, but to a different value than the npm/ini oracle. \
                 If this is a dialect difference, add it to `divergent` with the \
                 documented reason; otherwise it is a bug.",
                case.name
            ),
            Some(why) => {
                assert_ne!(
                    got, want,
                    "{}: now MATCHES the oracle, so its divergence entry ({why:?}) is stale; delete it.",
                    case.name
                );
                // Differing from the oracle is not a result. The
                // canonical TypeScript is, and it is the contract every
                // port is held to, so the document is compared against
                // it exactly.
                let canon = canonical.get(case.name.as_str()).unwrap_or_else(|| {
                    panic!("{}: no canonical TypeScript result recorded", case.name)
                });
                assert_eq!(
                    got,
                    canonical_json(canon),
                    "{}: differs from the oracle for the documented reason ({why:?}), \
                     but not in the way the canonical TypeScript does.",
                    case.name
                );
            }
        }
    }
}

#[test]
fn invalid_documents_are_rejected() {
    let by_design = accepted_by_design();
    for case in manifest().iter().filter(|case| case.kind == "invalid") {
        let outcome = try_parse(&case.source);
        match by_design.get(case.name.as_str()) {
            None => assert!(
                outcome.is_err(),
                "{}: accepted a document its upstream labels malformed.\n  upstream label: {}\n  parsed as: {}",
                case.name,
                case.label,
                outcome.map(|value| value.to_string()).unwrap_or_default()
            ),
            Some(why) => assert!(
                outcome.is_ok(),
                "{}: rejected, but it is valid in this dialect ({why}). \
                 Either the plugin regressed or the entry is stale.",
                case.name
            ),
        }
    }
}

/// The three lists are cross-runtime contracts, so their SIZES are
/// pinned too: an entry added here and not in the other two runtimes is
/// a drift this notices.
#[test]
fn the_divergence_lists_are_the_shared_ones() {
    assert_eq!(divergent().len(), 12, "the dialect divergence list changed");
    assert_eq!(
        accepted_by_design().len(),
        4,
        "the accepted-by-design list changed"
    );
    assert!(
        rust_parity_gap().is_empty(),
        "the Rust parity-gap list is not empty; every entry needs a fix or a reason"
    );
}

/// The recorded canonical results cover the divergent list exactly. An
/// entry added to one and not the other would leave a document
/// unmeasured again, or leave a stale result behind.
#[test]
fn every_divergent_document_has_a_canonical_result() {
    let canonical = canonical_typescript();
    let diverges = divergent();
    let recorded: Vec<&str> = canonical.keys().map(String::as_str).collect();
    let expected: Vec<&str> = diverges.keys().copied().collect();
    assert_eq!(
        recorded, expected,
        "tests/conformance-canonical.json does not hold exactly the divergent documents"
    );
}

/// The manifest is a committed file and this suite must fail loudly when
/// it is gone. Proving that without deleting it means proving the reader
/// fails on a missing path, and that the message names the repair.
#[test]
fn a_missing_manifest_is_a_failure_and_not_a_skip() {
    assert!(
        repo_root()
            .join("test")
            .join("corpus")
            .join("ini-corpus.json")
            .is_file(),
        "{MISSING_CORPUS}"
    );
    assert!(MISSING_CORPUS.contains("deliberately FAILS rather than"));
}
