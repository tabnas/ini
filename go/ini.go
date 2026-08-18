/* Copyright (c) 2021-2025 Richard Rodger, MIT License */

package tabnasini

import (
	"encoding/json"
	"fmt"
	"strings"
	"sync"

	hoover "github.com/tabnas/hoover/go"
	jsonic "github.com/tabnas/jsonic/go"
)

// VERSION is this module's version. It MUST equal ts/package.json
// "version": the release orchestrator rewrites both, and
// TestVersionMatchesPackageJSON fails the build if they drift.
const VERSION = "0.5.5"

// IniOptions configures the INI parser.
type IniOptions struct {
	Multiline *MultilineOptions
	Section   *SectionOptions
	Comment   *CommentOptions
}

// MultilineOptions controls multiline value continuation.
type MultilineOptions struct {
	// Continuation character before newline. Default: "\\".
	// Set to empty string to disable backslash continuation.
	Continuation *string
	// When true, indented continuation lines extend the previous value.
	Indent *bool
}

// SectionOptions controls section header handling.
type SectionOptions struct {
	// How to handle duplicate section headers.
	// "merge" (default): combine keys from all occurrences.
	// "override": last section occurrence replaces earlier ones.
	// "error": throw when a previously declared section header appears again.
	Duplicate string
}

// CommentOptions controls comment behavior.
type CommentOptions struct {
	Inline *InlineCommentOptions
}

// InlineCommentOptions controls inline comment behavior.
type InlineCommentOptions struct {
	// Whether inline comments are active. Default: false.
	Active *bool
	// Characters that start an inline comment. Default: ["#", ";"].
	Chars []string
	// Escape mechanisms for literal comment characters in values.
	Escape *InlineEscapeOptions
}

// InlineEscapeOptions controls escaping of inline comment characters.
type InlineEscapeOptions struct {
	// Allow \; and \# to produce literal ; and #. Default: true.
	Backslash *bool
	// Require whitespace before comment char to trigger. Default: false.
	Whitespace *bool
}

// resolved holds fully resolved options with defaults applied.
type resolved struct {
	multiline     bool
	continuation  string // "" means disabled
	indent        bool
	dupSection    string
	inlineActive  bool
	inlineChars   map[rune]bool
	inlineCharStr []string
	escBackslash  bool
	escWhitespace bool
}

// defaultParser is a lazily-created instance reused by the no-options Parse
// path, so repeated default calls don't rebuild the engine and grammar each
// time. Building the INI grammar (parsing the embedded grammar text, wiring
// Hoover blocks, and applying the rule spec) dominates a parse, so a
// rebuild-per-call Parse() is many times slower than reusing one instance —
// see perf_test.go. Parsing builds a fresh context per call and only reads
// instance state, so the shared instance is safe for concurrent use. Mirrors
// @tabnas/json's Parse. Option-taking calls still build a fresh instance,
// since their configuration cannot be shared.
var (
	defaultOnce   sync.Once
	defaultParser *jsonic.Jsonic
)

// Parse parses an INI string and returns a map.
func Parse(src string, opts ...IniOptions) (map[string]any, error) {
	var j *jsonic.Jsonic
	if len(opts) > 0 {
		j = MakeJsonic(opts[0])
	} else {
		defaultOnce.Do(func() { defaultParser = MakeJsonic() })
		j = defaultParser
	}
	result, err := j.Parse(src)
	if err != nil {
		return nil, err
	}
	if result == nil {
		return map[string]any{}, nil
	}
	if m, ok := result.(map[string]any); ok {
		return m, nil
	}
	return map[string]any{}, nil
}

