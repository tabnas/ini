/* Copyright (c) 2021-2025 Richard Rodger, MIT License */

package ini

import (
	"reflect"
	"testing"
)

// assert is a test helper that checks deep equality.
func assert(t *testing.T, name string, got, want any) {
	t.Helper()
	if !reflect.DeepEqual(got, want) {
		t.Errorf("%s:\n  got:  %#v\n  want: %#v", name, got, want)
	}
}


func TestHappy(t *testing.T) {
	j := MakeJsonic()

	r, err := j.Parse("a=1")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "simple", r, map[string]any{"a": "1"})

	r, err = j.Parse("[A]")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "section", r, map[string]any{"A": map[string]any{}})

	r, err = j.Parse("a=\nb=")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "empty-values", r, map[string]any{"a": "", "b": ""})
}

func TestInlineCommentsOff(t *testing.T) {
	// Default: inline comments are off. ; and # mid-value are literal.
	result, err := Parse("a = hello ; world")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "semicolon-literal", result, map[string]any{"a": "hello ; world"})

	result, err = Parse("a = hello # world")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "hash-literal", result, map[string]any{"a": "hello # world"})

	result, err = Parse("a = x;y;z")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "multi-semi", result, map[string]any{"a": "x;y;z"})
}

func TestLineComments(t *testing.T) {
	// Line-start comments always work.
	result, err := Parse("; comment\na = 1")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "semi-comment", result, map[string]any{"a": "1"})

	result, err = Parse("# comment\na = 1")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "hash-comment", result, map[string]any{"a": "1"})
}

func TestInlineActive(t *testing.T) {
	result, err := Parse("a = hello ; comment", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{Active: boolPtr(true)},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "semi-inline", result, map[string]any{"a": "hello"})

	result, err = Parse("a = hello # comment", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{Active: boolPtr(true)},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "hash-inline", result, map[string]any{"a": "hello"})
}

func TestInlineCustomChars(t *testing.T) {
	result, err := Parse("a = hello ; comment\nb = hello # not a comment", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Chars:  []string{";"},
			},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "custom-chars", result, map[string]any{
		"a": "hello",
		"b": "hello # not a comment",
	})
}

func TestInlineBackslashEscape(t *testing.T) {
	result, err := Parse("a = hello\\; world", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Backslash: boolPtr(true)},
			},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "backslash-semi", result, map[string]any{"a": "hello; world"})

	result, err = Parse("a = hello\\# world", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Backslash: boolPtr(true)},
			},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "backslash-hash", result, map[string]any{"a": "hello# world"})
}

func TestInlineWhitespacePrefix(t *testing.T) {
	result, err := Parse("a = x;y;z", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Whitespace: boolPtr(true)},
			},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "no-ws-literal", result, map[string]any{"a": "x;y;z"})

	result, err = Parse("a = hello ;comment", IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Whitespace: boolPtr(true)},
			},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "ws-comment", result, map[string]any{"a": "hello"})
}

func TestSections(t *testing.T) {
	result, err := Parse("[d]\ne = 2")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "simple-section", result, map[string]any{
		"d": map[string]any{"e": "2"},
	})
}

func TestNestedSections(t *testing.T) {
	result, err := Parse("[h.i]\nj = 3")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "nested", result, map[string]any{
		"h": map[string]any{
			"i": map[string]any{"j": "3"},
		},
	})
}

func TestSectionDuplicateMerge(t *testing.T) {
	result, err := Parse("[a]\nx=1\ny=2\n[a]\nz=3")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "merge", result, map[string]any{
		"a": map[string]any{"x": "1", "y": "2", "z": "3"},
	})
}

