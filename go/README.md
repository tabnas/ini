# ini (Go)

A Go port of [@tabnas/ini](https://github.com/tabnas/ini), a
[jsonic](https://github.com/tabnas/jsonic) syntax plugin that parses
[INI](https://en.wikipedia.org/wiki/INI_file) files into Go maps — with
sections, dot-nested keys, `[]` arrays, multiline values, and inline
comments.

## Install

```bash
go get github.com/tabnas/ini/go@latest
```

## Example

```go
package main

import (
	"fmt"

	tabnasini "github.com/tabnas/ini/go"
)

func main() {
	result, err := tabnasini.Parse("[database]\nhost = localhost\nport = 5432")
	if err != nil {
		panic(err)
	}
	fmt.Println(result)
	// map[database:map[host:localhost port:5432]]
}
```

INI values are strings by default (`port` is `"5432"`, not a number);
the keywords `true`/`false`/`null` resolve to `bool`/`bool`/`nil`. See
the [how-to guide](doc/guide.md#read-numbers-as-numbers) to lex numbers
as `float64`.

`tabnasini.Parse` builds a parser each call; for reuse and options use
`tabnasini.MakeJsonic(opts...)` and call `Parse` on the returned instance.

## Documentation

- [Tutorial](doc/tutorial.md) — a guided first parse: pairs, sections,
  arrays, a configured instance.
- [How-to guide](doc/guide.md) — task recipes (numbers, multiline,
  comments, duplicate sections, errors).
- [Reference](doc/reference.md) — `Parse`, `MakeJsonic`, every option
  with its default, return types, and the accepted syntax.
- [Concepts](doc/concepts.md) — how the package is built, plus
  [differences from the TS version](doc/concepts.md#differences-from-the-ts-version).

## Grammar

The grammar is the shared top-level
[`ini-grammar.jsonic`](../ini-grammar.jsonic), embedded into
[`ini.go`](ini.go). The railroad/syntax diagram (the grammar is the
same for both ports) is in [`../ts/doc/grammar.svg`](../ts/doc/grammar.svg).

## License

MIT. Copyright (c) Richard Rodger and other contributors.
