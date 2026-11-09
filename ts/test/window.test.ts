/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

// A parse from a reader (`tn.parseReader`, admin ADR-30 and ADR-33).
//
// `make()` builds the parser whole, as `new Tabnas().use(jsonic).use(Ini,
// options)` builds it, and declares `lex.window` on it, so that a reader is
// read a window at a time. The string check and hoover's blocks read the
// source only through the window, a scan that reaches the window's end
// filling it and reading on; the string check holds a quoted key's text
// before the string matcher reads it (tabnas/parser#326); and the
// multiline value matcher ends a value the window ends inside at the
// window's end, a token the engine cuts again. These tests hold it to
// that: `make()` declares the window and parses as that composition does;
// every row of the shared fixtures, under its file's options, and every
// document of the conformance corpus, under the default options and the
// option sets that install the multiline matcher and the inline comments,
// parses the same from a reader, in pieces of 1, 3 and 64 bytes, under
// windows of a few UTF-16 units and the engine's own, as from its text; so
// does what each scan decides on, at every distance from the window's end;
// and values and lines longer than the window, and a long document, parse
// the same too.

import { test, describe } from 'node:test'
import assert from 'node:assert'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { inspect } from 'node:util'

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { findSpecDir, loadSpecDir } from '@tabnas/support'

import { Ini, make } from '../dist/ini'
import type { IniOptions } from '../dist/ini'

import { OPTIONS } from './fixture-options'

const CORPUS = join(__dirname, '..', '..', 'test', 'corpus', 'ini-corpus.json')

type Reader = { read(buf: Uint8Array): number }
type Sizes = { margin: number; block: number; chunk: number }
type Seen = { value?: unknown; error?: string }

// A reader that hands its bytes over at most `size` at a time.
function pieces(bytes: Uint8Array, size: number): Reader {
  let at = 0
  return {
    read(buf: Uint8Array): number {
      const n = Math.min(size, buf.length, bytes.length - at)
      buf.set(bytes.subarray(at, at + n))
      at += n
      return n
    },
  }
}

// What a parse shows: its value, or its error's code, position, row,
// column and token text. An error's excerpt is left out: a parse from a
// reader shows a long line only as far back as the window reaches
// (tabnas/parser DIVERGENCE.md), the one difference allowed.
function seen(parse: () => unknown): Seen {
  try {
    return { value: parse() }
  } catch (e: any) {
    if (null == e || 'function' !== typeof e.toJSON) {
      return { error: 'throw ' + (e && e.name) + ' ' + (e && e.message) }
    }
    const d = e.toJSON()
    return {
      error:
        d.code + ' at ' + d.pos + ' ' + d.row + ':' + d.col + ' ' +
        JSON.stringify(d.token.src),
    }
  }
}

const show = (o: Seen): string =>
  undefined !== o.error ? 'error ' + o.error : 'value ' + inspect(o.value, { depth: 4 })

function same(a: Seen, b: Seen): boolean {
  if (a.error !== b.error) return false
  try {
    assert.deepStrictEqual(a.value, b.value)
    return true
  } catch (e) {
    return false
  }
}

// The pieces a reader hands its bytes over in, and the windows (margin and
// block) it is read under besides the engine's own: a few units, so that
// every scan meets the window's edge.
const PIECES = [1, 3, 64]
const WINDOWS = [[1, 1], [3, 2], [64, 64]]

