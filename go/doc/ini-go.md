# INI plugin for Jsonic (Go)

A Jsonic syntax plugin that parses INI format files into Go maps,
with support for sections, nested keys, arrays, multiline values,
and inline comments.

```go
import (
  jsonic "github.com/jsonicjs/jsonic/go"
  ini "github.com/jsonicjs/ini/go"
)
```

```bash
go get github.com/jsonicjs/ini/go@latest
```


## Tutorials

### Parse a basic INI file

Parse key-value pairs and sections into a nested map:

```go
result, err := ini.Parse("a = 1\nb = hello world")
// map[string]any{"a": "1", "b": "hello world"}

result, err = ini.Parse("[database]\nhost = localhost\nport = 5432")
// map[string]any{"database": map[string]any{"host": "localhost", "port": "5432"}}
```

### Parse nested sections and arrays

Use dot notation for nested sections and `[]` suffix for arrays:

```go
result, err := ini.Parse("[server.production]\nhost = example.com")
// map[string]any{"server": map[string]any{"production": map[string]any{"host": "example.com"}}}

result, err = ini.Parse("tags[] = web\ntags[] = api\ntags[] = v2")
// map[string]any{"tags": []any{"web", "api", "v2"}}
```

### Parse with multiline values

Enable backslash continuation and indent-based continuation:

```go
cont := "\\"
indent := true
result, err := ini.Parse("desc = first \\\n  second", ini.IniOptions{
  Multiline: &ini.MultilineOptions{
    Continuation: &cont,
    Indent:       &indent,
  },
})
// map[string]any{"desc": "first second"}
```


## How-to guides

### Enable inline comments

By default, `#` and `;` in values are treated as literal characters.
Activate inline comments to treat them as comment starters:

```go
active := true
result, err := ini.Parse("a = hello # comment", ini.IniOptions{
  Comment: &ini.CommentOptions{
    Inline: &ini.InlineCommentOptions{
      Active: &active,
    },
  },
})
// map[string]any{"a": "hello"}
```

### Escape inline comment characters

Use backslash escaping to include literal `#` or `;` in values
when inline comments are active:

```go
active := true
backslash := true
result, err := ini.Parse("color = red\\#FF0000", ini.IniOptions{
  Comment: &ini.CommentOptions{
    Inline: &ini.InlineCommentOptions{
      Active: &active,
      Escape: &ini.InlineEscapeOptions{
        Backslash: &backslash,
      },
    },
  },
})
// map[string]any{"color": "red#FF0000"}
```

Alternatively, use whitespace-prefix mode where only `#` or `;`
preceded by whitespace starts a comment:

```go
active := true
whitespace := true
result, err := ini.Parse("color = red#FF0000", ini.IniOptions{
  Comment: &ini.CommentOptions{
    Inline: &ini.InlineCommentOptions{
      Active: &active,
      Escape: &ini.InlineEscapeOptions{
        Whitespace: &whitespace,
      },
    },
  },
})
// map[string]any{"color": "red#FF0000"}
```

### Control duplicate section handling

Choose how repeated section headers are treated:

```go
// Merge (default): combine keys, last value wins for duplicates
result, _ := ini.Parse("[db]\nhost = a\n[db]\nport = 5", ini.IniOptions{
  Section: &ini.SectionOptions{Duplicate: "merge"},
})
// map[string]any{"db": map[string]any{"host": "a", "port": "5"}}

// Override: last section replaces earlier ones
result, _ = ini.Parse("[db]\nhost = a\n[db]\nport = 5", ini.IniOptions{
  Section: &ini.SectionOptions{Duplicate: "override"},
})
// map[string]any{"db": map[string]any{"port": "5"}}

// Error: panic on duplicate sections
// ini.Parse("[db]\nhost = a\n[db]\nport = 5", ini.IniOptions{
//   Section: &ini.SectionOptions{Duplicate: "error"},
// })
```

### Use boolean keys

Keys without a value assignment are set to `true`:

```go
result, err := ini.Parse("[features]\ndebug\nverbose")
// map[string]any{"features": map[string]any{"debug": true, "verbose": true}}
```


## Explanation

### How INI parsing works

The Ini plugin configures Jsonic with a custom grammar and lexer
matchers to handle INI syntax:

1. **Comments** (`#` and `;` at line start) are consumed by Jsonic's
   built-in comment lexer.
2. **Sections** (`[name]`) are parsed by the grammar's `dive` rule,
   which supports dot-separated nesting (`[a.b.c]`).
3. **Keys** are read by a custom matcher (`inikey`) that scans until
   `=`, newline, or end of input.
4. **Values** after `=` are read by another custom matcher (`inival`)
   that scans to end-of-line, handling escape sequences and optional
   multiline continuation.
5. **Quoted strings** (single or double quotes) are handled by
   Jsonic's built-in string lexer. Single-quoted values like
   `'{"x":1}'` are parsed as JSON.
6. **Special values** `true`, `false`, and `null` are resolved to
   their Go equivalents (`bool` / `nil`).

### Value resolution order

When a value is encountered, the parser applies these rules:

1. If the value is a quoted string starting with `'`, attempt JSON
   parsing (e.g., `'true'` becomes `bool true`).
2. If the value is a bracket character (`[` or `]`) at the start,
   it is concatenated with the following value token.
3. Unquoted values `true`, `false`, `null` resolve to their
   Go types (`bool` / `nil`).
4. Everything else remains a `string` (including numbers like `"42"`).


## Reference

### `Parse(src string, opts ...IniOptions) (map[string]any, error)`

Parses an INI string and returns a map. Convenience function that
creates a Jsonic instance internally.

### `MakeJsonic(opts ...IniOptions) *jsonic.Jsonic`

Creates a reusable Jsonic instance configured for INI parsing.
Use this when parsing multiple INI strings with the same options.

### `IniOptions`

```go
type IniOptions struct {
  Multiline *MultilineOptions
  Section   *SectionOptions
  Comment   *CommentOptions
}
```

### `MultilineOptions`

```go
type MultilineOptions struct {
  Continuation *string  // default: "\\"
  Indent       *bool    // default: false
}
```

### `SectionOptions`

```go
type SectionOptions struct {
  Duplicate string  // "merge" (default), "override", or "error"
}
```

### `CommentOptions`

```go
type CommentOptions struct {
  Inline *InlineCommentOptions
}
```

### `InlineCommentOptions`

```go
type InlineCommentOptions struct {
  Active *bool     // default: false
  Chars  []string  // default: ["#", ";"]
  Escape *InlineEscapeOptions
}
```

### `InlineEscapeOptions`

```go
type InlineEscapeOptions struct {
  Backslash  *bool  // default: true
  Whitespace *bool  // default: false
}
```
