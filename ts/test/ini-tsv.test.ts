/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

import { test, describe } from 'node:test'
import { deepEqual, ok, strictEqual } from 'node:assert'
import { readFileSync } from 'fs'
import { join } from 'path'

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini, IniOptions } from '../dist/ini'


function unescape(str: string): string {
  return str.replace(/\\r\\n|\\n|\\r|\\t/g, (m) => {
    if (m === '\\r\\n') return '\r\n'
    if (m === '\\n') return '\n'
    if (m === '\\r') return '\r'
    if (m === '\\t') return '\t'
    return m
  })
}


function loadTSV(name: string): { cols: string[]; row: number }[] {
  const specPath = join(__dirname, '..', '..', 'test', 'spec', name + '.tsv')
  const lines = readFileSync(specPath, 'utf8').split(/\r?\n/).filter(Boolean)
  const entries = lines.slice(1).map((line, i) => {
    const cols = line.split('\t').map(unescape)
    return { cols, row: i + 2 }
  })

  // A fixture that loads zero rows used to pass green: the loop below just
  // never ran. An emptied, renamed or header-only .tsv must be a failure.
  ok(entries.length > 0,
    `${name}.tsv: loaded 0 cases. An empty or header-only fixture proves ` +
    `nothing and must not pass.`)

  // A line with no tab used to be dropped by Go and to crash TypeScript on
  // `undefined.startsWith`. Reject it here, in both runtimes, by name.
  for (const e of entries) {
    ok(e.cols.length >= 2,
      `${name}.tsv row ${e.row}: expected 2 tab-separated columns, got ` +
      `${e.cols.length}: ${JSON.stringify(e.cols[0])}`)
  }

  return entries
}


function makeIni(opts?: IniOptions) {
  return new Tabnas().use(jsonic).use(Ini, opts || {})
}


// `ERROR:<code>` used to be a bare rejection marker whose text nobody read:
// TypeScript matched a hardcoded /Duplicate section/ whatever the cell said,
// and Go accepted any error or panic at all. Now the code IS the assertion --
// `ERROR:duplicate_section` requires the reported error to say
// "duplicate section". Keep both runtimes' checks identical.
function errorCodeMatches(code: string, message: string): boolean {
  const want = code.trim().replace(/_/g, ' ').toLowerCase()
  return '' !== want && message.toLowerCase().includes(want)
}


function runTSV(name: string, j: ReturnType<typeof makeIni>) {
  const entries = loadTSV(name)
  let checked = 0
  for (const { cols: [input, expected], row } of entries) {
    checked++
    if (expected.startsWith('ERROR:')) {
      const code = expected.slice('ERROR:'.length)
      let message: string | null = null
      try {
        const got = j.parse(input)
        message = null
        ok(false,
          `${name}.tsv row ${row}: expected error ${JSON.stringify(code)} for ` +
          `input=${JSON.stringify(input)}, but it parsed to ` +
          `${JSON.stringify(got)}`)
      } catch (err: any) {
        if (err && 'ERR_ASSERTION' === err.code) throw err
        message = String((err && err.message) || err)
      }
      ok(errorCodeMatches(code, message as string),
        `${name}.tsv row ${row}: input=${JSON.stringify(input)} was rejected, ` +
        `but not with the declared code ${JSON.stringify(code)}\n` +
        `  error: ${(message as string).split('\n')[0]}`)
    } else {
      try {
        deepEqual(j.parse(input), JSON.parse(expected))
      } catch (err: any) {
        err.message = `${name}.tsv row ${row}: input=${JSON.stringify(input)} expected=${expected}\n${err.message}`
        throw err
      }
    }
  }
  strictEqual(checked, entries.length,
    `${name}.tsv: ran ${checked} of ${entries.length} cases`)
}