// `text` parsed by `tn` whole, and from a reader in every pieces size,
// under the engine's window and each of WINDOWS: how many reader parses
// were compared. A difference is pushed onto `failures`. A document over
// 4 KiB is read in pieces of 64 bytes alone, under each window.
function sameFromReader(tn: Tabnas, at: string, text: string, failures: string[]): number {
  const whole = seen(() => tn.parse(text))
  const bytes = Buffer.from(text, 'utf8')
  const anyTn = tn as any
  let compared = 0
  for (const size of 4096 < bytes.length ? [64] : PIECES) {
    const reads: [string, () => unknown][] = [['', () => anyTn.parseReader(pieces(bytes, size))]]
    for (const [margin, block] of WINDOWS) {
      const sizes: Sizes = { margin, block, chunk: size }
      reads.push([
        ', margin ' + margin + ', block ' + block,
        () => anyTn.parseReaderWithSizes(pieces(bytes, size), sizes),
      ])
    }
    for (const [how, parse] of reads) {
      const read = seen(parse)
      compared++
      if (!same(read, whole)) {
        failures.push(
          at + ': ' + JSON.stringify(text.slice(0, 80)) + ' from a reader in pieces of ' +
          size + how + '\n  whole:  ' + show(whole) + '\n  reader: ' + show(read),
        )
      }
    }
  }
  return compared
}

// The option sets the corpus and the long documents are read under: the
// defaults, inline comments (hoover's value block ending at a comment
// character), the multiline matcher with continuation and indent, and the
// multiline matcher that inline comments preceded by whitespace install.
const OPTION_SETS: [string, IniOptions][] = [
  ['default', {}],
  ['inline-comments-active', OPTIONS['inline-comments-active']],
  ['multiline-both', OPTIONS['multiline-both']],
  ['inline-comments-whitespace-backslash', OPTIONS['inline-comments-whitespace-backslash']],
]

// The documents whose deciding character sits after a run of 0 to 40
// characters: a closing quote and what follows it on its line, a quoted
// key's closing quote, a continuation, an indented next line, a line end,
// an escape, an inline comment, a section's closing bracket. Each is the
// option set it is read under (OPTIONS' name, or '' for the defaults), the
// text before the run and the text after it. Kept in step with EDGES in
// rs/tests/reader_test.rs.
const EDGES: [string, string, string][] = [
  ['', 'a = "', '"\r\nb = 1\r\n'],
  ['', 'a = "', '"  \nb = 1\n'],
  ['', 'a = "', '" x\nb = 1\n'],
  ['', 'a = "', '\\""\nb = 1\n'],
  ['', '"', '"'],
  ['', '"', '" = 1'],
  ['', '"', '"\nb = 1\n'],
  ['', 'a = ', '\\;x\nb = 1\n'],
  ['', 'a = ', '\r\nb = 1\n'],
  ['', '[', ']\na = 1\n'],
  ['', '[', '\na = 1\n'],
  ['inline-comments-active', 'a = "', '" ; c\nb = 1\n'],
  ['inline-comments-active', 'a = ', '\\; x ; c\nb = 1\n'],
  ['multiline-both', 'a = ', '\\\n  two\nb = 1\n'],
  ['multiline-both', 'a = ', '\\\r\n  two\nb = 1\n'],
  ['multiline-both', 'a = ', '\n  two\nb = 1\n'],
  ['multiline-both', 'a = ', '\r\n  two\nb = 1\n'],
  ['multiline-both', 'a = ', '\\nx\nb = 1\n'],
  ['inline-comments-whitespace-backslash', 'a = ', ' \\; x ; c\nb = 1\n'],
  ['inline-comments-whitespace-backslash', 'a = ', '\\\n  two ; c\nb = 1\n'],
]