// MakeJsonic creates a jsonic instance configured for INI parsing.
func MakeJsonic(opts ...IniOptions) *jsonic.Jsonic {
	var o IniOptions
	if len(opts) > 0 {
		o = opts[0]
	}

	r := resolve(&o)

	bTrue := true
	bFalse := false

	jopts := jsonic.Options{
		Rule: &jsonic.RuleOptions{
			Start: "ini",
		},
		Number: &jsonic.NumberOptions{
			Lex: &bFalse,
		},
		Value: &jsonic.ValueOptions{
			Lex: &bTrue,
		},
		Comment: &jsonic.CommentOptions{
			Lex: &bTrue,
			Def: map[string]*jsonic.CommentDef{
				// Explicit Lex: post the comment.def merge alignment, a def for
				// a NEW comment name (not a jsonic default) is inactive unless
				// it sets Lex — so ini's `#` and `;` line comments turn it on.
				"hash": {Line: true, Start: "#", Lex: &bTrue},
				"semi": {Line: true, Start: ";", Lex: &bTrue},
			},
		},
		String: &jsonic.StringOptions{
			Lex:   &bTrue,
			Chars: `'"`,
		},
		Text: &jsonic.TextOptions{
			Lex: &bFalse,
		},
		Lex: &jsonic.LexOptions{
			EmptyResult: map[string]any{},
		},
	}

	j := jsonic.Make(jopts)

	pluginMap := optionsToMap(&o, r)
	if err := iniPlugin(j, pluginMap); err != nil {
		panic("ini plugin: " + err.Error())
	}

	return j
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

// iniPlugin is the jsonic plugin that adds INI parsing support.
func iniPlugin(j *jsonic.Jsonic, pluginOpts map[string]any) error {
	opts := mapToResolved(pluginOpts)

	// Resolve inline comment options for Hoover block config.
	inlineCharsInFixed := opts.inlineActive && !opts.escWhitespace

	// Build Hoover end.fixed arrays based on inline comment config.
	eolEndFixed := []string{"\n", "\r\n"}
	if inlineCharsInFixed {
		eolEndFixed = append(eolEndFixed, opts.inlineCharStr...)
	}
	eolEndFixed = append(eolEndFixed, "")

	keyEndFixed := []string{"=", "\n", "\r\n"}
	if inlineCharsInFixed {
		keyEndFixed = append(keyEndFixed, opts.inlineCharStr...)
	}
	keyEndFixed = append(keyEndFixed, "")

	// Build escape maps.
	eolEscape := map[string]string{"\\": "\\"}
	keyEscape := map[string]string{"\\": "\\"}
	if opts.inlineActive && opts.escBackslash {
		for _, ch := range opts.inlineCharStr {
			eolEscape[ch] = ch
			keyEscape[ch] = ch
		}
	}

	bTrue := true

	cfg := j.Config()

	// Disable JSON structure tokens except [ and ].
	delete(cfg.FixedTokens, "{")
	delete(cfg.FixedTokens, "}")
	delete(cfg.FixedTokens, ":")
	cfg.SortFixedTokens()

	// Register custom fixed tokens.
	j.Token("#EQ", "=")
	j.Token("#DOT", ".")
	cfg.SortFixedTokens()

	// Use Hoover plugin for key, value, and dive key matching.
	// Mirrors the TS: jsonic.use(Hoover, { ... })
	err := j.UseDefaults(hoover.Hoover, hoover.Defaults, map[string]any{
		"lex": map[string]any{
			"order": 8500000,
		},
		"block": []*hoover.Block{
			{
				Name: "endofline",
				Start: hoover.StartSpec{
					Rule: &hoover.HooverRuleSpec{
						Parent: &hoover.HooverRuleFilter{
							Include: []string{"pair", "elem"},
						},
					},
				},
				End: hoover.EndSpec{
					Fixed:   eolEndFixed,
					Consume: []string{"\n", "\r\n"},
				},
				EscapeChar:         "\\",
				Escape:             eolEscape,
				AllowUnknownEscape: &bTrue,
				PreserveEscapeChar: true,
				Trim:               true,
			},
			{
				Name:  "key",
				Token: "#HK",
				Start: hoover.StartSpec{
					Rule: &hoover.HooverRuleSpec{
						Current: &hoover.HooverRuleFilter{
							Exclude: []string{"dive"},
						},
						State: "oc",
					},
				},
				End: hoover.EndSpec{
					Fixed:   keyEndFixed,
					Consume: false,
				},
				Escape: keyEscape,
				Trim:   true,
			},
			{
				Name:  "divekey",
				Token: "#DK",
				Start: hoover.StartSpec{
					Rule: &hoover.HooverRuleSpec{
						Current: &hoover.HooverRuleFilter{
							Include: []string{"dive"},
						},
					},
				},
				End: hoover.EndSpec{
					// A section header lives on one line: newline terminates
					// the path segment so an unterminated header (`[a` with
					// no `]`) is a parse error instead of a section name that
					// silently swallows following lines up to the next `]`.
					//
					// "" is hoover's end-of-input delimiter. Without it a
					// header that runs to EOF leaves the block committed but
					// unterminated, which hoover reports as a generic
					// invalid_text bad token — raised by the lexer before any
					// alternate is consulted, so the grammar never gets to
					// say what actually went wrong. Ending at EOF emits the
					// segment token instead and lets the dive rule's close
					// state raise unterminated_section, the real diagnosis.
					Fixed:   []string{"]", ".", "\n", "\r\n", ""},
					Consume: false,
				},
				EscapeChar:         "\\",
				Escape:             map[string]string{"]": "]", ".": ".", "\\": "\\"},
				AllowUnknownEscape: &bTrue,
				// Same rule as the value block: an escape that is not one of
				// the three above keeps both characters, so `[C:\path]` stays
				// `C:\path` rather than losing the backslash.
				PreserveEscapeChar: true,
				Trim:               true,
			},
		},
	})
	if err != nil {
		return fmt.Errorf("failed to use hoover plugin: %w", err)
	}

	// Token references for val rule.
	ST := j.Token("#ST")
	OS := j.Token("#OS")
	CS := j.Token("#CS")
	EQ := j.Token("#EQ")
	DOT := j.Token("#DOT")
	HV := j.Token("#HV")
	ZZ := j.Token("#ZZ")

	// Custom multiline value matcher.
	// Needed when: (a) multiline continuation is enabled, or
	// (b) inline comments are active with whitespace-prefix detection.
	// Runs at higher priority than Hoover (8.5e6) to intercept values first.
	// Mirrors the TS: jsonic.options({ lex: { match: { multiline: { order: 8.4e6, ... } } } })
	needCustomMatcher := opts.multiline || (opts.inlineActive && opts.escWhitespace)

	if needCustomMatcher {
		makeMultilineMatcher := func(cfg *jsonic.LexConfig, _opts *jsonic.Options) jsonic.LexMatcher {
			return func(lex *jsonic.Lex, rule *jsonic.Rule) *jsonic.Token {
				// Only match in value context (same as Hoover endofline block).
				if rule == nil || rule.Parent == nil ||
					(rule.Parent.Name != "pair" && rule.Parent.Name != "elem") {
					return nil
				}
				if rule.State != "o" {
					return nil
				}

				pnt := lex.Cursor()
				src := lex.Src
				sI := pnt.SI
				rI := pnt.RI
				cI := pnt.CI
				startI := sI
				var chars []byte

				for sI < len(src) {
					c := src[sI]

					// Check for inline comment characters.
					if opts.inlineActive && opts.inlineChars[rune(c)] {
						if opts.escWhitespace {
							// Only treat as comment if preceded by whitespace.
							if len(chars) > 0 && (chars[len(chars)-1] == ' ' || chars[len(chars)-1] == '\t') {
								break
							}
							chars = append(chars, c)
							sI++
							cI++
							continue
						}
						break
					}

					// Check for backslash continuation before newline.
					if opts.continuation != "" && c == opts.continuation[0] {
						if sI+1 < len(src) && src[sI+1] == '\n' {
							sI += 2
							rI++
							cI = 1
							for sI < len(src) && (src[sI] == ' ' || src[sI] == '\t') {
								sI++
								cI++
							}
							continue
						}
						if sI+2 < len(src) && src[sI+1] == '\r' && src[sI+2] == '\n' {
							sI += 3
							rI++
							cI = 1
							for sI < len(src) && (src[sI] == ' ' || src[sI] == '\t') {
								sI++
								cI++
							}
							continue
						}
					}

					// Check for newline.
					if c == '\n' || (c == '\r' && sI+1 < len(src) && src[sI+1] == '\n') {
						// Indent continuation.
						if opts.indent {
							var nextI int
							if c == '\r' {
								nextI = sI + 2
							} else {
								nextI = sI + 1
							}
							if nextI < len(src) && (src[nextI] == ' ' || src[nextI] == '\t') {
								rI++
								cI = 1
								sI = nextI
								for sI < len(src) && (src[sI] == ' ' || src[sI] == '\t') {
									sI++
									cI++
								}
								chars = append(chars, ' ')
								continue
							}
						}
						// Normal newline: end value and consume.
						if c == '\r' {
							sI += 2
						} else {
							sI++
						}
						rI++
						cI = 1
						break
					}

					// Handle escape sequences.
					if c == '\\' && sI+1 < len(src) {
						next := src[sI+1]
						if opts.inlineActive && opts.escBackslash && opts.inlineChars[rune(next)] {
							chars = append(chars, next)
							sI += 2
							cI += 2
							continue
						}
						if next == '\\' {
							chars = append(chars, '\\')
							sI += 2
							cI += 2
							continue
						}
					}

					chars = append(chars, c)
					sI++
					cI++
				}

				valStr := strings.TrimSpace(string(chars))
				val := resolveValue(valStr)

				tkn := lex.Token("#HV", HV, val, src[startI:sI])
				pnt.SI = sI
				pnt.RI = rI
				pnt.CI = cI
				return tkn
			}
		}

		j.SetOptions(jsonic.Options{
			Lex: &jsonic.LexOptions{
				Match: map[string]*jsonic.MatchSpec{
					"multiline": {
						Order: 8400000, // Lower than Hoover (8.5e6), runs first.
						Make:  makeMultilineMatcher,
					},
				},
			},
		})
	}

	// ---- Grammar Rules ----
	// Rules ini, table, dive, map, pair are loaded from ini-grammar.jsonic
	// via j.Grammar(), mirroring the TS approach. State actions use the
	// @rulename-bo/bc/ac naming convention for auto-wiring.
	// The val rule is defined in Go code (needs custom open alts and
	// complex AC handler not expressible in the grammar file).

	var declaredSections map[string]bool

	// Function refs (matching @ names in the grammar file).
	// State actions (@ini-bo, @table-bo, @table-bc) are auto-wired by Grammar().
	refs := map[jsonic.FuncRef]any{
		// State actions.
		"@ini-bo": jsonic.StateAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			r.Node = make(map[string]any)
			declaredSections = make(map[string]bool)
		}),

		"@table-bo": jsonic.StateAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			r.Node = r.Parent.Node

			if r.Prev != nil && r.Prev != jsonic.NoRule {
				if dive, ok := r.Prev.U["dive"].([]string); ok && len(dive) > 0 {
					sectionKey := strings.Join(dive, "\x00")
					isDuplicate := declaredSections[sectionKey]

					if isDuplicate && opts.dupSection == "error" {
						// Flag the rule and let this rule's error ALTERNATE
						// raise the coded error (see the grammar's
						// table:open). Raising here is not portable: Go
						// discards a state action's return value, and an
						// error published on the context from bo is
						// overwritten when the alternates are matched. The
						// old panic became code "internal", which said
						// nothing about what the document did wrong.
						r.EnsureU()["dupsec"] = strings.Join(dive, ".")
						return
					}

					node, _ := nodeMap(r.Node)
					for dI := 0; dI < len(dive); dI++ {
						if dI == len(dive)-1 && isDuplicate && opts.dupSection == "override" {
							newSection := make(map[string]any)
							node[dive[dI]] = newSection
							node = newSection
						} else {
							if existing, ok := node[dive[dI]].(map[string]any); ok {
								node = existing
							} else {
								newSection := make(map[string]any)
								node[dive[dI]] = newSection
								node = newSection
							}
						}
					}
					r.Node = node
					declaredSections[sectionKey] = true
				}
			}
		}),

		"@table-bc": jsonic.StateAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			// The child `map` rule now builds its node as a *jsonic.OrderedMap
			// (jsonic's default object node), so unwrap both sides to their
			// underlying maps before merging the section's pairs up.
			if childMap, ok := nodeMap(r.Child.Node); ok {
				if node, ok := nodeMap(r.Node); ok {
					for k, v := range childMap {
						node[k] = v
					}
				}
			}
		}),

		// Alt actions.
		"@table-close-dive": jsonic.AltAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			if r.Child != nil && r.Child != jsonic.NoRule {
				if dive, ok := r.Child.U["dive"].([]string); ok {
					r.EnsureU()["dive"] = dive
				}
			}
		}),

		"@dive-push": jsonic.AltAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			dive := getDive(r.Parent)
			val, _ := r.O0.Val.(string)
			dive = append(dive, val)
			r.EnsureU()["dive"] = dive
			if r.Parent != nil && r.Parent != jsonic.NoRule {
				r.Parent.EnsureU()["dive"] = dive
			}
		}),

		// Propagate child dive array up when dive rule closes.
		// In TS, push() mutates the shared array in place, but Go's append
		// may create a new backing array, leaving parent references stale.
		"@dive-bc": jsonic.StateAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			if r.Child != nil && r.Child != jsonic.NoRule {
				if dive, ok := r.Child.U["dive"].([]string); ok {
					r.EnsureU()["dive"] = dive
					if r.Parent != nil && r.Parent != jsonic.NoRule {
						r.Parent.EnsureU()["dive"] = dive
					}
				}
			}
		}),

		"@pair-key-eq": jsonic.AltAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			key := tokenString(r.O0)
			// The map rule's node is a *jsonic.OrderedMap; unwrap it (reads
			// and existing-key updates go through the underlying map, new
			// keys via nodeSet to keep order).
			nm, _ := nodeMap(r.Node)
			if nm == nil {
				return
			}

			if _, isArr := nm[key].([]any); isArr {
				r.EnsureU()["key"] = key
				r.EnsureU()["ini_array"] = nm[key]
			} else if len(key) > 2 && strings.HasSuffix(key, "[]") {
				arrayKey := key[:len(key)-2]
				r.EnsureU()["key"] = arrayKey
				if existing, ok := nm[arrayKey].([]any); ok {
					r.EnsureU()["ini_array"] = existing
				} else if _, exists := nm[arrayKey]; exists {
					r.EnsureU()["ini_array"] = []any{nm[arrayKey]}
					nm[arrayKey] = r.EnsureU()["ini_array"]
				} else {
					arr := make([]any, 0)
					nodeSet(r.Node, arrayKey, arr)
					r.EnsureU()["ini_array"] = arr
				}
			} else {
				r.EnsureU()["key"] = key
				r.EnsureU()["pair"] = true
			}
		}),

		"@pair-key-bool": jsonic.AltAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			key := tokenString(r.O0)
			if key != "" {
				nodeSet(r.Parent.Node, key, true)
			}
		}),

		"@pair-close-err": jsonic.AltError(func(r *jsonic.Rule, ctx *jsonic.Context) *jsonic.Token {
			// Not used in Go (CL token is disabled).
			return nil
		}),

		// Did this table's before-open handler flag a duplicate section?
		"@is-duplicate-section": jsonic.AltCond(func(r *jsonic.Rule, ctx *jsonic.Context) bool {
			if r.U == nil {
				return false
			}
			_, flagged := r.U["dupsec"]
			return flagged
		}),

		// The duplicate itself. The dotted path is not any single token's
		// src, so it rides along as the {section} detail the message
		// template reads.
		"@duplicate-section": jsonic.AltError(func(r *jsonic.Rule, ctx *jsonic.Context) *jsonic.Token {
			tkn := altErrToken(r, ctx)
			if tkn == nil {
				return nil
			}
			section, _ := r.U["dupsec"].(string)
			// Bad returns the token to mark: it is copy-on-write against the
			// NoToken sentinel, so the RESULT must be used, not the receiver.
			return tkn.Bad("duplicate_section", map[string]any{"section": section})
		}),

		// The section header ran out before its closing bracket — at a
		// newline, or at end of input. The dive rule opened on the #DK
		// segment token, so that token carries both the text for the
		// message and the position to point at.
		"@dive-unterminated": jsonic.AltError(func(r *jsonic.Rule, ctx *jsonic.Context) *jsonic.Token {
			tkn := altErrToken(r, ctx)
			if tkn == nil {
				return nil
			}
			return tkn.Bad("unterminated_section")
		}),

		"@val-empty": jsonic.AltAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			r.Node = ""
		}),

		// Conditions.
		"@is-table-parent": jsonic.AltCond(func(r *jsonic.Rule, ctx *jsonic.Context) bool {
			return r.Parent != nil && r.Parent.Name == "table"
		}),

		"@is-table-grandparent": jsonic.AltCond(func(r *jsonic.Rule, ctx *jsonic.Context) bool {
			return r.Parent != nil && r.Parent.Parent != nil &&
				r.Parent.Parent.Name == "table"
		}),
	}

	// Parse grammar file and apply rules via j.Grammar() — same as TS approach.
	parser := jsonic.Make()
	parsed, err := parser.Parse(grammarText)
	if err != nil {
		return fmt.Errorf("failed to parse ini grammar: %w", err)
	}
	// The grammar text is parsed by a stock jsonic instance, which now
	// returns objects as insertion-ordered *jsonic.OrderedMap rather than a
	// bare map[string]any. The grammar-conversion helpers below (and the
	// parser's own MapToOptions/ResolveFuncRefs, which only recurse into
	// map[string]any) expect plain maps, and grammar key order is
	// irrelevant here, so flatten the whole tree to plain Go maps first.
	parsedMap, _ := deepPlain(parsed).(map[string]any)

	// Build GrammarSpec with both options and rules from the grammar text.
	grammarDef := &jsonic.GrammarSpec{
		Ref: refs,
	}
	if optionsMap, ok := parsedMap["options"].(map[string]any); ok {
		// Override string.chars placeholder with actual quote chars.
		if strOpts, ok := optionsMap["string"].(map[string]any); ok {
			strOpts["chars"] = `'"`
		}
		// Remove entries handled directly in Go code.
		// - line.check: set via cfg.LineCheck above.
		// - comment.def: grammar text has partial overrides (e.g. hash: {eatline:true})
		//   but Go's SetOptions replaces entire comment config, so keep jopts setup.
		// - fixed.token: not handled by MapToOptions, handled manually above.
		delete(optionsMap, "line")
		delete(optionsMap, "comment")
		delete(optionsMap, "fixed")
		grammarDef.OptionsMap = optionsMap
	}
	if ruleMap, ok := parsedMap["rule"].(map[string]any); ok {
		grammarDef.Rule = convertRuleMap(ruleMap)
	}
	if err := j.Grammar(grammarDef); err != nil {
		return fmt.Errorf("failed to apply ini grammar: %w", err)
	}

	// Line check: skip line matching inside val rule (matches TS @line-check).
	// Set after Grammar() to ensure it's not overwritten by SetOptions.
	cfg.LineCheck = func(lex *jsonic.Lex) *jsonic.LexCheckResult {
		if lex.Ctx != nil && lex.Ctx.Rule != nil && lex.Ctx.Rule.Name == "val" {
			return &jsonic.LexCheckResult{Done: true, Token: nil}
		}
		return nil
	}

	// Comment check: a comment marker is only a comment when it starts a
	// LINE. Inside a value (`k = ;x`) the comment matcher would otherwise
	// beat Hoover's endofline block and eat the rest of the line, after
	// which the value rule silently swallowed the NEXT line's pair.
	// Declining here lets the value lexers see the marker: with inline
	// comments off it becomes a literal, and with them on Hoover terminates
	// the value at the marker and the comment is lexed normally once the
	// value rule has closed. Mirrors the TS 'ini-comment-check' config
	// modifier.
	// Is the lexer inside the value of a `key = value` pair? Both checks
	// below only apply there. Mirrors the TS inValue() helper.
	inValue := func(lex *jsonic.Lex) bool {
		if lex.Ctx == nil || lex.Ctx.Rule == nil {
			return false
		}
		rule := lex.Ctx.Rule
		return rule.Name == "val" && rule.State == "o" && rule.Parent != nil &&
			(rule.Parent.Name == "pair" || rule.Parent.Name == "elem")
	}

	cfg.CommentCheck = func(lex *jsonic.Lex) *jsonic.LexCheckResult {
		if inValue(lex) {
			return &jsonic.LexCheckResult{Done: true, Token: nil}
		}
		return nil
	}

	// Text check: a value keyword is only a keyword when it is the WHOLE
	// value: `k = true` is the boolean, `k = true, false` is the string
	// `true, false`. The text matcher runs before Hoover and emits a #VL
	// for a keyword that merely *starts* the value, so `k = true, false`
	// silently became `true` and `k = null x` even grew a spurious `x`
	// key. Declining in value position hands the whole line to Hoover,
	// which does the same keyword lookup on the complete, trimmed value.
	// Mirrors the TS 'ini-text-check' config modifier.
	cfg.TextCheck = func(lex *jsonic.Lex) *jsonic.LexCheckResult {
		if inValue(lex) {
			return &jsonic.LexCheckResult{Done: true, Token: nil}
		}
		return nil
	}

	// String check: a quoted value only counts as quoted when the quotes
	// wrap the WHOLE value: `k = "a b"` is the string `a b`, but `k = "a"b`
	// is the literal text `"a"b`. Without this the string matcher consumed
	// just `"a"`, Hoover then lexed the trailing `b` as a fresh key, and the
	// document silently gained a property that was never written. An
	// unterminated quote is left to the string matcher, which abandons it
	// and lets Hoover take the raw line. Mirrors the TS 'ini-string-check'
	// config modifier.
	cfg.StringCheck = func(lex *jsonic.Lex) *jsonic.LexCheckResult {
		if !inValue(lex) {
			return nil
		}
		src := lex.Src
		sI := lex.Cursor().SI
		if sI >= len(src) {
			return nil
		}
		quote := rune(src[sI])
		if !cfg.StringChars[quote] {
			return nil
		}

		// Find the closing quote on this line.
		esc := byte(cfg.EscapeChar)
		eI := sI + 1
		for ; eI < len(src); eI++ {
			if src[eI] == esc {
				eI++
				continue
			}
			if src[eI] == byte(quote) || src[eI] == '\n' {
				break
			}
		}
		if eI >= len(src) || src[eI] != byte(quote) {
			return nil
		}

		// Only whitespace (and, when enabled, an inline comment) may
		// follow the closing quote.
		tI := eI + 1
		for tI < len(src) && (src[tI] == ' ' || src[tI] == '\t') {
			tI++
		}
		atLineEnd := tI >= len(src) || src[tI] == '\n' ||
			(src[tI] == '\r' && tI+1 < len(src) && src[tI+1] == '\n')
		atInlineComment := opts.inlineActive &&
			tI < len(src) && opts.inlineChars[rune(src[tI])] &&
			(!opts.escWhitespace || tI > eI+1)

		if atLineEnd || atInlineComment {
			return nil
		}
		// Trailing text after the closing quote: not a quoted value.
		return &jsonic.LexCheckResult{Done: true, Token: nil}
	}

	// ---- val rule ----
	// Mirrors TS: rs.fnref(refs).open([...], { custom: filter })
	// Prepends INI-specific alts, filters out json/list group alts,
	// and preserves hoover's prepended #HV alt.
	j.Rule("val", func(rs *jsonic.RuleSpec, _ *jsonic.Parser) {
		rs.AddBO(func(r *jsonic.Rule, ctx *jsonic.Context) {
			r.Node = jsonic.Undefined
		})

		HK := j.Token("#HK")
		DK := j.Token("#DK")

		// Filter out json,list group alts (matching TS custom filter)
		// and hoover-prepended #HK/#DK alts that don't belong in val.
		filtered := make([]*jsonic.AltSpec, 0, len(rs.OpenAlts()))
		for _, alt := range rs.OpenAlts() {
			if alt.G == "json,list" {
				continue
			}
			// Skip hoover-prepended alts for non-value tokens.
			if len(alt.S) == 1 && len(alt.S[0]) == 1 &&
				(alt.S[0][0] == HK || alt.S[0][0] == DK) {
				continue
			}
			filtered = append(filtered, alt)
		}

		// Prepend INI-specific alts before existing (hoover) alts.
		iniAlts := []*jsonic.AltSpec{
			// Since OS,CS,EQ,DOT are fixed tokens, they are lexed before
			// Hoover gets to run, so a value that *starts* with one of them
			// never reaches the endofline block. Concat the fixed token
			// source with the rest of the value instead. All four are
			// alternatives for the same slot (matching TS ['#OS #CS #EQ #DOT']).
			{S: [][]jsonic.Tin{{OS, CS, EQ, DOT}}, R: "val",
				U: map[string]any{"ini_prev": true}},
			// End of input: empty value.
			{S: [][]jsonic.Tin{{ZZ}},
				A: func(r *jsonic.Rule, ctx *jsonic.Context) {
					r.Node = ""
				}},
		}
		rs.ClearOpen()
		rs.AddOpen(append(iniAlts, filtered...)...)

		rs.AddAC(func(r *jsonic.Rule, ctx *jsonic.Context) {
			// Resolve value.
			if jsonic.IsUndefined(r.Node) || r.Node == nil {
				if r.O0 != nil && !r.O0.IsNoToken() {
					r.Node = resolveTokenVal(r.O0)
				} else {
					r.Node = ""
				}
			}

			// A #HV is Hoover's raw end-of-line span, so the value keywords
			// (true/false/null) are resolved here, on the WHOLE trimmed
			// value. The text matcher used to do it, but it matched a
			// keyword that merely started the value; it is declined in
			// value position now (see cfg.TextCheck). The custom multiline
			// matcher already resolves its own #HV via resolveValue.
			if r.O0 != nil && !r.O0.IsNoToken() && r.O0.Tin == HV {
				if s, ok := r.Node.(string); ok {
					r.Node = resolveValue(s)
				}
			}

			// Handle single-quoted JSON parsing.
			if r.O0 != nil && r.O0.Tin == ST && len(r.O0.Src) > 0 && r.O0.Src[0] == '\'' {
				if s, ok := r.Node.(string); ok {
					r.Node = tryParseJSON(s)
				}
			}

			// Handle ini_prev concatenation. A value can start with more
			// than one fixed token (`a = ==x`), and each one replaced the
			// val rule with a fresh one. Only the LAST val in that chain
			// runs this ac, so walk the whole chain: every link contributes
			// its token source, and every link's node is updated —
			// including the first, which is the node the pair rule reads.
			// Stopping at the first link left that node unset.
			for p := r.Prev; p != nil && p != jsonic.NoRule; p = p.Prev {
				if _, ok := p.U["ini_prev"]; !ok {
					break
				}
				r.Node = p.O0.Src + fmt.Sprintf("%v", r.Node)
				p.Node = r.Node
			}

			// Handle array push. Deliberately NOT an else-of the block
			// above: an array entry whose value starts with a fixed token
			// (`k[] = [x`) needs both the concatenation and the push, or
			// the entry is silently dropped.
			if r.Parent != nil && r.Parent != jsonic.NoRule {
				if arr, ok := r.Parent.EnsureU()["ini_array"].([]any); ok {
					arr = append(arr, r.Node)
					r.Parent.EnsureU()["ini_array"] = arr
					if key, ok := r.Parent.EnsureU()["key"].(string); ok {
						nodeSet(r.Parent.Node, key, arr)
					}
					return
				}
			}

			// Normal pair assignment.
			if r.Parent != nil && r.Parent != jsonic.NoRule {
				if key, ok := r.Parent.EnsureU()["key"].(string); ok {
					if _, isPair := r.Parent.EnsureU()["pair"]; isPair {
						nodeSet(r.Parent.Node, key, r.Node)
					}
				}
			}
		})
	})

	// INI has no array syntax, so `val` is restricted to scalars and maps
	// and Jsonic's inherited `list`/`elem` rules are unreachable. Remove
	// them so the grammar definition matches the TypeScript port.
	for _, name := range []string{"list", "elem"} {
		j.Rule(name, nil)
	}

	return nil
}

