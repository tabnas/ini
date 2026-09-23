// Copyright (c) 2021-2026 Richard Rodger, MIT License

// The engine's error carries a code, position, hint and a formatted
// report, so it is large by design and `Result<_, TabnasError>` trips
// clippy's `result_large_err`. The engine allows the lint at its own
// crate root for the same reason, and so do tabnas-json and
// tabnas-jsonic; boxing here instead would make `parse` return a
// different shape from `Tabnas::parse` and from the other two ports.
#![allow(clippy::result_large_err)]

//! An INI grammar plugin for the `tabnas` parsing engine.
//!
//! INI is the oldest configuration format still in daily use, and the
//! least specified: every implementation is a dialect. This one layers
//! on the relaxed-JSON grammar from
//! [`tabnas-jsonic`](https://github.com/tabnas/jsonic) and lexes keys,
//! values and section-header segments with
//! [`tabnas-hoover`](https://github.com/tabnas/hoover).
//!
//! ```text
//! [server]
//! host = localhost
//! port = 8080
//!
//! => {"server":{"host":"localhost","port":"8080"}}
//! ```
//!
//! This is the Rust port; the TypeScript package (`ts/src/ini.ts`) is
//! canonical and the Go port (`go/ini.go`) is the nearer structural
//! model. The three are held together by the shared `test/spec/*.tsv`
//! fixtures, which every runtime discovers and runs.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use indexmap::IndexMap;
use serde_json::json;
use tabnas::{
    ActionError, AltSpec, Context, GrammarError, GrammarSpec, ImperativeLexMatcher, LexCheckResult,
    Lexer, Options, Plugin, PluginError, Rule, RuleState, Tabnas, Token, Value,
};
use tabnas_hoover::{
    hoover, Block, Consume, EndSpec, HooverOptions, HooverRuleFilter, HooverRuleSpec, StartSpec,
};

/// The README's Rust examples run as doctests, so a stale one fails the
/// gate rather than misleading the reader. Its `text`, `toml` and `bash`
/// fences are skipped; rustdoc runs only the `rust` ones.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_examples {}

/// This crate's version. It MUST equal `ts/package.json` "version": the
/// release orchestrator rewrites both, and `tests/version_test.rs` fails
/// the build if they drift. Mirrors `VERSION` in `ts/src/ini.ts` and
/// `const VERSION` in `go/ini.go`.
pub const VERSION: &str = "0.5.7";

/// The engine's error, re-exported under this crate's name.
pub use tabnas::TabnasError as IniError;

// --- BEGIN EMBEDDED ini-grammar.jsonic ---
const GRAMMAR_TEXT: &str = r#"
# INI Grammar Definition
# Parsed by a standard Jsonic instance and passed to jsonic.grammar()
# Function references (@ prefixed) are resolved against the refs map

{
  options: rule: { start: ini exclude: jsonic }
  options: lex: { emptyResult: {} }
  options: fixed: token: { '#EQ': '=' '#DOT': '.' '#OB': null '#CB': null '#CL': null }
  options: line: { check: '@line-check' }
  options: number: { lex: false }
  options: string: { lex: true chars: QUOTE_CHARS abandon: true }
  options: text: { lex: false }
  # Declared error codes. The CODE is the cross-runtime contract; the message
  # text is not (see AGENTS.md). Both runtimes read this one block, so the two
  # catalogues cannot drift. Keys stay alphabetical: admin's descriptor
  # generator compares the TS and Go extractions and sorts only one side.
  options: error: {
    duplicate_section: 'duplicate section header: [{section}]'
    unterminated_section: 'unterminated section header: [{src}'
  }
  options: comment: def: {
    hash: { eatline: true }
    slash: null
    multi: null
    semi: { line: true start: ';' lex: true eatline: true }
  }

  rule: ini: open: [
    { s: '#OS' p: table b: 1 }
    { s: ['#HK #ST #VL' '#EQ'] p: table b: 2 }
    { s: ['#HV' '#OS'] p: table b: 2 }
    { s: ['#HK #ST #VL'] p: table b: 1 }
    { s: '#ZZ' }
  ]

  rule: table: open: [
    # Raised when the table's own before-open handler saw a section path it
    # had already declared and section.duplicate is 'error'. It flags the
    # rule rather than raising there, because a state action cannot raise a
    # coded error in both runtimes: TS can return a bad token from bo, Go
    # discards the return value, and a ctx error set in bo is overwritten by
    # alternate matching. An error ALTERNATE is the path both runtimes share.
    { c: '@is-duplicate-section' e: '@duplicate-section' }
    { s: '#OS' p: dive }
    { s: ['#HK #ST #VL' '#EQ'] p: map b: 2 }
    { s: ['#HV' '#OS'] p: map b: 2 }
    { s: ['#HK #ST #VL'] p: map b: 1 }
    { s: '#CS' p: map }
    { s: '#ZZ' }
  ]
  rule: table: close: [
    { s: '#OS' r: table b: 1 g: end }
    { s: '#CS' r: table a: '@table-close-dive' g: close }
    { s: '#ZZ' g: end }
  ]

  rule: dive: open: [
    { s: ['#DK' '#DOT'] a: '@dive-push' p: dive }
    { s: '#DK' a: '@dive-push' }
  ]
  rule: dive: close: [
    { s: '#CS' b: 1 g: close }
    # A section header lives on one line, so anything other than the closing
    # bracket here means the header was never closed. Unconditional (no s
    # key), so it matches only after the '#CS' alternate above has been
    # tried, and raises unterminated_section rather than the engine's
    # generic 'unexpected'.
    { e: '@dive-unterminated' }
  ]

  rule: map: open: {
    alts: [
      { s: ['#HK #ST #VL' '#EQ'] c: '@is-table-parent' p: pair b: 2 }
      { s: ['#HK #ST #VL'] c: '@is-table-parent' p: pair b: 1 }
    ]
    inject: { append: true }
  }
  rule: map: close: [
    { s: '#OS' b: 1 g: end }
    { s: '#ZZ' g: end }
  ]

  rule: pair: open: [
    { s: ['#HK #ST #VL' '#EQ'] c: '@is-table-grandparent' p: val a: '@pair-key-eq' }
    { s: ['#HK #ST #VL'] c: '@is-table-grandparent' a: '@pair-key-bool' }
  ]
  rule: pair: close: [
    { s: ['#HK #ST #VL' '#CL'] c: '@is-table-grandparent' e: '@pair-close-err' }
    { s: ['#HK #ST #VL'] b: 1 r: pair g: comma }
    { s: '#OS' b: 1 g: end }
  ]
}
"#;
// --- END EMBEDDED ini-grammar.jsonic ---

/// The quote characters INI values may be wrapped in. The grammar text
/// carries the placeholder `QUOTE_CHARS`, which every runtime replaces
/// with this before installing the document, exactly as the TypeScript
/// `grammarDef.options.string.chars` assignment does.
const QUOTE_CHARS: &str = "'\"";

/// The hoover matcher band. Lower than nothing else here; the custom
/// multiline matcher sits just below it so it sees a value first.
const HOOVER_ORDER: f64 = 8.5e6;
const MULTILINE_ORDER: f64 = 8.4e6;

/// The in-value probe's band. Below 1e6, so it runs before every
/// built-in matcher family and therefore before any `check` hook.
const PROBE_ORDER: f64 = 0.5e6;

const PROBE_REF: &str = "@ini-inval-probe";
const MULTILINE_REF: &str = "@ini-multiline";
const LINE_CHECK_REF: &str = "@line-check";
const COMMENT_CHECK_REF: &str = "@ini-comment-check";
const TEXT_CHECK_REF: &str = "@ini-text-check";
const STRING_CHECK_REF: &str = "@ini-string-check";

/// The plugin name, and the namespace `use_plugin` files its option bag
/// under.
pub const PLUGIN_NAME: &str = "ini";

/// The deepest a document may nest before the parse is refused with the
/// engine's `cancel` code.
///
/// The engine parses iteratively, but displaying, converting or dropping
/// a `Value` walks the tree with the call stack, and a section header a
/// few thousand segments deep ended the process: an abort, not an error.
/// The TypeScript and Go ports have no limit, so the refusal is a
/// recorded divergence (`../DIVERGENCE.md`). The number is the one
/// `tabnas-jsonic` and `tabnas-json` use, so every crate in the family
/// bounds nesting alike; `dive` is counted beside `map` and `list`,
/// because a section header is where an INI document nests.
pub const DEPTH_LIMIT: usize = 127;

// ---------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------

/// How a repeated section header is treated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Duplicate {
    /// Combine the keys of every occurrence; the last value wins for a
    /// repeated key. The default.
    #[default]
    Merge,
    /// The last occurrence replaces the earlier ones entirely.
    Override,
    /// A repeated header is rejected with the `duplicate_section` code.
    Error,
}

/// Multiline value continuation.
///
/// `Some(MultilineOptions::default())` is the TypeScript `multiline:
/// true`: backslash continuation on, indent continuation off. `None` is
/// multiline off.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MultilineOptions {
    /// The character that, before a newline, continues the value. `None`
    /// means the default `\`; `Some("")` disables it, which is the
    /// TypeScript `continuation: false`.
    pub continuation: Option<String>,
    /// When true, a line starting with whitespace continues the previous
    /// value even with no continuation character.
    pub indent: Option<bool>,
}

/// Section header handling.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SectionOptions {
    /// How a repeated section header is treated.
    pub duplicate: Duplicate,
}

/// Comment handling. Line comments are always on; this controls the
/// inline ones.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommentOptions {
    /// Inline comment behaviour.
    pub inline: Option<InlineCommentOptions>,
}

/// Inline comment behaviour.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InlineCommentOptions {
    /// Whether inline comments are active. Default: false.
    pub active: Option<bool>,
    /// The characters that start an inline comment. Default: `#` and `;`.
    pub chars: Option<Vec<String>>,
    /// How a literal comment character is written in a value.
    pub escape: Option<InlineEscapeOptions>,
}

