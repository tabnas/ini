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

		// No MatchError hook: an ERROR:<code> cell is compared against the
		// error's Code by the shared runner's default, which is the contract
		// this package wants. Both codes the fixtures pin —
		// duplicate_section and unterminated_section — are declared in
		// ini-grammar.jsonic and raised by both runtimes, so nothing has to
		// be resolved through message wording (which is deliberately not a
		// cross-runtime contract).
		support.Runner{
			Parse: func(input string) (any, error) {
				if hasOpts {
					return parseRecovering(input, opts)
				}
				return parseRecovering(input)
			},
		}.Spec(t, spec)
	}
}

// parseRecovering turns a recovered panic into an error. No parse path
// panics deliberately any more — the duplicate-section raise site now
// publishes a coded bad token — but the engine recovers only what runs
// inside its own parse loop, and a panic escaping here would take down the
// whole run rather than fail the row that caused it.
//
// A recovered value that is already an error is passed through unchanged:
// wrapping it in fmt.Errorf would strip a *TabnasError down to a plain
// error, and with it the Code the runner compares against ERROR:<code>.
func parseRecovering(input string, opts ...IniOptions) (got any, err error) {
	defer func() {
		if r := recover(); r != nil {
			if e, ok := r.(error); ok {
				got, err = nil, e
				return
			}
			got, err = nil, fmt.Errorf("%v", r)
		}
	}()
	return Parse(input, opts...)
}
