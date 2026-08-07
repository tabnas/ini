/* Copyright (c) 2021-2025 Richard Rodger, MIT License */

package tabnasini

import (
	"bufio"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

type tsvRow struct {
	cols   []string
	lineNo int
}

func loadTSV(path string) ([]tsvRow, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	var rows []tsvRow
	scanner := bufio.NewScanner(f)
	lineNo := 0
	for scanner.Scan() {
		lineNo++
		if lineNo == 1 {
			continue
		}
		line := scanner.Text()
		if line == "" {
			continue
		}
		cols := strings.Split(line, "\t")
		rows = append(rows, tsvRow{cols: cols, lineNo: lineNo})
	}
	return rows, scanner.Err()
}

func parseExpected(s string) (any, error) {
	if s == "" {
		return nil, nil
	}
	var val any
	err := json.Unmarshal([]byte(s), &val)
	if err != nil {
		return nil, err
	}
	return val, nil
}

func formatValue(v any) string {
	if v == nil {
		return "nil"
	}
	b, err := json.Marshal(v)
	if err != nil {
		return fmt.Sprintf("%v", v)
	}
	return string(b)
}

func normalizeValue(v any) any {
	switch val := v.(type) {
	case map[string]any:
		result := make(map[string]any)
		for k, v := range val {
			result[k] = normalizeValue(v)
		}
		return result
	case []any:
		result := make([]any, len(val))
		for i, v := range val {
			result[i] = normalizeValue(v)
		}
		return result
	case float64:
		if val == 0 {
			return float64(0)
		}
		return val
	default:
		return v
	}
}

func valuesEqual(got, expected any) bool {
	return deepCompare(normalizeValue(got), normalizeValue(expected))
}

func deepCompare(a, b any) bool {
	if a == nil && b == nil {
		return true
	}
	if a == nil || b == nil {
		return false
	}
	switch av := a.(type) {
	case map[string]any:
		bv, ok := b.(map[string]any)
		if !ok || len(av) != len(bv) {
			return false
		}
		for k, v := range av {
			if !deepCompare(v, bv[k]) {
				return false
			}
		}
		return true
	case []any:
		bv, ok := b.([]any)
		if !ok || len(av) != len(bv) {
			return false
		}
		for i := range av {
			if !deepCompare(av[i], bv[i]) {
				return false
			}
		}
		return true
	case float64:
		bv, ok := b.(float64)
		if !ok {
			return false
		}
		if math.IsNaN(av) && math.IsNaN(bv) {
			return true
		}
		return av == bv
	case string:
		bv, ok := b.(string)
		return ok && av == bv
	case bool:
		bv, ok := b.(bool)
		return ok && av == bv
	default:
		return reflect.DeepEqual(a, b)
	}
}

func tsvSpecDir() string {
	return filepath.Join("..", "test", "spec")
}

func tsvUnescape(s string) string {
	s = strings.ReplaceAll(s, "\\r\\n", "\r\n")
	s = strings.ReplaceAll(s, "\\n", "\n")
	s = strings.ReplaceAll(s, "\\r", "\r")
	s = strings.ReplaceAll(s, "\\t", "\t")
	return s
}

// errorCodeMatches turns an `ERROR:<code>` fixture cell into a real
// assertion: `duplicate_section` requires the reported error to say
// "duplicate section". Must stay identical to the TypeScript half in
// ts/test/ini-tsv.test.ts.
func errorCodeMatches(code, message string) bool {
	want := strings.ToLower(strings.ReplaceAll(strings.TrimSpace(code), "_", " "))
	return want != "" && strings.Contains(strings.ToLower(message), want)
}

// parseGuarded2 runs Parse with options, converting a panic into an error so
// a panicking rejection is still inspectable rather than silently swallowed.
func parseGuarded2(src string, opts ...IniOptions) (result map[string]any, err error) {
	defer func() {
		if r := recover(); r != nil {
			result = nil
			err = fmt.Errorf("panic: %v", r)
		}
	}()
	return Parse(src, opts...)
}

func runIniTSV(t *testing.T, file string, opts ...IniOptions) {
	t.Helper()
	path := filepath.Join(tsvSpecDir(), file)
	rows, err := loadTSV(path)
	if err != nil {
		t.Fatalf("failed to load %s: %v", file, err)
	}

	// A fixture that loads zero rows used to pass green: the loop below just
	// never ran. An emptied, renamed or header-only .tsv must be a failure.
	if len(rows) == 0 {
		t.Fatalf("%s: loaded 0 cases. An empty or header-only fixture proves "+
			"nothing and must not pass.", file)
	}

	for _, row := range rows {
		// A line with no tab used to be silently dropped here (and to crash
		// the TypeScript loader). Both runtimes now reject it by name.
		if len(row.cols) < 2 {
			t.Errorf("%s line %d: expected 2 tab-separated columns, got %d: %q",
				file, row.lineNo, len(row.cols), row.cols[0])
			continue
		}
		// TypeScript decodes escapes in EVERY column; Go used to decode only
		// `input`, so a `\n` in an `expected` cell meant two different things
		// in the two runtimes. TypeScript is canonical: decode both.
		input := tsvUnescape(row.cols[0])
		expectedStr := tsvUnescape(row.cols[1])

		if strings.HasPrefix(expectedStr, "ERROR:") {
			// `ERROR:<code>` used to be a bare rejection marker whose text
			// nobody read: any error or panic at all counted as a pass. Now
			// the code IS the assertion, matching the TypeScript half.
			code := strings.TrimPrefix(expectedStr, "ERROR:")
			got, perr := parseGuarded2(input, opts...)
			if perr == nil {
				t.Errorf("%s line %d: expected error %q for input %q, but it "+
					"parsed to %s", file, row.lineNo, code, row.cols[0],
					formatValue(any(got)))
				continue
			}
			if !errorCodeMatches(code, perr.Error()) {
				t.Errorf("%s line %d: input %q was rejected, but not with the "+
					"declared code %q\n  error: %v",
					file, row.lineNo, row.cols[0], code, perr)
			}
			continue
		}

		expected, err := parseExpected(expectedStr)
		if err != nil {
			t.Errorf("line %d: failed to parse expected %q: %v", row.lineNo, expectedStr, err)
			continue
		}

		got, parseErr := Parse(input, opts...)
		if parseErr != nil {
			t.Errorf("line %d: Parse(%q) error: %v", row.lineNo, row.cols[0], parseErr)
			continue
		}

		// Normalize: Parse returns map[string]any, compare against JSON-parsed expected.
		var gotAny any = got
		if !valuesEqual(gotAny, expected) {
			t.Errorf("line %d: Parse(%q)\n  got:      %s\n  expected: %s",
				row.lineNo, row.cols[0], formatValue(gotAny), formatValue(expected))
		}
	}
}

// --- TSV Test Functions ---

func TestTSVHappy(t *testing.T) {
	runIniTSV(t, "happy.tsv")
}

func TestTSVBasicValues(t *testing.T) {
	runIniTSV(t, "basic-values.tsv")
}

func TestTSVQuotedValues(t *testing.T) {
	runIniTSV(t, "quoted-values.tsv")
}

func TestTSVBareKey(t *testing.T) {
	runIniTSV(t, "bare-key.tsv")
}

func TestTSVKeyOverwrite(t *testing.T) {
	runIniTSV(t, "key-overwrite.tsv")
}

func TestTSVArrays(t *testing.T) {
	runIniTSV(t, "arrays.tsv")
}

func TestTSVEmptyInput(t *testing.T) {
	runIniTSV(t, "empty-input.tsv")
}

func TestTSVLineComments(t *testing.T) {
	runIniTSV(t, "line-comments.tsv")
}

func TestTSVInlineCommentsOff(t *testing.T) {
	runIniTSV(t, "inline-comments-off.tsv")
}

func TestTSVInlineCommentsActive(t *testing.T) {
	runIniTSV(t, "inline-comments-active.tsv", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{Active: boolPtr(true)},
		},
	})
}

