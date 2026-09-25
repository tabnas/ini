/* Copyright (c) 2021-2026 Richard Rodger and other contributors, MIT License */

//! In-language behaviour, the Rust twin of `go/ini_test.go` and
//! `ts/test/ini.test.ts`.
//!
//! Anything expressible as input to output belongs in a shared fixture
//! under `test/spec`, where every runtime runs it. What is here is what
//! a fixture cannot hold: the construction API, the plugin surface, the
//! threading contract, and the handful of documents whose expected value
//! is easier to read as a tree than as one JSON cell.

mod common;

use serde_json::json;
use tabnas_ini::{
    make, make_with, parse, parse_with, plugin, CommentOptions, Duplicate, IniOptions,
    InlineCommentOptions, InlineEscapeOptions, MultilineOptions, SectionOptions,
};

// --- option builders, spelled as the Go test spells them ---------------

fn inline_active() -> IniOptions {
    IniOptions::default().with_inline_comments()
}

fn inline_escape(backslash: Option<bool>, whitespace: Option<bool>) -> IniOptions {
    IniOptions {
        comment: Some(CommentOptions {
            inline: Some(InlineCommentOptions {
                active: Some(true),
                chars: None,
                escape: Some(InlineEscapeOptions {
                    backslash,
                    whitespace,
                }),
            }),
        }),
        ..Default::default()
    }
}

fn inline_chars(chars: &[&str]) -> IniOptions {
    IniOptions {
        comment: Some(CommentOptions {
            inline: Some(InlineCommentOptions {
                active: Some(true),
                chars: Some(chars.iter().map(|text| text.to_string()).collect()),
                escape: None,
            }),
        }),
        ..Default::default()
    }
}

fn multiline() -> IniOptions {
    IniOptions::default().with_multiline()
}

fn multiline_indent() -> IniOptions {
    IniOptions {
        multiline: Some(MultilineOptions {
            continuation: Some(String::new()),
            indent: Some(true),
        }),
        ..Default::default()
    }
}

fn duplicate(mode: Duplicate) -> IniOptions {
    IniOptions {
        section: Some(SectionOptions { duplicate: mode }),
        ..Default::default()
    }
}

// --- assertions --------------------------------------------------------

#[track_caller]
fn assert_parse(src: &str, options: &IniOptions, want: serde_json::Value) {
    let got = parse_with(src, options)
        .unwrap_or_else(|error| panic!("{src:?} did not parse: {error}"))
        .to_json();
    assert_eq!(common::norm(got), common::norm(want), "parsing {src:?}");
}

#[track_caller]
fn assert_default(src: &str, want: serde_json::Value) {
    let got = parse(src)
        .unwrap_or_else(|error| panic!("{src:?} did not parse: {error}"))
        .to_json();
    assert_eq!(common::norm(got), common::norm(want), "parsing {src:?}");
}

#[track_caller]
fn assert_rejected(src: &str, options: &IniOptions, code: &str) {
    match parse_with(src, options) {
        Ok(value) => panic!("{src:?} was accepted as {}", value.to_json()),
        Err(error) => assert_eq!(error.code, code, "rejecting {src:?}"),
    }
}

// --- the tests ---------------------------------------------------------

#[test]
fn happy() {
    let parser = make();
    for (src, want) in [
        ("a=1", json!({"a": "1"})),
        ("[A]", json!({"A": {}})),
        ("a=\nb=", json!({"a": "", "b": ""})),
    ] {
        assert_eq!(parser.parse(src).expect("parses").to_json(), want);
    }
}

#[test]
fn inline_comments_are_off_by_default() {
    assert_default("a = hello ; world", json!({"a": "hello ; world"}));
    assert_default("a = hello # world", json!({"a": "hello # world"}));
    assert_default("a = x;y;z", json!({"a": "x;y;z"}));
}

#[test]
fn line_comments_always_work() {
    assert_default("; comment\na = 1", json!({"a": "1"}));
    assert_default("# comment\na = 1", json!({"a": "1"}));

    let input = "; line comment\n# hash comment\na = 1";
    assert_default(input, json!({"a": "1"}));
    assert_parse(input, &inline_active(), json!({"a": "1"}));
}

#[test]
fn inline_comments_active() {
    let options = inline_active();
    assert_parse("a = hello ; comment", &options, json!({"a": "hello"}));
    assert_parse("a = hello # comment", &options, json!({"a": "hello"}));
    assert_parse("a = x;y", &options, json!({"a": "x"}));
    assert_parse(
        "a = value\nb = other",
        &options,
        json!({"a": "value", "b": "other"}),
    );
}

#[test]
fn inline_comment_custom_chars() {
    let options = IniOptions {
        comment: Some(CommentOptions {
            inline: Some(InlineCommentOptions {
                active: Some(true),
                chars: Some(vec![";".to_string()]),
                escape: None,
            }),
        }),
        ..Default::default()
    };
    assert_parse(
        "a = hello ; comment\nb = hello # not a comment",
        &options,
        json!({"a": "hello", "b": "hello # not a comment"}),
    );
}

