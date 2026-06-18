# How-to guide

Short, task-focused recipes. Each is self-contained and assumes you
have the plugin installed (see the [tutorial](tutorial.md) for the
basics). For full signatures and the complete option list, follow the
links into the [reference](reference.md).

Every recipe starts from the same three imports:

```js ignore
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'
```

## Use it as a plugin

`Ini` is a syntax plugin. Layer it on a `Tabnas` engine that already
has `jsonic`, then call `parse`:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini)

j.parse('x = 0\n[s]\na = 1\nb = 2')
// => { x: '0', s: { a: '1', b: '2' } }
```

The instance is reusable — build it once and parse many strings.

## Read numbers as numbers

Every INI value is a string by default (`a=1` gives `{ a: '1' }`). To
have numeric-looking values lexed as numbers, turn on jsonic's number
matcher *after* applying the plugin:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini)
j.options({ number: { lex: true } })

j.parse('a=1\nb=hello\nc=2.5\nd=true')
// => { a: 1, b: 'hello', c: 2.5, d: true }
```

Non-numeric text stays a string, and an empty value stays `''`:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini)
j.options({ number: { lex: true } })

j.parse('a=1abc\nb=\nc=0xFF')   // => { a: '1abc', b: '', c: 255 }
```

## Use boolean keys

A key with no `=` is set to `true`:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini)

j.parse('[features]\nname = app\ndebug\nverbose')
// => { features: { name: 'app', debug: true, verbose: true } }
```

The unquoted words `true`, `false`, and `null` as *values* resolve to
their JS equivalents:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini)

j.parse('a = true\nb = false\nc = null')   // => { a: true, b: false, c: null }
```

## Enable inline comments

By default `;` and `#` mid-value are literal. Activate inline comments
to make them end the value:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini, {
  comment: { inline: { active: true } },
})

j.parse('a = hello ; comment')   // => { a: 'hello' }
j.parse('a = hello # comment')   // => { a: 'hello' }
```

Restrict which characters start a comment with `chars`:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini, {
  comment: { inline: { active: true, chars: [';'] } },
})

j.parse('a = hello ; comment')        // => { a: 'hello' }
j.parse('b = hello # not a comment')  // => { b: 'hello # not a comment' }
```

## Escape inline comment characters

With inline comments active, a backslash escapes a comment char so it
stays in the value (`escape.backslash` is on by default):

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini, {
  comment: { inline: { active: true, escape: { backslash: true } } },
})

j.parse('a = hello\\; world')   // => { a: 'hello; world' }
j.parse('a = x\\;y ; comment')  // => { a: 'x;y' }
```

Or use whitespace-prefix mode, where a comment char only starts a
comment when whitespace precedes it:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini, {
  comment: { inline: { active: true, escape: { whitespace: true } } },
})

j.parse('a = x;y;z')        // => { a: 'x;y;z' }
j.parse('a = hello ;done')  // => { a: 'hello' }
```

## Parse multiline values

Pass `multiline: true` to enable backslash continuation: a `\` right
before the newline joins the next line, with a single space between:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini, { multiline: true })

j.parse('a = one \\\ntwo \\\nthree')   // => { a: 'one two three' }
```

For indent-based continuation instead, turn `indent` on and the
backslash off:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini, {
  multiline: { indent: true, continuation: false },
})

j.parse('a = line1\n  line2\n  line3')   // => { a: 'line1 line2 line3' }
```

## Control duplicate section handling

Choose what happens when a `[section]` header appears twice with the
`section.duplicate` option. The default, `'merge'`, combines keys
(last value wins on a clash):

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini, {
  section: { duplicate: 'merge' },
})

j.parse('[db]\nhost = a\n[db]\nport = 5')   // => { db: { host: 'a', port: '5' } }
```

`'override'` makes the later section replace the earlier one entirely:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini, {
  section: { duplicate: 'override' },
})

j.parse('[a]\nx=1\ny=2\n[a]\nz=3')   // => { a: { z: '3' } }
```

`'error'` throws on a repeated header — catch it like any parse error:

```js ignore
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini, {
  section: { duplicate: 'error' },
})

try {
  j.parse('[a]\nx=1\n[a]\ny=2')
} catch (err) {
  err.message   // includes: Duplicate section: [a]
}
```

## Keep spaces and bracket keys with quotes

Wrap a key or value in quotes to protect leading/trailing spaces or
literal brackets. A double-quoted value is taken verbatim; a
single-quoted value is JSON-decoded:

```js
import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '@tabnas/ini'

const j = new Tabnas().use(jsonic).use(Ini)

j.parse(`s5 = '   '`)              // => { s5: '   ' }
j.parse('"[disturbing]" = hey')    // => { '[disturbing]': 'hey' }
j.parse(`w = '{"y":{"z":6}}'`)     // => { w: { y: { z: 6 } } }
```

See [concepts](concepts.md#value-resolution) for the exact value
resolution order.
