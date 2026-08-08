/* Copyright (c) 2026 Richard Rodger, MIT License */

// Third-party INI conformance suite (Go half). Asserts exactly the same
// manifest, divergence list and rules as ts/test/conformance.test.ts —
// read the header comment there for why the corpus is assembled rather
// than official, and why the divergence lists are not an escape hatch.
//
// THIS TEST MUST NEVER SKIP. If the manifest is absent it FAILS.

package tabnasini

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

type conformanceCase struct {
	Name     string `json:"name"`
	Kind     string `json:"kind"`
	Source   string `json:"source"`
	Label    string `json:"label"`
	Expected any    `json:"expected"`
}

type conformanceManifest struct {
	Cases []conformanceCase `json:"cases"`
}

const missingCorpus = "INI conformance corpus not found at " +
	"test/corpus/ini-corpus.json.\n\n" +
	"The manifest is self-contained (sources + oracle values + pinned " +
	"upstream SHAs) and ships with the repo. This test deliberately " +
	"FAILS rather than skips: a conformance suite that silently does " +
	"not run reports green while measuring nothing."

// Keep in sync with DIVERGENT in ts/test/conformance.test.ts.
var conformanceDivergent = map[string]string{
	"go-ini/testdata/full.ini": "inline comments off by default; oracle " +
		"requires a section header to be the whole line",
	"inih/examples/test.ini": "oracle requires a section header to be the " +
		"whole line; inline comments off by default",
	"inih/fuzzing/testcases/case1.ini": "same section-header rule; inline " +
		"comments off by default",
	"inih/tests/multi_line.ini": "inline comments off by default",
	"inih/tests/normal.ini": "same section-header rule; inline comments " +
		"off by default",
	"iniparser/example/twisted.ini": "inline comments off by default, " +
		"backslash-escaped newline continues a value, degenerate quote " +
		"handling differs",
	"iniparser/test/ressources/bad_ini/twisted-ofval.ini": "a backslash " +
		"before a newline escapes it, so the value continues",
	"iniparser/test/ressources/good_ini/spaced2.ini": "inline comments off by default",
	"iniparser/test/ressources/old.ini":              "inline comments off by default",
	"iniparser/test/ressources/quotes.ini":           "inline comments off by default",
	"iniparser/test/ressources/utf8.ini":             "inline comments off by default",
	"npm-ini/test/fixtures/foo.ini": "with inline comments off, `\\;` is a " +
		"literal backslash then a literal `;`",
}

// Cases where the Go port does NOT yet match the canonical TypeScript
// behaviour. These are NOT dialect choices — they are parity gaps.
//
// Both gaps recorded here were owned by github.com/tabnas/hoover/go:
//
//  1. hoover.trimString trimmed only ' ', '\t', '\r', '\n', whereas the
//     TS block uses JS String.prototype.trim(), which also strips
//     U+FEFF. A leading BOM survived into the key in Go, not in TS.
//  2. hoover encoded the `""` end-of-input terminator as the byte 0, so
//     a literal NUL byte in the source was mistaken for it and ended the
//     span early. UTF-16 text decoded as UTF-8 is full of NULs, so those
//     documents were rejected outright.
//
// Both are now fixed: trimString strips U+FEFF (isTrimSpace), and the
// end-of-input sentinel is -1 rather than the byte 0, so a literal NUL no
// longer truncates. The four entries that lived here — go-ini
// UTF-8-BOM / UTF-16-LE-BOM / UTF-16-BE-BOM and inih bom.ini — were
// deleted when this test went red.
//
// Each entry is asserted to STILL be a gap, so fixing the root cause
// turns this test red and forces the entry to be deleted rather than
// linger. That is what happened here — keep the mechanism.
var conformanceGoParityGap = map[string]string{}

// Keep in sync with ACCEPTED_BY_DESIGN in ts/test/conformance.test.ts.
var conformanceAcceptedByDesign = map[string]string{
	"inih/tests/bad_comment.ini":                           "bare boolean key, a documented feature",
	"inih/tests/bad_multi.ini":                             "bare boolean key, a documented feature",
	"iniparser/test/ressources/bad_ini/ends_well.ini":      "bare boolean key, a documented feature",
	"iniparser/test/ressources/bad_ini/twisted-errors.ini": "bare boolean keys, a documented feature",
}

func loadConformanceManifest(t *testing.T) *conformanceManifest {
	t.Helper()
	path := filepath.Join("..", "test", "corpus", "ini-corpus.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(missingCorpus)
	}
	var m conformanceManifest
	if err := json.Unmarshal(raw, &m); err != nil {
		t.Fatalf("test/corpus/ini-corpus.json is not readable JSON: %v", err)
	}
	if len(m.Cases) == 0 {
		t.Fatal("test/corpus/ini-corpus.json is present but contains no cases.")
	}
	return &m
}

