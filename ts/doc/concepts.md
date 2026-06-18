# Concepts

Background on how `@tabnas/ini` is built and why. This is
understanding-oriented reading — for steps see the
[tutorial](tutorial.md) and [how-to guide](guide.md), and for exact
signatures and options see the [reference](reference.md).

## A grammar plugin, not a parser

`@tabnas/ini` does not contain a parser. It is a **plugin** for
[jsonic](https://github.com/tabnas/jsonic), which is itself a grammar
running on the [tabnas](https://github.com/tabnas/parser) engine. The
stack is three layers:

- **tabnas** — the engine: a rule-based parser over a configurable,
  matcher-based lexer.
- **jsonic** — the relaxed-JSON grammar applied to that engine, plus
  all the lexer matchers (strings, numbers, comments, fixed tokens).
- **Ini** — this plugin, which *reconfigures* jsonic into an INI
  parser.

When you write `new Tabnas().use(jsonic).use(Ini)` you are stacking
those layers in order. `Ini` must come after `jsonic` because it edits
the grammar jsonic installed.

## What the plugin does to the engine

The `Ini` function (in [`src/ini.ts`](../src/ini.ts)) makes a handful
of coordinated changes:

1. **Re-points the start rule.** The grammar's start becomes `ini`
   instead of jsonic's `val`, and jsonic's own rules are excluded from
   the start.
2. **Repurposes the fixed tokens.** `[` and `]` stay (they delimit
   sections); `{`, `}`, and `:` are dropped; `=` (`#EQ`) and `.`
   (`#DOT`) are added.
3. **Turns matchers off and on.** Number and text lexing are disabled
   (so a bare value is one string, not a number); the string matcher
   keeps only `'` and `"`; line comments for `#` and `;` are kept and
   set to eat the rest of the line.
4. **Installs the INI grammar.** The rules `ini`, `table`, `dive`,
   `map`, and `pair` are authored in
   [`ini-grammar.jsonic`](../../ini-grammar.jsonic), embedded into the
   source as a string, parsed by a *separate* jsonic instance, and
   applied with `tn.grammar(...)`. The `val` rule is defined in code
   because it needs an injection filter the grammar file cannot express.
5. **Adds value/key matchers via Hoover.** The
   [`@tabnas/hoover`](https://github.com/tabnas/hoover) plugin supplies
   the "scan to a terminator" matchers that read keys (`#HK`), values
   (`#HV`), and section path segments (`#DK`).
6. **Prunes unreachable rules.** INI has no array literal syntax, so
   jsonic's inherited `list` and `elem` rules can never be reached.
   They are removed (`tn.rule(name, null)`) so the live grammar — and
   the railroad diagram generated from it — contains only what INI
   uses. (This is why the diagram has no `list`/`elem`.)

The grammar is **data**. Editing `ini-grammar.jsonic` and re-running
the embed step (`npm run build`) is how the rules change; the two
embedded copies (TS and Go) are generated, never hand-edited.

## The grammar model

The engine parses with named **rules**, each having an **open** and a
**close** phase, each phase a list of **alternates**. An alternate
matches a short token pattern (at most two tokens of lookahead) and may
run an action, push a child rule, replace the current rule, or
backtrack a token. There is no search and no backtracking beyond that
lookahead — parsing is linear and deterministic. INI's five rules:

- **`ini`** — the start rule. Sets up the root object and dispatches to
  `table`.
- **`table`** — handles one section's worth of content: an optional
  `[...]` header (via `dive`) followed by a `map` of pairs. It loops to
  consume successive sections.
- **`dive`** — reads a `[a.b.c]` header into a path array, one `#DK`
  segment per dot level.
- **`map`** — a run of `pair`s belonging to the current section.
- **`pair`** — one `key = value` (or a bare boolean key).
- **`val`** — the right-hand side; reduced to scalars and the
  occasional embedded map.

### Sections and the `dive` rule

`dive` is where dot-nesting happens. For `[server.production]` it
pushes `server` then `production` onto a path array; the `table` rule's
state action then walks that path from the root object, creating
intermediate objects as needed, and points the section's `map` at the
deepest one. A literal dot is escaped (`\.`) so it stays inside a single
segment — `[x\.y\.z]` is the single key `x.y.z`, not a three-level path.

### Keys and values: the Hoover matchers

Most parsers tokenize values by character classes. INI values are
"whatever is left on the line," so this plugin uses Hoover's
**scan-to-terminator** matchers instead:

- the **key** matcher reads until `=`, a newline, or end of input;
- the **value** matcher (`endofline`) reads to the newline, handling
  escapes;
- the **divekey** matcher reads a section segment until `.` or `]`.

When `multiline` or whitespace-prefix inline comments are on, a small
**custom value matcher** is installed at a higher priority than
Hoover's, so it can intercept the value and apply continuation /
comment-termination logic before falling through.

## Value resolution

After a value's raw text is read, the `val` rule resolves it (see
[reference §Values](reference.md#values)):

1. A single-quoted value is `JSON.parse`d when valid — this is how
   `'{"y":{"z":6}}'` becomes a real object and `'[]'` becomes `[]`. If
   the JSON is invalid the inner text is kept.
2. A leading bracket character (`[` or `]`) that the fixed-token lexer
   split off is re-concatenated with the rest of the value, so
   `j0 = ]3,4[` stays the string `']3,4['`.
3. The bare keywords `true`, `false`, `null` resolve to their JS types.
4. Everything else is a trimmed string. Numbers are strings too
   (`a=1` ⇒ `'1'`) because the number matcher is off; turn it back on
   with `number.lex` if you want real numbers.

Double-quoted values go through jsonic's string lexer, so escapes and
spaces survive and brackets inside are literal — that is why
`"[disturbing]"` is a key spelling, not a section.

## Accepted vs rejected — edge cases

These follow from the model above and are pinned by the test suite:

- **Bare key ⇒ `true`.** A key with no `=` sets `key: true` — but only
  once the surrounding `map` exists, which a prior `key = value` pair
  establishes. So `a=1\nmykey` ⇒ `{ a: '1', mykey: true }`, while a
  bare key as the *first* token of the root or a section (`mykey`,
  `[s]\nmykey`) is rejected with an `unexpected` error.
- **`=` in a value is literal.** Only the first `=` splits key from
  value; `u = v = 5` ⇒ `{ u: 'v = 5' }`.
- **Empty value ⇒ `''`.** `a =` and `a` at end-of-input both resolve to
  an empty string for the value position; `a=\nb=` ⇒ `{ a: '', b: '' }`.
- **Numeric values are strings by default.** `0xFF` ⇒ `'0xFF'` until
  `number.lex` is enabled (then `255`).
- **Intermediate sections are not "declared".** `[a.b]` creates `a` on
  the way to `b`, but does not declare `[a]`; so under
  `duplicate: 'error'`, `[a.b]` then `[a]` does **not** throw.
- **Mid-value `;`/`#` are literal** unless inline comments are active;
  line-leading `;`/`#` are always comments.
- **No array literal syntax.** There is no `key = [1,2]` array form
  (`[1,2]` parses to the string `'[1,2]'`); arrays come only from the
  `key[] =` append syntax. This is the reason `list`/`elem` are pruned.

## Why this design

Building INI as a jsonic plugin rather than a bespoke parser buys three
things. The relaxed-JSON machinery (quoted strings with escapes,
JSON-decoding of single-quoted values, the keyword set) comes for free.
The grammar stays declarative data in one `.jsonic` file shared by both
language ports, so TS and Go cannot drift apart in structure. And the
same engine introspection that produces jsonic's railroad diagrams
produces this plugin's — the diagram you see is the grammar that
actually runs.

For how the Go port differs from this behavior, see
[../../go/doc/concepts.md](../../go/doc/concepts.md#differences-from-the-ts-version).
