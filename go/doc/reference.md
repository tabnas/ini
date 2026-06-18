# Reference (Go)

Complete, dry reference for `github.com/tabnas/ini/go`: the public API,
every option with its default and effect, and the INI syntax the parser
accepts. For a guided introduction see the [tutorial](tutorial.md); for
task recipes see the [how-to guide](guide.md); for how it works and how
it differs from the TS version see the [concepts](concepts.md).

## Package

```bash
go get github.com/tabnas/ini/go@latest
```

```go
import ini "github.com/tabnas/ini/go"
```

`Version` is a package constant.

## Functions

### `Parse`

```go
func Parse(src string, opts ...IniOptions) (map[string]any, error)
```

Parses an INI string and returns a `map[string]any`. Convenience
function — it builds a parser internally. With no `opts` it reuses a
single cached default instance (safe for concurrent use); each
option-taking call builds a fresh instance. Empty input (`""`) returns
an empty map. On a malformed parse it returns a non-`nil` `error`.

### `MakeJsonic`

```go
func MakeJsonic(opts ...IniOptions) *jsonic.Jsonic
```

Returns a reusable `*jsonic.Jsonic` instance configured for INI
parsing. Use it to parse many strings with the same options, or to
apply further jsonic configuration with `SetOptions` (e.g. enabling
number lexing). Its `Parse` method returns `(any, error)`; type-assert
the result to `map[string]any`.

## Options

```go
type IniOptions struct {
	Multiline *MultilineOptions
	Section   *SectionOptions
	Comment   *CommentOptions
}

type MultilineOptions struct {
	Continuation *string // default: "\\"
	Indent       *bool   // default: false
}

type SectionOptions struct {
	Duplicate string // "merge" (default), "override", "error"
}

type CommentOptions struct {
	Inline *InlineCommentOptions
}

type InlineCommentOptions struct {
	Active *bool    // default: false
	Chars  []string // default: ["#", ";"]
	Escape *InlineEscapeOptions
}

type InlineEscapeOptions struct {
	Backslash  *bool // default: true
	Whitespace *bool // default: false
}
```

All leaf fields are pointers (or slices); a `nil` pointer means "use
the default". Whole sub-structs are also pointers: a `nil`
`*MultilineOptions` means multiline is off, while a non-`nil` value
(even `&MultilineOptions{}`) turns it on.

### `Multiline`

`nil` ⇒ multiline off (a value ends at the newline). Non-`nil` enables
continuation.

| Field | Type | Default | Effect |
|---|---|---|---|
| `Continuation` | `*string` | `"\\"` | Character that, immediately before a newline, joins the next line onto the value (leading whitespace dropped, one space inserted). Set to `""` to disable backslash continuation. |
| `Indent` | `*bool` | `false` | When `true`, a following line that starts with whitespace continues the previous value, with no continuation character. |

Both modes can be combined. An escaped continuation character (`\\`
before a newline) is a literal backslash, not a continuation.

### `Section.Duplicate`

How a repeated `[section]` header is handled.

| Value | Effect |
|---|---|
| `"merge"` (default, also the value when `Section` is `nil`) | Keys from all occurrences are combined; a duplicate key takes the last value. |
| `"override"` | The later occurrence replaces the earlier section map entirely (subsections included). |
| `"error"` | A repeated header is rejected; `Parse` returns a non-`nil` `error` whose message contains `Duplicate section: [<path>]`. |

A header counts as "declared" only if written explicitly; intermediate
path segments created by a deeper header are not declared, so `[a.b]`
followed by `[a]` is not a duplicate.

### `Comment.Inline`

Inline (mid-value) comment handling. Off by default — `;` and `#`
inside a value are literal text. (Line-leading `;`/`#` comments always
work, regardless of this option.)

| Field | Type | Default | Effect |
|---|---|---|---|
| `Active` | `*bool` | `false` | Master switch. When `true`, a comment character ends the value. |
| `Chars` | `[]string` | `["#", ";"]` | The characters that start an inline comment. |
| `Escape.Backslash` | `*bool` | `true` | A backslash before a comment char produces the literal char (e.g. `\;` → `;`); the char does not terminate. |
| `Escape.Whitespace` | `*bool` | `false` | A comment char only starts a comment when preceded by whitespace; otherwise it is literal. |

## Return types

`Parse` returns `map[string]any` (`MakeJsonic(...).Parse` returns `any`,
always a `map[string]any` on success). Concrete value types:

| INI value | Go type |
|---|---|
| Section | `map[string]any` |
| Array (`key[] =`) | `[]any` |
| String / text | `string` |
| `true` / `false` | `bool` |
| `null` | `nil` |
| Number (only with number lexing enabled) | `float64` |
| Single-quoted JSON | the decoded value (`map[string]any`, `[]any`, `float64`, …) |

By default numeric values are strings (`a=1` ⇒ `"1"`). Enable number
lexing on a `MakeJsonic` instance with
`SetOptions(jsonic.Options{Number: &jsonic.NumberOptions{Lex: boolp(true)}})`.

## Syntax

The accepted grammar. The railroad diagram of the installed rules is in
the [TS doc `grammar.svg`](../../ts/doc/grammar.svg) (the grammar is the
same for both ports); the source grammar is
[`ini-grammar.jsonic`](../../ini-grammar.jsonic).

### Pairs

```ini
key = value
```

- The key runs up to the first `=`, a newline, or end of input, then is
  trimmed; the value runs to the end of the line, then is trimmed.
- A bare key with no `=` is a **boolean key** set to `true`, but only
  once the surrounding map exists (a prior `key = value` pair opens it).
- A later `key = value` overwrites an earlier one, unless the key uses
  array syntax.

### Sections

```ini
[name]
[a.b.c]
```

- A header opens a section; following keys nest under it.
- Dots split the header into a nested path (`[a.b.c]` ⇒
  `{a: {b: {c: {}}}}`). Escape a literal dot with `\.`.
- Top-level pairs before any header sit at the root.
- Repeated headers are governed by [`Section.Duplicate`](#sectionduplicate).

### Arrays

```ini
key[] = first
key[] = second
```

A `key[] =` line appends to a `[]any` under `key`; a plain `key =`
after array entries appends to the same array.

### Values

Resolved in order: single-quoted (JSON-decoded if valid, else the inner
text) → double-quoted (decoded string, spaces/escapes preserved) →
`true`/`false`/`null` keywords → otherwise a trimmed string (numeric
text stays a string unless number lexing is enabled).

### Comments

A `;` or `#` at the start of a line is a whole-line comment, always.
Mid-value `;`/`#` are literal unless [`Comment.Inline`](#commentinline)
is active.
