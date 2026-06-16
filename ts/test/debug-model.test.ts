/* Copyright (c) 2021-2026 Richard Rodger and other contributors, MIT License */

// Composition test: the INI grammar plugin layered with the official
// @tabnas/debug plugin. @tabnas/debug is a devDependency, but this resolves
// it dynamically and SKIPS when it is absent so the suite stays runnable
// outside the package; set TABNAS_DEBUG_PATH to a sibling checkout's built
// plugin to force it to run.

import { test, describe } from 'node:test'
import assert from 'node:assert'
import { createRequire } from 'node:module'

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '../dist/ini'

const req = createRequire(__filename)

function loadDebug(): any {
  const candidates = [process.env.TABNAS_DEBUG_PATH, '@tabnas/debug'].filter(
    Boolean,
  ) as string[]
  for (const c of candidates) {
    try {
      return req(c).Debug
    } catch {
      /* try next */
    }
  }
  return null
}

const Debug = loadDebug()
const skip = Debug ? false : '@tabnas/debug not available (set TABNAS_DEBUG_PATH)'


describe('compose: ini + @tabnas/debug', () => {

  test('parses normally with the debug plugin installed', { skip }, () => {
    const tn = new Tabnas().use(jsonic).use(Ini)
    tn.use(Debug, { print: false, trace: false })
    assert.deepEqual(
      JSON.parse(JSON.stringify(tn.parse('[A.B]\nc=1'))),
      { A: { B: { c: '1' } } },
    )
  })

  test('debug.model() returns the structured ini grammar', { skip }, () => {
    const tn = new Tabnas().use(jsonic).use(Ini)
    tn.use(Debug, { print: false, trace: false })
    const m = tn.debug.model()

    // The structured rule set and entry rule.
    assert.deepStrictEqual(
      m.rules.map((r: any) => r.name).sort(),
      ['dive', 'ini', 'map', 'pair', 'table', 'val'],
    )
    assert.equal(m.config.start, 'ini')
    assert.ok(
      m.plugins.some((p: any) => p.name === 'Ini'),
      'plugins should list Ini',
    )

    // Structural facts specific to the INI grammar:
    // the start rule `ini` opens a sequence of `table` rules.
    const ini = m.rules.find((r: any) => r.name === 'ini')
    assert.ok(
      ini.open.some((a: any) => a.push === 'table'),
      'ini should push table',
    )

    // a `table` (an [a.b] section header) opens both `dive` (the dotted
    // section path) and `map` (the section body of key=val pairs).
    const table = m.rules.find((r: any) => r.name === 'table')
    assert.ok(
      table.open.some((a: any) => a.push === 'dive'),
      'table should push dive',
    )
    assert.ok(
      table.open.some((a: any) => a.push === 'map'),
      'table should push map',
    )

    // a `pair` (key=val) opens a `val`.
    const pair = m.rules.find((r: any) => r.name === 'pair')
    assert.ok(
      pair.open.some((a: any) => a.push === 'val'),
      'pair should push val',
    )

    // The same edges appear in the normalised graph view.
    const tableEdge = m.graph.find((g: any) => g.name === 'table')
    assert.ok(tableEdge.openPush.includes('dive'))
    assert.ok(tableEdge.openPush.includes('map'))

    // The grammar portion is JSON-serialisable and round-trips.
    const grammar = {
      tokens: m.tokens,
      rules: m.rules,
      graph: m.graph,
      config: m.config,
      abnf: m.abnf,
    }
    assert.deepStrictEqual(JSON.parse(JSON.stringify(grammar)).rules, m.rules)
  })
})
