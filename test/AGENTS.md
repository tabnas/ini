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

The two loaders are **not** symmetric, and the format is looser than it
looks. Until that is fixed, write fixtures to the intersection:

- **Escapes.** `\n`, `\r`, `\r\n` and `\t` are decoded. A literal `\\` is
  **not** — there is no way to write a single backslash. TypeScript decodes
  *every* column, Go decodes only `input`, so a `\n` in an `expected` cell
  means different things in the two runtimes. Keep escapes out of `expected`.
- **`ERROR:` is a rejection marker, not a checked code.** Neither runner
  verifies the text after the colon: Go accepts any error or panic, and
  TypeScript matches `/Duplicate section/` whatever the cell says. Treat the
  suffix as a comment for the reader.
- **A line with no tab is dropped by Go and breaks TypeScript**, which then
  calls `.startsWith` on an undefined column. Every line must be blank or
  contain a tab — including `#`-leading ones, which are *not* comments here.

Fixing any of the three means changing both loaders together; the docs
should follow the code, not lead it.

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
