/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

// Cross-runtime conformance, driven by the shared `test/spec/*.tsv` fixtures
// at the repo root (see ../../test/AGENTS.md).
//
// The fixture loader, the escape codec, the `ERROR:<code>` contract and the
// row loop all come from @tabnas/support, whose Go half `go/ini_tsv_test.go`
// uses to run the SAME files — so the two implementations cannot drift
// without one of them going red, and neither can the two loaders.
//
// What is left here is only what is specific to ini: which options each
// fixture is parsed with, and the messages a rejection must carry.

import { Tabnas } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { findSpecDir, loadSpecDir, makeRunner } from '@tabnas/support'

import { Ini, IniOptions } from '../dist/ini'


// ini's fixtures carry no `opts` column: a whole file is parsed with one
// option set, named here. A fixture with no entry gets the defaults, so
// adding one runs it in both runtimes without editing a list.
//
// Keep in sync with tsvOptions in go/ini_tsv_test.go.
const OPTIONS: Record<string, IniOptions> = {
  'inline-comments-active': { comment: { inline: { active: true } } },
  'inline-comments-custom-chars': {
    comment: { inline: { active: true, chars: [';'] } },
  },
  'inline-comments-backslash': {
    comment: { inline: { active: true, escape: { backslash: true } } },
  },
  'inline-comments-backslash-disabled': {
    comment: { inline: { active: true, escape: { backslash: false } } },
  },
  'inline-comments-whitespace': {
    comment: { inline: { active: true, escape: { whitespace: true } } },
  },
  'inline-comments-whitespace-backslash': {
    comment: {
      inline: { active: true, escape: { whitespace: true, backslash: true } },
    },
  },
  'inline-comments-with-sections': { comment: { inline: { active: true } } },
  'value-comment-char-start-inline': { comment: { inline: { active: true } } },
  'sections-duplicate-override': { section: { duplicate: 'override' } },
  'sections-duplicate-error': { section: { duplicate: 'error' } },
  'multiline-backslash': { multiline: true },
  'multiline-indent': { multiline: { indent: true, continuation: false } },
  'multiline-both': { multiline: { continuation: '\\', indent: true } },
  'multiline-with-inline': {
    multiline: true,
    comment: { inline: { active: true } },
  },
  'multiline-escapes': {
    multiline: true,
    comment: { inline: { active: true, escape: { backslash: true } } },
  },
  'multiline-no-inline': { multiline: true },
}


// ini's `ERROR:<code>` cells are SYMBOLIC — they name the rejection this
// repo means, not the code the engine answers, which for most of them is a
// generic `invalid_text`. A code listed here additionally pins the MESSAGE
// the parser must produce; one that is not asserts rejection only, because
// engine-generated wording differs between the two runtimes.
//
// This is why the runner gets a `matchError` hook rather than using its
// default code comparison. Giving these rows real codes would pin more,
// and is worth doing — but it is a change to what the fixtures assert, not
// to who reads them, so it is not this commit's business.
//
// Keep in sync with tsvErrorMessages in go/ini_tsv_test.go.
const ERROR_MESSAGES: Record<string, RegExp> = {
  duplicate_section: /Duplicate section/,
}


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

  makeRunner({
    parse: (input) =>
      new Tabnas().use(jsonic).use(Ini, OPTIONS[name] || {}).parse(input),

    matchError: (err: any, want) => {
      const pattern = ERROR_MESSAGES[want]
      return undefined === pattern
        ? err instanceof Error
        : pattern.test(String(err?.message))
    },
  }).spec(spec)
}