func TestTSVInlineCommentsCustomChars(t *testing.T) {
	runIniTSV(t, "inline-comments-custom-chars.tsv", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Chars:  []string{";"},
			},
		},
	})
}

func TestTSVInlineCommentsBackslash(t *testing.T) {
	runIniTSV(t, "inline-comments-backslash.tsv", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Backslash: boolPtr(true)},
			},
		},
	})
}

func TestTSVInlineCommentsBackslashDisabled(t *testing.T) {
	runIniTSV(t, "inline-comments-backslash-disabled.tsv", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Backslash: boolPtr(false)},
			},
		},
	})
}

func TestTSVInlineCommentsWhitespace(t *testing.T) {
	runIniTSV(t, "inline-comments-whitespace.tsv", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Whitespace: boolPtr(true)},
			},
		},
	})
}

func TestTSVInlineCommentsWhitespaceBackslash(t *testing.T) {
	runIniTSV(t, "inline-comments-whitespace-backslash.tsv", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{
					Whitespace: boolPtr(true),
					Backslash:  boolPtr(true),
				},
			},
		},
	})
}

func TestTSVInlineCommentsWithSections(t *testing.T) {
	runIniTSV(t, "inline-comments-with-sections.tsv", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{Active: boolPtr(true)},
		},
	})
}

