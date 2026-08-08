/* Copyright (c) 2026 Richard Rodger, MIT License */

// The exported VERSION must equal package.json "version".
//
// This is the CI check for version drift. It exists because the constant HAS
// drifted: @tabnas/json exported Version = '1.0.0' for several releases while
// the package shipped 0.4.x, because nothing rewrote it and AGENTS.md wrongly
// claimed `make publish-go` kept it in sync. A release that bumps
// package.json and forgets the constant now fails here.

import { test, describe } from 'node:test'
import assert from 'node:assert'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { createRequire } from 'node:module'


// This file runs from dist-test/, so the package root is one level up.
const packageRoot = join(__dirname, '..')
const packageJsonPath = join(packageRoot, 'package.json')

const requireFrom = createRequire(__filename)

// Deliberately fatal, never skipped: a version check that silently does not
// run is the failure mode this test exists to prevent. If package.json cannot
// be read or parsed, this throws at load and the whole file fails.
function readPackageJson(): { name: string; version: string } {
  let raw: string
  try {
    raw = readFileSync(packageJsonPath, 'utf8')
  } catch (err: any) {
    throw new Error(
      `cannot read ${packageJsonPath}, so VERSION cannot be checked: ${err.message}`,
    )
  }
  try {
    return JSON.parse(raw)
  } catch (err: any) {
    throw new Error(`${packageJsonPath} is not readable JSON: ${err.message}`)
  }
}

const pkg = readPackageJson()

// Required from the package root, not the source file, so this also asserts
// that VERSION is reachable as public API for consumers.
const api = requireFrom('..')


describe('version', () => {

  test('VERSION matches package.json', () => {
    assert.notEqual(pkg.version, undefined, 'package.json has no version field')
    assert.equal(
      api.VERSION,
      pkg.version,
      `VERSION drift: ${pkg.name} exports ${api.VERSION} but package.json is ` +
      `${pkg.version}. Both are rewritten by admin/publish.sh at release; ` +
      `if you bumped one by hand, bump the other.`,
    )
  })

  test('VERSION is exported and looks like a semver', () => {
    assert.equal(typeof api.VERSION, 'string', 'VERSION must be exported as a string')
    assert.match(api.VERSION, /^\d+\.\d+\.\d+/, 'VERSION must be a semver')
  })

})
