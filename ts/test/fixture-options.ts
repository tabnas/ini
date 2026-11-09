/* Copyright (c) 2021-2025 Richard Rodger and other contributors, MIT License */

// The option set each shared fixture file is parsed with, as
// `ini-tsv.test.ts` and `window.test.ts` read the fixtures.

import type { IniOptions } from '../dist/ini'

// ini's fixtures carry no `opts` column: a whole file is parsed with one
// option set, named here. A fixture with no entry gets the defaults, so
// adding one runs it in every runtime without editing a list.
//
// Keep in sync with tsvOptions in go/ini_tsv_test.go.
export const OPTIONS: Record<string, IniOptions> = {
  'inline-comments-active': { comment: { inline: { active: true } } },
  'inline-comments-custom-chars': {
    comment: { inline: { active: true, chars: [';'] } },
  },
  // An EMPTY list is a choice, not an absence: inline comments stay
  // active with no character that starts one.
  'inline-comments-empty-chars': {
    comment: { inline: { active: true, chars: [] } },
  },
  // A marker is compared as ONE code unit where the value is scanned, so
  // '##' can never start a comment and 'é' can, while 'à' — which shares
  // its first UTF-8 byte — cannot.
  'inline-comments-marker-width': {
    comment: {
      inline: {
        active: true,
        chars: ['##', 'é'],
        escape: { whitespace: true },
      },
    },
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
  // A continuation of any length but one code unit continues nothing.
  'multiline-continuation-width': { multiline: { continuation: '~~' } },
}
