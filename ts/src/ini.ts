/* Copyright (c) 2021-2025 Richard Rodger, MIT License */

// The engine is the tabnas parser; jsonic supplies the relaxed-JSON
// grammar that the embedded grammar text is authored in. Engine types
// (RuleSpec, AltSpec, Lex, makePoint, Token, Tin) are re-exported by
// @tabnas/parser.
import { Tabnas, RuleSpec, AltSpec, Lex, makePoint, Token, Tin } from '@tabnas/parser'
import { jsonic } from '@tabnas/jsonic'
import { Hoover } from '@tabnas/hoover'

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
  # Declared error codes. The CODE is the cross-runtime contract; the message
  # text is not (see AGENTS.md). Both runtimes read this one block, so the two
  # catalogues cannot drift. Keys stay alphabetical: admin's descriptor
  # generator compares the TS and Go extractions and sorts only one side.
  options: error: {
    duplicate_section: 'duplicate section header: [{section}]'
    unterminated_section: 'unterminated section header: [{src}'
  }
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
    { s: ['#HK #ST #VL'] p: table b: 1 }
    { s: '#ZZ' }
  ]

  rule: table: open: [
    # Raised when the table's own before-open handler saw a section path it
    # had already declared and section.duplicate is 'error'. It flags the
    # rule rather than raising there, because a state action cannot raise a
    # coded error in both runtimes: TS can return a bad token from bo, Go
    # discards the return value, and a ctx error set in bo is overwritten by
    # alternate matching. An error ALTERNATE is the path both runtimes share.
    { c: '@is-duplicate-section' e: '@duplicate-section' }
    { s: '#OS' p: dive }
    { s: ['#HK #ST #VL' '#EQ'] p: map b: 2 }
    { s: ['#HV' '#OS'] p: map b: 2 }
    { s: ['#HK #ST #VL'] p: map b: 1 }
    { s: '#CS' p: map }
    { s: '#ZZ' }
  ]
  rule: table: close: [
    { s: '#OS' r: table b: 1 g: end }
    { s: '#CS' r: table a: '@table-close-dive' g: close }
    { s: '#ZZ' g: end }
  ]

  rule: dive: open: [
    { s: ['#DK' '#DOT'] a: '@dive-push' p: dive }
    { s: '#DK' a: '@dive-push' }
  ]
  rule: dive: close: [
    { s: '#CS' b: 1 g: close }
    # A section header lives on one line, so anything other than the closing
    # bracket here means the header was never closed. Unconditional (no s
    # key), so it matches only after the '#CS' alternate above has been
    # tried, and raises unterminated_section rather than the engine's
    # generic 'unexpected'.
    { e: '@dive-unterminated' }
  ]

  rule: map: open: {
    alts: [
      { s: ['#HK #ST #VL' '#EQ'] c: '@is-table-parent' p: pair b: 2 }
      { s: ['#HK #ST #VL'] c: '@is-table-parent' p: pair b: 1 }
    ]
    inject: { append: true }
  }
  rule: map: close: [
    { s: '#OS' b: 1 g: end }
    { s: '#ZZ' g: end }
  ]

  rule: pair: open: [
    { s: ['#HK #ST #VL' '#EQ'] c: '@is-table-grandparent' p: val a: '@pair-key-eq' }
    { s: ['#HK #ST #VL'] c: '@is-table-grandparent' a: '@pair-key-bool' }
  ]
  rule: pair: close: [
    { s: ['#HK #ST #VL' '#CL'] c: '@is-table-grandparent' e: '@pair-close-err' }
    { s: ['#HK #ST #VL'] b: 1 r: pair g: comma }
    { s: '#OS' b: 1 g: end }
  ]
}
`
// --- END EMBEDDED ini-grammar.jsonic ---

// Picks the token an error alternate should mark. An alternate with no
// token sequence matches zero tokens, so the rule's open slot holds the
// engine's NOTOKEN sentinel rather than anything from the source — marking
// that would both lose the position and scribble on a shared object. The
// lookahead token is then the one that actually stopped the parse.
// Returns undefined when neither is available, which tells the caller to
// raise nothing.
function altErrToken(r: any, ctx: any): any {
  if (0 < r.oN && null != r.o0) {
    return r.o0
  }
  return ctx?.t0 ?? undefined
}

// INI allocates its own root and section nodes, so it must follow the core's
// convention: nodes carry no prototype ("no prototype, like JSON" -
// @tabnas/parser builtins).
//
// With a plain `{}` literal a section named __proto__ is not an ordinary key.
// `node['__proto__']` reads back Object.prototype, which is truthy, so the
// `|| {}` reuse guard below hands that back as the section node and every
// key in that section is written onto Object.prototype - polluting every
// object in the process from a single parsed file. Allocating without a
// prototype makes __proto__ an ordinary own key, as jsonic and json5 do.
const node = () => Object.create(null)


function Ini(tn: Tabnas, _options: IniOptions) {
  // Resolve inline comment options. Needed before the config modifiers
  // below, which close over them.
  const inlineComment = {
    active: _options.comment?.inline?.active ?? false,
    chars: _options.comment?.inline?.chars ?? ['#', ';'],
    escape: {
      backslash: _options.comment?.inline?.escape?.backslash ?? true,
      whitespace: _options.comment?.inline?.escape?.whitespace ?? false,
    },
  }

  // Is `rule` the value rule of a `key = value` pair, in its open state?
  // The two lex checks below only apply inside a value.
  const inValue = (lex: Lex) => {
    const rule = (lex as any).ctx?.rule
    return (
      null != rule &&
      'val' === rule.name &&
      'o' === rule.state &&
      ('pair' === rule.parent?.name || 'elem' === rule.parent?.name)
    )
  }

  tn.options({
    config: {
      modify: {
        // Human descriptions for INI tokens, surfaced in railroad diagram
        // legends (read off the live config by @tabnas/railroad).
        'ini-tokendesc': (cfg: any) => {
          cfg.tokenDesc = Object.assign(cfg.tokenDesc || {}, {
            '#DK': 'section-header path segment, as in [a.b.c]',
            '#HK': 'property key (left of key = value)',
            '#HV': 'property value (right of key = value)',
          })
        },

        // A comment marker is only a comment when it starts a LINE. Inside
        // a value (`k = ;x`) the comment matcher (order 6e6) would otherwise
        // beat Hoover's endofline block (8.5e6) and eat the rest of the
        // line, after which the value rule silently swallowed the NEXT
        // line's pair. Declining here lets the value lexers see the marker:
        // with inline comments off it becomes a literal, and with them on
        // Hoover terminates the value at the marker and the comment is
        // lexed normally once the value rule has closed.
        //
        // The parser has no options.comment.check pass-through (unlike
        // options.line.check), so the hook is installed directly on the
        // built config, which configure() does before buildLexDispatch().
        'ini-comment-check': (cfg: any) => {
          cfg.comment.check = (lex: Lex) =>
            inValue(lex) ? { done: true, token: undefined } : undefined
        },

        // A value keyword is only a keyword when it is the WHOLE value:
        // `k = true` is the boolean, `k = true, false` is the string
        // `true, false`. The text matcher (order 8e6) runs before Hoover
        // (8.5e6) and emits a #VL for a keyword that merely *starts* the
        // value, so `k = true, false` silently became `true` and
        // `k = null x` even grew a spurious `x` key. Declining in value
        // position hands the whole line to Hoover, which does the same
        // keyword lookup (cfg.value.def) on the complete, trimmed value.
        'ini-text-check': (cfg: any) => {
          cfg.text.check = (lex: Lex) =>
            inValue(lex) ? { done: true, token: undefined } : undefined
        },

        // A quoted value only counts as quoted when the quotes wrap the
        // WHOLE value: `k = "a b"` is the string `a b`, but `k = "a"b` is
        // the literal text `"a"b`. Without this the string matcher (order
        // 5e6) consumed just `"a"`, Hoover then lexed the trailing `b` as
        // a fresh key, and the document silently gained a property that
        // was never written. An unterminated quote is left to the string
        // matcher, which abandons it and lets Hoover take the raw line.
        'ini-string-check': (cfg: any) => {
          cfg.string.check = (lex: Lex) => {
            if (!inValue(lex)) {
              return undefined
            }

            const src = lex.src
            const sI = lex.pnt.sI
            const quote = src[sI]
            if (!cfg.string.quoteMap[quote]) {
              return undefined
            }

            // Find the closing quote on this line.
            const esc = cfg.string.escChar ?? '\\'
            let eI = sI + 1
            for (; eI < src.length; eI++) {
              if (esc === src[eI]) {
                eI++
                continue
              }
              if (quote === src[eI] || '\n' === src[eI]) {
                break
              }
            }
            if (eI >= src.length || quote !== src[eI]) {
              return undefined
            }

            // Only whitespace (and, when enabled, an inline comment) may
            // follow the closing quote.
            let tI = eI + 1
            while (' ' === src[tI] || '\t' === src[tI]) {
              tI++
            }
            const atLineEnd =
              tI >= src.length ||
              '\n' === src[tI] ||
              ('\r' === src[tI] && '\n' === src[tI + 1])
            const atInlineComment =
              inlineComment.active &&
              inlineComment.chars.includes(src[tI]) &&
              (!inlineComment.escape.whitespace || tI > eI + 1)

            return atLineEnd || atInlineComment
              ? undefined
              : { done: true, token: undefined }
          }
        },
      },
    },
  })

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

  tn.use(Hoover as any, {
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
          // A section header lives on one line: newline terminates the
          // path segment so an unterminated header (`[a` with no `]`)
          // is a parse error instead of a section name that silently
          // swallows following lines up to the next `]`.
          //
          // '' is hoover's end-of-input delimiter. Without it a header that
          // runs to EOF leaves the block committed but unterminated, which
          // hoover reports as a generic `invalid_text` bad token — raised by
          // the lexer before any alternate is consulted, so the grammar
          // never gets to say what actually went wrong. Ending at EOF emits
          // the segment token instead and lets the dive rule's close state
          // raise `unterminated_section`, which is the real diagnosis.
          fixed: [']', '.', '\n', '\r\n', ''],
          consume: false,
        },
        escapeChar: '\\',
        escape: {
          ']': ']',
          '.': '.',
          '\\': '\\',
        },
        allowUnknownEscape: true,
        // Same rule as the value block: an escape that is not one of the
        // three above keeps both characters, so `[C:\path]` stays
        // `C:\path` rather than losing the backslash.
        preserveEscapeChar: true,
        trim: true,
      },
    ],
  })

  const dupSection = _options.section?.duplicate || 'merge'

  // Track explicitly declared section paths per parse call.
  // Cleared in the ini rule's bo handler, used in the table rule.
  const declaredSections = new Set<string>()

  const ST = tn.token.ST as number

  // Named function references for declarative grammar definition.
  const refs: Record<string, Function> = {
    // State actions (used by rule bo/bc/ac handlers).
    '@ini-bo': (r: any) => {
      r.node = node()
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
          // Flag the rule and let this rule's error ALTERNATE raise the
          // coded error (see the grammar's table:open). Raising here is not
          // portable: Go discards a state action's return value, and an
          // error published on the context from bo is overwritten when the
          // alternates are matched.
          r.u.dupsec = dive.join('.')
          return
        }

        for (let dI = 0; dI < dive.length; dI++) {
          if (dI === dive.length - 1 && isDuplicate && dupSection === 'override') {
            // Override: replace the section object entirely.
            r.node = r.node[dive[dI]] = node()
          } else {
            r.node = r.node[dive[dI]] = r.node[dive[dI]] || node()
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
        // A value can start with more than one fixed token (`a = ==x`),
        // and each one replaced the val rule with a fresh one. Only the
        // LAST val in that chain runs this ac, so walk the whole chain:
        // every link contributes its token source, and every link's node
        // is updated — including the first, which is the rule the pair
        // rule reads its child node from. Stopping at the first link
        // instead left that node unset, and the pair then took the
        // parent map as its value, producing a circular result.
        for (let p = r.prev; null != p && null != p.u.ini_prev; p = p.prev) {
          r.node = p.o0.src + r.node
          p.node = r.node
        }
      }

      // Not an `else`: an array entry whose value starts with a fixed
      // token (`k[] = [x`) needs both the concatenation above AND the
      // push, or the entry is silently dropped.
      if (r.parent.u.ini_array) {
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

    // Did this table's before-open handler flag a duplicate section?
    '@is-duplicate-section': (r: any) => null != r.u.dupsec,

    // The duplicate itself. The dotted path is not any single token's src,
    // so it rides along as the {section} detail the message template reads.
    '@duplicate-section': (r: any, ctx: any) => {
      const tkn: any = altErrToken(r, ctx)
      if (null == tkn) {
        return undefined
      }
      tkn.err = 'duplicate_section'
      tkn.use = Object.assign(tkn.use || {}, { section: r.u.dupsec })
      return tkn
    },

    // The section header ran out before its closing bracket — at a newline,
    // or at end of input. The dive rule opened on the #DK segment token, so
    // that token carries both the text for the message and the position to
    // point at.
    '@dive-unterminated': (r: any, ctx: any) => {
      const tkn: any = altErrToken(r, ctx)
      if (null == tkn) {
        return undefined
      }
      tkn.err = 'unterminated_section'
      return tkn
    },

    // Options callbacks.
    '@line-check': (lex: Lex) => {
      if ('val' === lex.ctx.rule.name) {
        return { done: true, token: undefined }
      }
    },
  }

  // Parse embedded grammar definition using a separate jsonic-grammar
  // engine, then install the resulting spec on this tabnas instance.
  const grammarDef = new Tabnas().use(jsonic).parse(grammarText)
  grammarDef.ref = refs
  grammarDef.options.string.chars = `'"`
  tn.grammar(grammarDef)

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
    const HV_TIN = tn.token('#HV') as Tin

    // Build a Set for fast comment char lookup in the matcher.
    const commentCharSet = new Set(inlineComment.chars)

    tn.options({
      lex: {
        match: {
          multiline: {
            // Lower order than Hoover (8.5e6) so this runs first.
            order: 8.4e6,
            make: (cfg: any) => {
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

                let val: any = chars.join('').trim()

                // Resolve value keywords (true/false/null) on the WHOLE
                // trimmed value, exactly as Hoover's endofline block does
                // for the values it lexes itself.
                if (cfg.value.lex && undefined !== cfg.value.def[val]) {
                  val = cfg.value.def[val].val
                }

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
  tn.rule('val', (rs: RuleSpec) => {
    rs.fnref(refs)
      .open(
        [
          // Since OS,CS,EQ,DOT are fixed tokens, they are lexed before
          // Hoover gets to run, so a value that *starts* with one of them
          // never reaches the endofline block. Concat the fixed token
          // source with the rest of the value instead.
          { s: ['#OS #CS #EQ #DOT'], r: 'val', u: { ini_prev: true } },
          { s: '#ZZ', a: '@val-empty' },
        ],
        {
          custom: (alts: AltSpec[]) =>
            alts.filter(
              (alt: AltSpec) =>
                (Array.isArray(alt.g) ? alt.g.join() : alt.g) !== 'json,list',
            ),
        },
      )
  })

  // INI has no array syntax: `val` is restricted to scalars and maps above,
  // leaving Jsonic's `list`/`elem` rules unreachable. Remove them from the
  // grammar definition so the parser — and its railroad diagram — only
  // contains the rules INI actually uses.
  for (const name of ['list', 'elem']) {
    tn.rule(name, null)
  }
}

// VERSION is this package's version. It MUST equal package.json "version":
// the release orchestrator rewrites both, and the version test fails the
// build if they drift. Mirrors `const VERSION` in go/ini.go.
const VERSION = '0.5.6'

export { VERSION, Ini }

export type { IniOptions, InlineCommentOptions }
