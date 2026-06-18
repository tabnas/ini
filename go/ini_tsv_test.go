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

func runIniTSV(t *testing.T, file string, opts ...IniOptions) {
	t.Helper()
	path := filepath.Join(tsvSpecDir(), file)
	rows, err := loadTSV(path)
	if err != nil {
		t.Fatalf("failed to load %s: %v", file, err)
	}

	for _, row := range rows {
		if len(row.cols) < 2 {
			continue
		}
		input := tsvUnescape(row.cols[0])
		expectedStr := row.cols[1]

		if strings.HasPrefix(expectedStr, "ERROR:") {
			func() {
				// The tabnas engine recovers state-action panics and returns
				// them as a parse error; accept either a panic or a non-nil err.
				defer func() {
					_ = recover()
				}()
				if _, perr := Parse(input, opts...); perr == nil {
					// Returned cleanly and did not panic: rejection missing.
					t.Errorf("line %d: expected panic or error for input %q", row.lineNo, row.cols[0])
				}
			}()
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
