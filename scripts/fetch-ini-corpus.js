#!/usr/bin/env node
/* Fetch the third-party INI conformance corpus at pinned commit SHAs.
 *
 * THERE IS NO AUTHORITATIVE INI CONFORMANCE SUITE. INI is a de facto format
 * with no specification and no cross-implementation test suite analogous to
 * toml-test, JSONTestSuite or yaml-test-suite. So this corpus is assembled
 * from the test data of four widely-used third-party INI implementations,
 * each pinned to an exact commit SHA. See scripts/build-ini-corpus.js for
 * the provenance of every document, label and expected value.
 *
 * THE UPSTREAM CHECKOUTS ARE NEVER COMMITTED: test/corpus/upstream/ is
 * gitignored. The assembled manifest test/corpus/ini-corpus.json IS
 * committed, on purpose — it is self-contained (sources, per-document
 * SHA-256, oracle values, pinned upstream SHAs), so both test suites run
 * with no network and cannot silently skip. This script is therefore a
 * REGENERATION and AUDIT tool, not a test prerequisite: nothing in the
 * test path runs it.
 *
 * Running it rewrites the committed manifest. That is a deliberate act and
 * shows up as a reviewable diff — if it changes anything without an
 * upstream SHA bump, something is wrong. Running it on a clean checkout at
 * the current pins reproduces the committed manifest byte for byte.
 *
 * Idempotent: a checkout already at the pinned SHA is left alone; the
 * manifest is always rebuilt.
 *
 * Written in Node rather than shell so it runs on any platform;
 * scripts/fetch-ini-corpus.sh is a thin wrapper for the documented shell
 * entry point.
 *
 * Usage:  node scripts/fetch-ini-corpus.js [--force]
 */
'use strict'

const fs = require('fs')
const path = require('path')
const { spawnSync } = require('child_process')

const { UPSTREAMS, build } = require('./build-ini-corpus.js')

const REPO = path.join(__dirname, '..')
const CORPUS = path.join(REPO, 'test', 'corpus')
const UPSTREAM = path.join(CORPUS, 'upstream')

const FORCE = process.argv.includes('--force')


function git(cwd, args, opts) {
  const r = spawnSync('git', args, {
    cwd,
    encoding: 'utf8',
    stdio: (opts && opts.quiet) ? 'pipe' : ['ignore', 'pipe', 'pipe'],
  })
  return { code: r.status, out: (r.stdout || '').trim(), err: (r.stderr || '').trim() }
}


function fetchOne(name, url, sha) {
  const dest = path.join(UPSTREAM, name)

  if (FORCE) fs.rmSync(dest, { recursive: true, force: true })

  if (fs.existsSync(path.join(dest, '.git'))) {
    const head = git(dest, ['rev-parse', 'HEAD'], { quiet: true })
    if (0 === head.code && head.out === sha) {
      console.log(`ok    ${name} @ ${sha} (already present)`)
      return
    }
    console.log(`stale ${name} (have ${head.out || 'unknown'}, want ${sha}) -- refetching`)
    fs.rmSync(dest, { recursive: true, force: true })
  }

  console.log(`fetch ${name} @ ${sha}`)
  fs.mkdirSync(dest, { recursive: true })

  const steps = [
    ['init', '-q'],
    ['remote', 'add', 'origin', url],
  ]
  for (const s of steps) {
    const r = git(dest, s)
    if (0 !== r.code) die(`git ${s.join(' ')} failed for ${name}: ${r.err}`)
  }

  // Fetch just the pinned commit where the server allows it; fall back to a
  // full fetch for servers without uploadpack.allowReachableSHA1InWant.
  let r = git(dest, ['fetch', '-q', '--depth', '1', 'origin', sha], { quiet: true })
  if (0 !== r.code) {
    r = git(dest, ['fetch', '-q', 'origin'])
    if (0 !== r.code) die(`git fetch failed for ${name} (${url}): ${r.err}`)
  }
  r = git(dest, ['checkout', '-q', sha])
  if (0 !== r.code) die(`git checkout ${sha} failed for ${name}: ${r.err}`)

  console.log(`ok    ${name} @ ${sha}`)
}


function die(msg) {
  console.error('ini corpus: ' + msg)
  process.exit(1)
}


function main() {
  fs.mkdirSync(UPSTREAM, { recursive: true })
  for (const name of Object.keys(UPSTREAMS)) {
    fetchOne(name, UPSTREAMS[name].url + '.git', UPSTREAMS[name].sha)
  }
  console.log('\nbuilding manifest with the npm/ini oracle...')
  build()
}

main()
