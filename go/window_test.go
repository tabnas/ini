// Copyright (c) 2026 Richard Rodger and other contributors, MIT License

package tabnasini

// window_test.go — a parse from a reader (Tabnas.ParseReader, admin ADR-30
// and ADR-33).
//
// MakeJsonic builds the parser whole, jsonic's base with the ini plugin
// installed, and declares lex.window on it, so that a reader is read a
// window at a time. The string check and hoover's blocks read the source
// only through the window, a scan that reaches the window's end filling it
// and reading on; the engine cuts again a quoted key the string matcher
// declines at the window's end (tabnas/parser#326, from parser v0.12.17);
// and the multiline value matcher ends a value the window ends inside at
// the window's end, a token the engine cuts again. These tests hold it to
// that: MakeJsonic declares the window and the plugin does not; every row
// of the shared fixtures, under its file's options, and every document of
// the conformance corpus, under the default options and the option sets
// that install the multiline matcher and the inline comments, parses the
// same from a reader, in pieces of 1, 3, 64 and 4096 bytes, as from its
// text; and so do long values and a long document.
//
// Go's window sizes are fixed at 64 KiB (the engine's small-window hook is
// unexported), and before each token the window reaches 64 KiB past the
// cursor, so a scan meets the window's edge only where it reads further
// than that. TestValuesCutAtEveryPlace puts what a scan decides on (a
// closing quote and what follows it, a quoted key's closing quote, a
// continuation, an indented next line, a CRLF, an escape, an inline
// comment) just past 64 KiB into the value or key, and moves the line
// along the source a byte at a time, read in pieces of 64 bytes, so that
// the window's edge falls before, inside and after it.

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	jsonic "github.com/tabnas/jsonic/go"
	tabnas "github.com/tabnas/parser/go"
	support "github.com/tabnas/support/go"
)

// readerMargin is how far past the cursor the window reaches before each
// token is cut: a scan that reads further meets the window's edge.
const readerMargin = 64 << 10

// pieces is a reader that hands its bytes over at most size at a time.
type pieces struct {
	text []byte
	at   int
	size int
}

func (p *pieces) Read(buf []byte) (int, error) {
	if p.at == len(p.text) {
		return 0, io.EOF
	}
	n := min(p.size, len(buf), len(p.text)-p.at)
	copy(buf, p.text[p.at:p.at+n])
	p.at += n
	return n, nil
}

// outcome is what a parse shows: its value, or its error's code, position,
// row, column and token text. An error's excerpt is left out: a parse from
// a reader shows a long line only as far back as the window reaches
// (tabnas/parser DIVERGENCE.md), and that is the one difference allowed.
type outcome struct {
	value any
	err   string
}

func outcomeOf(v any, err error) outcome {
	if err == nil {
		return outcome{value: v}
	}
	var te *tabnas.TabnasError
	if errors.As(err, &te) {
		return outcome{err: fmt.Sprintf("%s at %d %d:%d %q", te.Code, te.Pos, te.Row, te.Col, te.Src)}
	}
	return outcome{err: "not a TabnasError: " + err.Error()}
}

func (o outcome) String() string {
	if o.err != "" {
		return "error " + o.err
	}
	s := fmt.Sprintf("value %v", o.value)
	if len(s) > 300 {
		s = s[:300] + "..."
	}
	return s
}

func (o outcome) same(p outcome) bool {
	return o.err == p.err && reflect.DeepEqual(o.value, p.value)
}

// sameFromReader parses text whole and from a reader in pieces of each
// size, and reports where the two differ. It returns the number of reader
// parses compared.
func sameFromReader(t *testing.T, j *tabnas.Tabnas, at, text string, sizes []int) int {
	t.Helper()
	whole := outcomeOf(j.Parse(text))
	for _, size := range sizes {
		read := outcomeOf(j.ParseReader(&pieces{text: []byte(text), size: size}))
		if !read.same(whole) {
			show := text
			if len(show) > 80 {
				show = show[:80]
			}
			t.Errorf("%s: %q in pieces of %d:\n  whole:  %s\n  reader: %s", at, show, size, whole, read)
		}
	}
	return len(sizes)
}

// readerSizes are the pieces a document is handed over in: down to a byte,
// and for one over 8 KiB also in pieces longer than the window's margin.
// From parser v0.12.18 a read costs the Go window its own length however
// long the window is (tabnas/parser#321), so a long document is read a
// byte at a time too.
func readerSizes(text string) []int {
	if len(text) > 8<<10 {
		return []int{1, 3, 64, 4096, readerMargin + 3}
	}
	return []int{1, 3, 64, 4096}
}

