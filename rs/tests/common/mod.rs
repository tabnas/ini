/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! Shared test scaffolding: the per-fixture option table, and the two
//! conversions between the engine's results and the fixture runner's.
//!
//! Cargo compiles this module into EVERY integration test binary, so a
//! helper only one of them uses reads as dead code in the others. The
//! allow is about that compilation model, not about unused code.

#![allow(dead_code)]

use tabnas_ini::{
    CommentOptions, Duplicate, IniOptions, InlineCommentOptions, InlineEscapeOptions,
    MultilineOptions, SectionOptions,
};
use tabnas_support::{Failure, Value};

/// Every fixture in `test/spec`, named so a renamed or deleted file is a
/// failure rather than silently reduced coverage.
pub const FIXTURES: &[&str] = &[
    "arrays.tsv",
    "bare-key.tsv",
    "basic-values.tsv",
    "empty-input.tsv",
    "happy.tsv",
    "inline-comments-active.tsv",
    "inline-comments-backslash-disabled.tsv",
    "inline-comments-backslash.tsv",
    "inline-comments-custom-chars.tsv",
    "inline-comments-empty-chars.tsv",
    "inline-comments-marker-width.tsv",
    "inline-comments-off.tsv",
    "inline-comments-whitespace-backslash.tsv",
    "inline-comments-whitespace.tsv",
    "inline-comments-with-sections.tsv",
    "key-overwrite.tsv",
    "line-comments.tsv",
    "multiline-backslash.tsv",
    "multiline-both.tsv",
    "multiline-continuation-width.tsv",
    "multiline-escapes.tsv",
    "multiline-indent.tsv",
    "multiline-no-inline.tsv",
    "multiline-with-inline.tsv",
    "numbers-are-strings.tsv",
    "quoted-values.tsv",
    "sections-depth-limit.tsv",
    "sections-duplicate-error.tsv",
    "sections-duplicate-merge.tsv",
    "sections-duplicate-override.tsv",
    "sections-escaped-dots.tsv",
    "sections-over-value.tsv",
    "sections-unterminated.tsv",
    "sections.tsv",
    "value-comment-char-start-inline.tsv",
    "value-comment-char-start.tsv",
    "value-fixed-token-start.tsv",
    "value-keywords.tsv",
];

/// The fixtures that are parsed with something other than the defaults.
/// Keep in step with `tsvOptions` in `go/ini_tsv_test.go` and `OPTIONS`
/// in `ts/test/ini-tsv.test.ts`.
pub const OPTION_NAMES: &[&str] = &[
    "inline-comments-active",
    "inline-comments-backslash",
    "inline-comments-backslash-disabled",
    "inline-comments-custom-chars",
    "inline-comments-empty-chars",
    "inline-comments-marker-width",
    "inline-comments-whitespace",
    "inline-comments-whitespace-backslash",
    "inline-comments-with-sections",
    "multiline-backslash",
    "multiline-both",
    "multiline-continuation-width",
    "multiline-escapes",
    "multiline-indent",
    "multiline-no-inline",
    "multiline-with-inline",
    "sections-duplicate-error",
    "sections-duplicate-override",
    "value-comment-char-start-inline",
];

fn inline(active: bool) -> InlineCommentOptions {
    InlineCommentOptions {
        active: Some(active),
        ..Default::default()
    }
}

fn comment(inline: InlineCommentOptions) -> Option<CommentOptions> {
    Some(CommentOptions {
        inline: Some(inline),
    })
}

