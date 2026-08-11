package tabnasini

// ini_tsv_test.go — cross-runtime conformance, driven by the shared
// `test/spec/*.tsv` fixtures at the repo root (see ../test/AGENTS.md).
//
// The fixture loader, the escape codec, the ERROR: contract and the row
// loop all come from github.com/tabnas/support/go, whose TypeScript half
// ts/test/ini-tsv.test.ts uses to run the SAME files — so the two
// implementations cannot drift without one of them going red, and neither
// can the two loaders. (They had: this repo's loader decoded escapes in
// EVERY column, including the JSON `expected` one.)
//
// What is left here is only what is specific to ini: which options each
// fixture is parsed with, and the messages a rejection must carry.

import (
	"fmt"
	"strings"
	"testing"

	support "github.com/tabnas/support/go"
)

// ini's fixtures carry no `opts` column: a whole file is parsed with one
// option set, named here. A fixture with no entry gets the defaults, so
// adding one runs it in both runtimes without editing a list.
//
// Keep in sync with OPTIONS in ts/test/ini-tsv.test.ts.
func tsvOptions() map[string]IniOptions {
	inlineActive := func() *CommentOptions {
		return &CommentOptions{
			Inline: &InlineCommentOptions{Active: boolPtr(true)},
		}
	}
	noContinuation := ""
	backslash := "\\"

	return map[string]IniOptions{
		"inline-comments-active": {Comment: inlineActive()},
		"inline-comments-custom-chars": {Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Chars:  []string{";"},
			},
		}},
		"inline-comments-backslash": {Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Backslash: boolPtr(true)},
			},
		}},
		"inline-comments-backslash-disabled": {Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Backslash: boolPtr(false)},
			},
		}},
		"inline-comments-whitespace": {Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{Whitespace: boolPtr(true)},
			},
		}},
		"inline-comments-whitespace-backslash": {Comment: &CommentOptions{
			Inline: &InlineCommentOptions{
				Active: boolPtr(true),
				Escape: &InlineEscapeOptions{
					Whitespace: boolPtr(true),
					Backslash:  boolPtr(true),
				},
			},
		}},
		"inline-comments-with-sections":   {Comment: inlineActive()},
		"value-comment-char-start-inline": {Comment: inlineActive()},
		"sections-duplicate-override":     {Section: &SectionOptions{Duplicate: "override"}},
		"sections-duplicate-error":        {Section: &SectionOptions{Duplicate: "error"}},
		"multiline-backslash":             {Multiline: &MultilineOptions{}},
		"multiline-no-inline":             {Multiline: &MultilineOptions{}},
		"multiline-indent": {Multiline: &MultilineOptions{
			Indent:       boolPtr(true),
			Continuation: &noContinuation,
		}},
		"multiline-both": {Multiline: &MultilineOptions{
			Continuation: &backslash,
			Indent:       boolPtr(true),
		}},
		"multiline-with-inline": {
			Multiline: &MultilineOptions{},
			Comment:   inlineActive(),
		},
		"multiline-escapes": {
			Multiline: &MultilineOptions{},
			Comment: &CommentOptions{
				Inline: &InlineCommentOptions{
					Active: boolPtr(true),
					Escape: &InlineEscapeOptions{Backslash: boolPtr(true)},
				},
			},
		},
	}
}

// ini's `ERROR:<code>` cells are SYMBOLIC — they name the rejection this
// repo means, not the code the engine answers, which for most of them is a
// generic one. A code listed here additionally pins the MESSAGE the parser
// must produce; one that is not asserts rejection only, because
// engine-generated wording differs between the two runtimes.
//
// This is why the runner gets a MatchError hook rather than using its
// default code comparison.
//
// Keep in sync with ERROR_MESSAGES in ts/test/ini-tsv.test.ts.
var tsvErrorMessages = map[string]string{
	"duplicate_section": "Duplicate section",
}

// TestSpec runs every fixture in the spec directory, each with its own
// options. LoadSpecDir rejects an empty directory and the runner rejects an
// empty fixture, so neither can pass by running nothing.
func TestSpec(t *testing.T) {
	dir, err := support.FindSpecDir("")
	if err != nil {
		t.Fatal(err)
	}

	// MinCols 2 keeps a guard this repo already had: a line with no tab is
	// a failure, named by file and line, not a row silently dropped. (A
	// #-leading line with no tab is a comment to the shared loader and is
	// skipped before that check — there are none in these fixtures.)
	specs, err := support.LoadSpecDir(dir, &support.Options{MinCols: 2})
	if err != nil {
		t.Fatal(err)
	}

	options := tsvOptions()

	for _, spec := range specs {
		name := strings.TrimSuffix(spec.Name, ".tsv")
		opts, hasOpts := options[name]

		support.Runner{
			Parse: func(input string) (any, error) {
				if hasOpts {
					return parseRecovering(input, opts)
				}
				return parseRecovering(input)
			},

			MatchError: func(err error, want string, _ *support.Row) bool {
				pattern, pinned := tsvErrorMessages[want]
				if !pinned {
					// Symbolic: rejection is all that is asserted.
					return true
				}
				return strings.Contains(err.Error(), pattern)
			},
		}.Spec(t, spec)
	}
}

// parseRecovering turns a recovered state-action panic into an error. The
// tabnas engine recovers most of them itself, but not all, and a panic
// escaping the parse hook would take down the whole run rather than fail
// the row that caused it.
func parseRecovering(input string, opts ...IniOptions) (got any, err error) {
	defer func() {
		if r := recover(); r != nil {
			got, err = nil, fmt.Errorf("%v", r)
		}
	}()
	return Parse(input, opts...)
}