func TestTSVSections(t *testing.T) {
	runIniTSV(t, "sections.tsv")
}

func TestTSVSectionsEscapedDots(t *testing.T) {
	runIniTSV(t, "sections-escaped-dots.tsv")
}

func TestTSVSectionsDuplicateMerge(t *testing.T) {
	runIniTSV(t, "sections-duplicate-merge.tsv")
}

func TestTSVSectionsDuplicateOverride(t *testing.T) {
	runIniTSV(t, "sections-duplicate-override.tsv", IniOptions{
		Section: &SectionOptions{Duplicate: "override"},
	})
}

func TestTSVSectionsDuplicateError(t *testing.T) {
	runIniTSV(t, "sections-duplicate-error.tsv", IniOptions{
		Section: &SectionOptions{Duplicate: "error"},
	})
}

func TestTSVMultilineBackslash(t *testing.T) {
	runIniTSV(t, "multiline-backslash.tsv", IniOptions{
		Multiline: &MultilineOptions{},
	})
}

func TestTSVMultilineIndent(t *testing.T) {
	noBackslash := ""
	runIniTSV(t, "multiline-indent.tsv", IniOptions{
		Multiline: &MultilineOptions{
			Indent:       boolPtr(true),
			Continuation: &noBackslash,
		},
	})
}

func TestTSVMultilineBoth(t *testing.T) {
	bs := "\\"
	runIniTSV(t, "multiline-both.tsv", IniOptions{
		Multiline: &MultilineOptions{
			Continuation: &bs,
			Indent:       boolPtr(true),
		},
	})
}

func TestTSVMultilineWithInline(t *testing.T) {
	runIniTSV(t, "multiline-with-inline.tsv", IniOptions{
		Multiline: &MultilineOptions{},
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{Active: boolPtr(true)},
		},
	})
}

func TestTSVMultilineEscapes(t *testing.T) {
	runIniTSV(t, "multiline-escapes.tsv", IniOptions{
		Multiline: &MultilineOptions{},
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Backslash: boolPtr(true)},
			},
		},
	})
}

func TestTSVMultilineNoInline(t *testing.T) {
	runIniTSV(t, "multiline-no-inline.tsv", IniOptions{
		Multiline: &MultilineOptions{},
	})
}

func TestTSVNumbersAreStrings(t *testing.T) {
	runIniTSV(t, "numbers-are-strings.tsv")
}

// KNOWN GAP, LEFT FAILING ON PURPOSE (TypeScript only).
//
// A value whose last character is a backslash at end-of-input, with no
// trailing newline, is rejected by the TypeScript runtime
// ([jsonic/invalid_text]) and accepted by Go. Found by differential fuzzing
// of the two runtimes; the minimal case is `x=a\` with no newline.
//
// The expected values are the npm/ini oracle's, not TypeScript's, because Go
// and the oracle agree and TypeScript is the outlier -- the "Go has exposed a
// genuine TS defect" carve-out in test/AGENTS.md. This side passes; the
// TypeScript side is the honest red. Do not flip the fixture to ERROR to get
// TypeScript green: that would enshrine the defect.
func TestTSVEOFTrailingBackslash(t *testing.T) {
	runIniTSV(t, "eof-trailing-backslash.tsv")
}
