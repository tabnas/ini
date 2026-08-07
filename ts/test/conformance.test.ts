/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

/* Third-party INI conformance suite (TypeScript half).
 *
 * The corpus is NOT in this repo. It is fetched at pinned commit SHAs by
 * ./scripts/fetch-ini-corpus.sh into test/corpus/ (gitignored) and turned
 * into test/corpus/ini-corpus.json by scripts/build-ini-corpus.js. Read
 * the header of build-ini-corpus.js for the provenance of every label and
 * expected value, and for why INI has no authoritative suite.
 *
 * THIS TEST MUST NEVER SKIP. If the corpus is absent it FAILS with
 * instructions. A conformance test that quietly does not run turns a green
 * tick into a lie, which is the exact defect this suite exists to remove.
 *
 * Both halves are asserted:
 *   valid   -> must parse AND deep-equal the expected value
 *   invalid -> must be rejected with an error
 *
 * EXPECT THIS TO BE RED. It is a measuring instrument, not a target. Do
 * not skip a case, narrow the corpus, or loosen an assertion to make the
 * number look better.
 *
 * READ THE INVALID NUMBER WITH CARE. At the 2026-08 baseline this half
 * scores 5/6, which looks strong and is not. Four of those five rejections
 * are "unexpected character(s): <bare key>" -- the parser rejects a bare
 * key that is the FIRST pair of its map (`x` alone errors; `y=1` then `x`
 * parses), so it rejects those documents by accident, not by design. The
 * one document that is unambiguously malformed under every dialect,
 * inih/tests/bad_section.ini (`[section2` never closed), is ACCEPTED.
 */

import { test, describe } from 'node:test'
import assert from 'node:assert'
import { readFileSync, existsSync } from 'fs'
import { join } from 'path'

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '../dist/ini'


const REPO = join(__dirname, '..', '..')
const MANIFEST = join(REPO, 'test', 'corpus', 'ini-corpus.json')

const MISSING_CORPUS =
  'INI conformance corpus not found at test/corpus/ini-corpus.json.\n' +
  '\n' +
  'Fetch it (pinned SHAs, gitignored, never committed):\n' +
  '    ./scripts/fetch-ini-corpus.sh\n' +
  '\n' +
  'This test deliberately FAILS rather than skips: a conformance suite\n' +
  'that silently does not run reports green while measuring nothing.'


type Case = {
  name: string
  upstream: string
  kind: 'valid' | 'invalid'
  source: string
  expected: any
  oracleAccepts: boolean
  label: string
}

type Manifest = {
  counts: { valid: number; invalid: number; total: number }
  oracle: { name: string; sha: string }
  cases: Case[]
}


function loadManifest(): Manifest {
  if (!existsSync(MANIFEST)) {
    throw new Error(MISSING_CORPUS)
  }
  const m: Manifest = JSON.parse(readFileSync(MANIFEST, 'utf8'))
  if (!m || !Array.isArray(m.cases) || 0 === m.cases.length) {
    throw new Error(
      'INI conformance corpus at test/corpus/ini-corpus.json is present but ' +
      'contains no cases. Re-run ./scripts/fetch-ini-corpus.sh --force.'
    )
  }
  return m
}


// The documented setup, at its documented defaults. Comparing defaults
// against the oracle's defaults is the only like-for-like reading; turning
// on non-default options per document would be tuning the instrument.
function makeIni() {
  return new Tabnas().use(jsonic).use(Ini)
}


// Both sides through JSON so a null-prototype object and a plain object with
// the same content compare equal. This normalises representation only --
// it never makes differing values compare equal.
function norm(v: any): any {
  return JSON.parse(JSON.stringify(v === undefined ? null : v))
}


describe('conformance (third-party corpus)', () => {
  const manifest = loadManifest()
  const valid = manifest.cases.filter((c) => 'valid' === c.kind)
  const invalid = manifest.cases.filter((c) => 'invalid' === c.kind)

  test('corpus is loaded and has both halves', () => {
    assert.ok(valid.length > 0, 'corpus has no valid cases')
    assert.ok(invalid.length > 0, 'corpus has no invalid cases')
    assert.strictEqual(
      valid.length + invalid.length, manifest.cases.length,
      'every case must be classified valid or invalid')
  })

  describe('valid documents parse to the expected value', () => {
    for (const c of valid) {
      test(c.name, () => {
        const j = makeIni()
        let got: any
        try {
          got = j.parse(c.source)
        } catch (e: any) {
          assert.fail(
            `${c.name}: rejected a valid document\n` +
            `  error:    ${String(e && e.message).split('\n')[0]}\n` +
            `  expected: ${JSON.stringify(c.expected)}`)
        }
        assert.deepStrictEqual(
          norm(got), norm(c.expected),
          `${c.name}: parsed, but to a different value than the ` +
          `${manifest.oracle.name} oracle`)
      })
    }
  })

  describe('invalid documents are rejected', () => {
    for (const c of invalid) {
      test(c.name, () => {
        let got: any
        let threw = false
        try {
          got = makeIni().parse(c.source)
        } catch {
          threw = true
        }
        assert.ok(
          threw,
          `${c.name}: accepted a document its upstream labels malformed.\n` +
          `  upstream label: ${c.label}\n` +
          `  parsed as:      ${JSON.stringify(got)}\n` +
          `  note: the ${manifest.oracle.name} oracle also accepts this ` +
          `document (INI dialects disagree here) -- see ` +
          `scripts/build-ini-corpus.js.`)
      })
    }
  })

})