/// Escape mechanisms for a literal comment character inside a value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InlineEscapeOptions {
    /// Allow `\;` and `\#` to produce a literal `;` and `#`. Default: true.
    pub backslash: Option<bool>,
    /// Require whitespace before a comment character for it to start a
    /// comment. Default: false.
    pub whitespace: Option<bool>,
}

/// The INI plugin options.
///
/// Every field is optional and every default matches the canonical
/// TypeScript `IniOptions`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IniOptions {
    /// Multiline value continuation. `None` is off.
    pub multiline: Option<MultilineOptions>,
    /// Section header handling.
    pub section: Option<SectionOptions>,
    /// Comment handling.
    pub comment: Option<CommentOptions>,
}

impl IniOptions {
    /// Options with every default in force, the same instance
    /// `IniOptions::default()` gives.
    pub fn new() -> Self {
        Self::default()
    }

    /// Turn multiline continuation on with its defaults, the TypeScript
    /// `multiline: true`.
    pub fn with_multiline(mut self) -> Self {
        self.multiline = Some(MultilineOptions::default());
        self
    }

    /// Turn inline comments on with their defaults.
    pub fn with_inline_comments(mut self) -> Self {
        let inline = self
            .comment
            .get_or_insert_with(CommentOptions::default)
            .inline
            .get_or_insert_with(InlineCommentOptions::default);
        inline.active = Some(true);
        self
    }

    /// Set how a repeated section header is treated.
    pub fn with_duplicate(mut self, duplicate: Duplicate) -> Self {
        self.section = Some(SectionOptions { duplicate });
        self
    }
}

/// Every option with its default applied, computed once per install.
/// Mirrors the Go `resolved` struct and the TypeScript closure locals.
#[derive(Clone, Debug)]
struct Resolved {
    multiline: bool,
    /// `None` when continuation is disabled.
    continuation: Option<char>,
    indent: bool,
    dup_section: Duplicate,
    inline_active: bool,
    inline_chars: Vec<char>,
    inline_char_str: Vec<String>,
    esc_backslash: bool,
    esc_whitespace: bool,
}

impl Resolved {
    fn is_inline_char(&self, c: char) -> bool {
        self.inline_chars.contains(&c)
    }
}

/// The single UTF-16 code unit an option string names, or `None` when
/// the string is not exactly one.
///
/// Both places this is used compare one code unit of the SOURCE against
/// each whole option string: the canonical continuation test is
/// `c === continuation`, and the canonical comment test is
/// `commentCharSet.has(c)`, with `c` a `src[i]`. A string of any other
/// length can never be equal to one, so the feature it configures is
/// simply off, and an astral character (two code units in JavaScript)
/// is off with it. Taking the first character instead made
/// `continuation: "~~"` continue a line ending in one `~`, and made
/// `chars: ["##"]` cut a value at the first `#`.
fn one_code_unit(text: &str) -> Option<char> {
    let mut characters = text.chars();
    let first = characters.next()?;
    (characters.next().is_none() && first.len_utf16() == 1).then_some(first)
}

fn resolve(options: &IniOptions) -> Resolved {
    let mut resolved = Resolved {
        multiline: false,
        continuation: None,
        indent: false,
        dup_section: Duplicate::Merge,
        inline_active: false,
        inline_chars: vec!['#', ';'],
        inline_char_str: vec!["#".to_string(), ";".to_string()],
        esc_backslash: true,
        esc_whitespace: false,
    };

    if let Some(multiline) = &options.multiline {
        resolved.multiline = true;
        resolved.continuation = match multiline.continuation.as_deref() {
            None => Some('\\'),
            Some(text) => one_code_unit(text),
        };
        resolved.indent = multiline.indent.unwrap_or(false);
    }

    if let Some(section) = &options.section {
        resolved.dup_section = section.duplicate;
    }

    if let Some(inline) = options.comment.as_ref().and_then(|c| c.inline.as_ref()) {
        resolved.inline_active = inline.active.unwrap_or(false);
        // `None` is "the caller said nothing", and only that takes the
        // default. An EMPTY list is a choice: the canonical
        // `_options.comment?.inline?.chars ?? ['#', ';']` defaults on
        // absence alone, so `chars: []` leaves no inline comment
        // character at all and `a=x;y` keeps its semicolon.
        if let Some(chars) = inline.chars.as_ref() {
            // The WHOLE string is what hoover's `end.fixed` and its
            // escape map compare, exactly as the canonical
            // `eolEndFixed.push(...inlineComment.chars)` does, so a
            // marker of any length still terminates a hoovered span.
            resolved.inline_char_str = chars.clone();
            // The custom value matcher and the string check compare ONE
            // code unit instead (`commentCharSet.has(c)` and
            // `inlineComment.chars.includes(src[tI])`, both against a
            // `src[i]`), so an entry that is not exactly one UTF-16 code
            // unit can never start a comment there. Taking the first
            // character instead truncated `chars: ["##"]` to `#` and cut
            // `a=x ## note` short, and promoted an astral marker the
            // canonical set can never hold a single unit of.
            resolved.inline_chars = chars
                .iter()
                .filter_map(|text| one_code_unit(text))
                .collect();
        }
        if let Some(escape) = &inline.escape {
            resolved.esc_backslash = escape.backslash.unwrap_or(true);
            resolved.esc_whitespace = escape.whitespace.unwrap_or(false);
        }
    }

    resolved
}

// ---------------------------------------------------------------------
// The in-value probe
// ---------------------------------------------------------------------

// The four `check` hooks the canonical port installs all ask the same
// question: where is the parser, right now? The TypeScript hooks read
// `lex.ctx.rule`, and the Go ones read `lex.Ctx.Rule`. The Rust engine
// hands a check only the live `Lexer`, which carries no rule, so the
// answer is recorded one step earlier instead: a custom matcher in the
// band BELOW every built-in family (see `PROBE_ORDER`) receives the
// rule, writes what it sees here, and matches nothing. Every check that
// runs afterwards, in the same `next_token` call on the same thread,
// reads it back.
//
// Thread-local rather than instance state because a `Tabnas` is shared
// across threads by `parse`, and because the flags are written and read
// inside one lexer call: nothing outlives the token being lexed.
thread_local! {
    /// The lexer is inside a `val` rule, of any state or parent. This is
    /// the question `@line-check` asks.
    static IN_VAL: Cell<bool> = const { Cell::new(false) };
    /// The lexer is inside the OPEN `val` rule of a `key = value` pair,
    /// the TypeScript `inValue(lex)`.
    static IN_VALUE: Cell<bool> = const { Cell::new(false) };
}

fn in_val_rule(rule: &Rule) -> bool {
    rule.name.as_str() == "val"
}

fn in_value_position(rule: &Rule) -> bool {
    in_val_rule(rule)
        && rule.state == RuleState::Open
        && rule.parent_rule.as_ref().is_some_and(|parent| {
            let name = parent.name.as_str();
            name == "pair" || name == "elem"
        })
}

/// What the string check needs from the resolved configuration. Captured
/// after the grammar document is installed, because the document is what
/// sets `string.chars`.
#[derive(Debug, Default)]
struct StringCheckConfig {
    quotes: Vec<char>,
    escape_char: char,
}

// ---------------------------------------------------------------------
// Value helpers
// ---------------------------------------------------------------------

/// Assign a rule's node.
///
/// A pushed or replaced rule SHARES its parent's `Rc<RefCell<Value>>`,
/// so writing through `rule.node.borrow_mut()` would overwrite the
/// parent's node too. An assignment, `r.node = ...` in the canonical
/// TypeScript, installs a fresh cell instead, which is what this does.
fn set_node(rule: &mut Rule, value: Value) {
    rule.node = Rc::new(RefCell::new(value));
}

/// The JavaScript `String(value)` of a parsed value, for the fixed-token
/// concatenation in the `val` after-close hook and for a key whose token
/// carries a non-string value.
///
/// A composite reaches this only from the fixed-token concatenation, and
/// only ever as what the JSON reader made of a single-quoted value:
/// `a = ='[1,2]'` is the array `[1, 2]` with a `=` in front of its
/// `String()`. So the two composite arms are the ones ECMA-262 applies
/// to an ordinary array and an ordinary object.
///
/// Rendering the composite as JSON instead is the defect this replaces.
/// It put `=[1.0,2.0]` where the canonical implementation puts `=1,2`,
/// and `={"b":1.0}` where it puts `=[object Object]`.
fn js_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Text(text) => text.string.clone(),
        Value::Undefined => "undefined".to_string(),
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => js_number_to_string(*number),
        Value::Array(items) => js_array_string(items),
        // `Object.prototype.toString`. The object came from the JSON
        // reader, so it carries the ordinary prototype and its
        // `String()` is this constant. A jsonic object, allocated with
        // no prototype, would throw a `TypeError` instead, and cannot
        // reach here: INI nulls the `#OB` token, so no value in this
        // dialect opens a map.
        _ => "[object Object]".to_string(),
    }
}

/// `Array.prototype.toString`, which is `join(',')` with no separator
/// argument (ECMA-262 23.1.3.17 and 23.1.3.34): the elements are joined
/// with a comma, `null` and `undefined` contribute the empty string, and
/// every other element is converted by the rules above. A nested array
/// therefore flattens, so `[1,[2,3]]` is `1,2,3` and `[]` is empty.
///
/// The empty string for a null belongs to `join`, not to `String`: a
/// null VALUE is still `"null"`, and only a null ELEMENT disappears.
///
/// The recursion needs no depth bound. These values come from
/// `serde_json`, whose reader refuses to nest past 128, and a parsed
/// value is a tree, so the walk always terminates.
fn js_array_string(items: &[Value]) -> String {
    let mut joined = String::new();
    for (index, item) in items.iter().enumerate() {
        if 0 < index {
            joined.push(',');
        }
        match item {
            Value::Null | Value::Undefined => {}
            other => joined.push_str(&js_string(other)),
        }
    }
    joined
}