// ---- Helper functions ----

func boolOpt(p *bool, def bool) bool {
	if p != nil {
		return *p
	}
	return def
}

func stringOpt(p *string, def string) string {
	if p != nil {
		return *p
	}
	return def
}

func resolve(o *IniOptions) *resolved {
	r := &resolved{
		dupSection:    "merge",
		inlineChars:   map[rune]bool{'#': true, ';': true},
		inlineCharStr: []string{"#", ";"},
		escBackslash:  true,
	}

	if o.Multiline != nil {
		r.multiline = true
		r.continuation = stringOpt(o.Multiline.Continuation, "\\")
		r.indent = boolOpt(o.Multiline.Indent, false)
	}

	if o.Section != nil && o.Section.Duplicate != "" {
		r.dupSection = o.Section.Duplicate
	}

	if o.Comment != nil && o.Comment.Inline != nil {
		ic := o.Comment.Inline
		r.inlineActive = boolOpt(ic.Active, false)
		if ic.Chars != nil && len(ic.Chars) > 0 {
			r.inlineChars = make(map[rune]bool)
			r.inlineCharStr = ic.Chars
			for _, s := range ic.Chars {
				if len(s) > 0 {
					r.inlineChars[rune(s[0])] = true
				}
			}
		}
		if ic.Escape != nil {
			r.escBackslash = boolOpt(ic.Escape.Backslash, true)
			r.escWhitespace = boolOpt(ic.Escape.Whitespace, false)
		}
	}

	return r
}