func TestSectionDuplicateOverride(t *testing.T) {
	result, err := Parse("[a]\nx=1\ny=2\n[a]\nz=3", IniOptions{
		Section: &SectionOptions{Duplicate: "override"},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "override", result, map[string]any{
		"a": map[string]any{"z": "3"},
	})
}

func TestSectionDuplicateError(t *testing.T) {
	// The tabnas engine recovers state-action panics and surfaces them as a
	// returned parse error, so a duplicate section is reported via err (or a
	// raw panic on older engines). Accept either.
	expectDuplicateSectionError(t, func() (any, error) {
		return Parse("[a]\nx=1\n[a]\ny=2", IniOptions{
			Section: &SectionOptions{Duplicate: "error"},
		})
	}, "duplicate section")
}

// expectDuplicateSectionError asserts that parse rejects a duplicate section,
// whether by panicking or by returning a non-nil error.
func expectDuplicateSectionError(t *testing.T, parse func() (any, error), what string) {
	t.Helper()
	defer func() {
		if r := recover(); r != nil {
			return // panic is an acceptable rejection
		}
	}()
	if _, err := parse(); err == nil {
		t.Fatalf("expected panic or error for %s", what)
	}
}

func TestKeyByItself(t *testing.T) {
	// Bare key (without =) means key=true. Works after a key=value pair
	// (matching TS behavior where pair.close routes bare keys back to pair).
	result, err := Parse("a=1\nmykey")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "key-true", result, map[string]any{"a": "1", "mykey": true})
}

func TestArraySyntax(t *testing.T) {
	result, err := Parse("a[]=1\na[]=2")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "array", result, map[string]any{"a": []any{"1", "2"}})
}

func TestMultilineContinuation(t *testing.T) {
	result, err := Parse("a = hello \\\nworld", IniOptions{
		Multiline: &MultilineOptions{},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "backslash-cont", result, map[string]any{"a": "hello world"})

	// Multiple continuations.
	result, err = Parse("a = one \\\ntwo \\\nthree", IniOptions{
		Multiline: &MultilineOptions{},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "multi-cont", result, map[string]any{"a": "one two three"})
}

func TestMultilineIndent(t *testing.T) {
	noBackslash := ""
	result, err := Parse("a = hello\n    world", IniOptions{
		Multiline: &MultilineOptions{
			Indent:       boolPtr(true),
			Continuation: &noBackslash,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "indent-cont", result, map[string]any{"a": "hello world"})
}

func TestMultilineWithInlineComments(t *testing.T) {
	result, err := Parse("a = hello \\\nworld ;comment\nb = 2", IniOptions{
		Multiline: &MultilineOptions{},
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{Active: boolPtr(true)},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "multiline-inline", result, map[string]any{
		"a": "hello world",
		"b": "2",
	})
}

func TestQuotedValues(t *testing.T) {
	result, err := Parse(`a = "hello world"`)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "double-quoted", result, map[string]any{"a": "hello world"})

	result, err = Parse("a = 'hello world'")
	if err != nil {
		t.Fatal(err)
	}
	// Single-quoted values attempt JSON parse.
	assert(t, "single-quoted", result, map[string]any{"a": "hello world"})
}

func TestEmptyInput(t *testing.T) {
	result, err := Parse("")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "empty", result, map[string]any{})
}

func TestBooleanValues(t *testing.T) {
	result, err := Parse("a = true\nb = false")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "booleans", result, map[string]any{"a": true, "b": false})
}

func TestNullValue(t *testing.T) {
	result, err := Parse("a = null")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "null", result, map[string]any{"a": nil})
}

func TestMultiplePairs(t *testing.T) {
	result, err := Parse("a = 1\nb = x\nc = y y")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "multi-pairs", result, map[string]any{
		"a": "1",
		"b": "x",
		"c": "y y",
	})
}

func TestMixedSectionsAndPairs(t *testing.T) {
	result, err := Parse("x = 0\n[s]\na = 1\nb = 2")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "mixed", result, map[string]any{
		"x": "0",
		"s": map[string]any{"a": "1", "b": "2"},
	})
}

func TestUsePlugin(t *testing.T) {
	// Verify the plugin interface works directly.
	j := MakeJsonic()
	result, err := j.Parse("a=1\nb=2")
	if err != nil {
		t.Fatal(err)
	}
	m, ok := result.(map[string]any)
	if !ok {
		t.Fatalf("expected map, got %T", result)
	}
	assert(t, "plugin", m, map[string]any{"a": "1", "b": "2"})
}

func TestEqualsInValue(t *testing.T) {
	result, err := Parse("u = v = 5")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "eq-in-value", result, map[string]any{"u": "v = 5"})
}

