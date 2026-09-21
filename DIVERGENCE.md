# Divergences

Where a port does not reproduce the canonical TypeScript, it is recorded
here with a MEASURED table rather than described in prose. The shared
fixtures in [`test/spec`](test/spec) hold all three runtimes to one
behaviour, so nothing recorded here may reach one of those files.

Measurements below were taken on 2026-09-21 with the checkouts beside
this repository: `@tabnas/parser` and `@tabnas/jsonic` from `ts/`,
`github.com/tabnas/parser` 0.9.0 from `go/`, and `tabnas` 0.10.0 from
`rs/`.

## The Rust port

### Nesting past 127 levels is refused

A section header of more than `DEPTH_LIMIT` (127) segments is rejected
with the engine's `cancel` code. Counted alongside the `map` and `list`
rules, so a document nesting through both is bounded once.

| input | TypeScript | Go | Rust |
|---|---|---|---|
| `[a.a. ... ]` 127 deep, then `x=1` | `{"a":{...}}`, 771 bytes | same, 771 bytes | same, 771 bytes |
| 128 deep | `{...}`, 777 bytes | same, 777 bytes | `ERROR:cancel` |
| 1000 deep | `{...}`, 6009 bytes | same, 6009 bytes | `ERROR:cancel` |
| 5000 deep | `RangeError: Maximum call stack size exceeded` | `{...}`, 30009 bytes | `ERROR:cancel` |
| 10000 deep | not reached | `{...}`, 60009 bytes | **aborted the process** without this limit |

The engine parses iteratively, but displaying, converting or dropping a
`Value` walks the tree with the call stack. A header ten thousand
segments deep overflowed the stack and aborted the process: not an
error a caller can catch, and not a failure mode a library may have on
untrusted input. The limit turns it into an ordinary coded rejection.

The row at 5000 is the reason the limit is not simply a Rust weakness.
The canonical implementation has no limit and no crash guard either; it
reaches a `RangeError` at some depth between 1000 and 5000, with no
error code and no position. Go, whose stack grows, keeps going.

127 is the number `tabnas-json` and `tabnas-jsonic` already use, and the
one `serde_json` accepts, so every crate in the family bounds nesting
alike. No real configuration file comes near it.

Owner: this port. The repair, if a caller ever needs deeper documents,
is an iterative `to_json` and `Drop` in the engine, after which the limit
can be raised or removed here.

Pinned by `nesting_past_the_depth_limit_is_refused` in
[`rs/tests/ini_test.rs`](rs/tests/ini_test.rs). It cannot be a shared
fixture: the limit is this port's alone, and a fixture row would have to
be green in three runtimes.

### A section header that reopens a key holding a value

A document can name a key and then name a SECTION at the same path. The
canonical implementation walks the path with
`r.node[k] = r.node[k] || node()`, which keeps a truthy value where it
stands: the walk then continues from a string, and writing a property to
a string is a `TypeError` under the strict mode a compiled module runs
in. The error carries no code and no position, and it is not a parse
failure a caller can tell from any other.

| input | TypeScript | Go | Rust |
|---|---|---|---|
| `a=1` then `[a]` | `{"a":"1"}` | `{"a":{}}` | `{"a":{}}` |
| `a=1` then `[a]` then `x=2` | `TypeError: Cannot create property 'x' on string '1'` | `{"a":{"x":"2"}}` | `{"a":{"x":"2"}}` |
| `a=1` then `[a.b]` then `x=2` | `TypeError: Cannot create property 'b' on string '1'` | `{"a":{"b":{"x":"2"}}}` | `{"a":{"b":{"x":"2"}}}` |

This port replaces the value with the section, as the Go port does. A
host exception that escapes the parser is not a behaviour worth
reproducing, and the two ports that do not have one agree with each
other.

Owner: the canonical port. The repair is to decide what the dialect
means here and say so in `ts/doc/reference.md`: either keep the value and
refuse the header with a code, or replace it as the two ports do. This
row goes when that lands.

Pinned by `a_section_header_may_reopen_a_key_that_holds_a_value` in
[`rs/tests/ini_test.rs`](rs/tests/ini_test.rs). It cannot be a shared
fixture either: the row would have to be green in three runtimes, and
one of them raises a host error.

### A number that overflows a double inside a composite

A single-quoted value is read as JSON, and `JSON.parse` rounds a number
literal too large for a double to an infinity. `serde_json` refuses that
literal instead, with `number out of range`.

