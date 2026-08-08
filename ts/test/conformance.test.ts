/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

/* Third-party INI conformance suite (TypeScript half; the Go half is
 * go/ini_conformance_test.go and asserts exactly the same manifest).
 *
 * INI has no authoritative specification and no official test suite, so
 * this is an ASSEMBLED corpus: real .ini files taken verbatim from four
 * widely used implementations at pinned commit SHAs, with the expected
 * values produced by one of them (npm/ini) acting as the oracle. The
 * manifest test/corpus/ini-corpus.json carries the sources, the oracle
 * values, and the upstream provenance of every case.
 *
 * THIS TEST MUST NEVER SKIP. If the manifest is absent it FAILS. A
 * conformance test that quietly does not run turns a green tick into a
 * lie, which is the exact defect this suite exists to remove.
 *
 * What it asserts, and why the divergence lists are not an escape hatch:
 *
 *   1. EVERY valid document must PARSE. No exceptions, no list. A crash
 *      on a real-world .ini file is always a bug.
 *   2. Every valid document must equal the oracle value, UNLESS it is in
 *      DIVERGENT — and each DIVERGENT entry must then actually differ.
 *      An entry that starts matching fails the test and must be deleted,
 *      so the list cannot rot into a silent allowlist.
 *   3. Every document the upstreams label malformed must be REJECTED,
 *      unless it is in ACCEPTED_BY_DESIGN (documents that are only
 *      "malformed" because the upstream lacks bare boolean keys, which
 *      this plugin documents and supports) — and those must then parse.
 *
 * Every DIVERGENT entry names a documented option or dialect rule from
 * ts/doc/reference.md. If you cannot write such a reason for a new
 * mismatch, it is a bug: fix the plugin, do not extend the list.
 */

import { test, describe } from 'node:test'
import assert from 'node:assert'
import { existsSync, readFileSync } from 'fs'
import { join } from 'path'

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '../dist/ini'


const REPO = join(__dirname, '..', '..')
const MANIFEST = join(REPO, 'test', 'corpus', 'ini-corpus.json')

const MISSING_CORPUS =
  'INI conformance corpus not found at test/corpus/ini-corpus.json.\n' +
  '\n' +
  'The manifest is self-contained (sources + oracle values + pinned\n' +
  'upstream SHAs) and ships with the repo. This test deliberately FAILS\n' +
  'rather than skips: a conformance suite that silently does not run\n' +
  'reports green while measuring nothing.'


// Documents whose parse differs from the npm/ini oracle because the two
// implement different INI dialects. The reason must name the documented
// behaviour responsible.
const DIVERGENT: Record<string, string> = {
  'go-ini/testdata/full.ini':
    'inline comments are off by default (comment.inline.active), and the ' +
    'oracle treats `[comments] ; c` as a bare key because it requires a ' +
    'section header to be the whole line',
  'inih/examples/test.ini':
    'the oracle requires a section header to be the entire line, so ' +
    '`[protocol]  ; Protocol configuration` is a bare key for it; this ' +
    'plugin reads the section (as inih, iniparser and go-ini do). Inline ' +
    'comments are also off by default',
  'inih/fuzzing/testcases/case1.ini':
    'same section-header-with-trailing-comment rule, plus inline comments ' +
    'off by default',
  'inih/tests/multi_line.ini':
    'inline comments are off by default (comment.inline.active)',
  'inih/tests/normal.ini':
    'same section-header-with-trailing-comment rule, plus inline comments ' +
    'off by default',
  'iniparser/example/twisted.ini':
    'inline comments off by default, backslash-escaped newline continues ' +
    'a value by default, and quote handling for degenerate inputs ' +
    "(`s3='''`) differs from the oracle",
  'iniparser/test/ressources/bad_ini/twisted-ofval.ini':
    'a backslash before a newline escapes it, so the value continues ' +
    '(multiline.continuation); the oracle has no continuation',
  'iniparser/test/ressources/good_ini/spaced2.ini':
    'inline comments are off by default (comment.inline.active)',
  'iniparser/test/ressources/old.ini':
    'inline comments are off by default (comment.inline.active)',
  'iniparser/test/ressources/quotes.ini':
    'inline comments are off by default (comment.inline.active)',
  'iniparser/test/ressources/utf8.ini':
    'inline comments are off by default (comment.inline.active)',
  'npm-ini/test/fixtures/foo.ini':
    'with inline comments off, `\\;` is a literal backslash followed by a ' +
    'literal `;` — backslash escaping of comment chars is a ' +
    'comment.inline.escape option, so it does not apply when inline ' +
    'comments are inactive',
}

