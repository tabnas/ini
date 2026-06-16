/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

import { test, describe } from 'node:test'
import assert from 'node:assert'

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '../dist/ini'


const j = new Tabnas().use(jsonic).use(Ini)


describe('ini', () => {

  test('happy', () => {
    assert.deepEqual(j.parse('a=1'), { a: "1" })
    assert.deepEqual(j.parse('[A]'), { A: {} })
    assert.deepEqual(j.parse(`[A.B]\nc='2'`), { A: { B: { c: 2 } } })
    assert.deepEqual(j.parse('a[]=1\na[]=2'), { a: ['1', '2'] })
    assert.deepEqual(j.parse('a=\nb='), { a: '', b: '' })
    // Inline comments are off by default; ; and # mid-value are literal.
    assert.deepEqual(j.parse(';X\n#Y\na=1;2\nb=2'), { a: '1;2', b: '2' })
  })


  test('basic', () => {
    assert.deepEqual(j.parse(`
; comment
a = 1
b = x
c = y y
c0 = true
" c1  c2 " = null
'[]'='[]'

[d]
e = 2
e0[]=q q
e0[]=w w
"[]"="[]"

[f]
# x:11
g = 'G'
# x:12


[h.i]
j = [3,4]
j0 = ]3,4[
k = false

[l.m.n.o]
p = "P"
q = {x:1}
u = v = 5
w = '{"y":{"z":6}}'
aa = 7

`),
      {
        a: '1',
        b: 'x',
        c: 'y y',
        c0: true,
        ' c1  c2 ': null,
        '[]': [],
        d: {
          e: '2',
          e0: ['q q', 'w w'],
          '[]': '[]',
        },
        f: { g: 'G' },
        h: { i: { j: '[3,4]', j0: ']3,4[', k: false } },
        l: {
          m: {
            n: {
              o: {
                p: 'P',
                q: '{x:1}',
                u: 'v = 5',
                w: { y: { z: 6 } },
                aa: '7'
              },
            }
          }
        }
      })
  })

  // NOTE: Copyright (c) Isaac Z. Schlueter and Contributors, ISC License
  test('ini-module-test', () => {
    assert.deepEqual(j.parse(`
o = p

a with spaces   =     b  c

; wrap in quotes to JSON-decode and preserve spaces
" xa  n          p " = "\\"\\r\\nyoyoyo\\r\\r\\n"

; wrap in quotes to get a key with a bracket, not a section.
"[disturbing]" = hey you never know

; Test single quotes
s = 'something'

; Test mixing quotes

s1 = "something'

; Test double quotes
s2 = "something else"

; Test blank value
s3 =

; Test value with only spaces
s4 =

; Test quoted value with only spaces
s5 = '   '

; Test quoted value with leading and trailing spaces
s6 = ' a '

; Test no equal sign
s7

; Test bool(true)
true = true

; Test bool(false)
false = false

; Test null
null = null

; Test undefined
undefined = undefined

; Test arrays
zr[] = deedee
ar[] = one
ar[] = three
; This should be included in the array
ar   = this is included

; Test resetting of a value (and not turn it into an array)
br = cold
br = warm

eq = "eq=eq"

; a section
[a]
av = a val
e = { o: p, a: { av: a val, b: { c: { e: 'this [value]' } } } }
j = "{ o: \\"p\\", a: { av: \\"a val\\", b: { c: { e: \\"this [value]\\" } } } }"
"[]" = a square?

; Nested array
cr[] = four
cr[] = eight

; nested child without middle parent
; should create otherwise-empty a.b
[a.b.c]
e = 1
j = 2

; dots in the section name should be literally interpreted
[x\\.y\\.z]
x.y.z = xyz

[x\\.y\\.z.a\\.b\\.c]
a.b.c = abc

; this next one is not a comment!  it's escaped!
nocomment = this\\; this is not a comment

# Support the use of the number sign (#) as an alternative to the semicolon for indicating comments.
# http://en.wikipedia.org/wiki/INI_file#Comments

# this next one is not a comment!  it's escaped!
noHashComment = this\\# this is not a comment`),
      {
        " xa  n          p ": "\"\r\nyoyoyo\r\r\n",
        "[disturbing]": "hey you never know",
        "a": {
          "[]": "a square?",
          "av": "a val",
          "b": {
            "c": {
              "e": "1",
              "j": "2",
            },
          },
          "cr": [
            "four",
            "eight",
          ],
          "e": "{ o: p, a: { av: a val, b: { c: { e: 'this [value]' } } } }",
          "j": "{ o: \"p\", a: { av: \"a val\", b: { c: { e: \"this [value]\" } } } }",
        },
        "a with spaces": "b  c",
        "ar": [
          "one",
          "three",
          "this is included",
        ],
        "br": "warm",
        "eq": "eq=eq",
        "false": false,
        "null": null,
        "o": "p",
        "s": "something",
        "s1": "\"something'",
        "s2": "something else",
        "s3": "",
        "s4": "",
        "s5": "   ",
        "s6": " a ",
        "s7": true,
        "true": true,
        "undefined": "undefined",
        "x.y.z": {
          "a.b.c": {
            "a.b.c": "abc",
            "nocomment": "this\\; this is not a comment",
            "noHashComment": "this\\# this is not a comment",
          },
          "x.y.z": "xyz",
        },
        "zr": [
          "deedee",
        ],
      })
  })


})