/// ini's fixtures carry no `opts` column: a whole FILE is parsed with one
/// option set, named here. A fixture with no entry gets the defaults, so
/// adding one runs it in every runtime without editing a list.
pub fn options_for(name: &str) -> IniOptions {
    let inline_active = || IniOptions {
        comment: comment(inline(true)),
        ..Default::default()
    };
    match name {
        "inline-comments-active"
        | "inline-comments-with-sections"
        | "value-comment-char-start-inline" => inline_active(),
        "inline-comments-custom-chars" => IniOptions {
            comment: comment(InlineCommentOptions {
                active: Some(true),
                chars: Some(vec![";".to_string()]),
                escape: None,
            }),
            ..Default::default()
        },
        // An EMPTY list is a choice, not an absence: inline comments
        // stay active with no character that starts one.
        "inline-comments-empty-chars" => IniOptions {
            comment: comment(InlineCommentOptions {
                active: Some(true),
                chars: Some(Vec::new()),
                escape: None,
            }),
            ..Default::default()
        },
        // A marker is compared as ONE code unit where the value is
        // scanned, so "##" can never start a comment and "é" can, while
        // "à" cannot.
        "inline-comments-marker-width" => IniOptions {
            comment: comment(InlineCommentOptions {
                active: Some(true),
                chars: Some(vec!["##".to_string(), "é".to_string()]),
                escape: Some(InlineEscapeOptions {
                    backslash: None,
                    whitespace: Some(true),
                }),
            }),
            ..Default::default()
        },
        "inline-comments-backslash" => IniOptions {
            comment: comment(InlineCommentOptions {
                active: Some(true),
                chars: None,
                escape: Some(InlineEscapeOptions {
                    backslash: Some(true),
                    whitespace: None,
                }),
            }),
            ..Default::default()
        },
        "inline-comments-backslash-disabled" => IniOptions {
            comment: comment(InlineCommentOptions {
                active: Some(true),
                chars: None,
                escape: Some(InlineEscapeOptions {
                    backslash: Some(false),
                    whitespace: None,
                }),
            }),
            ..Default::default()
        },
        "inline-comments-whitespace" => IniOptions {
            comment: comment(InlineCommentOptions {
                active: Some(true),
                chars: None,
                escape: Some(InlineEscapeOptions {
                    backslash: None,
                    whitespace: Some(true),
                }),
            }),
            ..Default::default()
        },
        "inline-comments-whitespace-backslash" => IniOptions {
            comment: comment(InlineCommentOptions {
                active: Some(true),
                chars: None,
                escape: Some(InlineEscapeOptions {
                    backslash: Some(true),
                    whitespace: Some(true),
                }),
            }),
            ..Default::default()
        },
        "sections-duplicate-override" => IniOptions {
            section: Some(SectionOptions {
                duplicate: Duplicate::Override,
            }),
            ..Default::default()
        },
        "sections-duplicate-error" => IniOptions {
            section: Some(SectionOptions {
                duplicate: Duplicate::Error,
            }),
            ..Default::default()
        },
        "multiline-backslash" | "multiline-no-inline" => IniOptions {
            multiline: Some(MultilineOptions::default()),
            ..Default::default()
        },
        "multiline-indent" => IniOptions {
            multiline: Some(MultilineOptions {
                continuation: Some(String::new()),
                indent: Some(true),
            }),
            ..Default::default()
        },
        "multiline-both" => IniOptions {
            multiline: Some(MultilineOptions {
                continuation: Some("\\".to_string()),
                indent: Some(true),
            }),
            ..Default::default()
        },
        // A continuation of any length but one code unit continues
        // nothing.
        "multiline-continuation-width" => IniOptions {
            multiline: Some(MultilineOptions {
                continuation: Some("~~".to_string()),
                indent: None,
            }),
            ..Default::default()
        },
        "multiline-with-inline" => IniOptions {
            multiline: Some(MultilineOptions::default()),
            comment: comment(inline(true)),
            ..Default::default()
        },
        "multiline-escapes" => IniOptions {
            multiline: Some(MultilineOptions::default()),
            comment: comment(InlineCommentOptions {
                active: Some(true),
                chars: None,
                escape: Some(InlineEscapeOptions {
                    backslash: Some(true),
                    whitespace: None,
                }),
            }),
            ..Default::default()
        },
        _ => IniOptions::default(),
    }
}

/// Normalize a JSON tree for comparison: a whole-valued double is
/// rendered as an integer, so a value the engine carries as `6.0` and a
/// literal written `6` compare equal. serde_json distinguishes the two
/// representations; the parsed VALUE is the same one TypeScript and Go
/// produce, and the shared fixture runner compares numerically for this
/// reason.
pub fn norm(mut value: serde_json::Value) -> serde_json::Value {
    fn walk(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Number(number) => {
                if let Some(float) = number.as_f64() {
                    if float.fract() == 0.0 && float.abs() < 9.007_199_254_740_992e15 {
                        *value = serde_json::Value::from(float as i64);
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(walk),
            serde_json::Value::Object(entries) => entries.values_mut().for_each(walk),
            _ => {}
        }
    }
    walk(&mut value);
    value
}

/// The engine's parse result as the fixture runner's value, flattened
/// through JSON exactly as the Go runner's `jsonFlatten` does.
pub fn value_of(value: tabnas::Value) -> Value {
    Value::from(value.to_json())
}

/// The engine's error as the runner's failure: the code the `ERROR:`
/// cells pin, with the rendered message and position carried along for a
/// fixture that pins those instead.
pub fn failure_of(error: tabnas::TabnasError) -> Failure {
    let failure = Failure::new(error.code.clone()).with_message(error.to_string());
    if error.row > 0 && error.col > 0 {
        failure.at(error.row, error.col)
    } else {
        failure
    }
}