describe('ini-tsv', () => {

  test('happy', () => {
    runTSV('happy', makeIni())
  })

  test('basic-values', () => {
    runTSV('basic-values', makeIni())
  })

  test('quoted-values', () => {
    runTSV('quoted-values', makeIni())
  })

  test('bare-key', () => {
    runTSV('bare-key', makeIni())
  })

  test('key-overwrite', () => {
    runTSV('key-overwrite', makeIni())
  })

  test('arrays', () => {
    runTSV('arrays', makeIni())
  })

  test('empty-input', () => {
    runTSV('empty-input', makeIni())
  })

  test('line-comments', () => {
    runTSV('line-comments', makeIni())
  })

  test('inline-comments-off', () => {
    runTSV('inline-comments-off', makeIni())
  })

  test('inline-comments-active', () => {
    runTSV('inline-comments-active', makeIni({
      comment: { inline: { active: true } },
    }))
  })

  test('inline-comments-custom-chars', () => {
    runTSV('inline-comments-custom-chars', makeIni({
      comment: { inline: { active: true, chars: [';'] } },
    }))
  })

  test('inline-comments-backslash', () => {
    runTSV('inline-comments-backslash', makeIni({
      comment: { inline: { active: true, escape: { backslash: true } } },
    }))
  })

  test('inline-comments-backslash-disabled', () => {
    runTSV('inline-comments-backslash-disabled', makeIni({
      comment: { inline: { active: true, escape: { backslash: false } } },
    }))
  })

  test('inline-comments-whitespace', () => {
    runTSV('inline-comments-whitespace', makeIni({
      comment: { inline: { active: true, escape: { whitespace: true } } },
    }))
  })

  test('inline-comments-whitespace-backslash', () => {
    runTSV('inline-comments-whitespace-backslash', makeIni({
      comment: {
        inline: {
          active: true,
          escape: { whitespace: true, backslash: true },
        },
      },
    }))
  })

  test('inline-comments-with-sections', () => {
    runTSV('inline-comments-with-sections', makeIni({
      comment: { inline: { active: true } },
    }))
  })

  test('sections', () => {
    runTSV('sections', makeIni())
  })

  test('sections-escaped-dots', () => {
    runTSV('sections-escaped-dots', makeIni())
  })

  test('sections-duplicate-merge', () => {
    runTSV('sections-duplicate-merge', makeIni())
  })

  test('sections-duplicate-override', () => {
    runTSV('sections-duplicate-override', makeIni({
      section: { duplicate: 'override' },
    }))
  })

  test('sections-duplicate-error', () => {
    runTSV('sections-duplicate-error', makeIni({
      section: { duplicate: 'error' },
    }))
  })

  test('multiline-backslash', () => {
    runTSV('multiline-backslash', makeIni({ multiline: true }))
  })

  test('multiline-indent', () => {
    runTSV('multiline-indent', makeIni({
      multiline: { indent: true, continuation: false },
    }))
  })

  test('multiline-both', () => {
    runTSV('multiline-both', makeIni({
      multiline: { continuation: '\\', indent: true },
    }))
  })

  test('multiline-with-inline', () => {
    runTSV('multiline-with-inline', makeIni({
      multiline: true,
      comment: { inline: { active: true } },
    }))
  })

  test('multiline-escapes', () => {
    runTSV('multiline-escapes', makeIni({
      multiline: true,
      comment: { inline: { active: true, escape: { backslash: true } } },
    }))
  })

  test('multiline-no-inline', () => {
    runTSV('multiline-no-inline', makeIni({ multiline: true }))
  })

  test('numbers-are-strings', () => {
    runTSV('numbers-are-strings', makeIni())
  })

  // KNOWN GAP, LEFT FAILING ON PURPOSE (TypeScript only).
  //
  // A value whose last character is a backslash at end-of-input, with no
  // trailing newline, is rejected by the TypeScript runtime
  // ([jsonic/invalid_text]) and accepted by Go. Found by differential
  // fuzzing of the two runtimes; the minimal case is `x=a\` with no newline.
  //
  // The expected values here are the npm/ini oracle's, not TypeScript's,
  // because Go and the oracle agree and TypeScript is the outlier -- the
  // "Go has exposed a genuine TS defect" carve-out in test/AGENTS.md. Do not
  // flip these to ERROR to get green: that would enshrine the defect.
  // Phase 1 is instrumentation only, so this is recorded, not fixed.
  test('eof-trailing-backslash', () => {
    runTSV('eof-trailing-backslash', makeIni())
  })

})
