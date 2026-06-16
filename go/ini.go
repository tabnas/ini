/* Copyright (c) 2021-2025 Richard Rodger, MIT License */

package ini

import (
	"encoding/json"
	"fmt"
	"strings"

	hoover "github.com/tabnas/hoover/go"
	jsonic "github.com/tabnas/jsonic/go"
)

const Version = "0.1.6"

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

// Parse parses an INI string and returns a map.
func Parse(src string, opts ...IniOptions) (map[string]any, error) {
	var o IniOptions
	if len(opts) > 0 {
		o = opts[0]
	}
	j := MakeJsonic(o)
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
				"hash": {Line: true, Start: "#"},
				"semi": {Line: true, Start: ";"},
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
					Fixed:   []string{"]", "."},
					Consume: false,
				},
				EscapeChar:         "\\",
				Escape:             map[string]string{"]": "]", ".": ".", "\\": "\\"},
				AllowUnknownEscape: &bTrue,
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
						panic(fmt.Sprintf("Duplicate section: [%s]", strings.Join(dive, ".")))
					}

					node, _ := r.Node.(map[string]any)
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
			if childMap, ok := r.Child.Node.(map[string]any); ok {
				if nodeMap, ok := r.Node.(map[string]any); ok {
					for k, v := range childMap {
						nodeMap[k] = v
					}
				}
			}
		}),

		// Alt actions.
		"@table-close-dive": jsonic.AltAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			if r.Child != nil && r.Child != jsonic.NoRule {
				if dive, ok := r.Child.U["dive"].([]string); ok {
					r.U["dive"] = dive
				}
			}
		}),

		"@dive-push": jsonic.AltAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			dive := getDive(r.Parent)
			val, _ := r.O0.Val.(string)
			dive = append(dive, val)
			r.U["dive"] = dive
			if r.Parent != nil && r.Parent != jsonic.NoRule {
				r.Parent.U["dive"] = dive
			}
		}),

		// Propagate child dive array up when dive rule closes.
		// In TS, push() mutates the shared array in place, but Go's append
		// may create a new backing array, leaving parent references stale.
		"@dive-bc": jsonic.StateAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			if r.Child != nil && r.Child != jsonic.NoRule {
				if dive, ok := r.Child.U["dive"].([]string); ok {
					r.U["dive"] = dive
					if r.Parent != nil && r.Parent != jsonic.NoRule {
						r.Parent.U["dive"] = dive
					}
				}
			}
		}),

		"@pair-key-eq": jsonic.AltAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			key := tokenString(r.O0)
			nodeMap, _ := r.Node.(map[string]any)
			if nodeMap == nil {
				return
			}

			if _, isArr := nodeMap[key].([]any); isArr {
				r.U["key"] = key
				r.U["ini_array"] = nodeMap[key]
			} else if len(key) > 2 && strings.HasSuffix(key, "[]") {
				arrayKey := key[:len(key)-2]
				r.U["key"] = arrayKey
				if existing, ok := nodeMap[arrayKey].([]any); ok {
					r.U["ini_array"] = existing
				} else if _, exists := nodeMap[arrayKey]; exists {
					r.U["ini_array"] = []any{nodeMap[arrayKey]}
					nodeMap[arrayKey] = r.U["ini_array"]
				} else {
					arr := make([]any, 0)
					nodeMap[arrayKey] = arr
					r.U["ini_array"] = arr
				}
			} else {
				r.U["key"] = key
				r.U["pair"] = true
			}
		}),

		"@pair-key-bool": jsonic.AltAction(func(r *jsonic.Rule, ctx *jsonic.Context) {
			key := tokenString(r.O0)
			if key != "" {
				if nodeMap, ok := r.Parent.Node.(map[string]any); ok {
					nodeMap[key] = true
				}
			}
		}),

		"@pair-close-err": jsonic.AltError(func(r *jsonic.Rule, ctx *jsonic.Context) *jsonic.Token {
			// Not used in Go (CL token is disabled).
			return nil
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
	parsedMap := parsed.(map[string]any)

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
			// Bracket chars at start of value: concat with next value.
			// OS and CS are alternatives for the same slot (matching TS ['#OS #CS']).
			{S: [][]jsonic.Tin{{OS, CS}}, R: "val",
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

			// Handle single-quoted JSON parsing.
			if r.O0 != nil && r.O0.Tin == ST && len(r.O0.Src) > 0 && r.O0.Src[0] == '\'' {
				if s, ok := r.Node.(string); ok {
					r.Node = tryParseJSON(s)
				}
			}

			// Handle ini_prev concatenation.
			if r.Prev != nil && r.Prev != jsonic.NoRule {
				if _, ok := r.Prev.U["ini_prev"]; ok {
					valStr := fmt.Sprintf("%v", r.Node)
					r.Node = r.Prev.O0.Src + valStr
					r.Prev.Node = r.Node
					return
				}
			}

			// Handle array push.
			if r.Parent != nil && r.Parent != jsonic.NoRule {
				if arr, ok := r.Parent.U["ini_array"].([]any); ok {
					arr = append(arr, r.Node)
					r.Parent.U["ini_array"] = arr
					if key, ok := r.Parent.U["key"].(string); ok {
						if nodeMap, ok := r.Parent.Node.(map[string]any); ok {
							nodeMap[key] = arr
						}
					}
					return
				}
			}

			// Normal pair assignment.
			if r.Parent != nil && r.Parent != jsonic.NoRule {
				if key, ok := r.Parent.U["key"].(string); ok {
					if _, isPair := r.Parent.U["pair"]; isPair {
						if nodeMap, ok := r.Parent.Node.(map[string]any); ok {
							nodeMap[key] = r.Node
						}
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
	if dive, ok := r.U["dive"].([]string); ok {
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