Where the whole value is the literal, this port reads it with the JSON
number grammar and hands back the same infinity. Where the literal sits
INSIDE an array or an object, it cannot: the refusal comes from
`serde_json`'s scanner, before any visitor sees the number, and
`serde_json::Number` cannot hold a non-finite value at all, so there is
nothing to build the composite out of. The value then stays the source
text.

| input | TypeScript | Go | Rust |
|---|---|---|---|
| `a = '1e400'` | `{"a":null}`, the number `Infinity` | `{"a":"1e400"}` | `{"a":null}`, the number `Infinity` |
| `a = '-1e400'` | `{"a":null}`, the number `-Infinity` | `{"a":"-1e400"}` | `{"a":null}`, the number `-Infinity` |
| `a = '1e-400'` | `{"a":0}` | `{"a":0}` | `{"a":0}` |
| `a = '[1e400]'` | `{"a":[null]}`, the array holds `Infinity` | `{"a":"[1e400]"}` | `{"a":"[1e400]"}` |
| `a = '{"b":1e400}'` | `{"a":{"b":null}}`, `b` holds `Infinity` | `{"a":"{\"b\":1e400}"}` | `{"a":"{\"b\":1e400}"}` |

The first three rows are the cases this port now reproduces, and are in
the table so the boundary is visible: only the last two diverge.
`JSON.stringify` writes `null` for an infinity, which is why the
TypeScript column reads `null`; the parsed value is the infinity, and the
note beside each cell says so.

Owner: this port. The repair is a JSON reader that can express a
non-finite number, either `serde_json` growing one or this crate carrying
its own; a hand-written reader was declined here, because it would trade
one documented gap for a whole untested implementation of string escapes,
surrogate pairs and duplicate keys.

Pinned by `a_number_that_overflows_inside_a_composite_keeps_its_text` in
[`rs/tests/ini_test.rs`](rs/tests/ini_test.rs), next to
`a_single_quoted_number_too_large_for_a_double_is_infinity`, which pins
the case that IS reproduced. It cannot be a shared fixture: the row would
have to be green in three runtimes, and two of them keep the text.

## Inherited, and owned elsewhere

These are not ini's behaviour. They are recorded because a reader of this
repository can meet them through it, and each is repaired in the
repository that owns it.

### A very large or very small number renders differently

The parsed VALUE is identical in all three runtimes: one IEEE double. The
difference is in how the engine's `Value` spells it back out, and it
shows only for a number a single-quoted value hands to the JSON reader,
since `number.lex` is off in INI.

| input | TypeScript | Rust `Value::to_string` |
|---|---|---|
| `a = '{"y":6}'` | `{"a":{"y":6}}` | `{"a":{"y":6}}` |
| `a = '[1,2.5,3]'` | `{"a":[1,2.5,3]}` | `{"a":[1,2.5,3]}` |
| `a = '1e21'` | `{"a":1e+21}` | `{"a":1000000000000000000000}` |
| `a = '0.0000001'` | `{"a":1e-7}` | `{"a":0.0000001}` |
| `a = '1e400'` | `{"a":null}` | `{"a":inf}` |

ECMA-262 6.1.6.1.20 switches to exponent form at 1e21 and at 1e-7; the
engine's formatter does not, and it spells an infinity `inf` where
JavaScript spells it `Infinity`. The last row is the one the TypeScript
column renders through `JSON.stringify`, which writes `null` for an
infinity; `Value::to_json` writes `null` for it too, so the two agree
everywhere except the engine's own display. The shared fixtures compare values rather
than their rendering, so no row is affected, and the one place this port
spells a number itself (the fixed-token concatenation in the `val`
after-close hook) uses the specification's algorithm.

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

## The Go port

`conformanceGoParityGap` in
[`go/ini_conformance_test.go`](go/ini_conformance_test.go) is empty, and
every shared fixture passes in all three runtimes. The gaps below sit
outside both sets. Each was measured rather than inferred, and is
recorded here rather than left as a surprise for whoever meets it.

The last three were found while repairing the same three defects in the
Rust port, where the canonical TypeScript was measured for each one. The
Rust port now matches the canonical on all of them; the Go port does not
yet, so none of the three may reach a shared fixture until it does.

### A line holding nothing but a value keyword

`@pair-key-bool` sets the key only when the matched token carries a
STRING (`'string' === typeof key`). A line holding `true`, `false` or
`null` and nothing else does not: both value lexers resolve a keyword
that is the whole span, so the token carries the boolean or the null, and
the canonical port declares no key at all.