func TestBasicComprehensive(t *testing.T) {
	result, err := Parse(`
; comment
a = 1
b = x
c = y y
c0 = true
" c1  c2 " = null
'[]'='[]'

[d]
e = 2
e0[]=q q
e0[]=w w
"[]"="[]"

[f]
# x:11
g = 'G'
# x:12


[h.i]
j = [3,4]
j0 = ]3,4[
k = false

[l.m.n.o]
p = "P"
q = {x:1}
u = v = 5
w = '{"y":{"z":6}}'
aa = 7

`)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "basic-a", result["a"], "1")
	assert(t, "basic-b", result["b"], "x")
	assert(t, "basic-c", result["c"], "y y")
	assert(t, "basic-c0", result["c0"], true)
	assert(t, "basic-c1c2", result[" c1  c2 "], nil)
	assert(t, "basic-bracket-key", result["[]"], []any{})

	d := result["d"].(map[string]any)
	assert(t, "basic-d-e", d["e"], "2")
	assert(t, "basic-d-e0", d["e0"], []any{"q q", "w w"})
	assert(t, "basic-d-bracket", d["[]"], "[]")

	f := result["f"].(map[string]any)
	assert(t, "basic-f-g", f["g"], "G")

	hi := result["h"].(map[string]any)["i"].(map[string]any)
	assert(t, "basic-hi-j", hi["j"], "[3,4]")
	assert(t, "basic-hi-j0", hi["j0"], "]3,4[")
	assert(t, "basic-hi-k", hi["k"], false)

	lmno := result["l"].(map[string]any)["m"].(map[string]any)["n"].(map[string]any)["o"].(map[string]any)
	assert(t, "basic-lmno-p", lmno["p"], "P")
	assert(t, "basic-lmno-q", lmno["q"], "{x:1}")
	assert(t, "basic-lmno-u", lmno["u"], "v = 5")
	assert(t, "basic-lmno-w", lmno["w"], map[string]any{"y": map[string]any{"z": float64(6)}})
	assert(t, "basic-lmno-aa", lmno["aa"], "7")
}

func TestMultilineBackslashFull(t *testing.T) {
	// Basic continuation with \<LF>
	result, err := Parse("a = hello \\\nworld", IniOptions{Multiline: &MultilineOptions{}})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "basic-cont", result, map[string]any{"a": "hello world"})

	// Continuation with leading whitespace on next line (consumed)
	result, err = Parse("a = hello \\\n    world", IniOptions{Multiline: &MultilineOptions{}})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "ws-cont", result, map[string]any{"a": "hello world"})

	// Multiple continuations
	result, err = Parse("a = one \\\ntwo \\\nthree", IniOptions{Multiline: &MultilineOptions{}})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "multi-cont", result, map[string]any{"a": "one two three"})

	// No continuation: normal newline ends value
	result, err = Parse("a = hello\nb = world", IniOptions{Multiline: &MultilineOptions{}})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "no-cont", result, map[string]any{"a": "hello", "b": "world"})

	// Continuation with \<CR><LF>
	result, err = Parse("a = hello \\\r\nworld", IniOptions{Multiline: &MultilineOptions{}})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "crlf-cont", result, map[string]any{"a": "hello world"})

	// Escaped backslash before newline is NOT continuation
	result, err = Parse("a = path\\\\\nb = next", IniOptions{Multiline: &MultilineOptions{}})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "esc-bs", result, map[string]any{"a": "path\\", "b": "next"})

	// Continuation in a section
	result, err = Parse("[s]\na = hello \\\n    world", IniOptions{Multiline: &MultilineOptions{}})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "section-cont", result, map[string]any{"s": map[string]any{"a": "hello world"}})

	// Empty value with continuation
	result, err = Parse("a = \\\nworld", IniOptions{Multiline: &MultilineOptions{}})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "empty-cont", result, map[string]any{"a": "world"})

	// Inline comments off by default: ; is literal in value
	result, err = Parse("a = hello \\\nworld ;not-a-comment\nb = 2", IniOptions{Multiline: &MultilineOptions{}})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "no-inline", result, map[string]any{"a": "hello world ;not-a-comment", "b": "2"})
}