/// A double as JavaScript spells it: ECMA-262 6.1.6.1.20.
///
/// Neither Rust's shortest float formatter nor Go's
/// `FormatFloat('f', -1, 64)` is that algorithm. Both break an exact
/// decimal midpoint away from zero where the specification takes the
/// even digit, and neither switches to exponent form at 1e21 or 1e-7.
/// This is the implementation the other tabnas Rust ports carry, copied
/// rather than rewritten.
///
/// `number.lex` is off in INI, so a double can only arrive here from a
/// single-quoted value the JSON reader claimed, but that is enough for a
/// document to notice the difference.
fn js_number_to_string(number: f64) -> String {
    if number.is_nan() {
        return "NaN".to_string();
    }
    // Catches -0.0 as well: JavaScript spells both zeros "0".
    if number == 0.0 {
        return "0".to_string();
    }
    if number < 0.0 {
        return format!("-{}", js_number_to_string(-number));
    }
    if number.is_infinite() {
        return "Infinity".to_string();
    }

    // The specification wants the shortest digit string `s` that
    // round-trips (length `k`), and `n`, the position of the decimal
    // point relative to it. Rust's `{:e}` yields digits of exactly that
    // shortest length.
    let shortest = format!("{number:e}");
    let shortest_k = shortest
        .split_once('e')
        .map(|(mantissa, _)| mantissa.chars().filter(char::is_ascii_digit).count())
        .expect("a finite f64 always formats with an exponent");

    // Re-render to that same length to settle a tie. Where two digit
    // strings of length `k` are equally close to `number`, the
    // specification takes the one ending in an even digit; Rust's
    // shortest form does not, but its exactly-rounded fixed-precision
    // form does.
    let exponential = format!("{:.*e}", shortest_k - 1, number);
    let (mantissa, exponent) = exponential
        .split_once('e')
        .expect("a finite f64 always formats with an exponent");
    // Rounding can leave trailing zeros (and, on a carry, one digit too
    // many); dropping them keeps `s` shortest, which is what `k` means.
    let digits = mantissa
        .chars()
        .filter(|digit| *digit != '.')
        .collect::<String>();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let k = digits.len() as i32;
    let n = exponent
        .parse::<i32>()
        .expect("a formatted exponent is an integer")
        + 1;

    // The four cases of the specification, in its order. The range
    // bounds are `k <= n <= 21`, `0 < n <= 21` and `-6 < n <= 0`.
    if (k..=21).contains(&n) {
        // Integral, with n - k trailing zeros to restore.
        let mut text = digits.to_string();
        text.push_str(&"0".repeat((n - k) as usize));
        text
    } else if (1..=21).contains(&n) {
        let point = n as usize;
        format!("{}.{}", &digits[..point], &digits[point..])
    } else if (-5..=0).contains(&n) {
        format!("0.{}{}", "0".repeat(-n as usize), digits)
    } else {
        // Exponent form. `n - 1` is never 0 here, so the sign is never
        // "+0".
        let sign = if n - 1 < 0 { '-' } else { '+' };
        let power = (n - 1).abs();
        if k == 1 {
            format!("{digits}e{sign}{power}")
        } else {
            format!("{}.{}e{sign}{power}", &digits[..1], &digits[1..])
        }
    }
}

fn object_get(value: &Value, key: &str) -> Option<Value> {
    match value {
        Value::Object(entries) => entries.get(key).cloned(),
        Value::MapRef(map) => map.value.get(key).cloned(),
        _ => None,
    }
}

fn object_insert(value: &mut Value, key: String, entry: Value) {
    match value {
        Value::Object(entries) => {
            Arc::make_mut(entries).insert(key, entry);
        }
        Value::MapRef(map) => {
            Arc::make_mut(map).value.insert(key, entry);
        }
        _ => {
            let mut entries = IndexMap::new();
            entries.insert(key, entry);
            *value = Value::object(entries);
        }
    }
}

/// Take the value at `key` OUT of a container, leaving the key in place
/// with no value.
///
/// The slot is kept rather than removed so the key holds its position:
/// an `IndexMap` remembers insertion order, and a remove-then-insert
/// would move a section to the end of its parent every time it was
/// reopened.
///
/// Taking rather than cloning is what keeps a document of many sections
/// linear. A container is an `Arc`, so a clone leaves two owners and the
/// next write copies the whole map; a document of 5,000 sections then
/// copied a 5,000-key root once per section.
fn object_take(node: &mut Value, key: &str) -> Option<Value> {
    match node {
        Value::Object(entries) => Arc::make_mut(entries)
            .get_mut(key)
            .map(|slot| std::mem::replace(slot, Value::Undefined)),
        Value::MapRef(map) => Arc::make_mut(map)
            .value
            .get_mut(key)
            .map(|slot| std::mem::replace(slot, Value::Undefined)),
        _ => None,
    }
}

fn is_object(value: &Value) -> bool {
    matches!(value, Value::Object(_) | Value::MapRef(_))
}

fn empty_object() -> Value {
    Value::object(IndexMap::new())
}

/// The string a token contributes as a key: its decoded value when it
/// has one, otherwise its source. Mirrors the Go `tokenString`.
fn token_string(token: &Token) -> String {
    if token.is_no_token() {
        return String::new();
    }
    match &token.val {
        Value::String(text) => text.clone(),
        Value::Text(text) => text.string.clone(),
        Value::Undefined => token.src.as_str().to_string(),
        other => js_string(other),
    }
}

/// The key a BARE line declares, or `None` when its token carries no
/// string at all.
///
/// The canonical `@pair-key-bool` reads `r.o0.val` and sets the key only
/// when `'string' === typeof key`, so a line holding a value keyword and
/// nothing else declares nothing: both value lexers resolve a keyword
/// that is the whole span, and the token then carries a boolean or null.
fn bool_key(token: &Token) -> Option<String> {
    match &token.val {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Text(text) if !text.string.is_empty() => Some(text.string.clone()),
        _ => None,
    }
}

/// Pick the token an error alternate should mark.
///
/// An alternate with no token sequence matches zero tokens, so the
/// rule's open slot holds nothing from the source and the lookahead
/// token is the one that actually stopped the parse. The canonical
/// TypeScript reads `r.o0` then `ctx.t0`, and the Go port adds the
/// NOTOKEN sentinel as a last resort.
///
/// All three fallbacks are here, and the last one is reached more often
/// in this port than in the others: the engine fills its lookahead
/// buffer to the length the alternate under test asks for, and an
/// alternate with no token sequence asks for none, so `context.t0()` is
/// empty exactly where the other two runtimes have a positionless token
/// in it. A FRESH sentinel is built rather than the rule's, so marking
/// it cannot scribble on a shared object, and the diagnostic it renders
/// is the one TypeScript and Go render: the code, the message with its
/// `{section}` detail, and the start of the source.
fn alt_err_token(rule: &Rule, context: &Context) -> Option<Token> {
    if rule.os() > 0 {
        if let Some(token) = rule.o0().filter(|token| !token.is_no_token()) {
            return Some(token.clone());
        }
    }
    context.t0().cloned().or_else(|| Some(Token::no_token()))
}

/// The dive path recorded on a rule's `u` bag, as a vector of segments.
fn dive_of(bag: Option<&Value>) -> Option<Vec<String>> {
    match bag? {
        Value::Array(items) => Some(items.iter().map(js_string).collect()),
        _ => None,
    }
}

fn dive_value(dive: &[String]) -> Value {
    Value::array(dive.iter().cloned().map(Value::String).collect())
}

// ---------------------------------------------------------------------
// The grammar document
// ---------------------------------------------------------------------

/// Turn every whole-valued double in a parsed grammar into a JSON
/// integer. See the call site for why.
fn integralize(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Number(number) => {
            if let Some(float) = number.as_f64() {
                if float.fract() == 0.0 && float.abs() < 9.007_199_254_740_992e15 {
                    *value = serde_json::Value::from(float as i64);
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(integralize),
        serde_json::Value::Object(entries) => {
            entries.values_mut().for_each(integralize);
        }
        _ => {}
    }
}

/// Parse the embedded grammar text and turn it into the JSON document
/// the engine installs.
///
/// The grammar is AUTHORED in jsonic, so it can carry the comments that
/// explain it, and every runtime parses it with a stock jsonic instance
/// before installing it: TypeScript in `new Tabnas().use(jsonic).parse`,
/// Go in `jsonic.Make().Parse`, and this. One grammar, three readers.
fn grammar_document(resolved: &Resolved) -> Result<serde_json::Value, GrammarError> {
    static PARSED: OnceLock<Result<serde_json::Value, String>> = OnceLock::new();
    let parsed = PARSED
        .get_or_init(|| {
            tabnas_jsonic::make()
                .parse(GRAMMAR_TEXT)
                .map(|value| value.to_json())
                .map_err(|error| error.to_string())
        })
        .clone()
        .map_err(GrammarError)?;

    let mut document = parsed;
    // jsonic parses every number as a double, and `1` then reaches the
    // engine as the JSON `1.0`, which its integer fields refuse. The
    // other two runtimes never see this: JavaScript has one number type
    // and Go's decoder hands back a float the conversion code rounds.
    // Round whole doubles back to integers so `b: 1` means what it says.
    integralize(&mut document);
    let options = document
        .get_mut("options")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| GrammarError("tabnas-ini: the grammar has no `options` table".into()))?;

    // The grammar text carries `QUOTE_CHARS` as a placeholder, exactly
    // as the TypeScript and Go ports do, and every runtime substitutes
    // the real characters before installing the document.
    options
        .entry("string")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| GrammarError("tabnas-ini: options.string must be a table".into()))?
        .insert("chars".into(), json!(QUOTE_CHARS));

    // The three checks the canonical port installs through
    // `options.config.modify` callbacks. The Rust engine has no
    // load-time config modifier that can hold a live closure reachable
    // from a lexer check, so they are declared here as ordinary
    // serialized `check` references, beside the `line.check` the grammar
    // text already names. Same hooks, same order, named rather than
    // assigned.
    for (family, reference) in [
        ("comment", COMMENT_CHECK_REF),
        ("text", TEXT_CHECK_REF),
        ("string", STRING_CHECK_REF),
    ] {
        options
            .entry(family)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| GrammarError(format!("tabnas-ini: options.{family} must be a table")))?
            .insert("check".into(), json!(reference));
    }

    // The in-value probe, and the custom value matcher when the options
    // call for one. Both are ordinary serialized matcher entries.
    let mut matchers = json!({
        "iniprobe": { "order": PROBE_ORDER, "make": PROBE_REF },
    });
    if resolved.multiline || (resolved.inline_active && resolved.esc_whitespace) {
        matchers.as_object_mut().expect("a literal object").insert(
            "multiline".into(),
            json!({ "order": MULTILINE_ORDER, "make": MULTILINE_REF }),
        );
    }
    options
        .entry("lex")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| GrammarError("tabnas-ini: options.lex must be a table".into()))?
        .insert("match".into(), matchers);

    Ok(document)
}

