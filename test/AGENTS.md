# Agents Guide — shared spec fixtures

`spec/*.tsv` holds the cross-runtime conformance fixtures. Both runtimes run
the same files, so a change here affects TypeScript and Go together — edit
with that in mind.

## Format

Tab-separated, one case per line, with a header row naming the columns
(`input` `expected`). Blank lines are skipped.

| Column | Meaning |
|---|---|
| `input` | INI source. |
| `expected` | The parse result as JSON, or an `ERROR:`-prefixed cell for input that must be rejected. |

### What the loaders actually do — mind these

- **Escapes.** `\n`, `\r`, `\r\n` and `\t` are decoded. A literal `\\` is
  **not** — there is no way to write a single backslash, and a lone trailing
  `\` survives verbatim. Both runtimes now decode *every* column; Go used to
  decode only `input`, so a `\n` in an `expected` cell meant two different
  things in the two runtimes.
- **`ERROR:<code>` is a checked assertion.** The code is lowercased and its
  `_` turned into spaces, and the reported error message must contain the
  result — `ERROR:duplicate_section` requires an error saying "duplicate
  section". Both runtimes apply the identical rule (`errorCodeMatches`), and
  Go converts a panicking rejection into an inspectable error rather than
  swallowing it. It used to be a bare marker nobody read: Go accepted any
  error or panic at all, and TypeScript matched a hardcoded
  `/Duplicate section/` whatever the cell said.
- **A line with no tab is a failure in both runtimes**, named by file and
  line. It used to be silently dropped by Go and to crash TypeScript on
  `undefined.startsWith`. Every line must be blank or contain a tab —
  including `#`-leading ones, which are *not* comments here.
- **A fixture that loads zero cases fails.** An emptied, renamed or
  header-only `.tsv` used to pass green in both runtimes, because the loop
  over its rows simply never ran.

The docs should follow the code, not lead it.

### Known-failing fixtures

`eof-trailing-backslash.tsv` is **expected to fail in TypeScript** and pass
in Go. A value ending in a backslash at end-of-input with no trailing
newline (`x=a\`) is rejected by TypeScript (`[jsonic/invalid_text]`) and
accepted by Go; the npm/ini reference implementation agrees with Go, so the
expected values are the oracle's and TypeScript is the outlier — the "Go has
exposed a genuine TS defect" carve-out below. Found by differential fuzzing
of the two runtimes. Do **not** flip it to `ERROR` to get TypeScript green:
that enshrines the defect.

## The third-party conformance corpus

Separate from these fixtures, `ts/test/conformance.test.ts` and
`go/conformance_test.go` run a corpus of third-party INI documents from four
upstream implementations, pinned to commit SHAs and **never committed** —
fetched into the gitignored `test/corpus/` by `scripts/fetch-ini-corpus.sh`.
INI has no authoritative conformance suite; read the header of
`scripts/build-ini-corpus.js` for what that corpus is, where every label and
expected value comes from, and what the numbers do and do not prove.

Those tests **fail rather than skip** when the corpus is absent, and they are
expected to be red: they measure the gap between this parser and the
reference, and are not a target to be tuned green.

## Who runs what

- TypeScript: `ts/test/ini-tsv.test.ts` (`loadTSV`).
- Go: `go/ini_tsv_test.go` (`loadTSV` / `runIniTSV`).

Both name the same files. A fixture only one runtime runs proves nothing, so
wire a new file into both.

## Rules

- Prefer adding a fixture here over a one-off in-language assertion when a
  case is expressible as input → output. That is what keeps the two runtimes
  honest against each other.
- TypeScript is canonical. If the two runtimes disagree, the TS behaviour is
  the expected value — unless Go has exposed a genuine TS defect, or the
  difference is one of the intentional divergences the root `AGENTS.md`
  records, which stay out of these shared fixtures.
- A new fixture must pass in BOTH runtimes before it counts:
  `go test ./...` from `go/`, and **`npm run build && npm test`** from `ts/`.
  Plain `npm test` runs the previously compiled `dist-test/`, so it can pass
  without ever loading a newly added fixture.