func TestMultilineIndentFull(t *testing.T) {
	noBackslash := ""

	// Indented line continues previous value
	result, err := Parse("a = hello\n    world", IniOptions{
		Multiline: &MultilineOptions{Indent: boolPtr(true), Continuation: &noBackslash},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "indent-cont", result, map[string]any{"a": "hello world"})

	// Multiple indent continuations
	result, err = Parse("a = line1\n  line2\n  line3", IniOptions{
		Multiline: &MultilineOptions{Indent: boolPtr(true), Continuation: &noBackslash},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "multi-indent", result, map[string]any{"a": "line1 line2 line3"})

	// Non-indented line is a new key
	result, err = Parse("a = hello\nb = world", IniOptions{
		Multiline: &MultilineOptions{Indent: boolPtr(true), Continuation: &noBackslash},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "no-indent", result, map[string]any{"a": "hello", "b": "world"})

	// Tab indent
	result, err = Parse("a = hello\n\tworld", IniOptions{
		Multiline: &MultilineOptions{Indent: boolPtr(true), Continuation: &noBackslash},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "tab-indent", result, map[string]any{"a": "hello world"})

	// Indent continuation in section
	result, err = Parse("[s]\na = hello\n    world", IniOptions{
		Multiline: &MultilineOptions{Indent: boolPtr(true), Continuation: &noBackslash},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "section-indent", result, map[string]any{"s": map[string]any{"a": "hello world"}})
}

func TestMultilineBothModes(t *testing.T) {
	// Both continuation char and indent enabled
	bs := "\\"
	result, err := Parse("a = hello \\\nworld", IniOptions{
		Multiline: &MultilineOptions{Continuation: &bs, Indent: boolPtr(true)},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "both-bs", result, map[string]any{"a": "hello world"})

	result, err = Parse("a = hello\n    world", IniOptions{
		Multiline: &MultilineOptions{Continuation: &bs, Indent: boolPtr(true)},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "both-indent", result, map[string]any{"a": "hello world"})
}

func TestMultilineEscapes(t *testing.T) {
	// Multiline with inline comments active and backslash escaping
	result, err := Parse("a = one\\; two \\\nthree", IniOptions{
		Multiline: &MultilineOptions{},
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Backslash: boolPtr(true)},
			},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "esc-semi", result, map[string]any{"a": "one; two three"})

	result, err = Parse("a = one\\# two \\\nthree", IniOptions{
		Multiline: &MultilineOptions{},
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Backslash: boolPtr(true)},
			},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "esc-hash", result, map[string]any{"a": "one# two three"})
}

func TestMultilineNoInlineComments(t *testing.T) {
	// Multiline without inline comments: ; and # are literal
	result, err := Parse("a = one; two \\\nthree", IniOptions{
		Multiline: &MultilineOptions{},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "literal-semi", result, map[string]any{"a": "one; two three"})

	result, err = Parse("a = one# two \\\nthree", IniOptions{
		Multiline: &MultilineOptions{},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "literal-hash", result, map[string]any{"a": "one# two three"})
}

func TestSectionDuplicateMergeFull(t *testing.T) {
	// Default: merge keys from duplicate sections
	result, err := Parse("[a]\nx=1\ny=2\n[a]\nz=3")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "merge-basic", result, map[string]any{
		"a": map[string]any{"x": "1", "y": "2", "z": "3"},
	})

	// Duplicate key: last value wins
	result, err = Parse("[a]\nx=1\n[a]\nx=2")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "merge-dup-key", result, map[string]any{
		"a": map[string]any{"x": "2"},
	})

	// Nested duplicate sections merge
	result, err = Parse("[a.b]\nx=1\n[a.b]\ny=2")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "merge-nested", result, map[string]any{
		"a": map[string]any{"b": map[string]any{"x": "1", "y": "2"}},
	})

	// Intermediate path preserved when merging
	result, err = Parse("[a.b]\nx=1\n[a]\ny=2")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "merge-intermediate", result, map[string]any{
		"a": map[string]any{"b": map[string]any{"x": "1"}, "y": "2"},
	})
}

func TestSectionDuplicateMergeExplicit(t *testing.T) {
	result, err := Parse("[a]\nx=1\n[a]\ny=2", IniOptions{
		Section: &SectionOptions{Duplicate: "merge"},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "merge-explicit", result, map[string]any{
		"a": map[string]any{"x": "1", "y": "2"},
	})
}

func TestSectionDuplicateOverrideFull(t *testing.T) {
	opts := IniOptions{Section: &SectionOptions{Duplicate: "override"}}

	// Second occurrence replaces first
	result, err := Parse("[a]\nx=1\ny=2\n[a]\nz=3", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "override-basic", result, map[string]any{
		"a": map[string]any{"z": "3"},
	})

	// First occurrence works normally
	result, err = Parse("[a]\nx=1", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "override-single", result, map[string]any{
		"a": map[string]any{"x": "1"},
	})

	// Override clears subsections too
	result, err = Parse("[a.b]\nx=1\n[a]\ny=2\n[a]\nz=3", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "override-clears-sub", result, map[string]any{
		"a": map[string]any{"z": "3"},
	})

	// Non-duplicate sections unaffected
	result, err = Parse("[a]\nx=1\n[b]\ny=2", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "override-distinct", result, map[string]any{
		"a": map[string]any{"x": "1"}, "b": map[string]any{"y": "2"},
	})

	// Nested override
	result, err = Parse("[a.b]\nx=1\n[a.b]\ny=2", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "override-nested", result, map[string]any{
		"a": map[string]any{"b": map[string]any{"y": "2"}},
	})
}

func TestSectionDuplicateErrorFull(t *testing.T) {
	opts := IniOptions{Section: &SectionOptions{Duplicate: "error"}}

	// Single section: no error
	result, err := Parse("[a]\nx=1", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "error-single", result, map[string]any{
		"a": map[string]any{"x": "1"},
	})

	// Multiple distinct sections: no error
	result, err = Parse("[a]\nx=1\n[b]\ny=2", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "error-distinct", result, map[string]any{
		"a": map[string]any{"x": "1"}, "b": map[string]any{"y": "2"},
	})

	// Duplicate section: rejected (panic or error).
	expectDuplicateSectionError(t, func() (any, error) {
		return Parse("[a]\nx=1\n[a]\ny=2", opts)
	}, "duplicate section")

	// Duplicate nested section: rejected (panic or error).
	expectDuplicateSectionError(t, func() (any, error) {
		return Parse("[a.b]\nx=1\n[a.b]\ny=2", opts)
	}, "duplicate nested section")

	// Intermediate path is NOT a declared section
	result, err = Parse("[a.b]\nx=1\n[a]\ny=2", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "error-intermediate", result, map[string]any{
		"a": map[string]any{"b": map[string]any{"x": "1"}, "y": "2"},
	})
}

func TestInlineActiveBasic(t *testing.T) {
	opts := IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{Active: boolPtr(true)},
		},
	}

	result, err := Parse("a = hello ; comment", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "semi-inline", result, map[string]any{"a": "hello"})

	result, err = Parse("a = hello # comment", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "hash-inline", result, map[string]any{"a": "hello"})

	result, err = Parse("a = x;y", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "mid-semi", result, map[string]any{"a": "x"})

	result, err = Parse("a = value\nb = other", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "multi-vals", result, map[string]any{"a": "value", "b": "other"})
}

func TestInlineBackslashEscapeWithComment(t *testing.T) {
	opts := IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Backslash: boolPtr(true)},
			},
		},
	}

	// Escaped ; followed by unescaped ; comment
	result, err := Parse("a = x\\;y ; comment", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "esc-then-comment", result, map[string]any{"a": "x;y"})
}