// ---------------------------------------------------------------------
// The plugin
// ---------------------------------------------------------------------

/// Install the INI grammar on an engine that ALREADY carries jsonic.
///
/// Order matters: jsonic supplies the `val`, `map` and `pair` rules this
/// grammar extends, and hoover refuses an engine with no `val`. Applying
/// jsonic here when it is absent would silently accept the wrong order,
/// so the missing-grammar case is reported instead.
///
/// ```
/// let mut parser = tabnas_jsonic::make();
/// tabnas_ini::ini(&mut parser, &tabnas_ini::IniOptions::default())?;
/// let value = parser.parse("a = 1")?;
/// assert_eq!(value.to_string(), r#"{"a":"1"}"#);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn ini(parser: &mut Tabnas, options: &IniOptions) -> Result<(), IniPluginError> {
    let resolved = Arc::new(resolve(options));

    if !parser.rule_names().iter().any(|name| name == "val") {
        return Err(IniPluginError(
            "tabnas-ini: the jsonic grammar must be installed first: call \
             tabnas_jsonic::jsonic(&mut parser) before tabnas_ini::ini(..), or use make()"
                .into(),
        ));
    }

    let string_config = register_lex_hooks(parser, &resolved);
    register_multiline(parser, &resolved);
    install_hoover(parser, &resolved)?;
    register_refs(parser, &resolved);

    let document = grammar_document(&resolved)?;
    let spec = GrammarSpec::from_value(document)?;
    parser.grammar(&spec)?;

    // The string check reads `string.chars` and the escape character off
    // the resolved configuration, and the document above is what sets
    // them, so the snapshot is taken here rather than at registration.
    let config = parser.config();
    let _ = string_config.set(StringCheckConfig {
        quotes: config.string.chars.chars().collect(),
        escape_char: config.string.escape_char,
    });

    install_val_rule(parser, &resolved);

    // AFTER the documents, because `grammar` applies a document's
    // options and an options pass that does not mention `parse.budget`
    // is not required to preserve one. jsonic installs the same check
    // over `map` and `list`; this one adds `dive`, which is where an INI
    // document nests, and is otherwise identical.
    parser.parse_budget(1, within_depth_limit);

    // INI has no array syntax: `val` is restricted to scalars and maps
    // above, leaving jsonic's `list` and `elem` rules unreachable.
    // Remove them so the grammar -- and its railroad diagram -- holds
    // only the rules INI actually uses.
    for name in ["list", "elem"] {
        parser.remove_rule(name);
    }

    Ok(())
}

/// The plugin form of [`ini`], for [`Tabnas::use_plugin`].
///
/// The options are typed, so they travel in this closure rather than in
/// the engine's serialized plugin bag, exactly as
/// `tabnas_hoover::plugin` and `tabnas_directive::plugin` do. Installed
/// this way the grammar is re-applied to derived instances.
///
/// ```
/// let mut parser = tabnas_jsonic::make();
/// parser.use_plugin(tabnas_ini::plugin(tabnas_ini::IniOptions::default()), None)?;
/// assert_eq!(parser.parse("a=1")?.to_string(), r#"{"a":"1"}"#);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn plugin(options: IniOptions) -> Plugin {
    Plugin::new(PLUGIN_NAME, move |parser, _bag| {
        ini(parser, &options).map_err(|error| PluginError(error.0))
    })
}

/// A failure to install the plugin: a missing base grammar, a hoover
/// block the host refuses, or a grammar document the engine rejects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniPluginError(pub String);

impl std::fmt::Display for IniPluginError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for IniPluginError {}

impl From<GrammarError> for IniPluginError {
    fn from(error: GrammarError) -> Self {
        IniPluginError(error.0)
    }
}

impl From<PluginError> for IniPluginError {
    fn from(error: PluginError) -> Self {
        IniPluginError(error.0)
    }
}

impl From<tabnas_hoover::HooverError> for IniPluginError {
    fn from(error: tabnas_hoover::HooverError) -> Self {
        IniPluginError(error.0)
    }
}

// ---------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------

/// Build an INI parser with the default options.
///
/// ```
/// let parser = tabnas_ini::make();
/// let value = parser.parse("[a]\nb = c")?;
/// assert_eq!(value.to_string(), r#"{"a":{"b":"c"}}"#);
/// # Ok::<(), tabnas_ini::IniError>(())
/// ```
pub fn make() -> Tabnas {
    make_with(&IniOptions::default())
}

/// Build an INI parser with caller options.
///
/// Infallible by design: the grammar document is a fixed literal and the
/// jsonic base is installed one line earlier, so a failure here is a bug
/// in this crate rather than anything a caller did. The Go `MakeJsonic`
/// panics for the same reason.
///
/// ```
/// let options = tabnas_ini::IniOptions::default().with_inline_comments();
/// let parser = tabnas_ini::make_with(&options);
/// assert_eq!(parser.parse("a = x ; note")?.to_string(), r#"{"a":"x"}"#);
/// # Ok::<(), tabnas_ini::IniError>(())
/// ```
pub fn make_with(options: &IniOptions) -> Tabnas {
    let mut parser = tabnas_jsonic::make();
    parser
        .use_plugin(plugin(options.clone()), None)
        .expect("the INI grammar document is fixed, and the jsonic base is installed");
    parser
}

/// Parse an INI source string with the shared default parser.
///
/// The engine is built once, on first use, and reused after that. Both
/// other runtimes do the same (`sync.Once` in `go/ini.go`, a lazily
/// assigned module variable in the TypeScript tests), and reuse is safe
/// here for the same reason it is there: [`Tabnas::parse`] takes `&self`
/// and builds a fresh parse context per call, and `Tabnas` is `Send +
/// Sync`, so concurrent callers share one installed grammar instead of
/// each rebuilding it.
///
/// ```
/// let value = tabnas_ini::parse("a = 1\n[s]\nb = 2")?;
/// assert_eq!(value.to_string(), r#"{"a":"1","s":{"b":"2"}}"#);
/// # Ok::<(), tabnas_ini::IniError>(())
/// ```
pub fn parse(src: &str) -> Result<Value, IniError> {
    static DEFAULT: OnceLock<Tabnas> = OnceLock::new();
    DEFAULT.get_or_init(make).parse(src)
}

/// Parse an INI source string with caller options.
///
/// Every call builds a fresh parser, because the options decide the
/// hoover blocks and the lexer matchers and so cannot be shared. Reach
/// for [`make_with`] when parsing more than once with one option set.
///
/// ```
/// let options = tabnas_ini::IniOptions::default().with_multiline();
/// let value = tabnas_ini::parse_with("a = one \\\ntwo", &options)?;
/// assert_eq!(value.to_string(), r#"{"a":"one two"}"#);
/// # Ok::<(), tabnas_ini::IniError>(())
/// ```
pub fn parse_with(src: &str, options: &IniOptions) -> Result<Value, IniError> {
    make_with(options).parse(src)
}

// ---------------------------------------------------------------------
// Lexer hooks
// ---------------------------------------------------------------------

/// Register the in-value probe and the four `check` hooks, and hand
/// back the cell the string check reads its quote characters from.
fn register_lex_hooks(
    parser: &mut Tabnas,
    resolved: &Resolved,
) -> Arc<OnceLock<StringCheckConfig>> {
    // The probe. It matches nothing; it only records where the parser
    // is, for the checks below. See the thread-local block above.
    parser.imperative_lex_match_ref(PROBE_REF, |_lexer, rule, _context| {
        IN_VAL.with(|flag| flag.set(in_val_rule(rule)));
        IN_VALUE.with(|flag| flag.set(in_value_position(rule)));
        None
    });

    // Line matching is skipped inside a `val` rule, so a newline does
    // not end a value the hoover block is still gathering.
    parser.imperative_lex_check_ref(LINE_CHECK_REF, |_lexer| {
        if IN_VAL.with(Cell::get) {
            LexCheckResult::Skip
        } else {
            LexCheckResult::Continue
        }
    });

    // A comment marker is only a comment when it starts a LINE. Inside
    // a value (`k = ;x`) the comment matcher would otherwise beat
    // hoover's endofline block and eat the rest of the line, after which
    // the value rule silently swallowed the NEXT line's pair. Declining
    // here lets the value lexers see the marker: with inline comments
    // off it becomes a literal, and with them on hoover terminates the
    // value at the marker and the comment is lexed normally once the
    // value rule has closed.
    parser.imperative_lex_check_ref(COMMENT_CHECK_REF, |_lexer| {
        if IN_VALUE.with(Cell::get) {
            LexCheckResult::Skip
        } else {
            LexCheckResult::Continue
        }
    });

    // A value keyword is only a keyword when it is the WHOLE value:
    // `k = true` is the boolean, `k = true, false` is the string
    // `true, false`. The text matcher runs before hoover and emits a
    // `#VL` for a keyword that merely STARTS the value, so
    // `k = true, false` silently became `true` and `k = null x` even
    // grew a spurious `x` key. Declining in value position hands the
    // whole line to hoover, which does the same keyword lookup on the
    // complete, trimmed value.
    parser.imperative_lex_check_ref(TEXT_CHECK_REF, |_lexer| {
        if IN_VALUE.with(Cell::get) {
            LexCheckResult::Skip
        } else {
            LexCheckResult::Continue
        }
    });

    // A quoted value only counts as quoted when the quotes wrap the
    // WHOLE value: `k = "a b"` is the string `a b`, but `k = "a"b` is
    // the literal text `"a"b`. Without this the string matcher consumed
    // just `"a"`, hoover then lexed the trailing `b` as a fresh key, and
    // the document silently gained a property that was never written. An
    // unterminated quote is left to the string matcher, which abandons
    // it and lets hoover take the raw line.
    let config: Arc<OnceLock<StringCheckConfig>> = Arc::new(OnceLock::new());
    let for_check = Arc::clone(&config);
    let inline = Arc::new(resolved.clone());
    parser.imperative_lex_check_ref(STRING_CHECK_REF, move |lexer: &mut Lexer<'_>| {
        if !IN_VALUE.with(Cell::get) {
            return LexCheckResult::Continue;
        }
        let Some(string_config) = for_check.get() else {
            return LexCheckResult::Continue;
        };
        if string_quote_wraps_value(lexer, string_config, &inline) {
            LexCheckResult::Continue
        } else {
            LexCheckResult::Skip
        }
    });

    config
}

