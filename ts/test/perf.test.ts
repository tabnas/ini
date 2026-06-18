/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

import { test, describe } from 'node:test'
import assert from 'node:assert'

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Ini } from '../dist/ini'

// @tabnas/ini exports only the Ini plugin — there is no package-level
// convenience parse(src); users build their own instance with
// `new Tabnas().use(jsonic).use(Ini)`. Building that instance (parsing the
// embedded grammar text, wiring the Hoover blocks, applying the rule spec)
// dominates a parse, so reusing one instance across many parses is many times
// faster than rebuilding per parse.
//
// This guards the representative usage: it must NOT rebuild the grammar on
// every parse. It compares N parses on ONE reused instance against N
// instance-rebuilds (one build + parse each), on the SAME machine in the SAME
// run, and asserts reuse is dramatically cheaper. The check is machine-
// INDEPENDENT (both sides scale together on a slow box) with deliberately NO
// wall-clock budget.
describe('perf', () => {
  test('reuse-instance-stays-fast', () => {
    const src = 'a = 1\nb = 2\n[s]\nc = 3'
    const N = 500

    const make = () => new Tabnas().use(jsonic).use(Ini)

    // Warm both paths so the comparison is steady-state.
    const j = make()
    for (let i = 0; i < 50; i++) {
      j.parse(src)
    }
    for (let i = 0; i < 50; i++) {
      make().parse(src)
    }

    // Reuse one instance for N parses.
    const t0 = performance.now()
    for (let i = 0; i < N; i++) {
      j.parse(src)
    }
    const reuse = performance.now() - t0

    // Rebuild a fresh instance for each of N parses (the regression shape).
    const t1 = performance.now()
    for (let i = 0; i < N; i++) {
      make().parse(src)
    }
    const rebuild = performance.now() - t1

    const ratio = rebuild / reuse

    // Rebuilding per parse is many times slower than reuse. Require a clear
    // margin so this catches a regression where a convenience wrapper (or
    // caller) rebuilds per parse, without depending on absolute speed.
    assert.ok(
      ratio > 4,
      `Reusing one instance for ${N} parses (${reuse.toFixed(1)}ms) should be ` +
        `much faster than rebuilding per parse (${rebuild.toFixed(1)}ms, ratio ` +
        `${ratio.toFixed(1)}x); building the INI grammar dominates a parse, so ` +
        `reuse must stay cheap.`,
    )

    // Verify the reused instance still parses correctly across the run.
    assert.deepEqual(j.parse(src), { a: '1', b: '2', s: { c: '3' } })
  })
})