func TestInlineBackslashEscapeDisabled(t *testing.T) {
	opts := IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Backslash: boolPtr(false)},
			},
		},
	}

	// \; → \; (backslash preserved, ; did not terminate)
	result, err := Parse("a = hello\\; world", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "bs-disabled-semi", result, map[string]any{"a": "hello\\; world"})

	// Unescaped ; still terminates
	result, err = Parse("a = hello ; comment", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "bs-disabled-unesc", result, map[string]any{"a": "hello"})
}

func TestInlineWhitespacePrefixFull(t *testing.T) {
	opts := IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Whitespace: boolPtr(true)},
			},
		},
	}

	// No whitespace before ; → literal
	result, err := Parse("a = x;y;z", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "no-ws-literal", result, map[string]any{"a": "x;y;z"})

	// Whitespace before ; → inline comment
	result, err = Parse("a = hello ;comment", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "ws-comment-semi", result, map[string]any{"a": "hello"})

	// Tab before ; → inline comment
	result, err = Parse("a = hello\t;comment", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "tab-comment-semi", result, map[string]any{"a": "hello"})

	// Same for #
	result, err = Parse("a = x#y", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "no-ws-hash", result, map[string]any{"a": "x#y"})

	result, err = Parse("a = hello #comment", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "ws-comment-hash", result, map[string]any{"a": "hello"})
}