/// Does the quote at the cursor close on this line with nothing but
/// whitespace (and, when enabled, an inline comment) after it?
///
/// `true` also when there is no quote at all, so the string matcher runs
/// as usual; the caller only declines on a genuine trailing-text case.
fn string_quote_wraps_value(
    lexer: &Lexer<'_>,
    config: &StringCheckConfig,
    resolved: &Resolved,
) -> bool {
    // Read forward lazily, never into a buffer. This hook runs at every
    // value, and the rest of the document is what it can see: collecting
    // that made a document of n pairs cost n^2, with 16,000 pairs taking
    // three seconds where the same count of bare keys took a fifth of a
    // second. Nothing here looks past the end of the line.
    let remaining = lexer.remaining();
    let mut characters = remaining.chars();
    let Some(quote) = characters.next() else {
        return true;
    };
    if !config.quotes.contains(&quote) {
        return true;
    }

    // Find the closing quote on this line. An escape takes the character
    // after it with it, so `"a\"b"` closes at the LAST quote.
    let after_quote = loop {
        let Some(character) = characters.next() else {
            return true;
        };
        if character == config.escape_char {
            characters.next();
            continue;
        }
        if character == quote {
            break characters.as_str();
        }
        if character == '\n' {
            return true;
        }
    };

    // Only whitespace, and when enabled an inline comment, may follow.
    let tail = after_quote.trim_start_matches([' ', '\t']);
    let spaced = tail.len() < after_quote.len();
    let next = tail.chars().next();
    let at_line_end = match next {
        None | Some('\n') => true,
        Some('\r') => tail.starts_with("\r\n"),
        _ => false,
    };
    let at_inline_comment = resolved.inline_active
        && next.is_some_and(|character| resolved.is_inline_char(character))
        && (!resolved.esc_whitespace || spaced);

    at_line_end || at_inline_comment
}

// ---------------------------------------------------------------------
// hoover
// ---------------------------------------------------------------------

fn install_hoover(parser: &mut Tabnas, resolved: &Resolved) -> Result<(), IniPluginError> {
    // When inline comments are active without whitespace mode, the
    // comment characters terminate a hoovered span. With whitespace mode
    // on, the custom value matcher does the detection instead.
    let inline_in_fixed = resolved.inline_active && !resolved.esc_whitespace;

    let mut eol_end: Vec<String> = vec!["\n".into(), "\r\n".into()];
    if inline_in_fixed {
        eol_end.extend(resolved.inline_char_str.iter().cloned());
    }
    eol_end.push(String::new());

    let mut key_end: Vec<String> = vec!["=".into(), "\n".into(), "\r\n".into()];
    if inline_in_fixed {
        key_end.extend(resolved.inline_char_str.iter().cloned());
    }
    key_end.push(String::new());

    let mut eol_escape: HashMap<String, String> = HashMap::new();
    eol_escape.insert("\\".into(), "\\".into());
    let mut key_escape = eol_escape.clone();
    if resolved.inline_active && resolved.esc_backslash {
        for ch in &resolved.inline_char_str {
            eol_escape.insert(ch.clone(), ch.clone());
            key_escape.insert(ch.clone(), ch.clone());
        }
    }

    let endofline = Block {
        name: "endofline".into(),
        start: Some(StartSpec {
            fixed: None,
            consume: Consume::All,
            rule: Some(HooverRuleSpec {
                parent: Some(HooverRuleFilter::include(&["pair", "elem"])),
                ..Default::default()
            }),
        }),
        end: EndSpec {
            fixed: eol_end,
            consume: Consume::Only(vec!["\n".into(), "\r\n".into()]),
        },
        token: None,
        escape_char: Some('\\'),
        escape: eol_escape,
        allow_unknown_escape: Some(true),
        preserve_escape_char: true,
        trim: true,
    };

    let key = Block {
        name: "key".into(),
        start: Some(StartSpec {
            fixed: None,
            consume: Consume::All,
            rule: Some(HooverRuleSpec {
                current: Some(HooverRuleFilter::exclude(&["dive"])),
                state: Some("oc".into()),
                ..Default::default()
            }),
        }),
        end: EndSpec {
            fixed: key_end,
            consume: Consume::Never,
        },
        token: Some("#HK".into()),
        // No escape character, as in the canonical port: the map below
        // is inert without one, and a backslash in a key is literal.
        escape_char: None,
        escape: key_escape,
        allow_unknown_escape: None,
        preserve_escape_char: false,
        trim: true,
    };

    let divekey = Block {
        name: "divekey".into(),
        start: Some(StartSpec {
            fixed: None,
            consume: Consume::All,
            rule: Some(HooverRuleSpec {
                current: Some(HooverRuleFilter::include(&["dive"])),
                ..Default::default()
            }),
        }),
        end: EndSpec {
            // A section header lives on one line: a newline terminates
            // the path segment, so an unterminated header (`[a` with no
            // `]`) is a parse error instead of a section name that
            // silently swallows following lines up to the next `]`.
            //
            // The empty string is hoover's end-of-input delimiter.
            // Without it a header that runs to the end of the source
            // leaves the block committed but unterminated, which hoover
            // reports as a generic `invalid_text` bad token, raised by
            // the lexer before any alternate is consulted, so the
            // grammar never gets to say what actually went wrong. Ending
            // at the end of input emits the segment token instead and
            // lets the dive rule's close state raise
            // `unterminated_section`, which is the real diagnosis.
            fixed: vec![
                "]".into(),
                ".".into(),
                "\n".into(),
                "\r\n".into(),
                String::new(),
            ],
            consume: Consume::Never,
        },
        token: Some("#DK".into()),
        escape_char: Some('\\'),
        escape: HashMap::from([
            ("]".to_string(), "]".to_string()),
            (".".to_string(), ".".to_string()),
            ("\\".to_string(), "\\".to_string()),
        ]),
        allow_unknown_escape: Some(true),
        // Same rule as the value block: an escape that is not one of the
        // three above keeps both characters, so `[C:\path]` stays
        // `C:\path` rather than losing the backslash.
        preserve_escape_char: true,
        trim: true,
    };

    hoover(
        parser,
        HooverOptions::new(vec![endofline, key, divekey]).with_lex_order(HOOVER_ORDER),
    )?;
    Ok(())
}

// ---------------------------------------------------------------------
// The declared-section set
// ---------------------------------------------------------------------

// Per PARSE state, so it lives on the context's `u` bag rather than in
// the instance. The canonical TypeScript and Go ports keep it in a
// closure variable shared by every parse, which is safe only because
// neither runtime parses one instance from two threads. A `Tabnas` is
// `Send + Sync` and `parse` shares one instance, so here it has to be
// per parse or two concurrent documents would declare each other's
// sections.
const DECLARED: &str = "ini_declared";

fn declared_clear(context: &mut Context) {
    context.u.insert(DECLARED.to_string(), empty_object());
}

fn declared_has(context: &Context, key: &str) -> bool {
    match context.u.get(DECLARED) {
        // Membership is read WITHOUT cloning the entry: `object_get`
        // hands back a copy, which on a container is a second owner and
        // makes the next write copy the whole map.
        Some(Value::Object(entries)) => entries.contains_key(key),
        _ => false,
    }
}

fn declared_add(context: &mut Context, key: String) {
    // Take the set out of the bag rather than cloning it. `insert`
    // returns the previous value, so the set has one owner while it is
    // written to and the write is in place; reading it with `get` and
    // cloning made a document of many sections copy the whole set once
    // per section.
    let mut set = context
        .u
        .insert(DECLARED.to_string(), Value::Undefined)
        .filter(is_object)
        .unwrap_or_else(empty_object);
    object_insert(&mut set, key, Value::Bool(true));
    context.u.insert(DECLARED.to_string(), set);
}

