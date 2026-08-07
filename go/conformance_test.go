/* Copyright (c) 2026 Richard Rodger, MIT License */

/* Third-party INI conformance suite (Go half).
 *
 * The corpus is NOT in this repo. It is fetched at pinned commit SHAs by
 * ../scripts/fetch-ini-corpus.sh into ../test/corpus/ (gitignored) and
 * turned into ../test/corpus/ini-corpus.json by
 * ../scripts/build-ini-corpus.js. Read the header of build-ini-corpus.js
 * for the provenance of every label and expected value, and for why INI
 * has no authoritative suite.
 *
 * THIS TEST MUST NEVER SKIP. If the corpus is absent it FAILS with
 * instructions. A conformance test that quietly does not run turns a green
 * tick into a lie, which is the exact defect this suite exists to remove.
 *
 * The TypeScript half (ts/test/conformance.test.ts) reads the same
 * manifest and asserts the same two halves, so a divergence between the
 * runtimes shows up as a different set of failing case names.
 *
 * EXPECT THIS TO BE RED. It is a measuring instrument, not a target. Do
 * not t.Skip a case, narrow the corpus, or loosen an assertion to make the
 * number look better.
 *
 * READ THE INVALID NUMBER WITH CARE. At the 2026-08 baseline this half
 * scores 5/6, which looks strong and is not. Four of those five rejections
 * are "unexpected character(s): <bare key>" -- the parser rejects a bare
 * key that is the FIRST pair of its map (`x` alone errors; `y=1` then `x`
 * parses), so it rejects those documents by accident, not by design. The
 * one document that is unambiguously malformed under every dialect,
 * inih/tests/bad_section.ini (`[section2` never closed), is ACCEPTED.
 */

package tabnasini

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

// TestMain fetches the corpus when it is absent.
//
// The TypeScript half gets this from an npm `pretest` hook, but the shared
// CI workflow (tabnas/.github polyglot-ci.yml) runs `go test ./...`
// directly, and this repo cannot add a step to a workflow it does not own.
// Without this, CI would have no corpus, the conformance tests would report
// a missing-file failure forever, and the pressure would be to make them
// skip -- which is exactly the defect this suite exists to remove.
//
// Fetching here is idempotent and a no-op once the corpus is present. If it
// fails, the tests still FAIL LOUDLY with instructions. They never skip.
func TestMain(m *testing.M) {
	if _, err := os.Stat(conformanceManifestPath()); os.IsNotExist(err) {
		script := filepath.Join("..", "scripts", "fetch-ini-corpus.js")
		if _, serr := os.Stat(script); serr == nil {
			fmt.Fprintln(os.Stderr, "ini: conformance corpus absent, fetching...")
			cmd := exec.Command("node", script)
			cmd.Stdout = os.Stderr
			cmd.Stderr = os.Stderr
			if rerr := cmd.Run(); rerr != nil {
				fmt.Fprintf(os.Stderr,
					"ini: corpus fetch failed (%v); the conformance tests will "+
						"fail rather than skip.\n", rerr)
			}
		}
	}
	os.Exit(m.Run())
}

type conformanceCase struct {
	Name          string `json:"name"`
	Upstream      string `json:"upstream"`
	Kind          string `json:"kind"`
	Source        string `json:"source"`
	Expected      any    `json:"expected"`
	OracleAccepts bool   `json:"oracleAccepts"`
	Label         string `json:"label"`
}

type conformanceManifest struct {
	Counts struct {
		Valid   int `json:"valid"`
		Invalid int `json:"invalid"`
		Total   int `json:"total"`
	} `json:"counts"`
	Oracle struct {
		Name string `json:"name"`
		Sha  string `json:"sha"`
	} `json:"oracle"`
	Cases []conformanceCase `json:"cases"`
}

const missingCorpus = `INI conformance corpus not found at test/corpus/ini-corpus.json.

Fetch it (pinned SHAs, gitignored, never committed):
    ./scripts/fetch-ini-corpus.sh

This test deliberately FAILS rather than skips: a conformance suite that
silently does not run reports green while measuring nothing.`

func conformanceManifestPath() string {
	return filepath.Join("..", "test", "corpus", "ini-corpus.json")
}

func loadConformanceManifest(t *testing.T) *conformanceManifest {
	t.Helper()
	path := conformanceManifestPath()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("%s\n(underlying error: %v)", missingCorpus, err)
	}
	var m conformanceManifest
	if err := json.Unmarshal(raw, &m); err != nil {
		t.Fatalf("INI conformance corpus at %s is unreadable: %v\n"+
			"Re-run ./scripts/fetch-ini-corpus.sh --force.", path, err)
	}
	if len(m.Cases) == 0 {
		t.Fatalf("INI conformance corpus at %s is present but contains no "+
			"cases. Re-run ./scripts/fetch-ini-corpus.sh --force.", path)
	}
	return &m
}

// parseGuarded runs Parse, converting a panic into an error. The engine
// surfaces some rejections as panics from state actions; both count as a
// rejection, and neither may be allowed to abort the run.
func parseGuarded(src string) (result map[string]any, err error) {
	defer func() {
		if r := recover(); r != nil {
			result = nil
			err = fmt.Errorf("panic: %v", r)
		}
	}()
	return Parse(src)
}

func TestConformanceCorpusLoaded(t *testing.T) {
	m := loadConformanceManifest(t)
	var valid, invalid int
	for _, c := range m.Cases {
		switch c.Kind {
		case "valid":
			valid++
		case "invalid":
			invalid++
		default:
			t.Errorf("case %q has unclassified kind %q", c.Name, c.Kind)
		}
	}
	if valid == 0 {
		t.Error("corpus has no valid cases")
	}
	if invalid == 0 {
		t.Error("corpus has no invalid cases")
	}
}

// Valid documents must parse AND produce the expected value. Parsing
// without error is not enough -- asserting only "it did not throw" is how a
// wrong-value bug hides.
func TestConformanceValid(t *testing.T) {
	m := loadConformanceManifest(t)
	for _, c := range m.Cases {
		if c.Kind != "valid" {
			continue
		}
		t.Run(c.Name, func(t *testing.T) {
			got, err := parseGuarded(c.Source)
			if err != nil {
				t.Fatalf("%s: rejected a valid document\n  error:    %v\n  expected: %s",
					c.Name, err, formatValue(c.Expected))
			}
			var gotAny any = got
			if !valuesEqual(gotAny, c.Expected) {
				t.Fatalf("%s: parsed, but to a different value than the %s oracle\n"+
					"  got:      %s\n  expected: %s",
					c.Name, m.Oracle.Name, formatValue(gotAny), formatValue(c.Expected))
			}
		})
	}
}

// Invalid documents must be rejected.
func TestConformanceInvalid(t *testing.T) {
	m := loadConformanceManifest(t)
	for _, c := range m.Cases {
		if c.Kind != "invalid" {
			continue
		}
		t.Run(c.Name, func(t *testing.T) {
			got, err := parseGuarded(c.Source)
			if err == nil {
				var gotAny any = got
				t.Fatalf("%s: accepted a document its upstream labels malformed.\n"+
					"  upstream label: %s\n  parsed as:      %s\n"+
					"  note: the %s oracle also accepts this document (INI dialects\n"+
					"  disagree here) -- see scripts/build-ini-corpus.js.",
					c.Name, c.Label, formatValue(gotAny), m.Oracle.Name)
			}
		})
	}
}