func optionsToMap(o *IniOptions, r *resolved) map[string]any {
	m := make(map[string]any)
	m["_resolved"] = r
	return m
}

func mapToResolved(m map[string]any) *resolved {
	if m == nil {
		return resolve(&IniOptions{})
	}
	if r, ok := m["_resolved"].(*resolved); ok {
		return r
	}
	return resolve(&IniOptions{})
}

func getDive(r *jsonic.Rule) []string {
	if r == nil || r == jsonic.NoRule {
		return nil
	}
	if dive, ok := r.EnsureU()["dive"].([]string); ok {
		return dive
	}
	return nil
}

func tokenString(t *jsonic.Token) string {
	if t == nil || t.IsNoToken() {
		return ""
	}
	if s, ok := t.Val.(string); ok {
		return s
	}
	return t.Src
}

func resolveTokenVal(t *jsonic.Token) any {
	if !jsonic.IsUndefined(t.Val) {
		return t.Val
	}
	return t.Src
}

// altErrToken picks the token an error alternate should mark. An alternate
// with no token sequence matches zero tokens, so the rule's open slot holds
// the NoToken sentinel rather than anything from the source; the lookahead
// token is then the one that actually stopped the parse, and the only one
// carrying a usable position. Returns nil when neither is available, which
// tells the caller to raise nothing.
func altErrToken(r *jsonic.Rule, ctx *jsonic.Context) *jsonic.Token {
	if r.ON > 0 && r.O0 != nil && !r.O0.IsNoToken() {
		return r.O0
	}
	if ctx.T0 != nil {
		return ctx.T0
	}
	if r.O0 != nil {
		return r.O0
	}
	return nil
}