func TestInlineWhitespacePrefixWithBackslash(t *testing.T) {
	opts := IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{
					Whitespace: boolPtr(true),
					Backslash:  boolPtr(true),
				},
			},
		},
	}

	// No whitespace: literal
	result, err := Parse("a = x;y", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "no-ws", result, map[string]any{"a": "x;y"})

	// Whitespace present: comment
	result, err = Parse("a = hello ;comment", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "ws-comment", result, map[string]any{"a": "hello"})

	// Backslash escape overrides whitespace: literal
	result, err = Parse("a = hello \\;not-a-comment", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "bs-overrides-ws", result, map[string]any{"a": "hello ;not-a-comment"})
}

func TestInlineWithSections(t *testing.T) {
	opts := IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{Active: boolPtr(true)},
		},
	}

	result, err := Parse("[s]\na = val ; comment\nb = other", opts)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "section-inline", result, map[string]any{
		"s": map[string]any{"a": "val", "b": "other"},
	})
}

func TestLineCommentsAlwaysWork(t *testing.T) {
	input := "; line comment\n# hash comment\na = 1"

	// Inline off
	result, err := Parse(input)
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "line-off", result, map[string]any{"a": "1"})

	// Inline on
	result, err = Parse(input, IniOptions{
		Comment: &CommentOptions{
			Inline: &InlineCommentOptions{Active: boolPtr(true)},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "line-on", result, map[string]any{"a": "1"})
}

func TestDefaultNumbersAreStrings(t *testing.T) {
	result, err := Parse("a=1")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "int-str", result, map[string]any{"a": "1"})

	result, err = Parse("a=2.5")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "float-str", result, map[string]any{"a": "2.5"})

	result, err = Parse("a=-3")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "neg-str", result, map[string]any{"a": "-3"})

	result, err = Parse("a=0xFF")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "hex-str", result, map[string]any{"a": "0xFF"})
}

func TestEscapedDotsInSections(t *testing.T) {
	// Escaped dots in section names should be literal
	result, err := Parse("[x\\.y\\.z]\nx.y.z = xyz")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "escaped-dots", result, map[string]any{
		"x.y.z": map[string]any{"x.y.z": "xyz"},
	})

	// Nested escaped dots
	result, err = Parse("[x\\.y\\.z.a\\.b\\.c]\na.b.c = abc")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "nested-escaped-dots", result, map[string]any{
		"x.y.z": map[string]any{
			"a.b.c": map[string]any{"a.b.c": "abc"},
		},
	})
}

func TestArrayWithExistingKey(t *testing.T) {
	// Converting a key to an array with [] syntax when key already exists
	result, err := Parse("ar[]=one\nar[]=three\nar   = this is included")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "arr-include", result, map[string]any{
		"ar": []any{"one", "three", "this is included"},
	})
}

func TestKeyOverwrite(t *testing.T) {
	// Later value overwrites earlier
	result, err := Parse("br = cold\nbr = warm")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "overwrite", result, map[string]any{"br": "warm"})
}

func TestNestedSectionWithoutMiddleParent(t *testing.T) {
	// [a.b.c] creates intermediate [a] and [a.b] without declaring them
	result, err := Parse("[a]\nav = a val\n[a.b.c]\ne = 1\nj = 2")
	if err != nil {
		t.Fatal(err)
	}
	assert(t, "nested-no-middle", result, map[string]any{
		"a": map[string]any{
			"av": "a val",
			"b": map[string]any{
				"c": map[string]any{
					"e": "1",
					"j": "2",
				},
			},
		},
	})
}