| input | TypeScript | Go | Rust |
|---|---|---|---|
| `true` | `{}` | `{"true":true}` | `{}` |
| `false` | `{}` | `{"false":true}` | `{}` |
| `null` | `{}` | `{"null":true}` | `{}` |
| `mykey` | `{"mykey":true}` | `{"mykey":true}` | `{"mykey":true}` |

The Go port reads the token's SOURCE when its value is not a string,
which turns the keyword back into the key TypeScript declined to make.
No shared fixture covers it: `test/spec/bare-key.tsv` uses bare keys that
are text and quoted keys that are strings, and both runtimes agree on
every one of them.

TypeScript is canonical, so the Rust port follows TypeScript. Owner: the
Go port. The repair is one condition in `tokenString`'s caller, plus a
row in `bare-key.tsv` once all three agree.

### An explicitly empty inline comment marker list

`resolve` takes `ic.Chars != nil && len(ic.Chars) > 0` as "the caller
named markers", so an EMPTY list falls back to `#` and `;`. The canonical
`_options.comment?.inline?.chars ?? ['#', ';']` defaults on absence
alone, so an empty list leaves inline comments active with no character
that starts one.

Measured with `comment.inline.active` true.

| input | options | TypeScript | Go | Rust |
|---|---|---|---|---|
| `a=x;y` | `chars` absent | `{"a":"x"}` | `{"a":"x"}` | `{"a":"x"}` |
| `a=x;y` | `chars: []` | `{"a":"x;y"}` | `{"a":"x"}` | `{"a":"x;y"}` |
| `a=x#y` | `chars: []` | `{"a":"x#y"}` | `{"a":"x"}` | `{"a":"x#y"}` |

Owner: the Go port. The repair is dropping the length test, plus a row in
`inline-comments-custom-chars.tsv` once all three agree.

### A composite in the fixed-token concatenation

A value that starts with `[`, `]`, `=` or `.` concatenates that token
with the JavaScript `String()` of the rest, and a single-quoted value is
read as JSON first, so the rest can be an array or an object. The
canonical coercion joins an array's elements with a comma, flattens a
nested one, contributes nothing for a null or undefined ELEMENT, and
renders an object `[object Object]`. Go formats the Go value with `%v`
instead.

| input | TypeScript | Go | Rust |
|---|---|---|---|
| `a = ='[1,2]'` | `{"a":"=1,2"}` | `{"a":"=[1 2]"}` | `{"a":"=1,2"}` |
| `a = ='[]'` | `{"a":"="}` | `{"a":"=[]"}` | `{"a":"="}` |
| `a = ='[1,[2,3]]'` | `{"a":"=1,2,3"}` | `{"a":"=[1 [2 3]]"}` | `{"a":"=1,2,3"}` |
| `a = ='[null,true]'` | `{"a":"=,true"}` | `{"a":"=[<nil> true]"}` | `{"a":"=,true"}` |
| `a = ='{"b":1}'` | `{"a":"=[object Object]"}` | `{"a":"=map[b:1]"}` | `{"a":"=[object Object]"}` |

This is the array-join defect class the csv port met and repaired; the
shape of the repair is `jsKey` and `jsArrayKey` in
`github.com/tabnas/csv/go`, and `js_string` in
[`rs/src/lib.rs`](rs/src/lib.rs) here.

Owner: the Go port. The repair needs `jsNumberToString` as well, since a
number inside the array goes through the ECMA-262 formatter and not Go's.

### A single-quoted number too large for a double

`encoding/json` refuses a number literal that overflows a double, as
`serde_json` does, and `tryParseJSON` then keeps the source text. The
canonical `JSON.parse` rounds it to an infinity. The Rust half of this is
recorded above, with the composite case it still shares.

| input | TypeScript | Go | Rust |
|---|---|---|---|
| `a = '1e400'` | `{"a":null}`, the number `Infinity` | `{"a":"1e400"}` | `{"a":null}`, the number `Infinity` |
| `a = '-1e400'` | `{"a":null}`, the number `-Infinity` | `{"a":"-1e400"}` | `{"a":null}`, the number `-Infinity` |

Owner: the Go port. Go can hold `math.Inf(1)`, so the repair is the one
this port made: recognise a JSON number literal `encoding/json` refused
and parse it with `strconv.ParseFloat`.
