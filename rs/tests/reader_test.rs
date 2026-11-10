/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! A parse from a reader (`Tabnas::parse_reader`, admin ADR-30 and ADR-33).
//!
//! [`tabnas_ini::make`] and [`tabnas_ini::make_with`] build the parser
//! whole, jsonic's base with the plugin installed, and declare `lex.window`
//! on it, so that a reader is read a window at a time. The string check and
//! hoover's blocks read the source only through the window, a scan that
//! reaches the window's end filling it and reading on; the engine cuts
//! again a quoted key the string matcher declines at the window's end
//! (tabnas/parser#326, from tabnas-parser 0.12.17); and the multiline value
//! matcher ends a value the window ends inside at the window's end, a token
//! the engine cuts again. These tests hold it to that: the constructors
//! declare the window and the plugin does not; every row of the shared
//! fixtures, under its file's options, and every document of the
//! conformance corpus, under the default options and the option sets that
//! install the multiline matcher and the inline comments, parses the same
//! from a reader, in pieces of 1, 3 and 64 bytes, under windows of a few
//! characters as well as the engine's own, as from its text: the same
//! value, or the same error code, position, row, column and token text; so
//! does what each scan decides on, at every distance from the window's end;
//! and values and lines longer than the window, and a long document read
//! under a window of 256 characters that moves on many times, parse the
//! same too.

mod common;

use std::io::Read;
use std::path::{Path, PathBuf};

use tabnas::{Tabnas, WindowSizes};
use tabnas_ini::{make, make_with, plugin, IniOptions};
use tabnas_support::{find_spec_dir, load_spec_dir, SpecOptions};

use common::options_for;

/// A reader that hands its bytes over at most `size` at a time.
struct Pieces<'a> {
    bytes: &'a [u8],
    at: usize,
    size: usize,
}

impl<'a> Pieces<'a> {
    fn new(bytes: &'a [u8], size: usize) -> Self {
        Pieces { bytes, at: 0, size }
    }
}