// Compare through JSON on both sides so map ordering and numeric
// representation cannot make equal values look different. This
// normalises representation only.
func conformanceNorm(t *testing.T, v any) string {
	t.Helper()
	b, err := json.Marshal(canonicalJSON(v))
	if err != nil {
		t.Fatalf("cannot marshal parse result: %v", err)
	}
	return string(b)
}

// canonicalJSON reduces a value to plain JSON kinds with map keys sorted
// by encoding/json (which sorts map[string]any keys on marshal).
func canonicalJSON(v any) any {
	b, err := json.Marshal(v)
	if err != nil {
		return v
	}
	var out any
	if err := json.Unmarshal(b, &out); err != nil {
		return v
	}
	return out
}

// tryParse reports whether the document was rejected, treating a panic
// from a state action the same as a returned error.
func tryParse(src string) (result map[string]any, rejected bool) {
	defer func() {
		if r := recover(); r != nil {
			rejected = true
		}
	}()
	got, err := Parse(src)
	if err != nil {
		return nil, true
	}
	return got, false
}

func TestConformanceCorpusShape(t *testing.T) {
	m := loadConformanceManifest(t)
	valid, invalid := 0, 0
	for _, c := range m.Cases {
		switch c.Kind {
		case "valid":
			valid++
		case "invalid":
			invalid++
		default:
			t.Errorf("%s: unclassified case kind %q", c.Name, c.Kind)
		}
	}
	if valid != 30 {
		t.Errorf("valid case count changed: got %d, want 30", valid)
	}
	if invalid != 6 {
		t.Errorf("invalid case count changed: got %d, want 6", invalid)
	}
}

func TestConformanceValidDocumentsParse(t *testing.T) {
	m := loadConformanceManifest(t)
	for _, c := range m.Cases {
		if c.Kind != "valid" {
			continue
		}
		_, rejected := tryParse(c.Source)
		if _, known := conformanceGoParityGap[c.Name]; known {
			// A known parity gap; TestConformanceValidDocumentsMatchOracle
			// asserts the gap is still real so the entry cannot go stale.
			continue
		}
		if rejected {
			t.Errorf("%s: a real-world .ini file must not fail to parse", c.Name)
		}
	}
}

func TestConformanceValidDocumentsMatchOracle(t *testing.T) {
	m := loadConformanceManifest(t)
	for _, c := range m.Cases {
		if c.Kind != "valid" {
			continue
		}
		got, rejected := tryParse(c.Source)
		if rejected {
			if _, known := conformanceGoParityGap[c.Name]; !known {
				continue // already reported by TestConformanceValidDocumentsParse
			}
			// Rejected AND a known parity gap: the gap is still real.
			continue
		}
		gotJSON := conformanceNorm(t, got)
		wantJSON := conformanceNorm(t, c.Expected)
		if gapWhy, known := conformanceGoParityGap[c.Name]; known {
			if gotJSON == wantJSON {
				t.Errorf("%s: now matches the oracle, so its Go parity-gap "+
					"entry (%q) is stale — the hoover fix landed; delete it.",
					c.Name, gapWhy)
			}
			continue
		}
		why, diverges := conformanceDivergent[c.Name]
		if !diverges {
			if gotJSON != wantJSON {
				t.Errorf("%s: parsed, but to a different value than the "+
					"npm/ini oracle.\n  got:    %s\n  oracle: %s\n"+
					"If this is a dialect difference, add it to "+
					"conformanceDivergent with the documented reason; "+
					"otherwise it is a bug.", c.Name, gotJSON, wantJSON)
			}
			continue
		}
		if gotJSON == wantJSON {
			t.Errorf("%s: now MATCHES the oracle, so its divergence entry "+
				"(%q) is stale — delete it.", c.Name, why)
		}
	}
}

func TestConformanceInvalidDocuments(t *testing.T) {
	m := loadConformanceManifest(t)
	for _, c := range m.Cases {
		if c.Kind != "invalid" {
			continue
		}
		got, rejected := tryParse(c.Source)
		why, byDesign := conformanceAcceptedByDesign[c.Name]
		if !byDesign {
			if !rejected {
				t.Errorf("%s: accepted a document its upstream labels "+
					"malformed.\n  upstream label: %s\n  parsed as: %s",
					c.Name, c.Label, conformanceNorm(t, got))
			}
			continue
		}
		if rejected {
			t.Errorf("%s: rejected, but it is valid in this dialect (%s). "+
				"Either the plugin regressed or the entry is stale.",
				c.Name, why)
		}
	}
}