/// Walk `dive` from `root`, creating a plain object at each missing
/// segment, and hand back the value the section starts from.
///
/// `override_last` replaces the final segment's object outright, which
/// is what `section.duplicate: 'override'` means.
///
/// Iterative, not recursive. A container here is a value rather than a
/// reference, so the path has to be taken apart on the way down and put
/// back together on the way up, and doing that with the call stack
/// aborted the process on a section header thousands of segments deep.
/// The depth budget below bounds the vectors; the shape does not depend
/// on it.
fn open_section(root: &mut Value, dive: &[String], override_last: bool) -> Value {
    if !is_object(root) {
        *root = empty_object();
    }
    let mut levels: Vec<Value> = Vec::with_capacity(dive.len() + 1);
    levels.push(std::mem::replace(root, Value::Undefined));
    for (index, key) in dive.iter().enumerate() {
        let last = index + 1 == dive.len();
        let parent = levels.last_mut().expect("a level is always on the stack");
        let child = match object_take(parent, key) {
            Some(value) if is_object(&value) && !(last && override_last) => value,
            _ => empty_object(),
        };
        levels.push(child);
    }

    let mut current = levels.pop().expect("one level per segment plus the root");
    let section = current.clone();
    for key in dive.iter().rev() {
        let mut parent = levels.pop().expect("one level per segment plus the root");
        object_insert(&mut parent, key.clone(), current);
        current = parent;
    }
    *root = current;
    section
}

/// Write a finished section back into `root` at `dive`. Iterative, for
/// the reason [`open_section`] is.
fn close_section(root: &mut Value, dive: &[String], section: Value) {
    if dive.is_empty() {
        return;
    }
    if !is_object(root) {
        *root = empty_object();
    }
    let mut levels: Vec<Value> = Vec::with_capacity(dive.len());
    levels.push(std::mem::replace(root, Value::Undefined));
    for key in &dive[..dive.len() - 1] {
        let parent = levels.last_mut().expect("a level is always on the stack");
        let child = match object_take(parent, key) {
            Some(value) if is_object(&value) => value,
            _ => empty_object(),
        };
        levels.push(child);
    }

    let mut current = section;
    for key in dive.iter().rev() {
        let mut parent = levels.pop().expect("one level per segment");
        object_insert(&mut parent, key.clone(), current);
        current = parent;
    }
    *root = current;
}

/// How deep the parse is: the `map`, `list` and `dive` rules on the
/// stack, plus the rule the loop is working on, which the engine hands
/// over separately. Counted from the rule NAMES rather than
/// `rule_stack.len()`, because the stack holds about three rules per
/// container level and a length-based limit would encode that ratio.
fn depth(context: &Context) -> usize {
    let is_level = |name: &str| name == "map" || name == "list" || name == "dive";
    let ancestors = context
        .rule_stack
        .iter()
        .filter(|rule| is_level(&rule.name))
        .count();
    let current = usize::from(
        context
            .rule
            .as_ref()
            .is_some_and(|rule| is_level(&rule.name)),
    );
    ancestors + current
}

/// The budget check: [`DEPTH_LIMIT`] levels parse, the next one is
/// refused with the engine's `cancel` code.
fn within_depth_limit(context: &Context) -> bool {
    depth(context) <= DEPTH_LIMIT
}

// ---------------------------------------------------------------------
// The grammar's function references
// ---------------------------------------------------------------------

/// The path this table is writing into, kept apart from `dive`.
///
/// `dive` on a table's `u` means "the header that opened the NEXT
/// table", because the next table reads it through `r.prev.u.dive`.
/// Recording the path under that name would make every table inherit
/// its predecessor's section.
const INI_PATH: &str = "ini_path";

fn register_refs(parser: &mut Tabnas, resolved: &Resolved) {
    // --- state actions, wired by the @rule-phase convention ---

    parser.state_action_ref("@ini-bo", |rule, context| {
        set_node(rule, empty_object());
        declared_clear(context);
        Ok(())
    });

    let for_table_bo = Arc::new(resolved.clone());
    parser.state_action_ref("@table-bo", move |rule, context| {
        table_before_open(rule, context, &for_table_bo)
    });

    parser.state_action_ref("@table-bc", table_before_close);

    // Carry the completed child's dive path up. The canonical
    // TypeScript pushes onto an array the parent already holds, so the
    // parent sees the growth for free; a Rust rule cannot write its
    // parent's `u` bag, so each level reads its child's instead. The Go
    // port added `@dive-bc` for the same reason.
    parser.state_action_ref("@dive-bc", |rule, _context| {
        if let Some(dive) = rule
            .child_rule
            .as_ref()
            .and_then(|child| dive_of(child.u.get("dive")))
        {
            rule.u_mut().insert("dive".into(), dive_value(&dive));
        }
        Ok(())
    });

    // --- alternate actions ---

    parser.action_with_context("@table-close-dive", |rule, _context| {
        if let Some(dive) = rule
            .child_rule
            .as_ref()
            .and_then(|child| dive_of(child.u.get("dive")))
        {
            rule.u_mut().insert("dive".into(), dive_value(&dive));
        }
        Ok(())
    });

    parser.action_with_context("@dive-push", |rule, _context| {
        let mut dive = rule
            .parent_rule
            .as_ref()
            .and_then(|parent| dive_of(parent.u.get("dive")))
            .unwrap_or_default();
        if let Some(existing) = dive_of(rule.u.get("dive")) {
            dive = existing;
        }
        let segment = rule.o0().map(token_string).unwrap_or_default();
        dive.push(segment);
        rule.u_mut().insert("dive".into(), dive_value(&dive));
        Ok(())
    });

    parser.action_with_context("@pair-key-eq", pair_key_eq);

    parser.action_with_context("@pair-key-bool", |rule, _context| {
        // The canonical `'string' === typeof key`, which is NOT the same
        // as "the token's text". A line holding nothing but a value
        // keyword (`true` on its own) carries the BOOLEAN on its token,
        // because hoover and the text matcher both resolve a keyword
        // that is the whole span, so TypeScript declares no key at all
        // and answers `{}`. Measured, with the checkouts beside this
        // repository: TypeScript `{}`, Go `{"true":true}`. TypeScript is
        // canonical, so this port follows it and not the sibling.
        let Some(key) = rule.o0().and_then(bool_key) else {
            return Ok(());
        };
        if let Some(parent) = rule
            .parent_rule
            .as_ref()
            .map(|parent| Rc::clone(&parent.node))
        {
            object_insert(&mut parent.borrow_mut(), key, Value::Bool(true));
        }
        Ok(())
    });

    parser.action_with_context("@val-empty", |rule, _context| {
        set_node(rule, Value::String(String::new()));
        Ok(())
    });

    // --- conditions ---

    parser.alt_condition("@is-table-parent", |rule, _context| {
        rule.parent_rule
            .as_ref()
            .is_some_and(|parent| parent.name.as_str() == "table")
    });

    parser.alt_condition("@is-table-grandparent", |rule, _context| {
        rule.parent_rule
            .as_ref()
            .and_then(|parent| parent.parent_rule.clone())
            .is_some_and(|grandparent| grandparent.name.as_str() == "table")
    });

    parser.alt_condition("@is-duplicate-section", |rule, _context| {
        rule.u.contains_key("dupsec")
    });

    // --- error alternates ---

    // The duplicate itself. The dotted path is not any single token's
    // source, so it rides along as the `{section}` detail the message
    // template reads.
    parser.alt_error("@duplicate-section", |rule, context| {
        let mut token = alt_err_token(rule, context)?;
        let section = match rule.u.get("dupsec") {
            Some(Value::String(path)) => path.clone(),
            _ => String::new(),
        };
        token.bad_with_details(
            "duplicate_section",
            [("section".to_string(), Value::String(section))],
        );
        Some(token)
    });

    // The section header ran out before its closing bracket, at a
    // newline or at the end of input. The dive rule opened on the `#DK`
    // segment token, so that token carries both the text for the
    // message and the position to point at.
    parser.alt_error("@dive-unterminated", |rule, context| {
        let mut token = alt_err_token(rule, context)?;
        token.bad("unterminated_section");
        Some(token)
    });

    // Unreachable while `#CL` is removed from the fixed tokens, as it is
    // here and in the Go port. Registered because the grammar names it.
    parser.alt_error("@pair-close-err", |rule, _context| rule.c1().cloned());
}

fn table_before_open(
    rule: &mut Rule,
    context: &mut Context,
    resolved: &Resolved,
) -> Result<(), ActionError> {
    // r.node = r.parent.node: the table writes into the document root
    // until a section header moves it.
    let Some(root) = rule
        .parent_rule
        .as_ref()
        .map(|parent| Rc::clone(&parent.node))
    else {
        return Ok(());
    };
    rule.node = Rc::clone(&root);

    let Some(dive) = rule
        .prev_rule
        .as_ref()
        .and_then(|prev| dive_of(prev.u.get("dive")))
        .filter(|dive| !dive.is_empty())
    else {
        return Ok(());
    };

    // A null character separates the segments, so a dot inside a key
    // cannot collide with the separator.
    let section_key = dive.join("\u{0}");
    let is_duplicate = declared_has(context, &section_key);

    if is_duplicate && resolved.dup_section == Duplicate::Error {
        // Flag the rule and let this rule's error ALTERNATE raise the
        // coded error (see the grammar's table:open). Raising here is
        // not portable: Go discards a state action's return value, and
        // an error published on the context from a before-open hook is
        // overwritten when the alternates are matched.
        rule.u_mut()
            .insert("dupsec".into(), Value::String(dive.join(".")));
        return Ok(());
    }

    let override_last = is_duplicate && resolved.dup_section == Duplicate::Override;
    let section = open_section(&mut root.borrow_mut(), &dive, override_last);

    rule.u_mut().insert(INI_PATH.into(), dive_value(&dive));
    set_node(rule, section);
    declared_add(context, section_key);
    Ok(())
}

