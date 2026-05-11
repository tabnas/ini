/* Copyright (c) 2021-2025 Richard Rodger, MIT License */

// Import Jsonic types used by plugin.
import { Jsonic, RuleSpec, NormAltSpec, Lex, makePoint, Token } from 'jsonic'
import { Hoover } from '@jsonic/hoover'

type InlineCommentOptions = {
  // Whether inline comments are active. Default: false.
  active?: boolean
  // Characters that start an inline comment. Default: ['#', ';'].
  chars?: string[]
  // Escape mechanisms for literal comment characters in values.
  escape?: {
    // Allow \; and \# to produce literal ; and #. Default: true.
    backslash?: boolean
    // Require whitespace before comment char to trigger. Default: false.
    whitespace?: boolean
  }
}

type IniOptions = {
  multiline?: {
    // Character before newline indicating continuation. Default: '\\'.
    // Set to false to disable backslash continuation.
    continuation?: string | false
    // When true, a continuation line must be indented (leading whitespace).
    // Indented lines continue the previous value even without a continuation char.
    indent?: boolean
  } | boolean
  section?: {
    // How to handle duplicate section headers. Default: 'merge'.
    // 'merge':    combine keys from all occurrences (last value wins for duplicate keys)
    // 'override': last section occurrence replaces earlier ones entirely
    // 'error':    throw when a previously declared section header appears again
    duplicate?: 'merge' | 'override' | 'error'
  }
  comment?: {
    // Control inline comment behavior. Default: inactive.
    inline?: InlineCommentOptions
  }
}

// --- BEGIN EMBEDDED ini-grammar.jsonic ---
const grammarText = `
# INI Grammar Definition
# Parsed by a standard Jsonic instance and passed to jsonic.grammar()
# Function references (@ prefixed) are resolved against the refs map

{
  options: rule: { start: ini exclude: jsonic }
  options: lex: { emptyResult: {} }
  options: fixed: token: { '#EQ': '=' '#DOT': '.' '#OB': null '#CB': null '#CL': null }
  options: line: { check: '@line-check' }
  options: number: { lex: false }
  options: string: { lex: true chars: QUOTE_CHARS abandon: true }
  options: text: { lex: false }
  options: comment: def: {
    hash: { eatline: true }
    slash: null
    multi: null
    semi: { line: true start: ';' lex: true eatline: true }
  }

  rule: ini: open: [
    { s: '#OS' p: table b: 1 }
    { s: ['#HK #ST #VL' '#EQ'] p: table b: 2 }
    { s: ['#HV' '#OS'] p: table b: 2 }
    { s: '#ZZ' }
  ]

  rule: table: open: [
    { s: '#OS' p: dive }
    { s: ['#HK #ST #VL' '#EQ'] p: map b: 2 }
    { s: ['#HV' '#OS'] p: map b: 2 }
    { s: '#CS' p: map }
    { s: '#ZZ' }
  ]
  rule: table: close: [
    { s: '#OS' r: table b: 1 }
    { s: '#CS' r: table a: '@table-close-dive' }
    { s: '#ZZ' }
  ]

  rule: dive: open: [
    { s: ['#DK' '#DOT'] a: '@dive-push' p: dive }
    { s: '#DK' a: '@dive-push' }
  ]
  rule: dive: close: [
    { s: '#CS' b: 1 }
  ]

  rule: map: open: {
    alts: [
      { s: ['#HK #ST #VL' '#EQ'] c: '@is-table-parent' p: pair b: 2 }
      { s: ['#HK #ST #VL'] c: '@is-table-parent' p: pair b: 1 }
    ]
    inject: { append: true }
  }
  rule: map: close: [
    { s: '#OS' b: 1 }
    { s: '#ZZ' }
  ]

  rule: pair: open: [
    { s: ['#HK #ST #VL' '#EQ'] c: '@is-table-grandparent' p: val a: '@pair-key-eq' }
    { s: '#HK' c: '@is-table-grandparent' a: '@pair-key-bool' }
  ]
  rule: pair: close: [
    { s: ['#HK #ST #VL' '#CL'] c: '@is-table-grandparent' e: '@pair-close-err' }
    { s: ['#HK #ST #VL'] b: 1 r: pair }
    { s: '#OS' b: 1 }
  ]
}
`
// --- END EMBEDDED ini-grammar.jsonic ---

