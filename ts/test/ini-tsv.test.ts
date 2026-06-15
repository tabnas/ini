/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

import { test, describe } from 'node:test'
import { deepEqual, throws } from 'node:assert'
import { readFileSync } from 'fs'
import { join } from 'path'

import { Jsonic } from '@tabnas/jsonic'
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
  return lines.slice(1).map((line, i) => {
    const cols = line.split('\t').map(unescape)
    return { cols, row: i + 2 }
  })
}


function makeIni(opts?: IniOptions) {
  return Jsonic.make().use(Ini, opts || {})
}


function runTSV(name: string, j: ReturnType<typeof Jsonic.make>) {
  const entries = loadTSV(name)
  for (const { cols: [input, expected], row } of entries) {
    if (expected.startsWith('ERROR:')) {
      throws(() => j(input), /Duplicate section/,
        `${name}.tsv row ${row}: expected error for input=${JSON.stringify(input)}`)
    } else {
      try {
        deepEqual(j(input), JSON.parse(expected))
      } catch (err: any) {
        err.message = `${name}.tsv row ${row}: input=${JSON.stringify(input)} expected=${expected}\n${err.message}`
        throw err
      }
    }
  }
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

})
