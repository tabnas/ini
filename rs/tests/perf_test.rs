/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! A guard against the performance regression where the convenience
//! `parse` rebuilds the (expensive) INI engine and grammar on every call
//! instead of reusing a cached instance.
//!
//! Building the grammar, which means parsing the embedded grammar text
//! with jsonic, wiring the hoover blocks and applying the rule spec,
//! dominates a parse of a small document, so a rebuild-per-call `parse`
//! is many times slower than reusing one instance.
//!
//! The Rust twin of `go/perf_test.go` and `ts/test/perf.test.ts`. Only
//! the no-options path is cached; `parse_with` deliberately still builds
//! fresh, and `make_with` is what a caller reuses.
//!
//! The check is machine-INDEPENDENT: it compares `parse` against
//! instance reuse on the SAME machine in the SAME run, so a slow CI box
//! cannot make it flaky, both sides scaling together. There is
//! deliberately NO wall-clock budget.

use std::time::{Duration, Instant};

const SRC: &str = "a = 1\nb = 2\n[s]\nc = 3";
const N: usize = 4000;
const ROUNDS: usize = 20;

#[test]
fn parse_reuses_its_instance() {
    // Warm both paths so the comparison is steady state.
    for _ in 0..100 {
        let _ = tabnas_ini::parse(SRC);
    }
    let parser = tabnas_ini::make();
    for _ in 0..100 {
        let _ = parser.parse(SRC);
    }

    // The two loops are INTERLEAVED in rounds rather than run one after
    // the other. This machine shares its cores, and a long block of one
    // measurement can land entirely inside someone else's busy period;
    // alternating short batches spreads any such period across both
    // sides, which is what makes the ratio mean something.
    let mut convenience = Duration::ZERO;
    let mut reuse = Duration::ZERO;
    for _ in 0..ROUNDS {
        let start = Instant::now();
        for _ in 0..N / ROUNDS {
            tabnas_ini::parse(SRC).expect("the convenience path parses");
        }
        convenience += start.elapsed();

        let start = Instant::now();
        for _ in 0..N / ROUNDS {
            parser.parse(SRC).expect("the reuse path parses");
        }
        reuse += start.elapsed();
    }

    // A cached `parse` is about the same as instance reuse; 4x is
    // allowed for scheduling noise. A rebuild-per-call `parse` is many
    // times slower than that here, so this catches the regression
    // without depending on absolute speed.
    assert!(
        convenience <= reuse * 4,
        "parse appears to rebuild the grammar on every call: {N} parse calls took \
         {convenience:?} against {reuse:?} reusing one instance (ratio {:.1}x, limit 4x). \
         Cache a lazy default instance (see `parse` and its `OnceLock`).",
        convenience.as_secs_f64() / reuse.as_secs_f64()
    );
    eprintln!(
        "parse={convenience:?}  reuse={reuse:?}  ratio={:.2}x",
        convenience.as_secs_f64() / reuse.as_secs_f64().max(f64::MIN_POSITIVE)
    );
}

/// Building an instance really is the expensive part, which is what
/// makes the ratio above meaningful. Without this, a `parse` that
/// rebuilt a CHEAP grammar would pass the ratio test and the guard would
/// assert nothing.
#[test]
fn building_an_instance_costs_far_more_than_a_parse() {
    let parser = tabnas_ini::make();
    for _ in 0..100 {
        let _ = parser.parse(SRC);
    }

    let start = Instant::now();
    for _ in 0..50 {
        let _ = tabnas_ini::make();
    }
    let build = start.elapsed() / 50;

    let start = Instant::now();
    for _ in 0..500 {
        parser.parse(SRC).expect("parses");
    }
    let parse = start.elapsed() / 500;

    assert!(
        build > parse * 5,
        "building an instance ({build:?}) is not much dearer than one parse ({parse:?}), \
         so the reuse guard above would not notice a rebuild-per-call parse"
    );
}

/// A document of many sections must not cost time quadratic in their
/// number. A Rust container is a value rather than a reference, so the
/// section tree is taken apart and put back together as the parse walks
/// it; doing that by cloning made a 5,000-section document take seconds.
/// The ratio, not the wall clock, is the assertion.
#[test]
fn many_sections_stay_close_to_linear() {
    fn document(sections: usize) -> String {
        (0..sections)
            .map(|index| format!("[s{index}]\nk = {index}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn time(src: &str) -> f64 {
        let parser = tabnas_ini::make();
        let start = Instant::now();
        parser.parse(src).expect("parses");
        start.elapsed().as_secs_f64()
    }

    let small = document(500);
    let large = document(4000);
    // Warm the code paths so the first measurement is not the one that
    // pays for the lazy statics.
    let _ = time(&small);

    let small_time = time(&small).max(f64::MIN_POSITIVE);
    let large_time = time(&large);
    let growth = large_time / small_time;

    // Eight times the sections. Linear is 8x, quadratic is 64x; the
    // limit is set well above the former and well below the latter so
    // that a noisy machine cannot fail it and a quadratic regression
    // cannot pass it.
    assert!(
        growth < 30.0,
        "parsing 4,000 sections took {growth:.1}x parsing 500 ({large_time:?} against \
         {small_time:?}); eight times the input should not cost thirty times the time"
    );
}