// Documents an upstream ships as malformed that are valid INI under the
// dialect this plugin documents.
const ACCEPTED_BY_DESIGN: Record<string, string> = {
  'inih/tests/bad_comment.ini':
    '`This is an error` is a bare boolean key, a documented feature',
  'inih/tests/bad_multi.ini':
    '`  indented` is a bare boolean key, a documented feature',
  'iniparser/test/ressources/bad_ini/ends_well.ini':
    '`error is here` is a bare boolean key, a documented feature',
  'iniparser/test/ressources/bad_ini/twisted-errors.ini':
    'every line in it is a bare boolean key, a documented feature',
}


type Case = {
  name: string
  kind: 'valid' | 'invalid'
  source: string
  expected?: any
  label?: string
}


function loadManifest() {
  if (!existsSync(MANIFEST)) {
    throw new Error(MISSING_CORPUS)
  }
  const m = JSON.parse(readFileSync(MANIFEST, 'utf8'))
  if (!m || !Array.isArray(m.cases) || 0 === m.cases.length) {
    throw new Error(
      'test/corpus/ini-corpus.json is present but contains no cases.')
  }
  return m
}


// The documented setup at its documented defaults. Comparing defaults
// against the oracle's defaults is the only like-for-like reading;
// turning on non-default options per document would be tuning the
// instrument.
function makeIni() {
  return new Tabnas().use(jsonic).use(Ini)
}


// Both sides through JSON so a null-prototype object and a plain object
// with the same content compare equal. This normalises representation
// only — it never makes differing values compare equal.
function norm(v: any) {
  return JSON.parse(JSON.stringify(undefined === v ? null : v))
}


describe('conformance (third-party corpus)', () => {
  const manifest = loadManifest()
  const cases: Case[] = manifest.cases
  const valid = cases.filter((c) => 'valid' === c.kind)
  const invalid = cases.filter((c) => 'invalid' === c.kind)

  test('corpus is loaded and has both halves', () => {
    assert.strictEqual(valid.length, 30, 'valid case count changed')
    assert.strictEqual(invalid.length, 6, 'invalid case count changed')
    assert.strictEqual(valid.length + invalid.length, cases.length,
      'every case must be classified valid or invalid')
  })

  test('every valid document parses', () => {
    for (const c of valid) {
      assert.doesNotThrow(() => makeIni().parse(c.source),
        `${c.name}: a real-world .ini file must not crash the parser`)
    }
  })

  describe('valid documents match the npm/ini oracle', () => {
    for (const c of valid) {
      const why = DIVERGENT[c.name]
      test(c.name + (why ? ' (documented divergence)' : ''), () => {
        const got = norm(makeIni().parse(c.source))
        const want = norm(c.expected)
        if (undefined === why) {
          assert.deepStrictEqual(got, want,
            `${c.name}: parsed, but to a different value than the ` +
            `npm/ini oracle. If this is a dialect difference, add it to ` +
            `DIVERGENT with the documented reason; otherwise it is a bug.`)
        } else {
          assert.notDeepStrictEqual(got, want,
            `${c.name}: now MATCHES the oracle, so its DIVERGENT entry ` +
            `("${why}") is stale — delete it.`)
        }
      })
    }
  })

  describe('documents the upstreams label malformed', () => {
    for (const c of invalid) {
      const why = ACCEPTED_BY_DESIGN[c.name]
      test(c.name + (why ? ' (valid in this dialect)' : ''), () => {
        let threw = false
        let got: any
        try {
          got = makeIni().parse(c.source)
        } catch {
          threw = true
        }
        if (undefined === why) {
          assert.ok(threw,
            `${c.name}: accepted a document its upstream labels ` +
            `malformed.\n  upstream label: ${c.label}\n` +
            `  parsed as:      ${JSON.stringify(got)}`)
        } else {
          assert.ok(!threw,
            `${c.name}: rejected, but ACCEPTED_BY_DESIGN says it is ` +
            `valid here (${why}). Either the plugin regressed or the ` +
            `entry is stale.`)
        }
      })
    }
  })
})