// optionSets are the option sets the corpus and the long values are read
// under: the defaults, inline comments (hoover's value block ending at a
// comment character), the multiline matcher with continuation and indent,
// and the multiline matcher that inline comments preceded by whitespace
// install.
func optionSets() []struct {
	name string
	opts IniOptions
} {
	options := tsvOptions()
	return []struct {
		name string
		opts IniOptions
	}{
		{"default", IniOptions{}},
		{"inline-comments-active", options["inline-comments-active"]},
		{"multiline-both", options["multiline-both"]},
		{"inline-comments-whitespace-backslash", options["inline-comments-whitespace-backslash"]},
	}
}

// TestMakeJsonicDeclaresTheWindow: MakeJsonic declares lex.window; the
// plugin alone, which a host may install beneath matchers of its own,
// declares nothing.
func TestMakeJsonicDeclaresTheWindow(t *testing.T) {
	if !MakeJsonic().Config().Window {
		t.Error("MakeJsonic does not declare lex.window")
	}
	var o IniOptions
	// jsonic's base declares the window itself (tabnas/jsonic#121), so the
	// plugin goes on that base with the window turned off.
	no := false
	j := jsonic.Make(tabnas.Options{Lex: &tabnas.LexOptions{Window: &no}})
	if err := j.Use(iniPlugin, optionsToMap(&o, resolve(&o))); err != nil {
		t.Fatal(err)
	}
	if j.Config().Window {
		t.Error("the ini plugin declares lex.window")
	}
}

// TestFixturesFromAReader: every fixture row parses the same from a reader
// as from its text, under MakeJsonic with its file's options.
func TestFixturesFromAReader(t *testing.T) {
	dir, err := support.FindSpecDir("")
	if err != nil {
		t.Fatal(err)
	}
	specs, err := support.LoadSpecDir(dir, &support.Options{MinCols: 2})
	if err != nil {
		t.Fatal(err)
	}
	options := tsvOptions()
	compared := 0
	for n, spec := range specs {
		var opts []IniOptions
		if o, ok := options[strings.TrimSuffix(spec.Name, ".tsv")]; ok {
			opts = []IniOptions{o}
		}
		j := MakeJsonic(opts...)
		for _, row := range spec.Rows {
			input := row.UnescNamed("input")
			compared += sameFromReader(t, j, fmt.Sprintf("%s:%d", spec.Name, row.Line), input, readerSizes(input))
		}
		if (n+1)%10 == 0 || n+1 == len(specs) {
			t.Logf("reader fixtures: %d of %d files (%d%%), %d reader parses", n+1, len(specs), (n+1)*100/len(specs), compared)
		}
	}
	if compared < 800 {
		t.Errorf("only %d reader parses", compared)
	}
}

// TestCorpusFromAReader: every document of the conformance corpus, under
// each option set, parses the same from a reader as from its text.
func TestCorpusFromAReader(t *testing.T) {
	raw, err := os.ReadFile(filepath.Join("..", "test", "corpus", "ini-corpus.json"))
	if err != nil {
		t.Fatal(err)
	}
	var corpus struct {
		Cases []struct {
			Name   string `json:"name"`
			Source string `json:"source"`
		} `json:"cases"`
	}
	if err := json.Unmarshal(raw, &corpus); err != nil {
		t.Fatal(err)
	}
	if len(corpus.Cases) != 36 {
		t.Errorf("%d corpus documents, want 36", len(corpus.Cases))
	}
	compared := 0
	sets := optionSets()
	for n, set := range sets {
		j := MakeJsonic(set.opts)
		for _, c := range corpus.Cases {
			compared += sameFromReader(t, j, c.Name+" ["+set.name+"]", c.Source, readerSizes(c.Source))
		}
		t.Logf("reader corpus: %d of %d option sets (%d%%), %d reader parses", n+1, len(sets), (n+1)*100/len(sets), compared)
	}
	if compared < 400 {
		t.Errorf("only %d reader parses", compared)
	}
}

