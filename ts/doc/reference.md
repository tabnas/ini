# Reference

Complete, dry reference for `@tabnas/ini`: the public API, every
option with its default and effect, and the INI syntax the parser
accepts. For a guided introduction see the [tutorial](tutorial.md); for
task recipes see the [how-to guide](guide.md); for how it works see the
[concepts](concepts.md).

## Package

```bash
npm install @tabnas/ini @tabnas/parser @tabnas/jsonic @tabnas/hoover
```

Peer dependencies: `@tabnas/parser` (>=2), `@tabnas/jsonic` (>=2),
`@tabnas/hoover` (>=0).

## Exports

| Export | Kind | Description |
|---|---|---|
| `Ini` | value | The syntax plugin function. |
| `IniOptions` | type | The plugin's options object. |
| `InlineCommentOptions` | type | The `comment.inline` sub-object. |

```ts
import { Ini } from '@tabnas/ini'
import type { IniOptions, InlineCommentOptions } from '@tabnas/ini'
```

## `Ini` — the plugin

`Ini` is a [tabnas/jsonic](https://github.com/tabnas/jsonic) plugin.
There is no standalone `parse` function: you register `Ini` on an
engine and use that engine's `parse`.

```ts
const j = new Tabnas().use(jsonic).use(Ini, options?)
const result = j.parse(src)   // => a plain object (map)
```

- `jsonic` **must** be applied before `Ini` — `Ini` rewrites the
  jsonic grammar (it sets the start rule to `ini`, removes JSON
  structural tokens, and installs INI rules).
- `options` is an optional [`IniOptions`](#options) object.
- `j.parse(src)` returns an object. Empty input (`''`) returns `{}`.
- Values are strings unless they are the keywords `true`/`false`/`null`
  or a single-quoted JSON literal (see [Values](#values)). To lex
  numeric values as numbers, set `j.options({ number: { lex: true } })`
  after applying the plugin.

The plugin is idempotent and is re-run when you derive a child engine
with `make`/options, matching jsonic's plugin model.

## Options

```ts
type IniOptions = {
  multiline?: {
    continuation?: string | false   // default: '\\'
    indent?: boolean                // default: false
  } | boolean
  section?: {
    duplicate?: 'merge' | 'override' | 'error'   // default: 'merge'
  }
  comment?: {
    inline?: InlineCommentOptions
  }
}

type InlineCommentOptions = {
  active?: boolean        // default: false
  chars?: string[]        // default: ['#', ';']
  escape?: {
    backslash?: boolean   // default: true
    whitespace?: boolean  // default: false
  }
}
```

### `multiline`

Off by default; a value ends at the newline. Pass `true` (or any
object) to enable continuation. As a boolean, `true` means the defaults
below.

| Field | Type | Default | Effect |
|---|---|---|---|
| `continuation` | `string \| false` | `'\\'` | Character that, immediately before a newline, joins the next line onto the value (leading whitespace on the next line is dropped, one space is inserted). Set to `false` to disable backslash continuation. |
| `indent` | `boolean` | `false` | When `true`, a following line that starts with whitespace continues the previous value, even with no continuation character. |

Both modes may be combined (`{ continuation: '\\', indent: true }`).
An escaped continuation character (`\\` before a newline) is a literal
backslash, not a continuation.

### `section.duplicate`

How a repeated `[section]` header is handled.

| Value | Effect |
|---|---|
| `'merge'` (default) | Keys from all occurrences are combined into one section object; a duplicate key takes the last value. |
| `'override'` | The later occurrence replaces the earlier section object entirely (subsections included). |
| `'error'` | A repeated header throws `Error: Duplicate section: [<path>]`. |

A header only counts as "declared" if it is written explicitly.
Intermediate path segments created by a deeper header are **not**
declared, so `[a.b]` followed by `[a]` is not a duplicate.

### `comment.inline`

Inline (mid-value) comment handling. Off by default — `;` and `#`
inside a value are literal text. (Line-leading `;` and `#` comments
always work, regardless of this option.)

| Field | Type | Default | Effect |
|---|---|---|---|
| `active` | `boolean` | `false` | Master switch. When `true`, a comment character ends the value. |
| `chars` | `string[]` | `['#', ';']` | The characters that start an inline comment. |
| `escape.backslash` | `boolean` | `true` | A backslash before a comment char produces the literal char (e.g. `\;` → `;`) and the char does not terminate. |
| `escape.whitespace` | `boolean` | `false` | A comment char only starts a comment when preceded by whitespace; otherwise it is literal. |

## Syntax

The accepted grammar. The railroad diagram of the installed rules is in
[`grammar.svg`](grammar.svg) (ASCII in [`grammar.txt`](grammar.txt));
the source grammar is [`ini-grammar.jsonic`](../../ini-grammar.jsonic).

### Pairs

```ini
key = value
```

- The key runs up to the first `=`, a newline, or end of input, then is
  trimmed. The value runs to the end of the line, then is trimmed.
- A bare key with no `=` is a **boolean key**: it is set to `true`.
- A later `key = value` overwrites an earlier one (`br = cold` then
  `br = warm` ⇒ `{ br: 'warm' }`), unless the key uses array syntax.

### Sections

```ini
[name]
[a.b.c]
```

- A header opens a section; following keys nest under it.
- Dots split the header into a nested path: `[a.b.c]` ⇒
  `{ a: { b: { c: {} } } }`. Escape a literal dot with `\.`
  (`[x\.y]` ⇒ key `x.y`).
- Top-level pairs before any header sit at the root.
- Repeated headers are governed by [`section.duplicate`](#sectionduplicate).

### Arrays

```ini
key[] = first
key[] = second
```

- A `key[] =` line appends to an array under `key`.
- A plain `key =` after array entries appends to the same array
  (`ar[]=one`, `ar[]=three`, then `ar = this is included` ⇒
  `['one', 'three', 'this is included']`).

### Values

A value is resolved in this order:

1. **Single-quoted** (`'…'`) — JSON-decoded if valid (`'{"y":{"z":6}}'`
   ⇒ `{ y: { z: 6 } }`); otherwise the inner text.
2. **Double-quoted** (`"…"`) — the JSON-decoded string, preserving
   spaces and escapes; brackets inside are literal.
3. **`true` / `false` / `null`** (unquoted) — the JS boolean / `null`.
4. **Everything else** — a string, trimmed, including numeric-looking
   text (`42` ⇒ `'42'`) unless number lexing is enabled.

### Comments

```ini
; line comment
# line comment
```

- A `;` or `#` at the start of a line is a comment for the whole line —
  always, independent of options.
- Mid-value `;`/`#` are literal unless [`comment.inline`](#commentinline)
  is active.

### Keys with special characters

Wrap a key in quotes to include spaces or brackets literally:

```ini
" c1  c2 " = null
"[disturbing]" = hey you never know
```
