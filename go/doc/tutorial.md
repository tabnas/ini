# Tutorial — your first INI parse (Go)

This walks you from nothing to a working parse, then through sections,
arrays, and one option. Follow it in order; each step builds on the
last. When you finish you will have parsed key-value pairs, nested
sections, and an array, and configured a reusable parser.

`github.com/tabnas/ini/go` is a Go port of the
[`@tabnas/ini`](https://github.com/tabnas/ini) plugin for
[jsonic](https://github.com/tabnas/jsonic), a relaxed-JSON grammar on
the [tabnas](https://github.com/tabnas/parser) engine.

For a recipe-style index of individual tasks, see the
[how-to guide](guide.md). For exhaustive signatures and the accepted
syntax, see the [reference](reference.md). For how it works, see the
[concepts](concepts.md).

## 1. Install

```bash
go get github.com/tabnas/ini/go@latest
```

The engine and jsonic come in as transitive dependencies. (When
building from a source checkout before the tabnas packages are
published, see the sibling-checkout note in the [README](../README.md).)

## 2. Parse a string

Create `main.go`:

```go
package main

import (
	"fmt"

	ini "github.com/tabnas/ini/go"
)

func main() {
	result, err := ini.Parse("a = 1\nb = hello world")
	if err != nil {
		panic(err)
	}
	fmt.Println(result) // map[a:1 b:hello world]
}
```

Run it with `go run .`. `ini.Parse` is the zero-config convenience
function: it returns a `map[string]any` and an `error`. The value
`hello world` kept its internal space — values run to the end of the
line, no quotes needed. And `1` came back as the **string** `"1"`, not
a number: by default every INI value is text.

## 3. Inspect the result

`ini.Parse` always returns `map[string]any` for a successful parse, so
type-assert nested values and read them:

```go
result, _ := ini.Parse("[database]\nhost = localhost\nport = 5432")
db := result["database"].(map[string]any)
fmt.Println(db["host"]) // localhost
```

The concrete value types are predictable: strings → `string`, the
keywords `true`/`false`/`null` → `bool`/`bool`/`nil`, sections →
`map[string]any`, arrays → `[]any`. The full list is in the
[reference](reference.md#return-types).

## 4. Add a section

A `[name]` header starts a section; following keys nest under it. Use
dots for deeper nesting:

```go
result, _ := ini.Parse("[server.production]\nhost = example.com")
// map[string]any{"server": map[string]any{"production": map[string]any{"host": "example.com"}}}
```

`[server.production]` became a two-level nested map. That dot-path
nesting is the `dive` rule at work — see
[concepts](concepts.md#sections-and-the-dive-rule).

## 5. Collect repeated keys into an array

Append `[]` to a key to push values into a `[]any` instead of
overwriting:

```go
result, _ := ini.Parse("tags[] = web\ntags[] = api\ntags[] = v2")
// map[string]any{"tags": []any{"web", "api", "v2"}}
```

## 6. Make a configured, reusable parser

`ini.Parse` builds a fresh parser per call. To set options or reuse one
instance across many parses, use `MakeJsonic` and call `Parse` on it.
Options fields are pointers, so `nil` means "use the default" — a tiny
helper takes the address of a literal:

```go
func boolp(b bool) *bool { return &b }
```

Now turn on inline comments so `;` and `#` end a value:

```go
j := ini.MakeJsonic(ini.IniOptions{
	Comment: &ini.CommentOptions{
		Inline: &ini.InlineCommentOptions{Active: boolp(true)},
	},
})

result, _ := j.Parse("a = hello ; trailing comment")
// map[string]any{"a": "hello"}
```

`j.Parse` returns `(any, error)` (type-assert the `any` to
`map[string]any`); the convenience `ini.Parse` does the assertion for
you. Every option is documented in the [reference](reference.md#options).

## Where to go next

- [How-to guide](guide.md) — focused recipes (numbers, multiline,
  duplicate sections, errors).
- [Reference](reference.md) — the public API, every option, and the
  accepted syntax.
- [Concepts](concepts.md) — how the package is built, and how it
  differs from the TypeScript version.