describe('multiline', () => {

  test('backslash-continuation', () => {
    const jm = new Tabnas().use(jsonic).use(Ini, { multiline: true })

    // Basic continuation with \<LF>
    assert.deepEqual(jm.parse('a = hello \\\nworld'), { a: 'hello world' })

    // Continuation with leading whitespace on next line (consumed)
    assert.deepEqual(jm.parse('a = hello \\\n    world'), { a: 'hello world' })

    // Multiple continuations
    assert.deepEqual(jm.parse('a = one \\\ntwo \\\nthree'), { a: 'one two three' })

    // No continuation: normal newline ends value
    assert.deepEqual(jm.parse('a = hello\nb = world'), { a: 'hello', b: 'world' })

    // Continuation with \<CR><LF>
    assert.deepEqual(jm.parse('a = hello \\\r\nworld'), { a: 'hello world' })

    // Escaped backslash before newline is NOT continuation
    assert.deepEqual(jm.parse('a = path\\\\\nb = next'), { a: 'path\\', b: 'next' })

    // Continuation in a section
    assert.deepEqual(jm.parse('[s]\na = hello \\\n    world'), { s: { a: 'hello world' } })

    // Empty value with continuation
    assert.deepEqual(jm.parse('a = \\\nworld'), { a: 'world' })

    // Inline comments off by default: ; is literal in value
    assert.deepEqual(jm.parse('a = hello \\\nworld ;not-a-comment\nb = 2'),
      { a: 'hello world ;not-a-comment', b: '2' })
  })

  test('indent-continuation', () => {
    const ji = new Tabnas().use(jsonic).use(Ini, { multiline: { indent: true, continuation: false } })

    // Indented line continues previous value
    assert.deepEqual(ji.parse('a = hello\n    world'), { a: 'hello world' })

    // Multiple indent continuations
    assert.deepEqual(ji.parse('a = line1\n  line2\n  line3'), { a: 'line1 line2 line3' })

    // Non-indented line is a new key
    assert.deepEqual(ji.parse('a = hello\nb = world'), { a: 'hello', b: 'world' })

    // Tab indent
    assert.deepEqual(ji.parse('a = hello\n\tworld'), { a: 'hello world' })

    // Indent continuation in section
    assert.deepEqual(ji.parse('[s]\na = hello\n    world'),
      { s: { a: 'hello world' } })
  })

  test('multiline-with-boolean-option', () => {
    // multiline: true enables defaults (backslash continuation, no indent)
    const jm = new Tabnas().use(jsonic).use(Ini, { multiline: true })
    assert.deepEqual(jm.parse('a = hello \\\nworld'), { a: 'hello world' })
  })

  test('multiline-both-modes', () => {
    // Both continuation char and indent enabled
    const jb = new Tabnas().use(jsonic).use(Ini, {
      multiline: { continuation: '\\', indent: true }
    })

    // Backslash continuation works
    assert.deepEqual(jb.parse('a = hello \\\nworld'), { a: 'hello world' })

    // Indent continuation also works
    assert.deepEqual(jb.parse('a = hello\n    world'), { a: 'hello world' })
  })

  test('multiline-escapes', () => {
    // Multiline with inline comments active and backslash escaping
    const jm = new Tabnas().use(jsonic).use(Ini, {
      multiline: true,
      comment: { inline: { active: true, escape: { backslash: true } } },
    })

    // Escaped comment chars still work with continuation
    assert.deepEqual(jm.parse('a = one\\; two \\\nthree'),
      { a: 'one; two three' })

    // Escaped hash
    assert.deepEqual(jm.parse('a = one\\# two \\\nthree'),
      { a: 'one# two three' })
  })

  test('multiline-no-inline-comments', () => {
    // Multiline without inline comments: ; and # are literal
    const jm = new Tabnas().use(jsonic).use(Ini, { multiline: true })

    assert.deepEqual(jm.parse('a = one; two \\\nthree'),
      { a: 'one; two three' })

    assert.deepEqual(jm.parse('a = one# two \\\nthree'),
      { a: 'one# two three' })
  })
})