#[test]
fn inline_comment_backslash_escape() {
    let options = inline_escape(Some(true), None);
    assert_parse("a = hello\\; world", &options, json!({"a": "hello; world"}));
    assert_parse("a = hello\\# world", &options, json!({"a": "hello# world"}));
    assert_parse("a = x\\;y ; comment", &options, json!({"a": "x;y"}));
}

#[test]
fn inline_comment_backslash_escape_disabled() {
    let options = inline_escape(Some(false), None);
    assert_parse(
        "a = hello\\; world",
        &options,
        json!({"a": "hello\\; world"}),
    );
    assert_parse("a = hello ; comment", &options, json!({"a": "hello"}));
}

#[test]
fn inline_comment_whitespace_prefix() {
    let options = inline_escape(None, Some(true));
    assert_parse("a = x;y;z", &options, json!({"a": "x;y;z"}));
    assert_parse("a = hello ;comment", &options, json!({"a": "hello"}));
    assert_parse("a = hello\t;comment", &options, json!({"a": "hello"}));
    assert_parse("a = x#y", &options, json!({"a": "x#y"}));
    assert_parse("a = hello #comment", &options, json!({"a": "hello"}));
}

#[test]
fn inline_comment_whitespace_prefix_with_backslash() {
    let options = inline_escape(Some(true), Some(true));
    assert_parse("a = x;y", &options, json!({"a": "x;y"}));
    assert_parse("a = hello ;comment", &options, json!({"a": "hello"}));
    assert_parse(
        "a = hello \\;not-a-comment",
        &options,
        json!({"a": "hello ;not-a-comment"}),
    );
}

#[test]
fn inline_comments_inside_sections() {
    assert_parse(
        "[s]\na = val ; comment\nb = other",
        &inline_active(),
        json!({"s": {"a": "val", "b": "other"}}),
    );
}

#[test]
fn sections() {
    assert_default("[d]\ne = 2", json!({"d": {"e": "2"}}));
    assert_default("[h.i]\nj = 3", json!({"h": {"i": {"j": "3"}}}));
    assert_default(
        "x = 0\n[s]\na = 1\nb = 2",
        json!({"x": "0", "s": {"a": "1", "b": "2"}}),
    );
}

#[test]
fn nested_section_without_a_declared_middle_parent() {
    assert_default(
        "[a]\nav = a val\n[a.b.c]\ne = 1\nj = 2",
        json!({"a": {"av": "a val", "b": {"c": {"e": "1", "j": "2"}}}}),
    );
}

#[test]
fn escaped_dots_in_section_names() {
    assert_default(
        "[x\\.y\\.z]\nx.y.z = xyz",
        json!({"x.y.z": {"x.y.z": "xyz"}}),
    );
    assert_default(
        "[x\\.y\\.z.a\\.b\\.c]\na.b.c = abc",
        json!({"x.y.z": {"a.b.c": {"a.b.c": "abc"}}}),
    );
}

#[test]
fn duplicate_sections_merge_by_default() {
    assert_default(
        "[a]\nx=1\ny=2\n[a]\nz=3",
        json!({"a": {"x": "1", "y": "2", "z": "3"}}),
    );
    assert_default("[a]\nx=1\n[a]\nx=2", json!({"a": {"x": "2"}}));
    assert_default(
        "[a.b]\nx=1\n[a.b]\ny=2",
        json!({"a": {"b": {"x": "1", "y": "2"}}}),
    );
    assert_default(
        "[a.b]\nx=1\n[a]\ny=2",
        json!({"a": {"b": {"x": "1"}, "y": "2"}}),
    );
    assert_parse(
        "[a]\nx=1\n[a]\ny=2",
        &duplicate(Duplicate::Merge),
        json!({"a": {"x": "1", "y": "2"}}),
    );
}

#[test]
fn duplicate_sections_override() {
    let options = duplicate(Duplicate::Override);
    assert_parse(
        "[a]\nx=1\ny=2\n[a]\nz=3",
        &options,
        json!({"a": {"z": "3"}}),
    );
    assert_parse("[a]\nx=1", &options, json!({"a": {"x": "1"}}));
    assert_parse(
        "[a.b]\nx=1\n[a]\ny=2\n[a]\nz=3",
        &options,
        json!({"a": {"z": "3"}}),
    );
    assert_parse(
        "[a]\nx=1\n[b]\ny=2",
        &options,
        json!({"a": {"x": "1"}, "b": {"y": "2"}}),
    );
    assert_parse(
        "[a.b]\nx=1\n[a.b]\ny=2",
        &options,
        json!({"a": {"b": {"y": "2"}}}),
    );
}

#[test]
fn duplicate_sections_error() {
    let options = duplicate(Duplicate::Error);
    assert_parse("[a]\nx=1", &options, json!({"a": {"x": "1"}}));
    assert_parse(
        "[a]\nx=1\n[b]\ny=2",
        &options,
        json!({"a": {"x": "1"}, "b": {"y": "2"}}),
    );
    assert_rejected("[a]\nx=1\n[a]\ny=2", &options, "duplicate_section");
    assert_rejected("[a.b]\nx=1\n[a.b]\ny=2", &options, "duplicate_section");
    // An intermediate path is NOT a declared section.
    assert_parse(
        "[a.b]\nx=1\n[a]\ny=2",
        &options,
        json!({"a": {"b": {"x": "1"}, "y": "2"}}),
    );
}