// nodeMap returns the underlying string-keyed map for a parse node,
// unwrapping a *jsonic.OrderedMap (its Vals) or accepting a plain
// map[string]any. Reads via the returned map, and value updates to keys
// that already exist, are safe on either shape; use nodeSet to add new
// keys so a *OrderedMap keeps its key order. The bool reports whether node
// was one of those object shapes.
func nodeMap(node any) (map[string]any, bool) {
	switch m := node.(type) {
	case *jsonic.OrderedMap:
		if m.Vals == nil {
			m.Vals = map[string]any{}
		}
		return m.Vals, true
	case map[string]any:
		return m, true
	}
	return nil, false
}

// nodeSet assigns key=val on a parse node, whether it is a
// *jsonic.OrderedMap (via Set, so a new key is appended to Keys and order
// is preserved) or a plain map[string]any.
func nodeSet(node any, key string, val any) {
	switch m := node.(type) {
	case *jsonic.OrderedMap:
		m.Set(key, val)
	case map[string]any:
		m[key] = val
	}
}

// deepPlain converts a parsed value tree into plain Go containers,
// unwrapping every *jsonic.OrderedMap into a map[string]any (dropping the
// remembered key order) and recursing through nested objects and slices.
// The grammar-conversion code and the parser's own MapToOptions expect
// bare map[string]any, and grammar key order carries no meaning, so this
// normalization is safe here.
func deepPlain(v any) any {
	if om, ok := v.(*jsonic.OrderedMap); ok {
		out := make(map[string]any, len(om.Keys))
		for _, k := range om.Keys {
			out[k] = deepPlain(om.Vals[k])
		}
		return out
	}
	if m, ok := v.(map[string]any); ok {
		out := make(map[string]any, len(m))
		for k, val := range m {
			out[k] = deepPlain(val)
		}
		return out
	}
	if arr, ok := v.([]any); ok {
		out := make([]any, len(arr))
		for i, val := range arr {
			out[i] = deepPlain(val)
		}
		return out
	}
	return v
}