describe('section-duplicate', () => {

  test('merge-default', () => {
    const j = new Tabnas().use(jsonic).use(Ini)

    // Default: merge keys from duplicate sections
    assert.deepEqual(j.parse('[a]\nx=1\ny=2\n[a]\nz=3'),
      { a: { x: '1', y: '2', z: '3' } })

    // Duplicate key: last value wins
    assert.deepEqual(j.parse('[a]\nx=1\n[a]\nx=2'),
      { a: { x: '2' } })

    // Nested duplicate sections merge
    assert.deepEqual(j.parse('[a.b]\nx=1\n[a.b]\ny=2'),
      { a: { b: { x: '1', y: '2' } } })

    // Intermediate path preserved when merging
    assert.deepEqual(j.parse('[a.b]\nx=1\n[a]\ny=2'),
      { a: { b: { x: '1' }, y: '2' } })
  })

  test('merge-explicit', () => {
    const jm = new Tabnas().use(jsonic).use(Ini, { section: { duplicate: 'merge' } })

    assert.deepEqual(jm.parse('[a]\nx=1\n[a]\ny=2'),
      { a: { x: '1', y: '2' } })
  })

  test('override', () => {
    const jo = new Tabnas().use(jsonic).use(Ini, { section: { duplicate: 'override' } })

    // Second occurrence replaces first
    assert.deepEqual(jo.parse('[a]\nx=1\ny=2\n[a]\nz=3'),
      { a: { z: '3' } })

    // First occurrence works normally
    assert.deepEqual(jo.parse('[a]\nx=1'),
      { a: { x: '1' } })

    // Override clears subsections too
    assert.deepEqual(jo.parse('[a.b]\nx=1\n[a]\ny=2\n[a]\nz=3'),
      { a: { z: '3' } })

    // Non-duplicate sections unaffected
    assert.deepEqual(jo.parse('[a]\nx=1\n[b]\ny=2'),
      { a: { x: '1' }, b: { y: '2' } })

    // Nested override
    assert.deepEqual(jo.parse('[a.b]\nx=1\n[a.b]\ny=2'),
      { a: { b: { y: '2' } } })
  })

  test('error', () => {
    const je = new Tabnas().use(jsonic).use(Ini, { section: { duplicate: 'error' } })

    // Single section: no error
    assert.deepEqual(je.parse('[a]\nx=1'), { a: { x: '1' } })

    // Multiple distinct sections: no error
    assert.deepEqual(je.parse('[a]\nx=1\n[b]\ny=2'),
      { a: { x: '1' }, b: { y: '2' } })

    // Duplicate section: throws
    assert.throws(() => je.parse('[a]\nx=1\n[a]\ny=2'),
      /Duplicate section/)

    // Duplicate nested section: throws
    assert.throws(() => je.parse('[a.b]\nx=1\n[a.b]\ny=2'),
      /Duplicate section/)

    // Intermediate path is NOT a declared section
    // [a.b] creates intermediate [a] but does not declare it
    assert.deepEqual(je.parse('[a.b]\nx=1\n[a]\ny=2'),
      { a: { b: { x: '1' }, y: '2' } })
  })
})


