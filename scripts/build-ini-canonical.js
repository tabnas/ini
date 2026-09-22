#!/usr/bin/env node
/* Build test/corpus/ini-canonical.json: what the CANONICAL TypeScript
 * makes of every corpus document that this dialect reads differently
 * from the npm/ini oracle.
 *
 * Why the file exists
 * -------------------
 * The corpus's expected values are npm/ini's output. Twelve of the
 * thirty valid documents differ from it, each for a documented dialect
 * reason, so the oracle cannot say whether a runtime read those twelve
 * correctly. Asserting only that they DIFFER leaves any third value
 * green: 40% of the valid corpus parsed and then not measured. This
 * file is the other half of that assertion, and all three conformance
 * suites compare against it exactly.
 *
 * It is a GENERATED, COMMITTED file, for the same reason the manifest
 * is: the Go and Rust suites cannot run Node, and the suites must not
 * need a network or a build of a sibling to run.
 *
 * Which documents it covers
 * -------------------------
 * Exactly the keys already in the file, which each suite asserts are
 * exactly its divergent list. Adding a document to the divergent lists
 * therefore fails the suites until it is added here too -- pass its
 * name on the command line to do that:
 *
 *   node scripts/build-ini-canonical.js [name ...]
 *
 * Regenerate it after any change to the canonical dialect, from a built
 * ts/ (`npm run build` there first). A value that moves is a change to
 * what @tabnas/ini means by those documents, and belongs in a commit
 * that says so.
 */

const Fs = require('node:fs')
const Path = require('node:path')

const REPO = Path.join(__dirname, '..')
const CORPUS = Path.join(REPO, 'test', 'corpus', 'ini-corpus.json')
const CANONICAL = Path.join(REPO, 'test', 'corpus', 'ini-canonical.json')
const DIST = Path.join(REPO, 'ts', 'dist', 'ini.js')

function load(file, what) {
  if (!Fs.existsSync(file)) {
    throw new Error(`${what} is not on disk at ${file}`)
  }
  return JSON.parse(Fs.readFileSync(file, 'utf8'))
}

async function main() {
  if (!Fs.existsSync(DIST)) {
    throw new Error(
      `ts/dist/ini.js is not built; run \`npm run build\` in ts/ first`)
  }

  const { Tabnas } = require(Path.join(REPO, 'ts', 'node_modules', '@tabnas', 'parser'))
  const { jsonic } = require(Path.join(REPO, 'ts', 'node_modules', '@tabnas', 'jsonic'))
  const { Ini } = require(DIST)

  const corpus = load(CORPUS, 'the corpus manifest')
  const previous = Fs.existsSync(CANONICAL) ? load(CANONICAL, 'the canonical file') : {}

  const wanted = new Set([...Object.keys(previous), ...process.argv.slice(2)])
  const byName = new Map(corpus.cases.map((c) => [c.name, c]))

  const out = {}
  for (const name of [...wanted].sort()) {
    const test = byName.get(name)
    if (null == test) {
      throw new Error(`${name} is not in the corpus manifest`)
    }
    // A fresh instance per document, as the suites use: the plugin
    // carries no per-parse state a shared instance could leak, but the
    // recorded value must be what a caller gets, not what a warm one
    // gets.
    out[name] = new Tabnas().use(jsonic).use(Ini).parse(test.source)
  }

  Fs.writeFileSync(CANONICAL, JSON.stringify(out, null, 2) + '\n')
  process.stdout.write(
    `wrote ${Object.keys(out).length} canonical results to ${CANONICAL}\n`)
}

main().catch((err) => {
  process.stderr.write(String(err.message) + '\n')
  process.exit(1)
})
