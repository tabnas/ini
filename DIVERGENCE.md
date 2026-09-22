# Divergences

Where a port does not reproduce the canonical TypeScript, it is recorded
here with a MEASURED table rather than described in prose. The shared
fixtures in [`test/spec`](test/spec) hold all three runtimes to one
behaviour, so nothing recorded here may reach one of those files.

Measurements below were taken on 2026-09-22 with the checkouts beside
this repository: `@tabnas/parser` and `@tabnas/jsonic` from `ts/`,
`github.com/tabnas/parser` 0.9.0 from `go/`, and `tabnas` 0.10.0 from
`rs/`.

## The Rust port

### An escaped lone surrogate in a single-quoted value

A single-quoted value is read as JSON, and `JSON.parse` accepts an
escaped lone surrogate: a JavaScript string is UTF-16, so `"\ud800"` is
an ordinary one-code-unit string. A Rust `String` is scalar values and
cannot hold one, so `serde_json` refuses the whole document.

The fallback then kept the JSON SOURCE as the value, which lost the TYPE
along with the character: `a = '["\ud800"]'` stopped being an array at
all. This port now rewrites every UNPAIRED surrogate escape to `�`
and reads the repaired text, which is what Go's `encoding/json` does, so
the shape of the value survives and one character differs. A surrogate
PAIR is untouched and still decodes to its one character.

| input | TypeScript | Go | Rust |
|---|---|---|---|
| `a = '"\\ud800"'` | one code unit, U+D800 | U+FFFD | U+FFFD |
| `a = '"\\udfff"'` | one code unit, U+DFFF | U+FFFD | U+FFFD |
| `a = '"\\ud800x"'` | U+D800 then `x` | U+FFFD then `x` | U+FFFD then `x` |
| `a = '["\\ud800"]'` | an ARRAY of one such string | an ARRAY, U+FFFD | an ARRAY, U+FFFD |
| `a = '{"k":"\\ud800"}'` | an OBJECT, `k` holds it | an OBJECT, U+FFFD | an OBJECT, U+FFFD |
| `a = '"\\ud83d\\ude00"'` | one character, U+1F600 | same | same |

The doubled backslash is the INI source: a single-quoted value decodes
its escapes before the JSON reader sees it, so `a = '"\\ud800"'` hands
the reader `"\ud800"`.

Owner: this port. The repair is a JSON reader whose strings are UTF-16
rather than scalar values, which is what a shared fixture would need as
well: the expected column cannot ask both runtimes the same question,
because each decodes the escape in its own string type and both would
pass.

Pinned by
`a_lone_surrogate_in_a_single_quoted_value_becomes_the_replacement_character`
in [`rs/tests/ini_test.rs`](rs/tests/ini_test.rs).

### A single-quoted JSON value nested past 127 levels

`serde_json` refuses to recurse further than 128 levels and reports
`recursion limit exceeded`. `JSON.parse` has no limit, and neither does
Go's `encoding/json`. The fallback keeps the source text, so a value
that is an array in the other two runtimes is a string here.

| input | TypeScript | Go | Rust |
|---|---|---|---|
| `a = '[[ ... ]]'` 127 deep | an array | an array | an array |
| 128 deep | an array | an array | the source text |
| 200 deep | an array | an array | the source text |

The limit is wanted rather than merely met. `Value::from_json` builds the
tree with the call stack, and `Value::to_json` and the default drop walk
it back down the same way, so lifting the limit would move the crash out
of this crate and into whoever converts or drops the result. 127 is the
number the section-header limit above uses, and the number `tabnas-json`
and `tabnas-jsonic` use, so every crate in the family bounds nesting
alike.

Owner: this port, jointly with the engine. The repair is an iterative
`to_json` and `Drop` in the engine, after which `serde_json`'s
`unbounded_depth` feature could be turned on here.

Pinned by
`a_single_quoted_json_value_nested_past_the_depth_limit_keeps_its_text`
in [`rs/tests/ini_test.rs`](rs/tests/ini_test.rs).

## Inherited, and owned elsewhere

These are not ini's behaviour. They are recorded because a reader of this
repository can meet them through it, and each is repaired in the
repository that owns it.

### A very large or very small number renders differently

The parsed VALUE is identical in all three runtimes: one IEEE double. The
difference is in how the engine's `Value` spells it back out, and it
shows only for a number a single-quoted value hands to the JSON reader,
since `number.lex` is off in INI.

The Go column is `encoding/json` rather than an engine display: the Go
port hands back a `map[string]any` and the caller marshals it, so there
is no `Value` in between.

| input | TypeScript | Go `json.Marshal` | Rust `Value::to_string` |
|---|---|---|---|
| `a = '{"y":6}'` | `{"a":{"y":6}}` | `{"a":{"y":6}}` | `{"a":{"y":6}}` |
| `a = '[1,2.5,3]'` | `{"a":[1,2.5,3]}` | `{"a":[1,2.5,3]}` | `{"a":[1,2.5,3]}` |
| `a = '1e21'` | `{"a":1e+21}` | `{"a":1e+21}` | `{"a":1000000000000000000000}` |
| `a = '0.0000001'` | `{"a":1e-7}` | `{"a":1e-7}` | `{"a":0.0000001}` |
| `a = '1e400'` | `{"a":null}` | refused: `unsupported value: +Inf` | `{"a":inf}` |

