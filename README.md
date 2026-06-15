# @tabnas/ini

A [Jsonic](https://github.com/tabnas/jsonic) syntax plugin that parses
[INI](https://en.wikipedia.org/wiki/INI_file) format files into objects /
maps — with support for sections, nested keys, arrays, multiline values,
and inline comments.

This repository contains:

| Path | Description |
|---|---|
| [`ts/`](ts/) | TypeScript / JavaScript implementation (`@tabnas/ini`). |
| [`go/`](go/) | Go port (`github.com/tabnas/ini/go`). |
| [`test/spec/`](test/spec/) | Shared `.tsv` conformance fixtures, exercised by both runtimes. |

Start with [`ts/README.md`](ts/README.md) for the JS API or
[`go/README.md`](go/README.md) for Go.

## License

MIT. Copyright (c) Richard Rodger.