describe('window', () => {
  test('make-declares-the-window', () => {
    const tn = make() as any
    assert.equal(tn.internal().config.lex.window, true)
    // The plugin alone declares nothing: a host may install it beneath
    // matchers of its own.
    const composed = new Tabnas().use(jsonic).use(Ini, {}) as any
    assert.ok(!composed.internal().config.lex.window)
  })

  test('make-parses-as-the-composition-does', () => {
    let compared = 0
    const failures: string[] = []
    for (const spec of loadSpecDir(findSpecDir(__dirname), { minCols: 2 })) {
      const options = OPTIONS[spec.file.replace(/\.tsv$/, '')] || {}
      const made = make(options)
      const composed = new Tabnas().use(jsonic).use(Ini, options)
      for (const row of spec.rows) {
        const input = row.unescNamed('input')
        const a = seen(() => made.parse(input))
        const b = seen(() => composed.parse(input))
        compared++
        if (!same(a, b)) {
          failures.push(spec.file + ':' + row.line + '\n  make:     ' + show(a) +
            '\n  composed: ' + show(b))
        }
      }
    }
    assert.ok(200 < compared, '' + compared)
    assert.deepEqual(failures, [])
  })

  test('every-row-parses-the-same-from-a-reader', () => {
    const specs = loadSpecDir(findSpecDir(__dirname), { minCols: 2 })
    let compared = 0
    const failures: string[] = []
    specs.forEach((spec, n) => {
      const tn = make(OPTIONS[spec.file.replace(/\.tsv$/, '')] || {})
      for (const row of spec.rows) {
        compared += sameFromReader(tn, spec.file + ':' + row.line, row.unescNamed('input'), failures)
      }
      if (0 === (n + 1) % 10 || n + 1 === specs.length) {
        console.log('window fixtures: ' + (n + 1) + ' of ' + specs.length + ' files (' +
          Math.floor(((n + 1) * 100) / specs.length) + '%), ' + compared + ' reader parses')
      }
    })
    assert.ok(2000 < compared, '' + compared)
    assert.deepEqual(failures, [])
  })

  test('the-corpus-parses-the-same-from-a-reader', () => {
    const cases = JSON.parse(readFileSync(CORPUS, 'utf8')).cases
    let compared = 0
    const failures: string[] = []
    OPTION_SETS.forEach(([name, options], n) => {
      const tn = make(options)
      for (const c of cases) {
        compared += sameFromReader(tn, c.name + ' [' + name + ']', c.source, failures)
      }
      console.log('window corpus: ' + (n + 1) + ' of ' + OPTION_SETS.length + ' option sets (' +
        Math.floor(((n + 1) * 100) / OPTION_SETS.length) + '%), ' + compared + ' reader parses')
    })
    assert.equal(cases.length, 36)
    assert.ok(1000 < compared, '' + compared)
    assert.deepEqual(failures, [])
  })

  test('every-scan-meets-the-window-end', () => {
    // Each of EDGES with its run of 0 to 40 characters, from a reader in
    // pieces of 1 and 3 bytes under windows of one and three units, which
    // a scan's fills widen from the token's start, so that the window ends
    // at the deciding character, before it and after it.
    let compared = 0
    const failures: string[] = []
    EDGES.forEach(([name, before, after], n) => {
      const tn = make(OPTIONS[name] || {}) as any
      for (let run = 0; run <= 40; run++) {
        const text = before + 'x'.repeat(run) + after
        const whole = seen(() => tn.parse(text))
        const bytes = Buffer.from(text, 'utf8')
        for (const size of [1, 3]) {
          for (const [margin, block] of [[1, 1], [3, 2]]) {
            const read = seen(() =>
              tn.parseReaderWithSizes(pieces(bytes, size), { margin, block, chunk: size }),
            )
            compared++
            if (!same(read, whole)) {
              failures.push(
                'edge ' + n + ' [' + name + '], run ' + run + ': ' + JSON.stringify(text) +
                ' from a reader in pieces of ' + size + ', margin ' + margin + ', block ' +
                block + '\n  whole:  ' + show(whole) + '\n  reader: ' + show(read),
              )
            }
          }
        }
      }
      if (0 === (n + 1) % 5 || n + 1 === EDGES.length) {
        console.log('window ends: ' + (n + 1) + ' of ' + EDGES.length + ' documents (' +
          Math.floor(((n + 1) * 100) / EDGES.length) + '%), ' + compared + ' reader parses')
      }
    })
    assert.ok(3000 < compared, '' + compared)
    assert.deepEqual(failures, [])
  })

  test('values-longer-than-the-window-parse-the-same-from-a-reader', () => {
    // The engine's window reaches 64 KiB past the cursor before each token:
    // a value or a line longer than that meets its edge under the engine's
    // own sizes.
    const long = (64 << 10) + 2048
    const docs = [
      'a = ' + 'v'.repeat(long) + '\nb = 1\n',
      'a = "' + 'q'.repeat(long) + '"\nb = 1\n',
      'a = "q"' + ' '.repeat(long) + 'x\nb = 1\n',
      'a = "q"' + ' '.repeat(long) + ' ; note\nb = 1\n',
      'k'.repeat(long) + ' = 1\n',
      // A quoted key the window ends inside, with the end of the source
      // near: the string matcher reads it (tabnas/parser#326).
      '"' + 'k'.repeat(long) + '"\n',
      '"' + 'k'.repeat(long) + '" = 1\n',
      '[' + 's'.repeat(long) + ']\na = 1\n',
      '; ' + 'c'.repeat(long) + '\na = 1\n',
      'a = x ; ' + 'c'.repeat(long) + '\nb = 1\n',
      'a = one \\\n' + ' '.repeat(long) + 'two\nb = 1\n',
      'a = one\n' + ' '.repeat(long) + 'two\nb = 1\n',
      'a = ' + 'w'.repeat(long) + '\\\r\n two\r\nb = 1\r\n',
      'a = ' + 'e'.repeat(long) + '\\; x\n',
      '[' + 'u'.repeat(long) + '\na = 1\n',
      'a = "' + 'o'.repeat(long),
    ]
    const failures: string[] = []
    for (const [name, options] of OPTION_SETS) {
      const tn = make(options) as any
      docs.forEach((doc, n) => {
        const whole = seen(() => tn.parse(doc))
        for (const size of [4096, (64 << 10) + 3]) {
          const read = seen(() => tn.parseReader(pieces(Buffer.from(doc, 'utf8'), size)))
          if (!same(read, whole)) {
            failures.push('[' + name + '] document ' + n + ' in pieces of ' + size + ': whole ' +
              show(whole).slice(0, 300) + ', reader ' + show(read).slice(0, 300))
          }
        }
      })
      console.log('window long values: [' + name + '], ' + docs.length + ' documents')
    }
    assert.deepEqual(failures, [])
  })

  test('a-long-document-parses-the-same-from-a-reader', () => {
    // From a reader in pieces of 4 KiB, under the engine's window and under
    // one of 256 units that moves on many times; with an unterminated
    // section header at the end too, which the reader places where the
    // whole text does.
    const parts: string[] = ['; settings\nname = top\n']
    for (let i = 0; i < 2000; i++) {
      parts.push(
        '[section' + i + '.sub]\nkey' + i + ' = value ' + i + ' é\nquoted = "q;' + i + '"\n' +
        'path = a\\;b\\\\c ; note ' + i + '\nlist[] = ' + i + '\nflag\n' +
        'wrapped = one \\\n  two ' + i + '\n  indented ' + i + '\n# comment ' + i + '\n',
      )
    }
    const body = parts.join('')
    const text = body + 'last = 1\n'
    const broken = body + '[open\n'
    assert.ok(256 << 10 < text.length, '' + text.length)
    for (const [name, options] of OPTION_SETS) {
      const tn = make(options) as any
      ;[text, broken].forEach((doc, n) => {
        const whole = seen(() => tn.parse(doc))
        assert.equal(undefined === whole.error, 0 === n, show(whole).slice(0, 200))
        const bytes = Buffer.from(doc, 'utf8')
        for (const sizes of [undefined, { margin: 256, block: 256, chunk: 4096 }]) {
          const read = seen(() =>
            undefined === sizes
              ? tn.parseReader(pieces(bytes, 4096))
              : tn.parseReaderWithSizes(pieces(bytes, 4096), sizes),
          )
          assert.ok(same(read, whole), '[' + name + '] ' + show(read).slice(0, 300))
        }
      })
      console.log('window long document: [' + name + '], ' + text.length + ' characters')
    }
  })
})