ECMA-262 6.1.6.1.20 switches to exponent form at 1e21 and at 1e-7; the
engine's formatter does not, and it spells an infinity `inf` where
JavaScript spells it `Infinity`. The last row is the one the TypeScript
column renders through `JSON.stringify`, which writes `null` for an
infinity; `Value::to_json` writes `null` for it too, so the two agree
everywhere except the engine's own display. Go's `encoding/json` refuses
an infinity outright rather than writing `null` for it, which is a
property of that marshaller and not of the parse: the value in the map
is `math.Inf(1)`, the same double the other two hold. The shared
fixtures compare values rather than their rendering, so no row is
affected, and the one place each port spells a number itself (the
fixed-token concatenation in the `val` after-close hook) uses the
specification's algorithm.

Owner: [`github.com/tabnas/parser`](https://github.com/tabnas/parser),
in its `Value` display.

### hoover's escape row and block token position

Recorded in
[`github.com/tabnas/hoover`](https://github.com/tabnas/hoover). After a
mapped escape the Rust port advances the row on the escaped SOURCE
character where the canonical implementation advances it on the
replacement, and the block token is positioned at its start rather than
its end. The hoovered value is the same, so neither reaches an INI parse
result; both can move the row and column of an error reported afterwards.

Owner: `github.com/tabnas/hoover`.

## Shared by the Go and Rust ports

### A number that overflows a double inside a composite

A single-quoted value is read as JSON, and `JSON.parse` rounds a number
literal too large for a double to an infinity. `encoding/json` and
`serde_json` both refuse that literal instead.

Where the whole value is the literal, both ports now recognise the JSON
number grammar themselves and hand back the same infinity. Where the
literal sits INSIDE an array or an object, neither can: the refusal comes
from the scanner, before any value is built, and `serde_json::Number`
cannot hold a non-finite value at all, so there is nothing to build the
composite out of. The value then stays the source text.

| input | TypeScript | Go | Rust |
|---|---|---|---|
| `a = '1e400'` | `{"a":null}`, the number `Infinity` | same | same |
| `a = '-1e400'` | `{"a":null}`, the number `-Infinity` | same | same |
| `a = '1e-400'` | `{"a":0}` | `{"a":0}` | `{"a":0}` |
| `a = '[1e400]'` | `{"a":[null]}`, the array holds `Infinity` | `{"a":"[1e400]"}` | `{"a":"[1e400]"}` |
| `a = '{"b":1e400}'` | `{"a":{"b":null}}`, `b` holds `Infinity` | `{"a":"{\"b\":1e400}"}` | `{"a":"{\"b\":1e400}"}` |

The first three rows are the cases both ports now reproduce, and are in
the table so the boundary is visible: only the last two diverge.
`JSON.stringify` writes `null` for an infinity, which is why the
TypeScript column reads `null`; the parsed value is the infinity, and the
note beside each cell says so.

Owner: both ports, jointly with their JSON readers. The repair is a JSON
reader that can express a non-finite number. A hand-written reader was
declined, because it would trade one documented gap for a whole untested
implementation of string escapes, surrogate pairs and duplicate keys.

Pinned by `TestNumberThatOverflowsInsideACompositeKeepsItsText` in
[`go/ini_test.go`](go/ini_test.go) and
`a_number_that_overflows_inside_a_composite_keeps_its_text` in
[`rs/tests/ini_test.rs`](rs/tests/ini_test.rs), each next to the test
that pins the scalar case both ports DO reproduce
(`TestSingleQuotedNumberTooLargeForADoubleIsInfinity` and
`a_single_quoted_number_too_large_for_a_double_is_infinity`). It cannot
be a shared fixture: the expected column is JSON, and JSON has no way to
spell an infinity.

## The Go port

`conformanceGoParityGap` in
[`go/ini_conformance_test.go`](go/ini_conformance_test.go) is empty, and
every shared fixture passes in all three runtimes.

Five gaps sat outside both sets and are now repaired, each with a shared
fixture or a runtime test standing where its table used to:

- a line holding nothing but a value keyword declared a key, where the
  canonical declares none (`test/spec/bare-key.tsv`);
- an explicitly empty inline comment marker list fell back to `#` and
  `;`, where the canonical defaults on absence alone
  (`test/spec/inline-comments-empty-chars.tsv`);
- an inline comment marker of more than one byte was truncated to its
  first byte, which cut a value at any character of the Latin-1
  supplement block (`test/spec/inline-comments-marker-width.tsv`), and a
  multi-character `multiline.continuation` was truncated the same way
  (`test/spec/multiline-continuation-width.tsv`);
- a composite in the fixed-token concatenation was formatted with `%v`
  rather than the JavaScript `String()`
  (`test/spec/value-fixed-token-start.tsv`);
- a single-quoted number too large for a double kept its text rather
  than rounding to an infinity
  (`TestSingleQuotedNumberTooLargeForADoubleIsInfinity` in
  `go/ini_test.go`; the composite half of that one is still open and is
  recorded above).