func tryParseJSON(s string) any {
	var result any
	if err := json.Unmarshal([]byte(s), &result); err == nil {
		return result
	}
	return s
}

func resolveValue(s string) any {
	switch s {
	case "true":
		return true
	case "false":
		return false
	case "null":
		return nil
	}
	return s
}

func boolPtr(b bool) *bool {
	return &b
}

func stringPtr(s string) *string {
	return &s
}

// convertRuleMap converts a parsed rule map into typed GrammarRuleSpec map.
func convertRuleMap(ruleMap map[string]any) map[string]*jsonic.GrammarRuleSpec {
	rules := make(map[string]*jsonic.GrammarRuleSpec, len(ruleMap))
	for name, rDef := range ruleMap {
		rd, ok := rDef.(map[string]any)
		if !ok {
			continue
		}
		grs := &jsonic.GrammarRuleSpec{}
		if openDef, ok := rd["open"]; ok {
			grs.Open = convertAlts(openDef)
		}
		if closeDef, ok := rd["close"]; ok {
			grs.Close = convertAlts(closeDef)
		}
		rules[name] = grs
	}
	return rules
}

func convertAlts(def any) any {
	switch v := def.(type) {
	case []any:
		return convertAltList(v)
	case map[string]any:
		result := &jsonic.GrammarAltListSpec{}
		if alts, ok := v["alts"].([]any); ok {
			result.Alts = convertAltList(alts)
		}
		if inj, ok := v["inject"].(map[string]any); ok {
			result.Inject = &jsonic.GrammarInjectSpec{}
			if app, ok := inj["append"].(bool); ok {
				result.Inject.Append = app
			}
		}
		return result
	}
	return nil
}

func convertAltList(alts []any) []*jsonic.GrammarAltSpec {
	result := make([]*jsonic.GrammarAltSpec, 0, len(alts))
	for _, a := range alts {
		if am, ok := a.(map[string]any); ok {
			result = append(result, convertAlt(am))
		}
	}
	return result
}

func convertAlt(m map[string]any) *jsonic.GrammarAltSpec {
	ga := &jsonic.GrammarAltSpec{}

	if s, ok := m["s"]; ok {
		switch sv := s.(type) {
		case string:
			ga.S = sv
		case []any:
			strs := make([]string, len(sv))
			for i, v := range sv {
				strs[i], _ = v.(string)
			}
			ga.S = strs
		}
	}
	if b, ok := m["b"]; ok {
		ga.B = b
	}
	if p, ok := m["p"].(string); ok {
		ga.P = p
	}
	if r, ok := m["r"].(string); ok {
		ga.R = r
	}
	if a, ok := m["a"].(string); ok {
		ga.A = a
	}
	if c, ok := m["c"]; ok {
		ga.C = c
	}
	if e, ok := m["e"].(string); ok {
		ga.E = e
	}
	if u, ok := m["u"].(map[string]any); ok {
		ga.U = u
	}

	return ga
}
