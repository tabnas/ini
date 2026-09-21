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

/// The second recorded divergence: a section header may reopen a key
/// that already holds a value, and this port replaces the value with the
/// section, as the Go port does. The canonical port keeps the value and
/// then raises a host `TypeError` as soon as the section has a key of
/// its own. `../../DIVERGENCE.md` carries the measured table.
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

/// The one recorded divergence from the canonical TypeScript: a section
/// header deeper than [`DEPTH_LIMIT`] is refused with the engine's
/// `cancel` code, where TypeScript and Go accept it. See
/// `../../DIVERGENCE.md` for the measured table and the reason. It
/// cannot be a shared fixture, because a fixture row has to be green in
/// three runtimes.
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