describe('inline-comment', () => {

  test('off-by-default', () => {
    // Default: inline comments are off. ; and # mid-value are literal.
    const j = new Tabnas().use(jsonic).use(Ini)

    assert.deepEqual(j.parse('a = hello ; world'), { a: 'hello ; world' })
    assert.deepEqual(j.parse('a = hello # world'), { a: 'hello # world' })
    assert.deepEqual(j.parse('a = x;y;z'), { a: 'x;y;z' })

    // Line-start comments still work
    assert.deepEqual(j.parse('; comment\na = 1'), { a: '1' })
    assert.deepEqual(j.parse('# comment\na = 1'), { a: '1' })
  })

  test('active-basic', () => {
    // Inline comments active with defaults (chars: ['#', ';'])
    const j = new Tabnas().use(jsonic).use(Ini, {
      comment: { inline: { active: true } },
    })

    assert.deepEqual(j.parse('a = hello ; comment'), { a: 'hello' })
    assert.deepEqual(j.parse('a = hello # comment'), { a: 'hello' })
    assert.deepEqual(j.parse('a = x;y'), { a: 'x' })
    assert.deepEqual(j.parse('a = value\nb = other'), { a: 'value', b: 'other' })
  })

  test('custom-chars', () => {
    // Only ; is an inline comment char, not #
    const j = new Tabnas().use(jsonic).use(Ini, {
      comment: { inline: { active: true, chars: [';'] } },
    })

    assert.deepEqual(j.parse('a = hello ; comment'), { a: 'hello' })
    assert.deepEqual(j.parse('a = hello # not a comment'), { a: 'hello # not a comment' })
  })

  test('backslash-escape', () => {
    // Backslash escaping enabled (default when active)
    const j = new Tabnas().use(jsonic).use(Ini, {
      comment: { inline: { active: true, escape: { backslash: true } } },
    })

    assert.deepEqual(j.parse('a = hello\\; world'), { a: 'hello; world' })
    assert.deepEqual(j.parse('a = hello\\# world'), { a: 'hello# world' })
    assert.deepEqual(j.parse('a = x\\;y ; comment'), { a: 'x;y' })
  })

  test('backslash-escape-disabled', () => {
    // Backslash escaping explicitly disabled: \; keeps both chars but
    // the escapeChar still prevents ; from terminating. The difference
    // is that backslash is preserved in the output rather than consumed.
    const j = new Tabnas().use(jsonic).use(Ini, {
      comment: { inline: { active: true, escape: { backslash: false } } },
    })

    // \; → \; (backslash preserved, ; did not terminate)
    assert.deepEqual(j.parse('a = hello\\; world'), { a: 'hello\\; world' })

    // Unescaped ; still terminates
    assert.deepEqual(j.parse('a = hello ; comment'), { a: 'hello' })
  })

  test('whitespace-prefix', () => {
    // Whitespace-prefix mode: only treat as comment if preceded by whitespace
    const j = new Tabnas().use(jsonic).use(Ini, {
      comment: { inline: { active: true, escape: { whitespace: true } } },
    })

    // No whitespace before ;  →  literal
    assert.deepEqual(j.parse('a = x;y;z'), { a: 'x;y;z' })

    // Whitespace before ;  →  inline comment
    assert.deepEqual(j.parse('a = hello ;comment'), { a: 'hello' })
    assert.deepEqual(j.parse('a = hello\t;comment'), { a: 'hello' })

    // Same for #
    assert.deepEqual(j.parse('a = x#y'), { a: 'x#y' })
    assert.deepEqual(j.parse('a = hello #comment'), { a: 'hello' })
  })

  test('whitespace-prefix-with-backslash', () => {
    // Both whitespace and backslash escaping
    const j = new Tabnas().use(jsonic).use(Ini, {
      comment: {
        inline: {
          active: true,
          escape: { whitespace: true, backslash: true },
        },
      },
    })

    // No whitespace: literal
    assert.deepEqual(j.parse('a = x;y'), { a: 'x;y' })

    // Whitespace present: comment
    assert.deepEqual(j.parse('a = hello ;comment'), { a: 'hello' })

    // Backslash escape overrides whitespace: literal
    assert.deepEqual(j.parse('a = hello \\;not-a-comment'), { a: 'hello ;not-a-comment' })
  })

  test('with-multiline', () => {
    // Inline comments active + multiline continuation
    const j = new Tabnas().use(jsonic).use(Ini, {
      multiline: true,
      comment: { inline: { active: true } },
    })

    // Comment terminates continued value
    assert.deepEqual(j.parse('a = hello \\\nworld ;comment\nb = 2'),
      { a: 'hello world', b: '2' })

    // Escaped comment char in multiline value
    assert.deepEqual(j.parse('a = hello\\; \\\nworld'),
      { a: 'hello; world' })
  })

  test('with-sections', () => {
    const j = new Tabnas().use(jsonic).use(Ini, {
      comment: { inline: { active: true } },
    })

    assert.deepEqual(j.parse('[s]\na = val ; comment\nb = other'),
      { s: { a: 'val', b: 'other' } })
  })

  test('line-comments-always-work', () => {
    // Line-start comments work regardless of inline comment setting
    const jOff = new Tabnas().use(jsonic).use(Ini)
    const jOn = new Tabnas().use(jsonic).use(Ini, {
      comment: { inline: { active: true } },
    })

    const input = '; line comment\n# hash comment\na = 1'
    assert.deepEqual(jOff.parse(input), { a: '1' })
    assert.deepEqual(jOn.parse(input), { a: '1' })
  })
})