/// The rejection carries the offending path as the `{section}` detail
/// the message template reads, which is the one piece of the diagnostic
/// the grammar promises. The shared fixtures pin only the code.
#[test]
fn a_duplicate_section_names_itself() {
    let error = parse_with("[a.b]\nx=1\n[a.b]\ny=2", &duplicate(Duplicate::Error))
        .expect_err("a duplicate section is rejected");
    assert_eq!(error.code, "duplicate_section");
    assert!(
        error
            .to_string()
            .contains("duplicate section header: [a.b]"),
        "message was {error}"
    );
}

#[test]
fn an_unterminated_section_header_is_rejected() {
    let options = IniOptions::default();
    assert_rejected("[a", &options, "unterminated_section");
    assert_rejected("[a\nb=1", &options, "unterminated_section");
}

#[test]
fn a_bare_key_is_true() {
    assert_default("a=1\nmykey", json!({"a": "1", "mykey": true}));
}

#[test]
fn array_syntax() {
    assert_default("a[]=1\na[]=2", json!({"a": ["1", "2"]}));
    assert_default(
        "ar[]=one\nar[]=three\nar   = this is included",
        json!({"ar": ["one", "three", "this is included"]}),
    );
}

#[test]
fn multiline_backslash_continuation() {
    let options = multiline();
    assert_parse("a = hello \\\nworld", &options, json!({"a": "hello world"}));
    assert_parse(
        "a = hello \\\n    world",
        &options,
        json!({"a": "hello world"}),
    );
    assert_parse(
        "a = one \\\ntwo \\\nthree",
        &options,
        json!({"a": "one two three"}),
    );
    assert_parse(
        "a = hello\nb = world",
        &options,
        json!({"a": "hello", "b": "world"}),
    );
    assert_parse(
        "a = hello \\\r\nworld",
        &options,
        json!({"a": "hello world"}),
    );
    // An escaped backslash before a newline is not a continuation.
    assert_parse(
        "a = path\\\\\nb = next",
        &options,
        json!({"a": "path\\", "b": "next"}),
    );
    assert_parse(
        "[s]\na = hello \\\n    world",
        &options,
        json!({"s": {"a": "hello world"}}),
    );
    assert_parse("a = \\\nworld", &options, json!({"a": "world"}));
    // Inline comments are off by default, so a semicolon stays literal.
    assert_parse(
        "a = hello \\\nworld ;not-a-comment\nb = 2",
        &options,
        json!({"a": "hello world ;not-a-comment", "b": "2"}),
    );
}

#[test]
fn multiline_indent_continuation() {
    let options = multiline_indent();
    assert_parse(
        "a = hello\n    world",
        &options,
        json!({"a": "hello world"}),
    );
    assert_parse(
        "a = line1\n  line2\n  line3",
        &options,
        json!({"a": "line1 line2 line3"}),
    );
    assert_parse(
        "a = hello\nb = world",
        &options,
        json!({"a": "hello", "b": "world"}),
    );
    assert_parse("a = hello\n\tworld", &options, json!({"a": "hello world"}));
    assert_parse(
        "[s]\na = hello\n    world",
        &options,
        json!({"s": {"a": "hello world"}}),
    );
}

#[test]
fn multiline_both_modes() {
    let options = IniOptions {
        multiline: Some(MultilineOptions {
            continuation: Some("\\".to_string()),
            indent: Some(true),
        }),
        ..Default::default()
    };
    assert_parse("a = hello \\\nworld", &options, json!({"a": "hello world"}));
    assert_parse(
        "a = hello\n    world",
        &options,
        json!({"a": "hello world"}),
    );
}

#[test]
fn multiline_with_inline_comments() {
    let options = IniOptions {
        multiline: Some(MultilineOptions::default()),
        comment: Some(CommentOptions {
            inline: Some(InlineCommentOptions {
                active: Some(true),
                ..Default::default()
            }),
        }),
        ..Default::default()
    };
    assert_parse(
        "a = hello \\\nworld ;comment\nb = 2",
        &options,
        json!({"a": "hello world", "b": "2"}),
    );
}

#[test]
fn multiline_escapes() {
    let mut options = inline_escape(Some(true), None);
    options.multiline = Some(MultilineOptions::default());
    assert_parse(
        "a = one\\; two \\\nthree",
        &options,
        json!({"a": "one; two three"}),
    );
    assert_parse(
        "a = one\\# two \\\nthree",
        &options,
        json!({"a": "one# two three"}),
    );
}

#[test]
fn multiline_without_inline_comments() {
    let options = multiline();
    assert_parse(
        "a = one; two \\\nthree",
        &options,
        json!({"a": "one; two three"}),
    );
    assert_parse(
        "a = one# two \\\nthree",
        &options,
        json!({"a": "one# two three"}),
    );
}

