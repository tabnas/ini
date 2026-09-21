/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! Cross-runtime conformance, driven by the shared `test/spec/*.tsv`
//! fixtures at the repo root (see ../../test/AGENTS.md).
//!
//! The fixture loader, the escape codec, the `ERROR:<code>` contract and
//! the row loop all come from `tabnas-support`, whose TypeScript and Go
//! halves `ts/test/ini-tsv.test.ts` and `go/ini_tsv_test.go` use to run
//! the SAME files, so the three implementations cannot drift without one
//! of them going red, and neither can the three loaders.
//!
//! What is left here is only what is specific to ini: which options each
//! fixture is parsed with.

mod common;

use std::path::Path;

use common::{failure_of, options_for, value_of};
use tabnas_support::{find_spec_dir, load_spec_dir, report, Runner, SpecOptions};

/// One runner per fixture file, because the options are per file. The
/// directory listing is the source of truth for which fixtures exist:
/// `load_spec_dir` rejects an empty directory, and the runner rejects an
/// empty fixture, so neither can pass by running nothing.
///
/// `min_cols: 2` keeps a guard this repo already had: a line with no tab
/// is a failure, named by file and line, not a row silently dropped. A
/// `#`-leading line with no tab is a comment to the shared loader and is
/// skipped before that check; there are none in these fixtures.
#[test]
fn spec() {
    let dir = find_spec_dir(Some(Path::new(env!("CARGO_MANIFEST_DIR")))).expect("test/spec");
    let load = SpecOptions {
        min_cols: 2,
        ..SpecOptions::default()
    };
    let specs = load_spec_dir(&dir, &load).expect("the spec directory loads");
    assert!(!specs.is_empty(), "no fixtures found in {}", dir.display());

    let mut failures: Vec<String> = Vec::new();
    for spec in &specs {
        let name = spec.file.trim_end_matches(".tsv").to_string();
        let options = options_for(&name);
        // No `match_error` hook: an `ERROR:<code>` cell is compared
        // against the error's code by the shared runner's default, which
        // is the contract this package wants. Both codes the fixtures
        // pin, `duplicate_section` and `unterminated_section`, are
        // declared in ini-grammar.jsonic and raised by every runtime, so
        // nothing has to be resolved through message wording (which is
        // deliberately not a cross-runtime contract).
        let runner = Runner::new(move |input| {
            tabnas_ini::make_with(&options)
                .parse(input)
                .map(value_of)
                .map_err(failure_of)
        })
        .load(load.clone());
        failures.extend(runner.run_spec(spec).expect("the fixture runs"));
    }
    report(Ok(failures));
}

/// The census this suite is expected to cover. A fixture renamed or
/// removed would otherwise be a silent loss of coverage: the loop above
/// runs what it finds, and finding less is not a failure to it.
#[test]
fn every_fixture_is_present() {
    let dir = find_spec_dir(Some(Path::new(env!("CARGO_MANIFEST_DIR")))).expect("test/spec");
    for name in common::FIXTURES {
        assert!(dir.join(name).is_file(), "missing fixture {name}");
    }
    let found = std::fs::read_dir(&dir)
        .expect("the spec directory is readable")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "tsv"))
        .count();
    assert_eq!(
        found,
        common::FIXTURES.len(),
        "the spec directory holds {found} fixtures but this suite names {}",
        common::FIXTURES.len()
    );
}

/// Every name in the option table has to name a fixture that exists, or
/// the row it configures is quietly running with the defaults.
#[test]
fn every_named_option_set_has_a_fixture() {
    let dir = find_spec_dir(Some(Path::new(env!("CARGO_MANIFEST_DIR")))).expect("test/spec");
    for name in common::OPTION_NAMES {
        assert!(
            dir.join(format!("{name}.tsv")).is_file(),
            "the option table names {name}, which is not a fixture"
        );
    }
}