// TestValuesCutAtEveryPlace: what a value's scan decides on, just past the
// window's 64 KiB, with the value moved along the source a byte at a time
// and the source read in pieces of 64 bytes, so that the window's edge
// falls before, inside and after it.
func TestValuesCutAtEveryPlace(t *testing.T) {
	at := readerMargin + 20
	fill := func(c string, n int) string { return strings.Repeat(c, n) }
	options := tsvOptions()
	cases := []struct {
		opts string
		doc  string
	}{
		// The string check: a closing quote, then the end of the line,
		// trailing text, or an inline comment.
		{"default", "a = \"" + fill("q", at-2) + "\"\nb = 1\n"},
		{"default", "a = \"" + fill("q", at-2) + "\"x\nb = 1\n"},
		{"default", "a = \"" + fill("q", at-2) + "\"\r\nb = 1\n"},
		{"inline-comments-active", "a = \"" + fill("q", at-2) + "\" ; note\nb = 1\n"},
		// A quoted key, which the string matcher reads, with the end of
		// the source near (tabnas/parser#326).
		{"default", "\"" + fill("k", at-1) + "\"\nb = 1\n"},
		{"default", "\"" + fill("k", at-1) + "\" = 1\nb = 1\n"},
		// hoover's value block: a newline, a CRLF, an escaped and a bare
		// comment character.
		{"default", "a = " + fill("v", at) + "\nb = 1\n"},
		{"default", "a = " + fill("v", at) + "\r\nb = 1\n"},
		{"inline-comments-active", "a = " + fill("v", at) + "\\; x ; note\nb = 1\n"},
		// The multiline matcher: a continuation, an indented next line, a
		// CRLF, an escape, and an inline comment after white space.
		{"multiline-both", "a = " + fill("v", at) + "\\\n  two\nb = 1\n"},
		{"multiline-both", "a = " + fill("v", at) + "\\\r\n  two\nb = 1\n"},
		{"multiline-both", "a = " + fill("v", at) + "\n  two\nb = 1\n"},
		{"multiline-both", "a = " + fill("v", at) + "\r\n  two\nb = 1\n"},
		{"multiline-both", "a = " + fill("v", at) + "\r\nb = 1\n"},
		{"inline-comments-whitespace-backslash", "a = " + fill("v", at) + "\\; x ; note\nb = 1\n"},
	}
	compared := 0
	for n, c := range cases {
		opts, ok := options[c.opts]
		if !ok && c.opts != "default" {
			t.Fatalf("no option set %s", c.opts)
		}
		j := MakeJsonic(opts)
		for shift := 0; shift < 64; shift++ {
			doc := "; " + fill("p", shift) + "\n" + c.doc
			compared += sameFromReader(t, j, fmt.Sprintf("case %d [%s], shifted %d", n, c.opts, shift), doc, []int{64})
		}
		t.Logf("reader cut at every place: %d of %d cases (%d%%)", n+1, len(cases), (n+1)*100/len(cases))
	}
	if compared < 700 {
		t.Errorf("only %d reader parses", compared)
	}
}

// TestLongDocumentFromAReader: a document of over a megabyte, its values,
// lines and comments long and short, the long ones longer than the window,
// parses the same from a reader as from its text under each option set,
// and so does the same document with an unterminated section header at its
// end, which a parse from a reader places where the whole text does after
// the window has moved on many times.
func TestLongDocumentFromAReader(t *testing.T) {
	long := readerMargin + 2<<10
	var b strings.Builder
	b.WriteString("; settings\nname = top\n")
	for i := 0; i < 4000; i++ {
		fmt.Fprintf(&b, "[section%d.sub]\nkey%d = value %d \u00e9\nquoted = \"q;%d\"\n", i, i, i, i)
		fmt.Fprintf(&b, "path = a\\;b\\\\c ; note %d\nlist[] = %d\nflag\n", i, i)
		fmt.Fprintf(&b, "wrapped = one \\\n  two %d\n  indented %d\n# comment %d\n", i, i, i)
		if i == 1000 || i == 3000 {
			fmt.Fprintf(&b, "longvalue = %s\n", strings.Repeat("v", long))
			fmt.Fprintf(&b, "longquoted = \"%s\"\n", strings.Repeat("q", long))
			fmt.Fprintf(&b, "%s = 1\n", strings.Repeat("k", long))
			fmt.Fprintf(&b, "; %s\n", strings.Repeat("c", long))
			fmt.Fprintf(&b, "spaced = \"q\"%s\n", strings.Repeat(" ", long))
		}
	}
	body := b.String()
	text := body + "last = 1\n"
	broken := body + "[open\n"
	if len(text) < 1<<20 {
		t.Fatalf("the document is %d bytes", len(text))
	}
	for n, set := range optionSets() {
		j := MakeJsonic(set.opts)
		if _, err := j.Parse(text); err != nil {
			t.Fatalf("[%s] the long document does not parse: %v", set.name, err)
		}
		if _, err := j.Parse(broken); err == nil {
			t.Fatalf("[%s] the broken document parses", set.name)
		}
		sameFromReader(t, j, "["+set.name+"] the long document", text, []int{64, 4096, readerMargin + 3})
		sameFromReader(t, j, "["+set.name+"] the long document with an open section", broken, []int{4096, readerMargin + 3})
		t.Logf("reader long document: %d of 4 option sets (%d%%), %d bytes", n+1, (n+1)*25, len(text))
	}
}