describe('number-lex', () => {

  // Enable number lexing via post-config so Jsonic parses numeric values as numbers
  function makeWithNumbers() {
    const jn = new Tabnas().use(jsonic).use(Ini)
    jn.options({ number: { lex: true } })
    return jn
  }

  test('integers', () => {
    const jn = makeWithNumbers()

    assert.deepEqual(jn.parse('a=1'), { a: 1 })
    assert.deepEqual(jn.parse('a=0'), { a: 0 })
    assert.deepEqual(jn.parse('a=-3'), { a: -3 })
    assert.deepEqual(jn.parse('a=+2'), { a: 2 })
    assert.deepEqual(jn.parse('a=42\nb=99'), { a: 42, b: 99 })
  })

  test('floats', () => {
    const jn = makeWithNumbers()

    assert.deepEqual(jn.parse('a=2.5'), { a: 2.5 })
    assert.deepEqual(jn.parse('a=0.0'), { a: 0 })
    assert.deepEqual(jn.parse('a=-1.25'), { a: -1.25 })
  })

  test('scientific-notation', () => {
    const jn = makeWithNumbers()

    assert.deepEqual(jn.parse('a=1e10'), { a: 1e10 })
  })

  test('hex', () => {
    const jn = makeWithNumbers()

    assert.deepEqual(jn.parse('a=0xFF'), { a: 255 })
  })

  test('mixed-types', () => {
    const jn = makeWithNumbers()

    // Numbers and strings coexist
    assert.deepEqual(jn.parse('a=1\nb=hello\nc=2.5\nd=true'),
      { a: 1, b: 'hello', c: 2.5, d: true })

    // Non-numeric strings stay as strings
    assert.deepEqual(jn.parse('a=1abc'), { a: '1abc' })

    // Empty value stays as empty string
    assert.deepEqual(jn.parse('a=\nb=1'), { a: '', b: 1 })
  })

  test('in-sections', () => {
    const jn = makeWithNumbers()

    assert.deepEqual(jn.parse('[s]\na=42\nb=text'),
      { s: { a: 42, b: 'text' } })

    assert.deepEqual(jn.parse('[s]\na=1\n[t]\nb=2'),
      { s: { a: 1 }, t: { b: 2 } })
  })

  test('arrays', () => {
    const jn = makeWithNumbers()

    assert.deepEqual(jn.parse('a[]=1\na[]=2\na[]=hello'),
      { a: [1, 2, 'hello'] })
  })

  test('default-numbers-are-strings', () => {
    // Without number.lex, all values are strings
    const j = new Tabnas().use(jsonic).use(Ini)

    assert.deepEqual(j.parse('a=1'), { a: '1' })
    assert.deepEqual(j.parse('a=2.5'), { a: '2.5' })
    assert.deepEqual(j.parse('a=-3'), { a: '-3' })
    assert.deepEqual(j.parse('a=0xFF'), { a: '0xFF' })
  })
})
