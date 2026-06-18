# @tabnas/ini

A [jsonic](https://github.com/tabnas/jsonic) syntax plugin that parses
[INI](https://en.wikipedia.org/wiki/INI_file) files into JavaScript
objects — with sections, dot-nested keys, `[]` arrays, multiline
values, and inline comments.

[![npm version](https://img.shields.io/npm/v/@tabnas/ini.svg)](https://npmjs.com/package/@tabnas/ini)
[![build](https://github.com/tabnas/ini/actions/workflows/build.yml/badge.svg)](https://github.com/tabnas/ini/actions/workflows/build.yml)

## Install

```bash
npm install @tabnas/ini @tabnas/parser @tabnas/jsonic @tabnas/hoover
```

`@tabnas/parser` (>=2), `@tabnas/jsonic` (>=2), and `@tabnas/hoover`
(>=0) are peer dependencies.

## Example

`Ini` is a plugin. Layer it on a `Tabnas` engine that already has
`jsonic`:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini)

j.parse('[database]\nhost = localhost\nport = 5432')
// => { database: { host: 'localhost', port: '5432' } }
```

INI values are strings by default (`port` is `'5432'`, not `5432`); the
keywords `true`/`false`/`null` resolve to JS types. See the
[how-to guide](doc/guide.md#read-numbers-as-numbers) to lex numbers.

## Documentation

- [Tutorial](doc/tutorial.md) — a guided first parse: pairs, sections,
  arrays, one option.
- [How-to guide](doc/guide.md) — task recipes (numbers, multiline,
  comments, duplicate sections, errors).
- [Reference](doc/reference.md) — the `Ini` plugin, every option with
  its default, and the accepted syntax.
- [Concepts](doc/concepts.md) — how the plugin sits on the engine, the
  grammar model, and edge cases.

## Grammar diagram

The installed grammar as a railroad/syntax diagram, generated from the
live grammar with [`@tabnas/railroad`](https://github.com/tabnas/railroad):

![ini grammar railroad diagram](doc/grammar.svg)

A vertical ASCII version is in [`doc/grammar.txt`](doc/grammar.txt). The
grammar source is the top-level
[`ini-grammar.jsonic`](../ini-grammar.jsonic), embedded into
[`src/ini.ts`](src/ini.ts) by [`embed-grammar.js`](embed-grammar.js).

## License

MIT. Copyright (c) Richard Rodger and other contributors.
