# INI plugin for Jsonic (TypeScript)

A Jsonic syntax plugin that parses INI format files into JavaScript
objects, with support for sections, nested keys, arrays, multiline
values, and inline comments.

```bash
npm install @jsonic/ini
```

Requires `jsonic` >= 2 and `@jsonic/hoover` >= 0 as peer dependencies.


## Tutorials

### Parse a basic INI file

Parse key-value pairs and sections into a nested object:

```typescript
import { Jsonic } from 'jsonic'
import { Ini } from '@jsonic/ini'

const j = Jsonic.make().use(Ini)

j("a = 1\nb = hello world")
// { a: '1', b: 'hello world' }

j("[database]\nhost = localhost\nport = 5432")
// { database: { host: 'localhost', port: '5432' } }
```

### Parse nested sections and arrays

Use dot notation for nested sections and `[]` suffix for arrays:

```typescript
import { Jsonic } from 'jsonic'
import { Ini } from '@jsonic/ini'

const j = Jsonic.make().use(Ini)

j("[server.production]\nhost = example.com")
// { server: { production: { host: 'example.com' } } }

j("tags[] = web\ntags[] = api\ntags[] = v2")
// { tags: ['web', 'api', 'v2'] }
```

### Parse with multiline values

Enable backslash continuation and indent-based continuation:

```typescript
import { Jsonic } from 'jsonic'
import { Ini } from '@jsonic/ini'

const j = Jsonic.make().use(Ini, {
  multiline: { continuation: '\\', indent: true }
})

j("desc = first \\\n  second")    // { desc: 'first second' }
j("desc = first\n  second")       // { desc: 'first second' }
```


## How-to guides

### Enable inline comments

By default, `#` and `;` in values are treated as literal characters.
Activate inline comments to treat them as comment starters:

```typescript
const j = Jsonic.make().use(Ini, {
  comment: {
    inline: { active: true }
  }
})

j("a = hello # this is a comment")  // { a: 'hello' }
j("b = value ; also a comment")     // { b: 'value' }
```

### Escape inline comment characters

Use backslash escaping to include literal `#` or `;` in values
when inline comments are active:

```typescript
const j = Jsonic.make().use(Ini, {
  comment: {
    inline: { active: true, escape: { backslash: true } }
  }
})

j("color = red\\#FF0000")  // { color: 'red#FF0000' }
```

Alternatively, use whitespace-prefix mode where only `#` or `;`
preceded by whitespace starts a comment:

```typescript
const j = Jsonic.make().use(Ini, {
  comment: {
    inline: { active: true, escape: { whitespace: true } }
  }
})

j("color = red#FF0000")          // { color: 'red#FF0000' }
j("color = red #FF0000 comment") // { color: 'red' }
```

### Control duplicate section handling

Choose how repeated section headers are treated:

```typescript
// Merge (default): combine keys, last value wins for duplicates
const jMerge = Jsonic.make().use(Ini, {
  section: { duplicate: 'merge' }
})
jMerge("[db]\nhost = a\n[db]\nport = 5")
// { db: { host: 'a', port: '5' } }

// Override: last section replaces earlier ones
const jOver = Jsonic.make().use(Ini, {
  section: { duplicate: 'override' }
})
jOver("[db]\nhost = a\n[db]\nport = 5")
// { db: { port: '5' } }

// Error: throw on duplicate sections
const jErr = Jsonic.make().use(Ini, {
  section: { duplicate: 'error' }
})
// jErr("[db]\nhost = a\n[db]\nport = 5") // throws Error
```

### Use boolean keys

Keys without a value assignment are set to `true`:

```typescript
const j = Jsonic.make().use(Ini)

j("[features]\ndebug\nverbose")
// { features: { debug: true, verbose: true } }
```


## Explanation

### How INI parsing works

The Ini plugin configures Jsonic with a custom grammar and lexer
matchers to handle INI syntax:

1. **Comments** (`#` and `;` at line start) are consumed by Jsonic's
   built-in comment lexer.
2. **Sections** (`[name]`) are parsed by the grammar's `dive` rule,
   which supports dot-separated nesting (`[a.b.c]`).
3. **Keys** are read by a custom Hoover matcher (`#HK` token) that
   scans until `=`, newline, or end of input.
4. **Values** after `=` are read by another Hoover matcher (`#HV`
   token) that scans to end-of-line, handling escape sequences.
5. **Quoted strings** (single or double quotes) are handled by
   Jsonic's built-in string lexer. Single-quoted values like
   `'{"x":1}'` are parsed as JSON.
6. **Special values** `true`, `false`, and `null` are resolved to
   their JavaScript equivalents.

### Value resolution order

When a value is encountered, the parser applies these rules:

1. If the value is a quoted string starting with `'`, attempt JSON
   parsing (e.g., `'[1,2]'` becomes `[1,2]`).
2. If the value is a bracket character (`[` or `]`) at the start,
   it is concatenated with the following value token.
3. Unquoted values `true`, `false`, `null` resolve to their
   JavaScript types.
4. Everything else remains a string (including numbers like `"42"`).


## Reference

### `Ini` (Plugin)

The plugin function. Register with `Jsonic.make().use(Ini, options)`.

### `IniOptions`

```typescript
type IniOptions = {
  multiline?: {
    continuation?: string | false  // default: '\\'
    indent?: boolean               // default: false
  } | boolean
  section?: {
    duplicate?: 'merge' | 'override' | 'error'  // default: 'merge'
  }
  comment?: {
    inline?: InlineCommentOptions
  }
}
```

### `InlineCommentOptions`

```typescript
type InlineCommentOptions = {
  active?: boolean     // default: false
  chars?: string[]     // default: ['#', ';']
  escape?: {
    backslash?: boolean   // default: true
    whitespace?: boolean  // default: false
  }
}
```