impl Read for Pieces<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.size.min(buf.len()).min(self.bytes.len() - self.at);
        buf[..n].copy_from_slice(&self.bytes[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

/// What a parse shows: its value, or its error's code, position, row,
/// column and token text. An error's excerpt is left out: a parse from a
/// reader shows a long line only as far back as the window reaches
/// (tabnas/parser `DIVERGENCE.md`), the one difference allowed.
fn outcome(parsed: Result<tabnas::Value, tabnas::TabnasError>) -> String {
    match parsed {
        Ok(value) => format!("ok {}", value.to_json()),
        Err(e) => format!(
            "err {} at {} {}:{} {:?}",
            e.code, e.pos, e.row, e.col, e.src
        ),
    }
}

/// The first `n` bytes of `s`, cut back to a character boundary.
fn cut(s: &str, n: usize) -> &str {
    let mut end = n.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// The pieces a document is handed over in, and the windows (margin and
/// block, in characters) it is read under besides the engine's own: a few
/// characters, so that every scan meets the window's edge.
const PIECES: [usize; 3] = [1, 3, 64];
const WINDOWS: [(usize, usize); 3] = [(1, 1), (3, 2), (64, 64)];

/// The reads that differ from the whole text's parse, described.
fn differences(at: &str, text: &str, whole: &str, reads: &[(String, String)]) -> Vec<String> {
    reads
        .iter()
        .filter(|(_, read)| read != whole)
        .map(|(how, read)| {
            format!(
                "{at}: {:?} from a reader in {how}\n  whole:  {}\n  reader: {}",
                cut(text, 80),
                cut(whole, 300),
                cut(read, 300)
            )
        })
        .collect()
}

/// `text` parsed by `parser` whole, and from a reader in each of [`PIECES`]
/// under the engine's window and each of [`WINDOWS`]: how many reader
/// parses were compared, and where they differ. A document over 4 KiB is
/// read in pieces of 64 bytes alone, under each window.
fn same_from_reader(parser: &Tabnas, at: &str, text: &str) -> (usize, Vec<String>) {
    let whole = outcome(parser.parse(text));
    let bytes = text.as_bytes();
    let pieces: &[usize] = if bytes.len() > 4096 { &[64] } else { &PIECES };
    let mut reads: Vec<(String, String)> = Vec::new();
    for &size in pieces {
        reads.push((
            format!("pieces of {size}"),
            outcome(parser.parse_reader(Pieces::new(bytes, size))),
        ));
        for (margin, block) in WINDOWS {
            let sizes = WindowSizes {
                margin,
                block,
                chunk: size,
            };
            reads.push((
                format!("pieces of {size}, margin {margin}, block {block}"),
                outcome(parser.parse_reader_with_sizes(Pieces::new(bytes, size), sizes)),
            ));
        }
    }
    (reads.len(), differences(at, text, &whole, &reads))
}

/// The option sets the corpus is read under: the defaults, inline comments
/// (hoover's value block ending at a comment character), the multiline
/// matcher with continuation and indent, and the multiline matcher that
/// inline comments preceded by whitespace install.
const OPTION_SETS: [&str; 4] = [
    "",
    "inline-comments-active",
    "multiline-both",
    "inline-comments-whitespace-backslash",
];

#[test]
fn make_reads_a_reader_a_window_at_a_time() {
    assert!(make().options.lex.window);
    assert!(make_with(&options_for("multiline-both")).options.lex.window);
    // The plugin alone declares nothing: a host may install it beneath
    // matchers of its own.
    let mut parser = tabnas_jsonic::make();
    parser
        .use_plugin(plugin(IniOptions::default()), None)
        .expect("the plugin installs");
    assert!(!parser.options.lex.window);
}

/// Every fixture row, whole, through [`make_with`] and through the plugin
/// on jsonic's base, which is what it builds, but for the window.
#[test]
fn make_parses_as_the_plugin_on_jsonic() {
    let dir = find_spec_dir(Some(Path::new(env!("CARGO_MANIFEST_DIR")))).expect("test/spec");
    let specs = load_spec_dir(&dir, &SpecOptions::default()).expect("the spec directory loads");
    let mut compared = 0;
    let mut failures = Vec::new();
    for spec in &specs {
        let options = options_for(spec.file.trim_end_matches(".tsv"));
        let made = make_with(&options);
        let mut composed = tabnas_jsonic::make();
        composed
            .use_plugin(plugin(options.clone()), None)
            .expect("the plugin installs");
        for row in &spec.rows {
            let input = row.unesc_named("input");
            let (a, b) = (outcome(made.parse(&input)), outcome(composed.parse(&input)));
            compared += 1;
            if a != b {
                failures.push(format!(
                    "{}\n  make:     {a}\n  composed: {b}",
                    row.location()
                ));
            }
        }
    }
    assert!(compared > 200, "{compared}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every row of the shared fixtures, under its file's options, parses the
/// same from a reader as from its text.
#[test]
fn every_row_parses_the_same_from_a_reader() {
    let dir = find_spec_dir(Some(Path::new(env!("CARGO_MANIFEST_DIR")))).expect("test/spec");
    let specs = load_spec_dir(&dir, &SpecOptions::default()).expect("the spec directory loads");
    let mut compared = 0;
    let mut failures = Vec::new();
    for (n, spec) in specs.iter().enumerate() {
        let parser = make_with(&options_for(spec.file.trim_end_matches(".tsv")));
        for row in &spec.rows {
            let (count, found) =
                same_from_reader(&parser, &row.location(), &row.unesc_named("input"));
            compared += count;
            failures.extend(found);
        }
        if (n + 1) % 10 == 0 || n + 1 == specs.len() {
            eprintln!(
                "reader fixtures: {} of {} files ({}%), {compared} reader parses",
                n + 1,
                specs.len(),
                (n + 1) * 100 / specs.len()
            );
        }
    }
    assert!(compared > 2_000, "{compared}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn corpus_path() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.join("test").join("corpus").join("ini-corpus.json")
}

/// Every document of the conformance corpus, under each of
/// [`OPTION_SETS`], parses the same from a reader as from its text.
#[test]
fn the_corpus_parses_the_same_from_a_reader() {
    let raw = std::fs::read_to_string(corpus_path()).expect("test/corpus/ini-corpus.json");
    let corpus: serde_json::Value = serde_json::from_str(&raw).expect("the corpus is JSON");
    let cases = corpus["cases"].as_array().expect("the corpus has cases");
    let mut compared = 0;
    let mut failures = Vec::new();
    for (n, name) in OPTION_SETS.iter().enumerate() {
        let parser = make_with(&options_for(name));
        for case in cases {
            let source = case["source"].as_str().expect("a case has a source");
            let at = format!("{} [{name}]", case["name"].as_str().unwrap_or_default());
            let (count, found) = same_from_reader(&parser, &at, source);
            compared += count;
            failures.extend(found);
        }
        eprintln!(
            "reader corpus: {} of {} option sets ({}%), {compared} reader parses",
            n + 1,
            OPTION_SETS.len(),
            (n + 1) * 100 / OPTION_SETS.len()
        );
    }
    assert_eq!(cases.len(), 36);
    assert!(compared > 1_000, "{compared}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The documents whose deciding character sits after a run of 0 to 40
/// characters: a closing quote and what follows it on its line, a quoted
/// key's closing quote, a continuation, an indented next line, a line
/// end, an escape, an inline comment, a section's closing bracket. Each is
/// the option set it is read under, the text before the run and the text
/// after it.
const EDGES: [(&str, &str, &str); 20] = [
    ("", "a = \"", "\"\r\nb = 1\r\n"),
    ("", "a = \"", "\"  \nb = 1\n"),
    ("", "a = \"", "\" x\nb = 1\n"),
    ("", "a = \"", "\\\"\"\nb = 1\n"),
    ("", "\"", "\""),
    ("", "\"", "\" = 1"),
    ("", "\"", "\"\nb = 1\n"),
    ("", "a = ", "\\;x\nb = 1\n"),
    ("", "a = ", "\r\nb = 1\n"),
    ("", "[", "]\na = 1\n"),
    ("", "[", "\na = 1\n"),
    ("inline-comments-active", "a = \"", "\" ; c\nb = 1\n"),
    ("inline-comments-active", "a = ", "\\; x ; c\nb = 1\n"),
    ("multiline-both", "a = ", "\\\n  two\nb = 1\n"),
    ("multiline-both", "a = ", "\\\r\n  two\nb = 1\n"),
    ("multiline-both", "a = ", "\n  two\nb = 1\n"),
    ("multiline-both", "a = ", "\r\n  two\nb = 1\n"),
    ("multiline-both", "a = ", "\\nx\nb = 1\n"),
    (
        "inline-comments-whitespace-backslash",
        "a = ",
        " \\; x ; c\nb = 1\n",
    ),
    (
        "inline-comments-whitespace-backslash",
        "a = ",
        "\\\n  two ; c\nb = 1\n",
    ),
];

/// What each scan decides on, at every distance from the window's end:
/// each of [`EDGES`] with its run of 0 to 40 characters, from a reader in
/// pieces of 1 and 3 bytes under windows of one and three characters,
/// which a scan's fills widen from the token's start, so that the window
/// ends at the deciding character, before it and after it.
#[test]
fn every_scan_meets_the_window_end() {
    let mut compared = 0;
    let mut failures = Vec::new();
    for (n, (name, before, after)) in EDGES.iter().enumerate() {
        let parser = make_with(&options_for(name));
        for run in 0..=40 {
            let text = format!("{before}{}{after}", "x".repeat(run));
            let whole = outcome(parser.parse(&text));
            let mut reads = Vec::new();
            for size in [1, 3] {
                for (margin, block) in [(1, 1), (3, 2)] {
                    let sizes = WindowSizes {
                        margin,
                        block,
                        chunk: size,
                    };
                    reads.push((
                        format!("pieces of {size}, margin {margin}, block {block}"),
                        outcome(
                            parser
                                .parse_reader_with_sizes(Pieces::new(text.as_bytes(), size), sizes),
                        ),
                    ));
                }
            }
            compared += reads.len();
            failures.extend(differences(
                &format!("edge {n} [{name}], run {run}"),
                &text,
                &whole,
                &reads,
            ));
        }
        if (n + 1) % 5 == 0 || n + 1 == EDGES.len() {
            eprintln!(
                "reader window ends: {} of {} documents ({}%), {compared} reader parses",
                n + 1,
                EDGES.len(),
                (n + 1) * 100 / EDGES.len()
            );
        }
    }
    assert!(compared > 3_000, "{compared}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Values and lines longer than the engine's window, which reaches 64 KiB
/// past the cursor before each token, parse the same from a reader as from
/// their text, under each option set: a value, a quoted value and the text
/// after its closing quote, a key, a quoted key, a section name, a comment,
/// values continued onto the next line by a backslash and by indentation,
/// and the errors.
#[test]
fn values_longer_than_the_window_parse_the_same_from_a_reader() {
    let long = 64 * 1024 + 2048;
    let documents = [
        format!("a = {}\nb = 1\n", "v".repeat(long)),
        format!("a = \"{}\"\nb = 1\n", "q".repeat(long)),
        format!("a = \"q\"{}x\nb = 1\n", " ".repeat(long)),
        format!("a = \"q\"{} ; note\nb = 1\n", " ".repeat(long)),
        format!("{} = 1\n", "k".repeat(long)),
        // A quoted key the window ends inside, with the end of the source
        // near: the string matcher reads it (tabnas/parser#326).
        format!("\"{}\"\n", "k".repeat(long)),
        format!("\"{}\" = 1\n", "k".repeat(long)),
        format!("[{}]\na = 1\n", "s".repeat(long)),
        format!("; {}\na = 1\n", "c".repeat(long)),
        format!("a = x ; {}\nb = 1\n", "c".repeat(long)),
        format!("a = one \\\n{}two\nb = 1\n", " ".repeat(long)),
        format!("a = one\n{}two\nb = 1\n", " ".repeat(long)),
        format!("a = {}\\\r\n two\r\nb = 1\r\n", "w".repeat(long)),
        format!("a = {}\\; x\n", "e".repeat(long)),
        format!("[{}\na = 1\n", "u".repeat(long)),
        format!("a = \"{}", "o".repeat(long)),
    ];
    let mut failures = Vec::new();
    for name in OPTION_SETS {
        let parser = make_with(&options_for(name));
        for (n, document) in documents.iter().enumerate() {
            let whole = outcome(parser.parse(document));
            let reads: Vec<(String, String)> = [4096, 64 * 1024 + 3]
                .iter()
                .map(|&size| {
                    (
                        format!("pieces of {size}"),
                        outcome(parser.parse_reader(Pieces::new(document.as_bytes(), size))),
                    )
                })
                .collect();
            failures.extend(differences(
                &format!("[{name}] document {n}"),
                document,
                &whole,
                &reads,
            ));
        }
        eprintln!(
            "reader long values: [{name}], {} documents",
            documents.len()
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A long document, under the engine's window and under one of 256
/// characters that moves on many times before the end, parses the same as
/// its text, under each option set; so does the same document with an
/// unterminated section header at its end, which the reader places where
/// the whole text does.
#[test]
fn a_long_document_parses_the_same_from_a_reader() {
    let mut body = String::from("; settings\nname = top\n");
    for i in 0..2_000 {
        body.push_str(&format!(
            "[section{i}.sub]\nkey{i} = value {i} \u{e9}\nquoted = \"q;{i}\"\n\
             path = a\\;b\\\\c ; note {i}\nlist[] = {i}\nflag\n\
             wrapped = one \\\n  two {i}\n  indented {i}\n# comment {i}\n"
        ));
    }
    let text = format!("{body}last = 1\n");
    let broken = format!("{body}[open\n");
    assert!(text.len() > 256 * 1024, "{}", text.len());
    let small = WindowSizes {
        margin: 256,
        block: 256,
        chunk: 4096,
    };
    let mut failures = Vec::new();
    for name in OPTION_SETS {
        let parser = make_with(&options_for(name));
        for (n, document) in [&text, &broken].into_iter().enumerate() {
            let whole = outcome(parser.parse(document));
            assert_eq!(whole.starts_with("ok "), 0 == n, "{}", cut(&whole, 200));
            let reads = vec![
                (
                    "pieces of 4096".to_string(),
                    outcome(parser.parse_reader(Pieces::new(document.as_bytes(), 4096))),
                ),
                (
                    "pieces of 4096, margin 256, block 256".to_string(),
                    outcome(
                        parser
                            .parse_reader_with_sizes(Pieces::new(document.as_bytes(), 4096), small),
                    ),
                ),
            ];
            failures.extend(differences(
                &format!("[{name}] document {n}"),
                document,
                &whole,
                &reads,
            ));
        }
        eprintln!("reader long document: [{name}], {} bytes", text.len());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
