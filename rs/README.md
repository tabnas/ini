# tabnas-ini (Rust)

The [INI](https://en.wikipedia.org/wiki/INI_file) grammar plugin for the
[`tabnas`](https://github.com/tabnas/parser) parsing engine, crate
`tabnas_ini`.

It reads INI text into plain values: `[section]` headers, dotted section
paths (`[a.b.c]`), `key = value` pairs, repeated-key arrays
(`tags[] = x`), bare boolean keys, quoted values, line and inline
comments, and values that span lines by backslash continuation or by
indent.

It is not standalone. The relaxed-JSON core (`val` / `map` / `pair`)
comes from the [`tabnas-jsonic`](https://github.com/tabnas/jsonic)
plugin; this crate installs the INI rules over it, sets the start rule to
`ini`, and lexes keys, values and section-path segments with
[`tabnas-hoover`](https://github.com/tabnas/hoover), because an INI value
is not a JSON value.

This is the Rust port of the canonical TypeScript implementation in
[`../ts`](../ts); the TypeScript version is authoritative and this crate
tracks it. The Go port is in [`../go`](../go). All three embed the same
grammar, authored once in
[`../ini-grammar.jsonic`](../ini-grammar.jsonic).

## Use

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let value = tabnas_ini::parse("[server]\nhost = localhost\nport = 8080")?;
    assert_eq!(
        value.to_string(),
        r#"{"server":{"host":"localhost","port":"8080"}}"#
    );
    Ok(())
}
```

Numbers stay strings. INI has no number syntax, so `port = 8080` is the
text `8080`, and `2.5`, `-3` and `0xFF` are the text a reader wrote.
The keywords `true`, `false` and `null` are values when they are the
WHOLE value, and text when they are not:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(tabnas_ini::parse("a = true")?.to_string(), r#"{"a":true}"#);
    assert_eq!(
        tabnas_ini::parse("a = true false")?.to_string(),
        r#"{"a":"true false"}"#
    );
    Ok(())
}
```

`parse` reuses one shared instance, because building the grammar costs
much more than a parse. To hold your own instance, build one:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = tabnas_ini::make();
    assert_eq!(
        parser.parse("[a.b]\nx = 1")?.to_string(),
        r#"{"a":{"b":{"x":"1"}}}"#
    );
    assert_eq!(
        parser.parse("tags[] = one\ntags[] = two")?.to_string(),
        r#"{"tags":["one","two"]}"#
    );
    assert_eq!(parser.parse("debug")?.to_string(), r#"{"debug":true}"#);
    Ok(())
}
```

Options are a typed struct with a default per field, so a parser with one
setting changed names that setting and nothing else:

```rust
use tabnas_ini::{make_with, Duplicate, IniOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Inline comments are OFF by default, because a `;` or `#` inside a
    // value is a literal in many INI dialects.
    let plain = tabnas_ini::make();
    assert_eq!(
        plain.parse("a = x ; note")?.to_string(),
        r#"{"a":"x ; note"}"#
    );

    let commented = make_with(&IniOptions::default().with_inline_comments());
    assert_eq!(commented.parse("a = x ; note")?.to_string(), r#"{"a":"x"}"#);

    // A repeated section header merges by default, and can replace or be
    // refused instead.
    let refused = make_with(&IniOptions::default().with_duplicate(Duplicate::Error));
    assert_eq!(
        refused.parse("[a]\nx = 1\n[a]\ny = 2").unwrap_err().code,
        "duplicate_section"
    );
    Ok(())
}
```

A value spans lines when `multiline` is set, by a trailing continuation
character, by an indented next line, or by both:

```rust
use tabnas_ini::{make_with, IniOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = make_with(&IniOptions::default().with_multiline());
    assert_eq!(
        parser.parse("a = one \\\n    two")?.to_string(),
        r#"{"a":"one two"}"#
    );
    Ok(())
}
```

To layer INI on an instance of your own, install the plugin, or call the
grammar function directly. Either way the instance must already carry the
jsonic grammar, which supplies the `val` rule both this plugin and hoover
extend:

```rust
use tabnas_ini::IniOptions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut parser = tabnas_jsonic::make();
    parser.use_plugin(tabnas_ini::plugin(IniOptions::default()), None)?;
    assert_eq!(parser.parse("a = 1")?.to_string(), r#"{"a":"1"}"#);

    let mut direct = tabnas_jsonic::make();
    tabnas_ini::ini(&mut direct, &IniOptions::default())?;
    assert_eq!(direct.parse("[t]\nx = 1")?.to_string(), r#"{"t":{"x":"1"}}"#);
    Ok(())
}
```

Parse errors are the engine's `TabnasError`, re-exported as `IniError`,
with `code`, `row`, `col` and a report that shows the offending source
with a caret under it. This plugin declares two codes of its own,
`duplicate_section` and `unterminated_section`; everything else surfaces
through the engine's own codes, `unexpected` for a malformed section
header and `cancel` for a document nested past the depth limit among
them. The CODE is the cross-runtime contract, and the message wording is
not.

## Install

None of these crates is published to a registry, so they are consumed as
**sibling checkouts**, the standard tabnas development model. Clone
`https://github.com/tabnas/parser`, `https://github.com/tabnas/jsonic`,
`https://github.com/tabnas/hoover` and `https://github.com/tabnas/json`
next to this repository, then point at the ones you name directly:

```toml
[dependencies]
tabnas-ini = { path = "../ini/rs" }
tabnas-jsonic = { path = "../jsonic/rs" }
tabnas = { path = "../parser/rs" }
```

All three entries are needed for the examples above. A crate's
dependencies are not passed on to its dependents, so `tabnas-ini` alone
does not put `tabnas` or `tabnas_jsonic` in your extern prelude. Only
`IniError` is re-exported.

`json` is the one checkout with no entry in that table, and it is not
optional: `tabnas-jsonic` reaches the strict-JSON core through its own
path dependency on `../../json/rs`. Cargo reads every manifest in the
graph before it compiles anything, so without that checkout the build
stops while resolving, and no example is ever reached.

Running this crate's own test suite needs a fifth checkout,
`https://github.com/tabnas/support`, which holds the shared fixture
loader and runner. It is a development dependency, so a crate that only
consumes the library does not need it.

## Differences from the canonical TypeScript

The shared fixtures in [`../test/spec`](../test/spec) hold all three
runtimes to one answer. What differs is the shape of the API and a few
points where the host language has no way to say what JavaScript says.
Any difference in what a document PARSES TO is recorded in
[`../DIVERGENCE.md`](../DIVERGENCE.md) instead.

- **The crate adds functions the plugin does not need.** TypeScript
  exports the plugin and nothing else: a caller builds an engine and
  registers `Ini` on it. This crate also carries `parse`, `parse_with`,
  `make` and `make_with`, which is the shape the Go port has, because a
  Rust caller that wants one line should not have to assemble three
  crates to get it. `ini` and `plugin` are the plugin itself, and are
  what the TypeScript export corresponds to.
- **Options are a typed struct.** TypeScript takes a plain object whose
  every field is optional; here the struct spells the same shape, an
  absent field is `None`, and `IniOptions::default()` is the TypeScript
  `{}`. `Duplicate` names the three settings TypeScript spells as
  strings, and `MultilineOptions::continuation` is `Some("")` where
  TypeScript writes `false`. The builders `with_multiline`,
  `with_inline_comments` and `with_duplicate` cover the common settings.
- **Section state is per parse.** The canonical ports keep the set of
  declared section headers in a closure shared by every parse, which is
  safe there because neither runtime parses one instance from two
  threads. A `Tabnas` is `Send + Sync` and `parse` shares one instance,
  so the set lives on the parse context here.
- **A section node is rebuilt, not referenced.** A `Value` container sits
  behind an `Arc` and copies when a second handle writes to it, so a
  table rule cannot hold a live handle on its section the way a
  JavaScript object reference does. The section is written back into the
  document as the rule closes, and the document that comes out is the
  same one.
- **The lexer hooks read a probe.** The four `check` hooks all ask where
  the parser is, which TypeScript reads from `lex.ctx.rule`. A Rust check
  is handed the lexer alone, so a custom matcher ordered below every
  built-in family records the rule for them, one step earlier in the same
  token attempt.
- **Nesting is bounded.** A section path of a few thousand segments
  builds a value tree the engine walks with the call stack, so a parse
  budget refuses one deeper than 127 levels with the engine's `cancel`
  code. Every runtime bounds it at the same number.
- **Key order is document order**, because the result is built on an
  `IndexMap`.
- **An escaped lone surrogate becomes U+FFFD.** A single-quoted value is
  read as JSON, and `JSON.parse` keeps a lone surrogate because a
  JavaScript string is UTF-16. A Rust `String` cannot hold one, so this
  port substitutes the replacement character, as Go's `encoding/json`
  does, which keeps the type and the shape of the value. A single-quoted
  JSON value nested past 127 levels keeps its source text, for the reason
  nesting is bounded above, where the other two runtimes read it as a
  value. Both are in `../DIVERGENCE.md`.
- **Columns count Unicode scalar values.** An astral character advances
  the column by one, where TypeScript counts UTF-16 units and advances by
  two. That is the engine's unit, recorded in its own `DIVERGENCE.md`.
- **Malformed UTF-8 cannot reach the parser.** The engine parses a
  `&str`, so a document that is not valid UTF-8 has to be decoded before
  it arrives.

## Build and test

The engine, the jsonic core, the hoover block lexer, and the fixture
runner are path dependencies on sibling checkouts, so there is nothing to
fetch:

```bash
cargo test --all-targets
```

Or, from the repository root, `make test-rs`. For what CI would say,
including formatting and the lockfile check, run `ci/rust/run.sh`.

The suite runs every shared `../test/spec/*.tsv` fixture, the same files
the TypeScript and Go suites run, discovered by listing the directory so
a new fixture runs everywhere at once, with a census test so a renamed
or deleted fixture is a failure rather than a silent loss of coverage. It
also runs the third-party corpus in `../test/corpus/ini-corpus.json`,
with the same divergence lists the other two suites carry and the same
canonical results in `../test/corpus/ini-canonical.json`. Beside them
are the in-language tests for what a fixture cannot express: the option
matrix, the API surface, the embedded grammar, the version constants,
hostile input, and the shared default parser under threads.

## License

MIT.