#[test]
fn quoted_values() {
    assert_default(r#"a = "hello world""#, json!({"a": "hello world"}));
    // A single-quoted value is offered to the JSON reader first.
    assert_default("a = 'hello world'", json!({"a": "hello world"}));
    assert_default(r#"a = '{"y":{"z":6}}'"#, json!({"a": {"y": {"z": 6}}}));
}

#[test]
fn empty_input() {
    assert_default("", json!({}));
}

#[test]
fn value_keywords() {
    assert_default("a = true\nb = false", json!({"a": true, "b": false}));
    assert_default("a = null", json!({"a": null}));
}

#[test]
fn numbers_are_strings_by_default() {
    assert_default("a=1", json!({"a": "1"}));
    assert_default("a=2.5", json!({"a": "2.5"}));
    assert_default("a=-3", json!({"a": "-3"}));
    assert_default("a=0xFF", json!({"a": "0xFF"}));
}

#[test]
fn multiple_pairs() {
    assert_default(
        "a = 1\nb = x\nc = y y",
        json!({"a": "1", "b": "x", "c": "y y"}),
    );
}

#[test]
fn an_equals_sign_inside_a_value() {
    assert_default("u = v = 5", json!({"u": "v = 5"}));
}

#[test]
fn a_later_value_overwrites_an_earlier_one() {
    assert_default("br = cold\nbr = warm", json!({"br": "warm"}));
}

/// The whole of the Go `TestBasicComprehensive`, one document
/// exercising the grammar end to end.
#[test]
fn basic_comprehensive() {
    let src = r#"
; comment
a = 1
b = x
c = y y
c0 = true
" c1  c2 " = null
'[]'='[]'

[d]
e = 2
e0[]=q q
e0[]=w w
"[]"="[]"

[f]
# x:11
g = 'G'
# x:12


[h.i]
j = [3,4]
j0 = ]3,4[
k = false

[l.m.n.o]
p = "P"
q = {x:1}
u = v = 5
w = '{"y":{"z":6}}'
aa = 7

"#;
    let got = common::norm(
        parse(src)
            .expect("the comprehensive document parses")
            .to_json(),
    );
    assert_eq!(
        got,
        json!({
            "a": "1",
            "b": "x",
            "c": "y y",
            "c0": true,
            " c1  c2 ": null,
            "[]": [],
            "d": { "e": "2", "e0": ["q q", "w w"], "[]": "[]" },
            "f": { "g": "G" },
            "h": { "i": { "j": "[3,4]", "j0": "]3,4[", "k": false } },
            "l": { "m": { "n": { "o": {
                "p": "P",
                "q": "{x:1}",
                "u": "v = 5",
                "w": { "y": { "z": 6 } },
                "aa": "7"
            } } } }
        })
    );
}

// --- the construction API ---------------------------------------------

#[test]
fn make_builds_a_reusable_instance() {
    let parser = make();
    assert_eq!(parser.parse("a=1").unwrap().to_json(), json!({"a": "1"}));
    assert_eq!(parser.parse("b=2").unwrap().to_json(), json!({"b": "2"}));
}

#[test]
fn make_with_applies_the_options() {
    let parser = make_with(&inline_active());
    assert_eq!(
        parser.parse("a = x ; note").unwrap().to_json(),
        json!({"a": "x"})
    );
}

#[test]
fn the_plugin_installs_on_a_jsonic_instance() {
    let mut parser = tabnas_jsonic::make();
    parser
        .use_plugin(plugin(IniOptions::default()), None)
        .expect("the plugin installs on jsonic");
    assert_eq!(
        parser.parse("a=1\nb=2").unwrap().to_json(),
        json!({"a": "1", "b": "2"})
    );
}

/// A derived instance rebuilds itself from the plugins registered
/// through `use_plugin`, so the grammar has to survive the rebuild.
#[test]
fn a_derived_instance_keeps_the_grammar() {
    let mut parser = tabnas_jsonic::make();
    parser
        .use_plugin(plugin(IniOptions::default()), None)
        .expect("the plugin installs");
    let child = parser.derive(|_options| {}).expect("the instance derives");
    assert_eq!(
        child.parse("[s]\na=1").unwrap().to_json(),
        json!({"s": {"a": "1"}})
    );
}

/// The plugin refuses a bare engine rather than installing half a
/// grammar: hoover needs a `val` rule and this grammar extends jsonic's
/// `map` and `pair`.
#[test]
fn the_plugin_refuses_an_engine_with_no_jsonic() {
    let mut bare = tabnas::Tabnas::new();
    let error = tabnas_ini::ini(&mut bare, &IniOptions::default())
        .expect_err("a bare engine has no val rule");
    assert!(
        error
            .to_string()
            .contains("jsonic grammar must be installed"),
        "message was {error}"
    );
}

/// `list` and `elem` are unreachable once `val` is restricted to scalars
/// and maps, so they are removed: the rule set, and the railroad diagram
/// drawn from it, hold only the rules INI actually uses.
#[test]
fn the_unreachable_jsonic_rules_are_removed() {
    let parser = make();
    let names = parser.rule_names();
    for name in ["ini", "table", "dive", "map", "pair", "val"] {
        assert!(names.iter().any(|have| have == name), "missing rule {name}");
    }
    for name in ["list", "elem"] {
        assert!(
            !names.iter().any(|have| have == name),
            "rule {name} remains"
        );
    }
}

/// `parse` shares one instance across calls and across threads. The
/// engine parses through a shared reference with a fresh context per
/// call, so the installed grammar is read-only during a parse; the
/// declared-section set, which the canonical ports keep in a closure,
/// lives on the context here for exactly this reason.
#[test]
fn the_shared_parser_is_usable_from_several_threads() {
    let handles: Vec<_> = (0..8)
        .map(|index| {
            std::thread::spawn(move || {
                let src = format!("[s{index}]\nk = v{index}");
                let got = parse(&src).expect("parses").to_json();
                assert_eq!(
                    got,
                    json!({ format!("s{index}"): { "k": format!("v{index}") } })
                );
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("no thread panicked");
    }
}

/// Two documents parsed on two threads must not see each other's
/// declared sections. With the set on the instance rather than the
/// context, one of these would report a duplicate its own source does
/// not contain.
#[test]
fn concurrent_parses_do_not_share_declared_sections() {
    let parser = std::sync::Arc::new(make_with(&duplicate(Duplicate::Error)));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let parser = std::sync::Arc::clone(&parser);
            std::thread::spawn(move || {
                for _ in 0..50 {
                    assert!(parser.parse("[a]\nx=1\n[b]\ny=2").is_ok());
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("no thread panicked");
    }
}

// --- untrusted input ---------------------------------------------------

/// Deep nesting, very long input, unterminated constructs, empty input,
/// control characters and odd Unicode must not panic, hang or overflow
/// the stack. Each of these is parsed for its outcome, whatever that
/// outcome is; the assertion is that there is one.
#[test]
fn hostile_input_is_answered_rather_than_crashing() {
    let deep_section = format!("[{}]\nx=1", vec!["a"; 500].join("."));
    let long_value = format!("a = {}", "x".repeat(200_000));
    let many_pairs = (0..20_000)
        .map(|index| format!("k{index} = {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let cases = [
        String::new(),
        "\n\n\n".to_string(),
        "[".to_string(),
        "[[[[[[[[[[".to_string(),
        "]]]]]]".to_string(),
        "=".repeat(1000),
        "a".repeat(100_000),
        "[a".repeat(2000),
        deep_section,
        long_value,
        many_pairs,
        "a = \u{0}\u{1}\u{7f}".to_string(),
        "\u{feff}a = 1".to_string(),
        "[\u{1f600}]\n\u{1f600} = \u{1f600}".to_string(),
        "a = \\".to_string(),
        "\r\n\r\n".to_string(),
    ];
    for src in cases {
        let outcome = parse(&src);
        // Either answer is fine. A panic, a hang or a stack overflow is
        // not, and none of those would reach this line.
        let _ = outcome.map(|value| value.to_json());
    }
}

/// A section header may reopen a key that already holds a value, and
/// the section replaces it: last writer wins, which is how the dialect
/// treats a repeated key. The canonical port used to keep the value and
/// then raise a host `TypeError` as soon as the section had a key of its
/// own; it now replaces it too, so this is ordinary behaviour rather
/// than a divergence, and `../../test/spec/sections-over-value.tsv`
/// holds all three runtimes to it.
#[test]
fn a_section_header_may_reopen_a_key_that_holds_a_value() {
    assert_default("a=1\n[a]", json!({"a": {}}));
    assert_default("a=1\n[a]\nx=2", json!({"a": {"x": "2"}}));
    assert_default("a=1\n[a.b]\nx=2", json!({"a": {"b": {"x": "2"}}}));
}

/// Neither a long document nor a wide one may cost more than linear
/// time. Both shapes were quadratic once: a value rule that kept a
/// handle on the enclosing map made every key copy it, and a lexer check
/// that buffered the rest of the source re-read the document at every
/// value.
///
/// The ratio, not the clock: the same work at four times the size on the
/// same machine in the same run, so a slow or loaded box cannot make it
/// flaky. The bound is generous because it has to survive a debug build
/// under a loaded CI box; the failure it exists to catch is quadratic,
/// which lands at sixteen.
#[test]
fn a_large_document_costs_linear_time() {
    fn pairs(count: usize) -> String {
        (0..count)
            .map(|index| format!("k{index} = value{index}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    let measure = |count: usize| {
        let source = pairs(count);
        let start = std::time::Instant::now();
        let value = parse(&source).expect("a document of pairs parses");
        assert!(value.to_json().is_object());
        start.elapsed().as_secs_f64()
    };

    // A warm-up parse, so the shared instance is built before the clock
    // starts: building the grammar is what `perf_test.rs` measures.
    let _ = parse("a = 1");
    let small = measure(1000).max(1e-6);
    let large = measure(4000);
    assert!(
        large < small * 8.0,
        "4000 pairs took {large:.3}s against {small:.3}s for 1000: \
         four times the input is costing more than eight times the time"
    );
}

/// A key named `__proto__` is an ordinary key here, as it is in the
/// canonical port, which allocates its nodes without a prototype for
/// exactly this reason. A Rust map has no prototype chain to pollute, so
/// the test pins the VALUE rather than the mechanism.
#[test]
fn prototype_keys_are_ordinary_keys() {
    assert_default("__proto__ = polluted", json!({"__proto__": "polluted"}));
    assert_default("[__proto__]\nx = 1", json!({"__proto__": {"x": "1"}}));
    assert_default("constructor = 1", json!({"constructor": "1"}));
}

/// Keeps the shared option table compiled into this binary too, so a
/// change to it that does not build is caught by every suite.
#[test]
fn the_option_table_is_exhaustive() {
    for name in common::OPTION_NAMES {
        assert_ne!(
            common::options_for(name),
            IniOptions::default(),
            "{name} is in the option table but resolves to the defaults"
        );
    }
}

/// A section header deeper than [`DEPTH_LIMIT`] is refused with the
/// engine's `cancel` code. Every runtime now bounds nesting at the same
/// number, and `../../test/spec/sections-depth-limit.tsv` holds all
/// three to the boundary; this covers the far side of it, where the
/// canonical runtime used to overflow the host stack.
#[test]
fn nesting_past_the_depth_limit_is_refused() {
    let header = |segments: usize| format!("[{}]\nx = 1", vec!["a"; segments].join("."));

    // At the limit, an ordinary document.
    let value = parse(&header(tabnas_ini::DEPTH_LIMIT)).expect("127 segments parse");
    assert!(value.to_json().is_object());

    // One past it, a coded rejection rather than a crash.
    for segments in [tabnas_ini::DEPTH_LIMIT + 1, 1_000, 10_000, 100_000] {
        let error = parse(&header(segments)).expect_err("a header past the depth limit is refused");
        assert_eq!(error.code, "cancel", "at {segments} segments");
    }
}

// --- the option defaults ------------------------------------------------

/// An EMPTY marker list is a choice, not an omission.
///
/// The canonical `_options.comment?.inline?.chars ?? ['#', ';']`
/// defaults only when the caller said nothing, so `chars: []` leaves
/// inline comments active with no character that starts one. Reading
/// `Option<Vec<_>>` as "absent OR empty" put the `#` and `;` defaults
/// back and truncated every value at the first one.
#[test]
fn an_explicitly_empty_inline_marker_list_leaves_no_marker() {
    let empty = IniOptions {
        comment: Some(CommentOptions {
            inline: Some(InlineCommentOptions {
                active: Some(true),
                chars: Some(Vec::new()),
                escape: None,
            }),
        }),
        ..Default::default()
    };

    // Measured against ts/src/ini.ts: {"a":"x;y"} and {"a":"x#y"}.
    assert_parse("a=x;y", &empty, json!({"a": "x;y"}));
    assert_parse("a=x#y", &empty, json!({"a": "x#y"}));
    assert_parse("a=x ;y", &empty, json!({"a": "x ;y"}));

    // The default is still the default, and a list that names a marker
    // still names it.
    assert_parse("a=x;y", &inline_active(), json!({"a": "x"}));
    assert_parse("a=x;y#z", &inline_chars(&[";"]), json!({"a": "x"}));

    // A marker that is the empty STRING starts nothing either, because
    // no source character equals it.
    assert_parse("a=x;y", &inline_chars(&[""]), json!({"a": "x;y"}));
}

/// An inline marker the canonical matcher can never match starts no
/// comment there.
///
/// The value matcher asks `commentCharSet.has(c)` and the string check
/// asks `inlineComment.chars.includes(src[tI])`, both with ONE UTF-16
/// code unit of the source, so a marker of any other length is equal to
/// nothing and starts no comment. Taking its first character instead cut
/// `chars: ["##"]` down to `#`, and promoted an astral marker, which is
/// two code units in JavaScript, to one the scanner could match.
///
/// hoover's `end.fixed` compares the WHOLE string, which is why the
/// same `["##"]` still ends a value in the non-whitespace mode: the two
/// halves are measured side by side below so the boundary is visible.
#[test]
fn an_inline_marker_that_is_not_one_code_unit_starts_no_comment() {
    let marker = |chars: &[&str], whitespace: bool| IniOptions {
        comment: Some(CommentOptions {
            inline: Some(InlineCommentOptions {
                active: Some(true),
                chars: Some(chars.iter().map(|text| text.to_string()).collect()),
                escape: Some(InlineEscapeOptions {
                    backslash: Some(true),
                    whitespace: Some(whitespace),
                }),
            }),
        }),
        ..Default::default()
    };

    // The whitespace mode, where the custom value matcher does the
    // detection. Every row measured against ts/src/ini.ts.
    assert_parse(
        "a=x ## note",
        &marker(&["##"], true),
        json!({"a": "x ## note"}),
    );
    assert_parse(
        "a=x # note",
        &marker(&["##"], true),
        json!({"a": "x # note"}),
    );
    assert_parse(
        "a=x // note",
        &marker(&["//"], true),
        json!({"a": "x // note"}),
    );
    // Two code units in JavaScript, one `char` here.
    assert_parse(
        "a=x \u{1F600} note",
        &marker(&["\u{1F600}"], true),
        json!({"a": "x \u{1F600} note"}),
    );
    // One code unit, and not the one in the source.
    assert_parse(
        "a=x # note",
        &marker(&["\u{e9}"], true),
        json!({"a": "x # note"}),
    );
    // A list that names both still works through the one that matches.
    assert_parse("a=x # note", &marker(&[";;", "#"], true), json!({"a": "x"}));

    // The escape branch reads one code unit too, so a backslash before a
    // marker the set cannot hold is an ordinary backslash.
    assert_parse(
        "a=x \\## note",
        &marker(&["##"], true),
        json!({"a": "x \\## note"}),
    );

    // The string check reads one code unit as well: with no marker the
    // scanner can match, `## note` is trailing text, the quotes are not
    // the value's own, and the whole line is the value.
    assert_parse(
        "a=\"x\" ## note",
        &marker(&["##"], true),
        json!({"a": "\"x\" ## note"}),
    );

    // Without the whitespace mode hoover's `end.fixed` holds the whole
    // string, so the SAME marker does end a value, and a single `#` does
    // not. This half was already right and is measured to keep it so.
    assert_parse("a=x ## note", &marker(&["##"], false), json!({"a": "x"}));
    assert_parse(
        "a=x # note",
        &marker(&["##"], false),
        json!({"a": "x # note"}),
    );
    assert_parse(
        "a=\"x\" ## note",
        &marker(&["##"], false),
        json!({"a": "\"x\""}),
    );
}

/// A continuation string the canonical scanner can never match turns
/// continuation off.
///
/// The canonical test is `c === continuation` for one UTF-16 code unit
/// `c` of the source, so a string of any other length matches nothing.
/// Taking its first character instead made `continuation: "~~"` continue
/// a line that ends in a single `~`, which the canonical implementation
/// does not.
#[test]
fn a_continuation_string_that_is_not_one_code_unit_is_off() {
    let cont = |text: &str| IniOptions {
        multiline: Some(MultilineOptions {
            continuation: Some(text.to_string()),
            indent: None,
        }),
        ..Default::default()
    };

    // One code unit: the line continues. Measured: {"a":"one b = two"}.
    assert_parse(
        "a = one ~\nb = two",
        &cont("~"),
        json!({"a": "one b = two"}),
    );

    // Two: nothing continues, and the `~` is an ordinary character.
    for text in ["~~", "ab", ""] {
        assert_parse(
            "a = one ~\nb = two",
            &cont(text),
            json!({"a": "one ~", "b": "two"}),
        );
    }
}

// --- a single-quoted value is JSON --------------------------------------

/// `JSON.parse` rounds a number literal too large for a double to
/// infinity; `serde_json` refuses it.
///
/// The canonical implementation reads `a = '1e400'` as the NUMBER
/// `Infinity`. Leaving `serde_json`'s refusal to the fallback kept the
/// source text instead, so the value was the string `"1e400"`. A literal
/// too small to represent is not affected: both readers round `1e-400`
/// to zero.
#[test]
fn a_single_quoted_number_too_large_for_a_double_is_infinity() {
    // A literal with no exponent overflows the same way, so one is built
    // rather than written out.
    let spelt_out = format!("a = '1{}'", "0".repeat(400));
    for (src, positive) in [
        ("a = '1e400'", true),
        ("a = '-1e400'", false),
        // JSON whitespace around a top-level value is allowed.
        ("a = ' 1e400 '", true),
        (spelt_out.as_str(), true),
    ] {
        let value = parse(src).unwrap_or_else(|error| panic!("{src:?} did not parse: {error}"));
        let tabnas::Value::Object(entries) = &value else {
            panic!("{src:?} did not parse to an object: {value}")
        };
        match entries.get("a") {
            Some(tabnas::Value::Number(number)) => {
                assert!(number.is_infinite(), "{src:?} gave {number}");
                assert_eq!(
                    number.is_sign_positive(),
                    positive,
                    "{src:?} has the wrong sign"
                );
            }
            other => panic!("{src:?} gave {other:?} rather than a number"),
        }
    }

    // Unaffected neighbours, each measured against the canonical
    // implementation.
    assert_default("a = '1e-400'", json!({"a": 0}));
    assert_default("a = '1'", json!({"a": 1}));
    // Not JSON number syntax, so the text stands, as `JSON.parse` fails
    // on all four.
    for (src, text) in [
        ("a = 'inf'", "inf"),
        ("a = '+1'", "+1"),
        ("a = '01'", "01"),
        ("a = '1.'", "1."),
    ] {
        assert_default(src, json!({ "a": text }));
    }
}

/// A recorded divergence: a number that overflows INSIDE a composite
/// keeps its source text, where the canonical implementation reads the
/// composite with an infinity in it. See `../../DIVERGENCE.md` for the
/// measured table and the reason. It cannot be a shared fixture, because
/// a fixture row has to be green in three runtimes.
#[test]
fn a_number_that_overflows_inside_a_composite_keeps_its_text() {
    assert_default("a = '[1e400]'", json!({"a": "[1e400]"}));
    assert_default(r#"a = '{"b":1e400}'"#, json!({"a": r#"{"b":1e400}"#}));
}

/// A recorded divergence: an escaped LONE SURROGATE in a single-quoted
/// JSON value becomes the replacement character.
///
/// A JavaScript string is UTF-16, so `JSON.parse('"\ud800"')` is an
/// ordinary one-code-unit string. A Rust `String` is scalar values and
/// cannot hold one, so this port substitutes U+FFFD, which is what Go's
/// `encoding/json` does. See `../../DIVERGENCE.md` for the measured
/// table and the reason. It cannot be a shared fixture, because the two
/// runtimes would be asked different questions and both would pass.
#[test]
fn a_lone_surrogate_in_a_single_quoted_value_becomes_the_replacement_character() {
    // The value stays a STRING, an ARRAY and an OBJECT respectively,
    // which is what keeping the JSON source text used to lose.
    assert_default(r#"a = '"\\ud800"'"#, json!({"a": "\u{FFFD}"}));
    assert_default(r#"a = '"\\udfff"'"#, json!({"a": "\u{FFFD}"}));
    assert_default(r#"a = '"\\ud800x"'"#, json!({"a": "\u{FFFD}x"}));
    assert_default(r#"a = '["\\ud800"]'"#, json!({"a": ["\u{FFFD}"]}));
    assert_default(r#"a = '{"k":"\\ud800"}'"#, json!({"a": {"k": "\u{FFFD}"}}));

    // A surrogate PAIR is one character in both runtimes and is left
    // alone: the repair scanner must not touch it.
    assert_default(r#"a = '"\\ud83d\\ude00"'"#, json!({"a": "\u{1F600}"}));

    // A document the repair cannot rescue keeps its text, as before.
    assert_default(r#"a = '["\\ud800",]'"#, json!({"a": r#"["\ud800",]"#}));
}

/// A recorded divergence: a single-quoted JSON value nested past 127
/// levels keeps its source text.
///
/// `serde_json` refuses to recurse further, and `JSON.parse` has no such
/// limit. The refusal is the cap this port wants on untrusted input, for
/// the reason `nesting_past_the_depth_limit_is_refused` gives and at the
/// same number. See `../../DIVERGENCE.md`.
#[test]
fn a_single_quoted_json_value_nested_past_the_depth_limit_keeps_its_text() {
    let nest = |depth: usize| format!("a = '{}{}'", "[".repeat(depth), "]".repeat(depth));

    // 127 is read as JSON, and is an array.
    let value = parse(&nest(127)).expect("127 levels parse");
    let json = value.to_json();
    assert!(
        json.get("a").is_some_and(serde_json::Value::is_array),
        "127 levels should be an array, got {json}"
    );

    // 128 is not, and the value is the source text it arrived as.
    let value = parse(&nest(128)).expect("128 levels parse");
    let json = value.to_json();
    assert_eq!(
        json.get("a").and_then(serde_json::Value::as_str),
        Some(format!("{}{}", "[".repeat(128), "]".repeat(128)).as_str()),
        "128 levels should keep its text"
    );
}

// --- the fixed-token concatenation --------------------------------------

/// A value that starts with a fixed token concatenates that token with
/// the JavaScript `String()` of the rest, and a composite coerces the
/// way ECMA-262 says rather than as JSON.
///
/// An array joins its elements with a comma and flattens, a null or
/// undefined ELEMENT contributes nothing, and an object is
/// `[object Object]`. Rendering the value as JSON instead produced
/// `=[1.0,2.0]` and `={"b":1.0}`, neither of which the canonical
/// implementation can produce for any input.
#[test]
fn a_fixed_token_value_coerces_a_composite_as_javascript_does() {
    // Every row measured against ts/src/ini.ts.
    for (src, want) in [
        ("a = ='[1,2]'", "=1,2"),
        ("a = ='[]'", "="),
        ("a = ='[1,[2,3]]'", "=1,2,3"),
        ("a = ='[[1],[2]]'", "=1,2"),
        ("a = ='[null,true]'", "=,true"),
        (r#"a = ='[1.5,"x"]'"#, "=1.5,x"),
        (r#"a = ='{"b":1}'"#, "=[object Object]"),
        ("a = =='[1,2]'", "==1,2"),
        // The number goes through the ECMA-262 formatter, which
        // switches to exponent form where Rust's does not.
        ("a = ='[1e21,1e-7]'", "=1e+21,1e-7"),
        ("a = ='1e400'", "=Infinity"),
    ] {
        assert_default(src, json!({ "a": want }));
    }

    // Without a leading fixed token the value stays the parsed
    // composite, so the coercion is not reached at all.
    assert_default(r#"a = '[1.5,"x"]'"#, json!({"a": [1.5, "x"]}));
}

#[test]
fn declared_error_codes_carry_their_own_hints() {
    // A declared code without a hint of its own falls back to the engine's
    // hint for an UNKNOWN code, which tells the reader the error is
    // probably a bug in jsonic or a plugin.
    for (src, options, code, want) in [
        (
            "[a]\nx=1\n[a]\ny=2",
            duplicate(Duplicate::Error),
            "duplicate_section",
            "declared more than once",
        ),
        (
            "[a\nb=1",
            IniOptions::default(),
            "unterminated_section",
            "closed with ]",
        ),
    ] {
        let error = parse_with(src, &options).expect_err(src);
        assert_eq!(error.code, code, "rejecting {src:?}");
        assert!(
            error.hint.contains(want) && !error.hint.contains("probably a bug"),
            "{src:?}: {}",
            error.hint
        );
    }
    let config = make().config();
    for code in config.error.keys() {
        assert!(config.hint.contains_key(code), "{code} has no hint");
    }
}
