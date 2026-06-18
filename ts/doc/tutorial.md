# Tutorial — your first INI parse

This walks you from nothing to a working parse, then through sections,
arrays, and one option. Follow it in order; each step builds on the
last. When you finish you will have parsed key-value pairs, nested
sections, and an array, and turned on a parser option.

`@tabnas/ini` is a syntax plugin for [jsonic](https://github.com/tabnas/jsonic),
which is itself a grammar on the [tabnas](https://github.com/tabnas/parser)
engine. You install all three and compose them.

For a recipe-style index of individual tasks, see the
[how-to guide](guide.md). For exhaustive signatures, see the
[reference](reference.md). For how it works underneath, see the
[concepts](concepts.md).

## 1. Install

```bash
npm install @tabnas/ini @tabnas/parser @tabnas/jsonic @tabnas/hoover
```

`@tabnas/parser` (the engine), `@tabnas/jsonic` (the relaxed-JSON
grammar) and `@tabnas/hoover` (the value/key matcher this plugin uses)
are peer dependencies.

## 2. Make a parser

The plugin is a function called `Ini`. Build a `Tabnas` engine, layer
`jsonic` on it, then layer `Ini`:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini)
```

`j` is a reusable parser. Call `j.parse(text)` as many times as you
like.

## 3. Parse key-value pairs

INI is `key = value`, one per line:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini)

j.parse('a = 1\nb = hello world')   // => { a: '1', b: 'hello world' }
```

Two things to notice. The value `hello world` keeps its internal space
— values run to the end of the line, no quotes needed. And `1` came
back as the **string** `'1'`, not the number `1`: by default every INI
value is text. (Step 6 of the [how-to guide](guide.md#read-numbers-as-numbers)
shows how to opt into numbers.)

## 4. Add a section

A `[name]` header starts a section. Keys after it nest under that name.
Use dots in the header for deeper nesting:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini)

j.parse('[database]\nhost = localhost\nport = 5432')
// => { database: { host: 'localhost', port: '5432' } }

j.parse('[server.production]\nhost = example.com')
// => { server: { production: { host: 'example.com' } } }
```

`[server.production]` became a nested object `{ server: { production:
{ ... } } }`. That dot-path nesting is the `dive` rule at work — see
[concepts](concepts.md#sections-and-the-dive-rule).

## 5. Collect repeated keys into an array

Append `[]` to a key to push values into an array instead of
overwriting:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini)

j.parse('tags[] = web\ntags[] = api\ntags[] = v2')
// => { tags: ['web', 'api', 'v2'] }
```

Each `tags[] =` line appends; the result is one key holding a list.

## 6. Turn on an option

The plugin's behavior is configured by a second argument to `use(Ini,
options)`. By default `;` and `#` inside a value are literal text. Turn
on inline comments so they end the value instead:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini, {
  comment: { inline: { active: true } },
})

j.parse('a = hello ; trailing comment')   // => { a: 'hello' }
```

Without `comment.inline.active`, the same input parses to `{ a: 'hello
; trailing comment' }`. The option changed how the value matcher reads
the line. Every option is listed in the [reference](reference.md#options).

## Where to go next

- [How-to guide](guide.md) — focused recipes (numbers, multiline,
  duplicate sections, errors).
- [Reference](reference.md) — the public API, every option, and the
  full accepted syntax.
- [Concepts](concepts.md) — how the plugin sits on the engine, the
  grammar model, and accepted-vs-rejected edge cases.
