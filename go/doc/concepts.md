# Concepts (Go)

Background on how `github.com/tabnas/ini/go` is built and why, plus how
it differs from the TypeScript original. This is
understanding-oriented reading — for steps see the
[tutorial](tutorial.md) and [how-to guide](guide.md), and for exact
signatures and options see the [reference](reference.md).

## A grammar plugin, not a parser

The package does not contain a bespoke parser. It is a **plugin** for
[jsonic](https://github.com/tabnas/jsonic) (the Go port,
`github.com/tabnas/jsonic/go`), which is itself a relaxed-JSON grammar
running on the [tabnas](https://github.com/tabnas/parser) engine. The
stack is three layers:

- **tabnas** — the engine: a rule-based parser over a configurable,
  matcher-based lexer.
- **jsonic** — the relaxed-JSON grammar and all the lexer matchers
  (strings, numbers, comments, fixed tokens).
- **ini** — this package, which reconfigures jsonic into an INI parser.

`MakeJsonic` constructs a jsonic instance, then `iniPlugin` (in
[`ini.go`](../ini.go)) edits it into an INI parser. `Parse` is a thin
convenience wrapper that builds (or reuses) an instance and asserts the
result to `map[string]any`.

## What the plugin does to the engine

`iniPlugin` makes the same coordinated changes as the TS version:

1. **Re-points the start rule** to `ini`, excluding jsonic's own rules.
2. **Repurposes the fixed tokens.** `[` and `]` stay (section
   delimiters); `{`, `}`, and `:` are removed; `=` (`#EQ`) and `.`
   (`#DOT`) are added.
3. **Turns matchers off and on.** Number and text lexing are disabled
   so a bare value is one string; the string matcher keeps only `'`
   and `"`; line comments for `#` and `;` eat the rest of the line.
4. **Installs the INI grammar.** The rules `ini`, `table`, `dive`,
   `map`, and `pair` are authored once in
   [`ini-grammar.jsonic`](../../ini-grammar.jsonic), embedded into the
   source as a string, parsed by a separate jsonic instance, and
   applied with `j.Grammar(...)`. The `val` rule is defined in Go code
   because it needs custom open alternates and an after-close handler
   the grammar file cannot express.
5. **Adds value/key matchers via Hoover.** The
   [hoover](https://github.com/tabnas/hoover) plugin supplies the
   "scan to a terminator" matchers that read keys (`#HK`), values
   (`#HV`), and section path segments (`#DK`).
6. **Prunes unreachable rules.** INI has no array literal syntax, so
   jsonic's inherited `list` and `elem` rules are removed
   (`j.Rule(name, nil)`), keeping the live grammar — and the railroad
   diagram generated from it — limited to what INI uses.

The grammar is **data** shared between the two ports: the same
`ini-grammar.jsonic` is embedded into both `ts/src/ini.ts` and
`go/ini.go` by the embed step, so the rule structure cannot drift.

## The grammar model

The engine parses with named **rules**, each having an **open** and a
**close** phase, each phase a list of **alternates**. An alternate
matches a short token pattern (at most two tokens of lookahead) and may
run an action, push a child rule, replace the current rule, or
backtrack a token. There is no backtracking search — parsing is linear
and deterministic. INI's five rules:

- **`ini`** — start rule; sets up the root map and dispatches to
  `table`.
- **`table`** — one section's content: an optional `[...]` header (via
  `dive`) then a `map` of pairs; loops over successive sections.
- **`dive`** — reads a `[a.b.c]` header into a path slice.
- **`map`** — a run of `pair`s in the current section.
- **`pair`** — one `key = value` (or a bare boolean key).
- **`val`** — the right-hand side; scalars plus the occasional embedded
  map.

### Sections and the `dive` rule

For `[server.production]`, `dive` pushes `server` then `production`
onto a `[]string` path; `table`'s state action walks that path from the
root map, creating intermediate maps as needed, and points the
section's `map` at the deepest one. A literal dot is escaped (`\.`) so
it stays in one segment — `[x\.y\.z]` is the single key `x.y.z`.

### Keys and values: the Hoover matchers

INI values are "whatever is left on the line," so this package uses
hoover's **scan-to-terminator** matchers rather than character-class
tokenizing: the key matcher reads to `=`/newline/end-of-input, the
value matcher (`endofline`) reads to the newline handling escapes, and
the divekey matcher reads a section segment to `.`/`]`. When multiline
or whitespace-prefix inline comments are on, a small **custom value
matcher** runs at higher priority to apply continuation /
comment-termination before falling through to hoover.

## Value resolution

After a value's raw text is read, the `val` rule's after-close handler
resolves it:

1. A single-quoted value is JSON-decoded (`encoding/json`) when valid —
   `'{"y":{"z":6}}'` becomes a real `map[string]any` (with `z` as
   `float64(6)`). Invalid JSON keeps the inner text.
2. A leading bracket character split off by the fixed-token lexer is
   re-concatenated, so `j0 = ]3,4[` stays the string `"]3,4["`.
3. The keywords `true`, `false`, `null` resolve to `bool` / `bool` /
   `nil`.
4. Everything else is a trimmed `string`; numeric text stays a string
   unless number lexing is enabled.

## Accepted vs rejected — edge cases

Pinned by the test suite (`ini_test.go`):

- **Bare key ⇒ `true`** once the map exists: `a=1\nmykey` ⇒
  `{"a": "1", "mykey": true}`. A bare key as the very first token of the
  root or a section is rejected.
- **`=` in a value is literal**: only the first `=` splits the pair, so
  `u = v = 5` ⇒ `{"u": "v = 5"}`.
- **Empty value ⇒ `""`**: `a=\nb=` ⇒ `{"a": "", "b": ""}`.
- **Numeric values are strings by default**: `0xFF` ⇒ `"0xFF"`.
- **Intermediate sections are not "declared"**, so under
  `Duplicate: "error"`, `[a.b]` then `[a]` does not error.
- **Mid-value `;`/`#` are literal** unless inline comments are active;
  line-leading `;`/`#` are always comments.
- **No array literal syntax**: `[1,2]` parses to the string `"[1,2]"`;
  arrays come only from `key[] =`.

## Differences from the TS version

The TypeScript implementation is authoritative; this is a faithful
port. Both ports embed the same grammar file and pass the same shared
`.tsv` conformance fixtures (`test/spec/*.tsv`), so a *successful* parse
produces equivalent values. The differences are in API shape, host
types, and a few mechanics.

### API shape

| | TypeScript | Go |
|---|---|---|
| Entry | `new Tabnas().use(jsonic).use(Ini, opts?)` then `.parse(src)` | `tabnasini.Parse(src, opts...)` or `tabnasini.MakeJsonic(opts...).Parse(src)` |
| Plugin form | `Ini` is a function you pass to `use` | applied internally by `MakeJsonic`; there is no exported `use`-style plugin entry |
| Options | one `IniOptions` object with optional fields | `IniOptions` struct with **pointer** leaf fields (`nil` = default), passed as a variadic argument |
| Number lexing | `j.options({ number: { lex: true } })` | `j.SetOptions(tabnasjsonic.Options{Number: &tabnasjsonic.NumberOptions{Lex: boolp(true)}})` |

### Value types

| Value | TypeScript | Go |
|---|---|---|
| Object / section | plain object | `map[string]any` |
| Array | JS array | `[]any` |
| String | `string` | `string` |
| `true` / `false` | `boolean` | `bool` |
| `null` / empty result | `null` (and `{}` for `''`) | `nil` (and `map[string]any{}` for `''`) |
| Number (lexing on) | JS `number` | `float64` |
| Single-quoted JSON number (e.g. `'…6…'`) | JS `number` `6` | `float64(6)` |

### Error handling

- **Duplicate section under `error`.** TS throws synchronously
  (`Error: Duplicate section: [..]`). Go raises it inside a state-action
  `panic`, which the engine recovers and surfaces as a non-`nil`
  `error` from `Parse` (older engine builds may surface a raw panic).
  The test helper accepts either; in your code, check `err`.
- **Parse failures generally.** TS `parse` throws; Go `Parse` returns
  an `error` and does not panic for ordinary syntax errors.

### Known internal mechanics

- **Dive-path propagation.** In TS the section path is built with
  `Array.push`, which mutates the shared array in place. Go's `append`
  may reallocate the backing array, so the Go port adds an extra
  `@dive-bc` state action to propagate the (possibly new) slice from a
  child `dive` up to its parent. This is invisible in the output but is
  why the Go grammar wiring has one more handler than the TS one.
- **The `#CL` (colon) close-error alternate.** The shared grammar file
  carries a `pair`-close error alternate keyed on `#CL`, and the grammar
  disables the colon token (`'#CL': null`) in both ports — so that
  alternate (`@pair-close-err`) is never reached. The difference is only
  that the Go handler is written as an explicit no-op.

For the canonical (TS) explanation of the design, see
[../../ts/doc/concepts.md](../../ts/doc/concepts.md).