function Ini(jsonic: Jsonic, _options: IniOptions) {
  // Resolve inline comment options.
  const inlineComment = {
    active: _options.comment?.inline?.active ?? false,
    chars: _options.comment?.inline?.chars ?? ['#', ';'],
    escape: {
      backslash: _options.comment?.inline?.escape?.backslash ?? true,
      whitespace: _options.comment?.inline?.escape?.whitespace ?? false,
    },
  }

  // Build Hoover end.fixed arrays based on inline comment config.
  // When active without whitespace mode, include comment chars as terminators.
  // When whitespace mode is on, the custom value matcher handles detection instead.
  const inlineCharsInFixed =
    inlineComment.active && !inlineComment.escape.whitespace

  const eolEndFixed: string[] = ['\n', '\r\n']
  if (inlineCharsInFixed) {
    eolEndFixed.push(...inlineComment.chars)
  }
  eolEndFixed.push('')

  const keyEndFixed: string[] = ['=', '\n', '\r\n']
  if (inlineCharsInFixed) {
    keyEndFixed.push(...inlineComment.chars)
  }
  keyEndFixed.push('')

  // Build escape maps. Always include '\\' -> '\\'.
  // Add comment char escapes when inline comments are active with backslash escaping.
  const eolEscape: Record<string, string> = { '\\': '\\' }
  const keyEscape: Record<string, string> = { '\\': '\\' }
  if (inlineComment.active && inlineComment.escape.backslash) {
    for (const ch of inlineComment.chars) {
      eolEscape[ch] = ch
      keyEscape[ch] = ch
    }
  }

  jsonic.use(Hoover, {
    lex: {
      order: 8.5e6,
    },
    block: [
      {
        name: 'endofline',
        start: {
          rule: {
            parent: {
              include: ['pair', 'elem'],
            },
          },
        },
        end: {
          fixed: eolEndFixed,
          consume: ['\n', '\r\n'],
        },
        escapeChar: '\\',
        escape: eolEscape,
        allowUnknownEscape: true,
        preserveEscapeChar: true,
        trim: true,
      },
      {
        name: 'key',
        token: '#HK',
        start: {
          rule: {
            current: {
              exclude: ['dive'],
            },
            state: 'oc',
          },
        },
        end: {
          fixed: keyEndFixed,
          consume: false,
        },
        escape: keyEscape,
        trim: true,
      },
      {
        name: 'divekey',
        token: '#DK',
        start: {
          rule: {
            current: {
              include: ['dive'],
            },
          },
        },
        end: {
          fixed: [']', '.'],
          consume: false,
        },
        escapeChar: '\\',
        escape: {
          ']': ']',
          '.': '.',
          '\\': '\\',
        },
        allowUnknownEscape: true,
        trim: true,
      },
    ],
  })

  const dupSection = _options.section?.duplicate || 'merge'

  // Track explicitly declared section paths per parse call.
  // Cleared in the ini rule's bo handler, used in the table rule.
  const declaredSections = new Set<string>()

  const ST = jsonic.token.ST as number

  // Named function references for declarative grammar definition.
  const refs: Record<string, Function> = {
    // State actions (used by rule bo/bc/ac handlers).
    '@ini-bo': (r: any) => {
      r.node = {}
      declaredSections.clear()
    },

    '@table-bo': (r: any) => {
      r.node = r.parent.node

      if (r.prev.u.dive) {
        let dive = r.prev.u.dive
        // Use null char as separator to avoid collisions with dots in key names.
        let sectionKey = dive.join('\x00')
        let isDuplicate = declaredSections.has(sectionKey)

        if (isDuplicate && dupSection === 'error') {
          throw new Error(
            'Duplicate section: [' + dive.join('.') + ']'
          )
        }

        for (let dI = 0; dI < dive.length; dI++) {
          if (dI === dive.length - 1 && isDuplicate && dupSection === 'override') {
            // Override: replace the section object entirely.
            r.node = r.node[dive[dI]] = {}
          } else {
            r.node = r.node[dive[dI]] = r.node[dive[dI]] || {}
          }
        }

        declaredSections.add(sectionKey)
      }
    },

    '@table-bc': (r: any) => {
      Object.assign(r.node, r.child.node)
    },

    '@val-ac': (r: any) => {
      if (ST === r.o0.tin && "'" === r.o0.src[0]) {
        try {
          r.node = JSON.parse(r.node)
        } catch (e) {
          // Invalid JSON, just accept val as given
        }
      }

      if (null != r.prev.u.ini_prev) {
        r.prev.node = r.node = r.prev.o0.src + r.node
      } else if (r.parent.u.ini_array) {
        r.parent.u.ini_array.push(r.node)
      }
    },

    // Alt actions.
    '@table-close-dive': (r: any) => (r.u.dive = r.child.u.dive),
    '@dive-push': (r: any) => (r.u.dive = r.parent.u.dive || []).push(r.o0.val),

    '@pair-key-eq': (r: any) => {
      let key = '' + r.o0.val
      if (Array.isArray(r.node[key])) {
        r.u.ini_array = r.node[key]
      } else {
        r.u.key = key
        if (2 < key.length && key.endsWith('[]')) {
          key = r.u.key = key.slice(0, -2)
          r.node[key] = r.u.ini_array = Array.isArray(r.node[key])
            ? r.node[key]
            : undefined === r.node[key]
              ? []
              : [r.node[key]]
        } else {
          r.u.pair = true
        }
      }
    },

    '@pair-key-bool': (r: any) => {
      let key = r.o0.val
      if ('string' === typeof key && 0 < key.length) {
        r.parent.node[key] = true
      }
    },

    '@val-empty': (r: any) => (r.node = ''),

    // Conditions.
    '@is-table-parent': (r: any) => 'table' === r.parent.name,
    '@is-table-grandparent': (r: any) => 'table' === r.parent.parent.name,

    // Error handlers.
    '@pair-close-err': (r: any) => r.c1,

    // Options callbacks.
    '@line-check': (lex: Lex) => {
      if ('val' === lex.ctx.rule.name) {
        return { done: true, token: undefined }
      }
    },
  }

  // Parse embedded grammar definition using a separate standard Jsonic instance.
  const grammarDef = Jsonic.make()(grammarText)
  grammarDef.ref = refs
  grammarDef.options.string.chars = `'"`
  jsonic.grammar(grammarDef)

  // Custom value lex matcher.
  // Needed when: (a) multiline continuation is enabled, or
  // (b) inline comments are active with whitespace-prefix detection.
  // Runs at higher priority than Hoover's endofline block to intercept values.
  const multiline = true === _options.multiline ? {} : _options.multiline
  const needCustomMatcher =
    !!multiline || (inlineComment.active && inlineComment.escape.whitespace)

  if (needCustomMatcher) {
    const continuation: string | false = multiline
      ? (multiline.continuation !== undefined ? multiline.continuation : '\\')
      : false
    const indent = multiline ? (multiline.indent || false) : false
    const HV_TIN = jsonic.token('#HV') as number

    // Build a Set for fast comment char lookup in the matcher.
    const commentCharSet = new Set(inlineComment.chars)

    jsonic.options({
      lex: {
        match: {
          multiline: {
            // Lower order than Hoover (8.5e6) so this runs first.
            order: 8.4e6,
            make: () => {
              return function multilineMatcher(lex: Lex): Token | undefined {
                // Only match in value context during rule open state
                // (same as Hoover endofline block, which defaults to state 'o').
                let ctx = (lex as any).ctx
                let parentName = ctx?.rule?.parent?.name
                if (parentName !== 'pair' && parentName !== 'elem') {
                  return undefined
                }
                if (ctx?.rule?.state !== 'o') {
                  return undefined
                }

                let src = lex.src
                let sI = lex.pnt.sI
                let rI = lex.pnt.rI
                let cI = lex.pnt.cI
                let startI = sI
                let chars: string[] = []

                while (sI < src.length) {
                  let c = src[sI]

                  // Check for inline comment characters (end value).
                  if (inlineComment.active && commentCharSet.has(c)) {
                    if (inlineComment.escape.whitespace) {
                      // Only treat as comment if preceded by whitespace.
                      if (
                        chars.length > 0 &&
                        (chars[chars.length - 1] === ' ' ||
                          chars[chars.length - 1] === '\t')
                      ) {
                        break
                      }
                      // Not preceded by whitespace: treat as literal.
                      chars.push(c)
                      sI++; cI++
                      continue
                    }
                    break
                  }

                  // Check for backslash continuation before newline.
                  if (false !== continuation && c === continuation) {
                    if (src[sI + 1] === '\n') {
                      // \<LF> continuation
                      sI += 2; rI++; cI = 0
                      // Consume leading whitespace on continuation line.
                      while (sI < src.length &&
                        (src[sI] === ' ' || src[sI] === '\t')) {
                        sI++; cI++
                      }
                      continue
                    }
                    if (src[sI + 1] === '\r' && src[sI + 2] === '\n') {
                      // \<CR><LF> continuation
                      sI += 3; rI++; cI = 0
                      while (sI < src.length &&
                        (src[sI] === ' ' || src[sI] === '\t')) {
                        sI++; cI++
                      }
                      continue
                    }
                  }

                  // Check for newline.
                  if (c === '\n' || (c === '\r' && src[sI + 1] === '\n')) {
                    // Indent continuation: next line starts with whitespace.
                    if (indent) {
                      let nextI = c === '\r' ? sI + 2 : sI + 1
                      if (nextI < src.length &&
                        (src[nextI] === ' ' || src[nextI] === '\t')) {
                        rI++; cI = 0
                        sI = nextI
                        // Consume leading whitespace.
                        while (sI < src.length &&
                          (src[sI] === ' ' || src[sI] === '\t')) {
                          sI++; cI++
                        }
                        chars.push(' ')
                        continue
                      }
                    }

                    // Normal newline: end value and consume the newline.
                    if (c === '\r') { sI += 2 } else { sI++ }
                    rI++; cI = 0
                    break
                  }

                  // Handle escape sequences.
                  if (c === '\\' && sI + 1 < src.length) {
                    let next = src[sI + 1]
                    if (
                      inlineComment.active &&
                      inlineComment.escape.backslash &&
                      commentCharSet.has(next)
                    ) {
                      chars.push(next)
                      sI += 2; cI += 2
                      continue
                    }
                    if (next === '\\') {
                      chars.push('\\')
                      sI += 2; cI += 2
                      continue
                    }
                  }

                  chars.push(c)
                  sI++; cI++
                }

                let val: string | undefined = chars.join('').trim()

                let pnt = makePoint(lex.pnt.len, sI, rI, cI)
                let tkn = lex.token(
                  HV_TIN, val, src.substring(startI, sI), pnt)
                tkn.use = { block: 'endofline' }

                lex.pnt.sI = sI
                lex.pnt.rI = rI
                lex.pnt.cI = cI

                return tkn
              }
            }
          }
        }
      }
    })
  }

  // Val rule needs custom injection modifier not supported by grammar spec.
  // Note: state actions (@ini-bo, @table-bo, @table-bc, @val-ac) are
  // auto-applied by fnref() via the @rulename-{bo,ao,bc,ac} convention.
  jsonic.rule('val', (rs: RuleSpec) => {
    rs.fnref(refs)
      .open(
        [
          // Since OS,CS are fixed tokens, concat them with string value
          // if they appear as first char in a RHS value.
          { s: ['#OS #CS'], r: 'val', u: { ini_prev: true } },
          { s: '#ZZ', a: '@val-empty' },
        ],
        {
          custom: (alts: NormAltSpec[]) =>
            alts.filter((alt: NormAltSpec) => alt.g.join() !== 'json,list'),
        },
      )
  })
}

export { Ini }

export type { IniOptions, InlineCommentOptions }