fn table_before_close(rule: &mut Rule, _context: &mut Context) -> Result<(), ActionError> {
    // Object.assign(r.node, r.child.node). The child `map` and `dive`
    // rules SHARE this rule's node cell, so the merge is a copy of a map
    // onto itself: no change in any runtime, and in this one an
    // expensive one, because `child_node` is a second owner of the same
    // `Arc` and the first write copies the whole map. A document of
    // 5,000 sections paid that once per section. Skipping it when the
    // cells are the same object is what the canonical `Object.assign(x,
    // x)` does for free. The merge is kept for a child that genuinely
    // allocates its own node.
    let child_is_self = rule
        .child_rule
        .as_ref()
        .is_some_and(|child| Rc::ptr_eq(&child.node, &rule.node));
    if child_is_self {
        // Drop this rule's copy of its child's node as well. It is a
        // second owner of the same container, and the next write to the
        // document would copy the whole thing to get around it. Nothing
        // reads a `table` rule's child node after this point.
        rule.child_node = Value::Undefined;
    } else if let Value::Object(entries) = rule.child_node.clone() {
        let mut node = rule.node.borrow_mut();
        if is_object(&node) {
            for (key, value) in entries.iter() {
                object_insert(&mut node, key.clone(), value.clone());
            }
        }
    }

    // Write the finished section back into the document root. The
    // canonical ports hold a live reference to the section object and
    // never need this; a Rust container is a value, so the tree is
    // reassembled on the way out.
    let Some(dive) = dive_of(rule.u.get(INI_PATH)) else {
        return Ok(());
    };
    let Some(root) = rule
        .parent_rule
        .as_ref()
        .map(|parent| Rc::clone(&parent.node))
    else {
        return Ok(());
    };
    let section = rule.node.borrow().clone();
    close_section(&mut root.borrow_mut(), &dive, section);
    Ok(())
}

fn pair_key_eq(rule: &mut Rule, _context: &mut Context) -> Result<(), ActionError> {
    let key = rule.o0().map(token_string).unwrap_or_default();
    let existing = object_get(&rule.node.borrow(), &key);

    if matches!(existing, Some(Value::Array(_))) {
        rule.u_mut().insert("key".into(), Value::String(key));
        rule.u_mut().insert("ini_array".into(), Value::Bool(true));
        return Ok(());
    }

    // `2 < key.length` in the canonical TypeScript, where a string's
    // length is its UTF-16 code units. Counting scalar values instead
    // would disagree for a key holding an astral character.
    if key.encode_utf16().count() > 2 && key.ends_with("[]") {
        let array_key = key[..key.len() - 2].to_string();
        let current = object_get(&rule.node.borrow(), &array_key);
        let array = match current {
            Some(Value::Array(items)) => Value::Array(items),
            Some(value) if !value.is_undefined() => Value::array(vec![value]),
            _ => Value::array(Vec::new()),
        };
        object_insert(&mut rule.node.borrow_mut(), array_key.clone(), array);
        rule.u_mut().insert("key".into(), Value::String(array_key));
        rule.u_mut().insert("ini_array".into(), Value::Bool(true));
        return Ok(());
    }

    rule.u_mut().insert("key".into(), Value::String(key));
    rule.u_mut().insert("pair".into(), Value::Bool(true));
    Ok(())
}

// ---------------------------------------------------------------------
// The val rule
// ---------------------------------------------------------------------

/// Is this alternate the one that starts a list on `[`?
///
/// INI spells a section header `[a]`, so the JSON core's list alternate
/// has to go or `[a]` would open an array. It is identified by its group
/// tags rather than its position, because the position moves whenever
/// the layers below add an alternate. The tags are compared as a SET:
/// the canonical TypeScript joins them as `json,list` and this engine
/// stores them as `list,json`, and the alternate is the same one.
fn is_json_list_alt(alt: &AltSpec) -> bool {
    let mut tags: Vec<&str> = alt
        .g
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .collect();
    tags.sort_unstable();
    tags == ["json", "list"]
}

fn install_val_rule(parser: &mut Tabnas, _resolved: &Resolved) {
    let os = parser.token("#OS");
    let cs = parser.token("#CS");
    let eq = parser.token("#EQ");
    let dot = parser.token("#DOT");
    let zz = parser.token("#ZZ");
    let st = parser.token("#ST");

    parser.define_rule("val", move |spec| {
        // Clear the node this rule inherits from its parent, which is
        // the enclosing map. jsonic's own value alternates do it with
        // `@reset$`; the alternate hoover prepends for its `#HV` token
        // carries no action, so an INI value would keep the map.
        //
        // Not tidiness: jsonic's val before-close stashes the inherited
        // node in the rule's `u` bag, which holds a second handle on
        // that map for as long as the value rule lives, and the next key
        // written into it then copies the whole map through
        // `Arc::make_mut`. A document of n pairs cost n^2 without this
        // (4000 pairs took 2.2 seconds, 20,000 exhausted memory). The Go
        // port resets the node in its own `val` before-open hook.
        spec.add_bo(|rule, _context| set_node(rule, Value::Undefined));

        let kept: Vec<AltSpec> = std::mem::take(&mut spec.open)
            .into_iter()
            .filter(|alt| !is_json_list_alt(alt))
            .collect();

        // Since `[`, `]`, `=` and `.` are fixed tokens they are lexed
        // before hoover gets to run, so a value that STARTS with one of
        // them never reaches the endofline block. Concatenate the fixed
        // token's source with the rest of the value instead. All four
        // are alternatives for one slot, the TypeScript
        // `['#OS #CS #EQ #DOT']`.
        let mut open = vec![
            AltSpec {
                s: vec![vec![os, cs, eq, dot]],
                r: Some("val".to_string()),
                u: HashMap::from([("ini_prev".to_string(), Value::Bool(true))]),
                ..Default::default()
            },
            // End of input: an empty value.
            AltSpec {
                s: vec![vec![zz]],
                a: vec!["@val-empty".to_string()],
                ..Default::default()
            },
        ];
        open.extend(kept);
        spec.open = open;

        spec.add_ac(move |rule, _context| val_after_close(rule, st));
    });
}

/// The value `JSON.parse` gives a number literal too large for a double,
/// which is the one document `serde_json` refuses where `JSON.parse`
/// succeeds.
///
/// `JSON.parse("1e400")` is `Infinity` and `JSON.parse("-1e400")` is
/// `-Infinity`, because the specification rounds an unrepresentable
/// magnitude to the nearest double. `serde_json` reports
/// `number out of range` instead, and the caller then kept the source
/// text, so `a = '1e400'` was the STRING `"1e400"` in this port and the
/// number `Infinity` in the canonical one. A literal too SMALL to
/// represent is not affected: both readers round `1e-400` to zero.
///
/// `Some` only for a whole document that is one JSON number and whose
/// value is not finite. Anything else, including a number that overflows
/// INSIDE an array or an object, is left to the caller, which keeps the
/// text; `DIVERGENCE.md` records that remaining gap.
fn overflowed_json_number(text: &str) -> Option<f64> {
    // The JSON whitespace set, which `JSON.parse` also allows around a
    // top-level value.
    let trimmed = text.trim_matches([' ', '\t', '\n', '\r']);
    if !is_json_number(trimmed) {
        return None;
    }
    // A valid JSON number `serde_json` rejected can only be one it could
    // not fit in a double, so the parse below succeeds and is infinite.
    // Testing for that rather than assuming it keeps this arm from
    // quietly claiming any other failure.
    trimmed
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_infinite())
}

/// Whether the whole of `text` is a JSON number literal (RFC 8259
/// section 6). Rust's own float parser is wider than that grammar: it
/// takes `inf`, `NaN`, `+1`, `1.` and `.5`, none of which `JSON.parse`
/// accepts, so the grammar is checked here rather than inferred from a
/// successful parse.
fn is_json_number(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    if chars.peek() == Some(&'-') {
        chars.next();
    }
    match chars.next() {
        // A leading zero admits no further integer digits.
        Some('0') => {}
        Some(digit) if digit.is_ascii_digit() => {
            while chars.peek().is_some_and(char::is_ascii_digit) {
                chars.next();
            }
        }
        _ => return false,
    }
    if chars.peek() == Some(&'.') {
        chars.next();
        if !chars.peek().is_some_and(char::is_ascii_digit) {
            return false;
        }
        while chars.peek().is_some_and(char::is_ascii_digit) {
            chars.next();
        }
    }
    if matches!(chars.peek(), Some('e' | 'E')) {
        chars.next();
        if matches!(chars.peek(), Some('+' | '-')) {
            chars.next();
        }
        if !chars.peek().is_some_and(char::is_ascii_digit) {
            return false;
        }
        while chars.peek().is_some_and(char::is_ascii_digit) {
            chars.next();
        }
    }
    chars.next().is_none()
}

/// Rewrite every UNPAIRED surrogate escape in JSON source text to a
/// `�` escape, or `None` when there is none to rewrite.
///
/// `JSON.parse` accepts a lone surrogate: a JavaScript string is UTF-16,
/// so `"\ud800"` is a perfectly ordinary one-code-unit string. A Rust
/// `String` is scalar values and cannot hold one at all, so `serde_json`
/// refuses the document outright and the caller then kept the JSON
/// SOURCE as the value: `a = '["\ud800"]'` stopped being an array.
///
/// Substituting the replacement character keeps the TYPE and the SHAPE
/// of the value, which is the larger half of the canonical result, and
/// is what Go's `encoding/json` does here, so the two ports that cannot
/// hold a lone surrogate agree. The one character that differs is
/// recorded in `../DIVERGENCE.md`.
///
/// Scanned as bytes, which is safe because every character this looks at
/// (`"`, `\`, `u` and the hex digits) is ASCII, and because the only
/// slices taken start and end on one of them. An escape other than
/// `\uXXXX` takes the WHOLE next character with it, so a multi-byte one
/// cannot leave the cursor inside a character.
fn replace_lone_surrogates(text: &str) -> Option<String> {
    /// The code unit a `\uXXXX` escape at `at` names.
    fn escaped_unit(bytes: &[u8], at: usize) -> Option<u16> {
        if bytes.get(at) != Some(&b'\\') || bytes.get(at + 1) != Some(&b'u') {
            return None;
        }
        let digits = bytes.get(at + 2..at + 6)?;
        digits.iter().try_fold(0u16, |unit, byte| {
            let digit = (*byte as char).to_digit(16)?;
            Some(unit * 16 + digit as u16)
        })
    }
    const HIGH: std::ops::Range<u16> = 0xD800..0xDC00;
    const LOW: std::ops::Range<u16> = 0xDC00..0xE000;

    let bytes = text.as_bytes();
    let mut repaired = String::new();
    let mut copied = 0usize;
    let mut index = 0usize;
    let mut in_string = false;
    let mut replaced = false;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                in_string = !in_string;
                index += 1;
            }
            b'\\' if in_string => match escaped_unit(bytes, index) {
                Some(unit)
                    if HIGH.contains(&unit)
                        && escaped_unit(bytes, index + 6)
                            .is_some_and(|next| LOW.contains(&next)) =>
                {
                    index += 12;
                }
                Some(unit) if HIGH.contains(&unit) || LOW.contains(&unit) => {
                    repaired.push_str(&text[copied..index]);
                    repaired.push_str("\\ufffd");
                    index += 6;
                    copied = index;
                    replaced = true;
                }
                Some(_) => index += 6,
                // Any other escape, whose escaped character may be
                // several bytes long.
                None => {
                    index += 1;
                    index += text[index..]
                        .chars()
                        .next()
                        .map_or(0, char::len_utf8)
                        .max(1);
                }
            },
            _ => index += 1,
        }
    }

    replaced.then(|| {
        repaired.push_str(&text[copied..]);
        repaired
    })
}

