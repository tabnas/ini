/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! The embedded grammar is the grammar.
//!
//! `ini-grammar.jsonic` at the repository root is the source of truth
//! for every runtime, and `ts/embed-grammar.js` copies it verbatim into
//! `ts/src/ini.ts`, `go/ini.go` and `rs/src/lib.rs`. Nothing at run time
//! re-reads the file, so a hand-edit between the markers, or a forgotten
//! `npm run embed`, would leave this port parsing a grammar the other
//! two do not have. This suite is what notices.

use std::fs;
use std::path::{Path, PathBuf};

const BEGIN: &str = "// --- BEGIN EMBEDDED ini-grammar.jsonic ---";
const END: &str = "// --- END EMBEDDED ini-grammar.jsonic ---";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rs/ has a parent")
        .to_path_buf()
}

/// The text between the markers in `src/lib.rs`, unwrapped from its raw
/// string literal.
fn embedded() -> String {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
        .expect("src/lib.rs is readable");
    let start = source.find(BEGIN).expect("the BEGIN marker is present") + BEGIN.len();
    let end = source.find(END).expect("the END marker is present");
    let block = &source[start..end];
    let open = block
        .find("r#\"")
        .expect("the embedded block is a raw string literal")
        + 3;
    let close = block
        .rfind("\"#;")
        .expect("the raw string literal is closed");
    // The embedder writes a newline after the opening delimiter, which
    // the Go and TypeScript blocks carry too.
    block[open..close]
        .strip_prefix('\n')
        .expect("the embedded text starts on its own line")
        .to_string()
}

#[test]
fn the_embedded_grammar_matches_the_file_on_disk() {
    let on_disk = fs::read_to_string(repo_root().join("ini-grammar.jsonic"))
        .expect("ini-grammar.jsonic is readable");
    assert_eq!(
        embedded(),
        on_disk,
        "src/lib.rs has drifted from ini-grammar.jsonic; \
         run `npm run embed` from ts/ rather than editing between the markers"
    );
}

/// The same block, in the other two ports. A change embedded into one
/// runtime and not the others is the failure this catches.
#[test]
fn every_runtime_embeds_the_same_grammar() {
    let on_disk = fs::read_to_string(repo_root().join("ini-grammar.jsonic"))
        .expect("ini-grammar.jsonic is readable");
    for (path, open, close) in [
        ("ts/src/ini.ts", "const grammarText = `\n", "`"),
        ("go/ini.go", "const grammarText = `\n", "`"),
    ] {
        let source = fs::read_to_string(repo_root().join(path))
            .unwrap_or_else(|_| panic!("{path} is readable"));
        let start = source
            .find(BEGIN)
            .unwrap_or_else(|| panic!("{path} has a BEGIN marker"))
            + BEGIN.len();
        let end = source
            .find(END)
            .unwrap_or_else(|| panic!("{path} has an END marker"));
        let block = &source[start..end];
        let from = block
            .find(open)
            .unwrap_or_else(|| panic!("{path} opens a literal"))
            + open.len();
        let to = from
            + block[from..]
                .find(close)
                .unwrap_or_else(|| panic!("{path} closes its literal"));
        // TypeScript embeds into a template literal, which needs the
        // backslash doubled; the embedder does that and nothing else.
        let text = if path.ends_with(".ts") {
            block[from..to].replace("\\\\", "\\")
        } else {
            block[from..to].to_string()
        };
        assert_eq!(text, on_disk, "{path} has drifted from ini-grammar.jsonic");
    }
}

/// The grammar declares two error codes, and the code is the
/// cross-runtime contract. Reading them off the live instance proves the
/// document's `options.error` block reached the engine rather than being
/// dropped on the way in.
#[test]
fn the_declared_error_codes_are_installed() {
    let parser = tabnas_ini::make();
    let options = parser.config();
    for code in ["duplicate_section", "unterminated_section"] {
        assert!(
            options.error.contains_key(code),
            "the grammar declares {code}, but the instance does not carry it"
        );
    }
}

/// The lexer options the grammar sets, read back off the built instance.
/// Each one is load bearing: numbers stay strings, bare text is hoover's
/// to lex, quotes are the two INI uses, and an unterminated string is
/// abandoned so hoover can take the raw line.
#[test]
fn the_grammar_options_are_in_force() {
    let options = tabnas_ini::make().config();
    assert!(!options.number.lex, "number.lex should be off");
    assert!(!options.text.lex, "text.lex should be off");
    assert!(options.string.lex, "string.lex should be on");
    assert!(options.string.abandon, "string.abandon should be on");
    assert_eq!(options.string.chars, "'\"");
    assert_eq!(options.rule.start, "ini");
    assert_eq!(options.rule.exclude, "jsonic");
}

/// `{`, `}` and `:` stop being fixed tokens, and `=` and `.` become
/// them. A section header is `[a.b]`, not an object literal.
#[test]
fn the_fixed_token_table_is_the_ini_one() {
    let parser = tabnas_ini::make();
    for source in ["{", "}", ":"] {
        assert!(
            parser.fixed(source).is_none(),
            "{source} is still a fixed token"
        );
    }
    for source in ["=", ".", "[", "]"] {
        assert!(
            parser.fixed(source).is_some(),
            "{source} is not a fixed token"
        );
    }
}
