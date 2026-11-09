/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

// Cross-runtime conformance, driven by the shared `test/spec/*.tsv` fixtures
// at the repo root (see ../../test/AGENTS.md).
//
// The fixture loader, the escape codec, the `ERROR:<code>` contract and the
// row loop all come from @tabnas/support, whose Go and Rust halves
// `go/ini_tsv_test.go` and `rs/tests/parity_test.rs` use to run the SAME
// files — so the implementations cannot drift without one of them going
// red, and neither can the loaders.
//
// What is left here is only what is specific to ini: which options each
// fixture is parsed with, and the messages a rejection must carry.

import { describe } from 'node:test'

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { findSpecDir, loadSpecDir, makeRunner } from '@tabnas/support'

import { Ini } from '../dist/ini'

import { OPTIONS } from './fixture-options'


// One runner per fixture file, because the options are per file. The
// directory listing is the source of truth for which fixtures exist —
// `loadSpecDir` rejects an empty directory, and the runner rejects an
// empty fixture, so neither can pass by running nothing.
// `minCols: 2` keeps a guard this repo already had: a line with no tab is
// a failure, named by file and line, not a row silently dropped. (A
// `#`-leading line with no tab is a comment to the shared loader and is
// skipped before that check — there are none in these fixtures.)
for (const spec of loadSpecDir(findSpecDir(__dirname), { minCols: 2 })) {
  const name = spec.file.replace(/\.tsv$/, '')

  // No matchError hook: an `ERROR:<code>` cell is compared against the
  // error's `code` by the shared runner's default, which is the contract
  // this package wants. Both codes the fixtures pin — duplicate_section and
  // unterminated_section — are declared in ini-grammar.jsonic and raised by
  // every runtime, so nothing has to be resolved through message wording
  // (which is deliberately not a cross-runtime contract).
  makeRunner({
    parse: (input) =>
      new Tabnas().use(jsonic).use(Ini, OPTIONS[name] || {}).parse(input),
  }).spec(spec)
}


// The same fixtures again, each through a parser that has had an options()
// call naming nothing. Twin of the "after an unrelated SetOptions" pass in
// go/ini_tsv_test.go: the Go port lost its depth limit and its line,
// comment, text and string checks to exactly such a call.
describe('after an unrelated options() call', () => {
  for (const spec of loadSpecDir(findSpecDir(__dirname), { minCols: 2 })) {
    const name = spec.file.replace(/\.tsv$/, '')
    makeRunner({
      parse: (input) => {
        const k = new Tabnas().use(jsonic).use(Ini, OPTIONS[name] || {})
        k.options({})
        return k.parse(input)
      },
    }).spec(spec)
  }
})