fn val_after_close(rule: &mut Rule, st: tabnas::Tin) {
    // A single-quoted value carries JSON: `k = '{"a":1}'` is the object.
    // An invalid document is kept as the string it already is.
    let single_quoted = rule
        .o0()
        .is_some_and(|token| token.tin == st && token.src.as_str().starts_with('\''));
    if single_quoted {
        let text = match &*rule.node.borrow() {
            Value::String(text) => Some(text.clone()),
            Value::Text(text) => Some(text.string.clone()),
            _ => None,
        };
        if let Some(text) = text {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                set_node(rule, Value::from_json(&parsed));
            } else if let Some(number) = overflowed_json_number(&text) {
                set_node(rule, Value::Number(number));
            } else if let Some(repaired) = replace_lone_surrogates(&text) {
                // A lone surrogate is the other thing `JSON.parse`
                // accepts and `serde_json` refuses. Re-read the repaired
                // text rather than assuming it now parses: the document
                // may be invalid for some further reason, and then the
                // value stays the string it already is.
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&repaired) {
                    set_node(rule, Value::from_json(&parsed));
                }
            }
        }
    }

    // A value can start with more than one fixed token (`a = ==x`), and
    // each one replaced the val rule with a fresh one. Only the LAST val
    // in that chain runs this hook, so walk the whole chain: every link
    // contributes its token source, and every link's node is updated,
    // including the first, which is the rule the pair rule reads its
    // child node from. Stopping at the first link instead left that node
    // unset, and the pair then took the parent map as its value,
    // producing a circular result.
    if rule
        .prev_rule
        .as_ref()
        .is_some_and(|prev| prev.u.contains_key("ini_prev"))
    {
        let mut text = js_string(&rule.node.borrow());
        let mut cursor = rule.prev_rule.clone();
        while let Some(link) = cursor {
            if !link.u.contains_key("ini_prev") {
                break;
            }
            let source = link
                .o
                .first()
                .map(|token| token.src.as_str().to_string())
                .unwrap_or_default();
            text = format!("{source}{text}");
            // The canonical `p.node = r.node` (ts/src/ini.ts:530,
            // go/ini.go:1033), which this port had never carried. The
            // write goes THROUGH the cell rather than replacing it:
            // `link.node` is the very `Rc<RefCell<Value>>` the engine
            // froze as the parent's child link when the pair rule pushed
            // the FIRST val of this chain, so only a write into that cell
            // reaches the node the pair later reads. `set_node` swaps the
            // `Rc` instead, which leaves the frozen handle untouched.
            *link.node.borrow_mut() = Value::String(text.clone());
            cursor = link.prev_rule.clone();
        }
        set_node(rule, Value::String(text));
    }

    // Deliberately NOT an `else` of the block above: an array entry
    // whose value starts with a fixed token (`k[] = [x`) needs both the
    // concatenation AND the push, or the entry is silently dropped.
    let Some(parent) = rule.parent_rule.clone() else {
        return;
    };
    if !parent.u.contains_key("ini_array") {
        return;
    }
    let Some(Value::String(key)) = parent.u.get("key").cloned() else {
        return;
    };
    // The array is read back off the parent's node rather than out of
    // its `u` bag: a Rust rule cannot write its parent's bag, and a
    // container here is a value rather than a reference, so the grown
    // array has to be stored back where the document will find it.
    let value = rule.node.borrow().clone();
    let mut node = parent.node.borrow_mut();
    let mut items = match object_get(&node, &key) {
        Some(Value::Array(items)) => (*items).clone(),
        _ => Vec::new(),
    };
    items.push(value);
    object_insert(&mut node, key, Value::array(items));
}

// ---------------------------------------------------------------------
// The custom value matcher
// ---------------------------------------------------------------------

/// The lexer configuration the value matcher reads: the `value.lex` flag
/// and the `value.def` table, so a keyword is resolved exactly as
/// hoover's endofline block resolves the values it lexes itself.
struct ValueConfig {
    lex: bool,
    definitions: HashMap<String, Value>,
}

impl ValueConfig {
    fn from_options(options: &Options) -> Self {
        Self {
            lex: options.value.lex,
            definitions: options
                .value
                .definitions
                .iter()
                .map(|(source, definition)| {
                    (
                        source.clone(),
                        definition.val.clone().unwrap_or(Value::Undefined),
                    )
                })
                .collect(),
        }
    }

    fn resolve(&self, text: &str) -> Value {
        if self.lex {
            if let Some(value) = self.definitions.get(text) {
                return value.clone();
            }
        }
        Value::String(text.to_string())
    }
}

/// Register the custom value matcher.
///
/// It is needed when multiline continuation is on, or when inline
/// comments are active with whitespace-prefix detection, and it runs at
/// [`MULTILINE_ORDER`], just below hoover, so it sees a value first.
fn register_multiline(parser: &mut Tabnas, resolved: &Resolved) {
    let resolved = Arc::new(resolved.clone());
    parser.lex_match_factory_ref(MULTILINE_REF, move |options: &Options| {
        let config = Arc::new(ValueConfig::from_options(options));
        let resolved = Arc::clone(&resolved);
        let matcher: ImperativeLexMatcher = Arc::new(
            move |lexer: &mut Lexer<'_>, rule: &mut Rule, _context: &mut Context| {
                multiline_match(lexer, rule, &resolved, &config)
            },
        );
        Some(matcher)
    });
}

fn multiline_match(
    lexer: &mut Lexer<'_>,
    rule: &Rule,
    resolved: &Resolved,
    config: &ValueConfig,
) -> Option<Token> {
    // Only in value context during a rule's open state, the same gate
    // hoover's endofline block applies.
    let parent_is_pair = rule.parent_rule.as_ref().is_some_and(|parent| {
        let name = parent.name.as_str();
        name == "pair" || name == "elem"
    });
    if !parent_is_pair || rule.state != RuleState::Open {
        return None;
    }

    // Read forward lazily, never into a buffer. This matcher runs at
    // every value and what it can see is the rest of the document, so
    // collecting that first would make a document of n values cost n
    // squared. `rest` is always a character boundary: every step below
    // advances by whole characters.
    let source = lexer.remaining();
    let mut rest = source;
    let mut text = String::new();

    while let Some(c) = rest.chars().next() {
        // An inline comment character ends the value.
        if resolved.inline_active && resolved.is_inline_char(c) {
            if resolved.esc_whitespace {
                // Only a comment when whitespace comes first.
                if matches!(text.chars().last(), Some(' ') | Some('\t')) {
                    break;
                }
                text.push(c);
                rest = &rest[c.len_utf8()..];
                continue;
            }
            break;
        }

        // A continuation character before a newline joins the lines.
        if resolved.continuation == Some(c) {
            let after = &rest[c.len_utf8()..];
            if let Some(next) = after
                .strip_prefix('\n')
                .or_else(|| after.strip_prefix("\r\n"))
            {
                rest = next.trim_start_matches([' ', '\t']);
                continue;
            }
        }

        // A newline ends the value, unless the next line is indented and
        // indent continuation is on. A lone carriage return is ordinary
        // text, as it is in the canonical port.
        if '\n' == c || rest.starts_with("\r\n") {
            let after = if '\r' == c { &rest[2..] } else { &rest[1..] };
            if resolved.indent && after.starts_with([' ', '\t']) {
                rest = after.trim_start_matches([' ', '\t']);
                text.push(' ');
                continue;
            }
            rest = after;
            break;
        }

        // Escapes.
        if '\\' == c {
            let after = &rest[1..];
            if let Some(next) = after.chars().next() {
                if resolved.inline_active && resolved.esc_backslash && resolved.is_inline_char(next)
                {
                    text.push(next);
                    rest = &after[next.len_utf8()..];
                    continue;
                }
                if '\\' == next {
                    text.push('\\');
                    rest = &after[1..];
                    continue;
                }
            }
        }

        text.push(c);
        rest = &rest[c.len_utf8()..];
    }

    // Resolve the value keywords on the WHOLE trimmed value, exactly as
    // hoover's endofline block does for the values it lexes itself.
    let value = config.resolve(tabnas_hoover::js_trim(&text));

    // Owned, which releases the borrow of the lexer's source before the
    // cursor is moved. It is what the token carries in any case.
    let consumed: String = source[..source.len() - rest.len()].to_string();
    let advance = consumed.chars().count();

    let point = lexer.point();
    lexer.advance_chars(advance);
    let tin = lexer.token_tin("#HV");
    let mut token = lexer.token("#HV", tin, value, consumed, point);
    token
        .use_data_mut()
        .insert("block".to_string(), Value::String("endofline".to_string()));
    Some(token)
}
