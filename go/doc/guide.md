# How-to guide (Go)

Short, task-focused recipes. Each is self-contained and assumes you
have the package installed (see the [tutorial](tutorial.md) for the
basics). For full signatures and the complete option list, follow the
links into the [reference](reference.md).

Several recipes need a pointer to a literal. Define this helper once:

```go
func boolp(b bool) *bool { return &b }
```

## Parse once vs reuse an instance

`tabnasini.Parse` builds a fresh parser each call — convenient for one-offs:

```go
result, err := tabnasini.Parse("x = 0\n[s]\na = 1\nb = 2")
// map[string]any{"x": "0", "s": map[string]any{"a": "1", "b": "2"}}
```

To parse many strings with the same options, build one instance with
`MakeJsonic` and reuse it:

```go
j := tabnasini.MakeJsonic()
r1, _ := j.Parse("a=1")        // map[string]any{"a": "1"}
r2, _ := j.Parse("b=2")        // map[string]any{"b": "2"}
_ = r1
_ = r2
```

`MakeJsonic` returns a `*tabnasjsonic.Jsonic`; its `Parse` returns
`(any, error)`, so type-assert the result to `map[string]any`. (The
no-options `tabnasini.Parse` already reuses one cached default instance
internally.)

## Read numbers as numbers

Every INI value is a string by default (`a=1` gives `{"a": "1"}`). To
lex numeric-looking values as numbers, build an instance and turn
jsonic's number matcher on with `SetOptions`:

```go
import tabnasjsonic "github.com/tabnas/jsonic/go"

j := tabnasini.MakeJsonic()
j.SetOptions(tabnasjsonic.Options{
	Number: &tabnasjsonic.NumberOptions{Lex: boolp(true)},
})

result, _ := j.Parse("a=1\nb=hello\nc=2.5")
// map[string]any{"a": float64(1), "b": "hello", "c": float64(2.5)}
```

Numbers come back as `float64` (matching `encoding/json`); non-numeric
text stays a `string`.

## Use boolean keys

A key with no `=` is set to `true` — but only after at least one
`key = value` pair has opened the surrounding map:

```go
result, _ := tabnasini.Parse("[features]\nname = app\ndebug\nverbose")
// map[string]any{"features": map[string]any{"name": "app", "debug": true, "verbose": true}}
```

The unquoted keywords `true`, `false`, and `null` as *values* resolve
to `bool` / `bool` / `nil`:

```go
result, _ := tabnasini.Parse("a = true\nb = false\nc = null")
// map[string]any{"a": true, "b": false, "c": nil}
```

## Enable inline comments

By default `;` and `#` mid-value are literal. Activate inline comments
to make them end the value:

```go
j := tabnasini.MakeJsonic(tabnasini.IniOptions{
	Comment: &tabnasini.CommentOptions{
		Inline: &tabnasini.InlineCommentOptions{Active: boolp(true)},
	},
})

r1, _ := j.Parse("a = hello ; comment") // map[string]any{"a": "hello"}
r2, _ := j.Parse("a = hello # comment") // map[string]any{"a": "hello"}
_, _ = r1, r2
```

Restrict which characters start a comment with `Chars`:

```go
result, _ := tabnasini.Parse("a = hello ; comment\nb = hello # not a comment",
	tabnasini.IniOptions{
		Comment: &tabnasini.CommentOptions{
			Inline: &tabnasini.InlineCommentOptions{
				Active: boolp(true),
				Chars:  []string{";"},
			},
		},
	})
// map[string]any{"a": "hello", "b": "hello # not a comment"}
```

## Escape inline comment characters

With inline comments active, a backslash escapes a comment char so it
stays in the value (`Backslash` defaults to `true`):

```go
result, _ := tabnasini.Parse("a = hello\\; world", tabnasini.IniOptions{
	Comment: &tabnasini.CommentOptions{
		Inline: &tabnasini.InlineCommentOptions{
			Active: boolp(true),
			Escape: &tabnasini.InlineEscapeOptions{Backslash: boolp(true)},
		},
	},
})
// map[string]any{"a": "hello; world"}
```

Or use whitespace-prefix mode, where a comment char only starts a
comment when whitespace precedes it:

```go
result, _ := tabnasini.Parse("a = x;y;z", tabnasini.IniOptions{
	Comment: &tabnasini.CommentOptions{
		Inline: &tabnasini.InlineCommentOptions{
			Active: boolp(true),
			Escape: &tabnasini.InlineEscapeOptions{Whitespace: boolp(true)},
		},
	},
})
// map[string]any{"a": "x;y;z"}   // no whitespace before ';', so it's literal
```

## Parse multiline values

A non-`nil` `Multiline` enables backslash continuation: a `\` right
before the newline joins the next line, with a single space between:

```go
result, _ := tabnasini.Parse("a = one \\\ntwo \\\nthree", tabnasini.IniOptions{
	Multiline: &tabnasini.MultilineOptions{},
})
// map[string]any{"a": "one two three"}
```

For indent-based continuation instead, turn `Indent` on and disable the
continuation character (set it to `""`):

```go
noBackslash := ""
result, _ := tabnasini.Parse("a = line1\n  line2\n  line3", tabnasini.IniOptions{
	Multiline: &tabnasini.MultilineOptions{
		Indent:       boolp(true),
		Continuation: &noBackslash,
	},
})
// map[string]any{"a": "line1 line2 line3"}
```

## Control duplicate section handling

Choose what happens when a `[section]` header appears twice with
`Section.Duplicate`. The default, `"merge"`, combines keys (last value
wins on a clash):

```go
result, _ := tabnasini.Parse("[a]\nx=1\ny=2\n[a]\nz=3", tabnasini.IniOptions{
	Section: &tabnasini.SectionOptions{Duplicate: "merge"},
})
// map[string]any{"a": map[string]any{"x": "1", "y": "2", "z": "3"}}
```

`"override"` makes the later section replace the earlier one entirely:

```go
result, _ := tabnasini.Parse("[a]\nx=1\ny=2\n[a]\nz=3", tabnasini.IniOptions{
	Section: &tabnasini.SectionOptions{Duplicate: "override"},
})
// map[string]any{"a": map[string]any{"z": "3"}}
```

`"error"` rejects a repeated header. The engine surfaces the rejection
as a non-`nil` `error` (or, on some engine builds, a recovered panic),
so check `err`:

```go
_, err := tabnasini.Parse("[a]\nx=1\n[a]\ny=2", tabnasini.IniOptions{
	Section: &tabnasini.SectionOptions{Duplicate: "error"},
})
// err != nil  — message contains "Duplicate section: [a]"
```

## Keep spaces and bracket keys with quotes

Wrap a key or value in quotes to protect leading/trailing spaces or
literal brackets. A double-quoted value is taken verbatim; a
single-quoted value is JSON-decoded:

```go
r1, _ := tabnasini.Parse(`a = "hello world"`)       // map[string]any{"a": "hello world"}
r2, _ := tabnasini.Parse("a = 'hello world'")       // map[string]any{"a": "hello world"}
r3, _ := tabnasini.Parse(`w = '{"y":{"z":6}}'`)     // {"w": {"y": {"z": float64(6)}}}
_, _, _ = r1, r2, r3
```

See [concepts](concepts.md#value-resolution) for the exact value
resolution order.
