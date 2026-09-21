# Agents Guide: rs/

The Rust port of the canonical TypeScript in [`../ts`](../ts). Read
[`../AGENTS.md`](../AGENTS.md) first: it holds the cross-runtime rules,
and this file only covers what is specific to this crate.

## Layout

| Path | |
|---|---|
| `src/lib.rs` | the whole port: the typed options, the embedded grammar text, every closure the grammar names, the hoover blocks, the lexer hooks, the custom value matcher, the `val` rule, `ini`, `plugin`, `make`, `make_with`, `parse`, `parse_with` |
| `tests/parity_test.rs` | every shared `../test/spec/*.tsv` fixture through `tabnas_support::Runner`, one runner per file, with the per-file option table |
| `tests/ini_test.rs` | in-language behaviour: the Rust twin of `go/ini_test.go`, plus the construction API, the threading contract and hostile input |
| `tests/conformance_test.rs` | the third-party corpus in `../test/corpus/ini-corpus.json`, with the same divergence lists the other two runtimes carry |
| `tests/grammar_test.rs` | the embedded grammar against `../ini-grammar.jsonic`, and against the TypeScript and Go embeds |
| `tests/perf_test.rs` | `parse` reuses its instance, building one really is dear, and many sections stay near linear |
| `tests/version_test.rs` | Cargo.toml == `VERSION` == ts/package.json == the Go `const VERSION` |
| `tests/common/mod.rs` | the per-fixture option table and the two conversions the runner needs |
| `README.md` | the crate front page, prose-gated; its `rust` fences are doctests of this crate |

Crate `tabnas-ini`, library `tabnas_ini`. The engine (`tabnas`), the
relaxed-JSON base (`tabnas-jsonic`), the block lexer (`tabnas-hoover`)
and the fixture runner (`tabnas-support`, dev only) are **path
dependencies on sibling checkouts**. jsonic takes the JSON core
(`tabnas-json`) the same way, so that checkout is needed too. None is
published, so there is no registry version to fall back on.

```bash
cargo build --all-targets
cargo test --all-targets && cargo test --doc
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt
```

`make test-rs` from the repository root is the fast loop; `ci/rust/run.sh`
is the full gate and adds `fmt --check`, the lockfile check and the MSRV
pin.

## How the grammar is layered

`ini` does, in order:

1. Refuses an instance with no `val` rule. jsonic supplies `val`, `map`
   and `pair`, and hoover refuses a bare engine, so installing over
   nothing would leave half a grammar.
2. Registers the in-value probe and the four lexer `check` hooks.
3. Registers the custom value matcher factory.
4. Installs the three hoover blocks: `endofline` (`#HV`), `key` (`#HK`)
   and `divekey` (`#DK`).
5. Registers every closure the grammar names.
6. Parses the embedded grammar text with a stock jsonic instance and
   installs the result.
7. Snapshots the string configuration the string check reads, which the
   document above is what sets.
8. Rewrites the `val` rule, removes `list` and `elem`, and installs the
   depth budget.

Order is load bearing at every step. The hoover blocks must exist before
the grammar document, because the document's `val` alternates sit in
front of hoover's. The refs must exist before the document, because the
document is what looks them up. The budget goes last, because a grammar
document's options pass is not required to preserve one.

## The in-value probe

Four `check` hooks in the canonical port ask the same question: where is
the parser right now? TypeScript reads `lex.ctx.rule` and Go reads
`lex.Ctx.Rule`. **The Rust engine hands a check only the live `Lexer`,
which carries no rule**, so the answer is recorded one step earlier: a
custom matcher in the band below every built-in family (order 0.5e6,
where the first built-in band is 1e6) receives the rule, writes two flags
into a thread-local, and matches nothing. Every check that runs
afterwards, in the same `next_token` call on the same thread, reads them
back.

Thread-local rather than instance state, because a `Tabnas` is shared
across threads by `parse` and because the flags are written and read
inside one lexer call. Do not move them onto the instance.

Two flags, not one: `@line-check` asks only whether the rule is a `val`,
where the other three ask the fuller question the TypeScript `inValue`
asks (a `val` in its open state whose parent is a `pair` or an `elem`).

## Why the section tree is taken apart and put back together

A container here is a `Value`, which is an `Arc`, not a reference. The
canonical ports hold a live reference to the section object and mutate it
in place; this port cannot, so `open_section` walks the dive path down
from the document root and `close_section` writes the finished section
back up. Three rules about that code:

- **Iterative, never recursive.** A section header thousands of segments
  deep aborted the process through the call stack.
- **Take, do not clone.** `object_take` replaces a slot's value with
  nothing and keeps the key in place, so the container has ONE owner
  while it is written to. Reading with `object_get` (which clones) leaves
  two owners, and the next write copies the whole map: a document of
  5,000 sections took 5 seconds instead of 0.6.
- **The key keeps its position.** A remove-then-insert would move a
  section to the end of its parent every time it was reopened.

The same rule governs the declared-section set and the `table` rule's
`child_node`. `table_before_close` drops its copy of a child node that
shares its own cell, which is what the canonical `Object.assign(x, x)`
costs nothing to do.

## Per-parse state lives on the context

The canonical ports keep `declaredSections` in a closure, which is
instance state shared by every parse. `Tabnas` is `Send + Sync` and
`parse` shares one instance across threads, so here it lives in
`context.u` under `ini_declared`. `concurrent_parses_do_not_share_declared_sections`
in `ini_test.rs` fails if it moves back.

## `dive` on a table means the NEXT table

A table reads its section path from `r.prev.u.dive`, so `dive` on a
table's `u` bag means "the header that opened the table after this one",
not "the section this table is writing into". Recording the path under
that name would make every table inherit its predecessor's section. The
path this table is writing into is `ini_path`; keep the two apart.

## A rule cannot write its parent's `u` bag

`rule.parent_rule` is an immutable snapshot, so the canonical
`r.parent.u.dive.push(...)`, which grows an array the parent already
holds, has no counterpart. Each level reads its CHILD's bag instead:
`@dive-push` builds its own path from its parent's, `@dive-bc` copies the
completed child's path up, and `@table-close-dive` does the same at the
table. The Go port added `@dive-bc` for the same reason, a slice being a
value there.

The array a `k[] = v` pair grows is read back off the parent's NODE for
the same reason, rather than out of the parent's `u` bag.

## The fixed-token chain needs no write-back

A value that starts with `[`, `]`, `=` or `.` replaces the `val` rule
once per leading fixed token, and the canonical port walks that chain
writing the accumulated text into EVERY link, because the pair rule reads
its child node from the first one. This engine's parent takes the LAST
rule of a replacement chain as its child, so only the current rule's node
is set. Writing into the earlier links would be worse than redundant:
those links never reach their close state, so they still share the pair's
node cell, and the write replaced the enclosing map with a string.

## The depth budget is a crash fix

`ini` ends by installing a parse budget that refuses nesting past
`DEPTH_LIMIT` (127) with the engine's `cancel` code, counting `map`,
`list` and `dive` rules alike. jsonic installs the same check over `map`
and `list` only, which leaves a section header, the one place an INI
document nests, unbounded. The engine parses iteratively, but displaying,
converting or dropping a `Value` walks the tree with the call stack, and
a header ten thousand segments deep aborted the process rather than
erroring. TypeScript and Go have no limit, so the refusal is a recorded
divergence (`../DIVERGENCE.md`) that must never reach a shared fixture.

## The error alternate with no tokens

`table.open`'s duplicate-section alternate matches zero tokens, and the
engine fills its lookahead buffer to the length the alternate under test
asks for, so `context.t0()` is empty exactly where TypeScript and Go have
a positionless token in it. `alt_err_token` therefore ends in a fresh
NOTOKEN sentinel, which is the Go port's last fallback. The rendered
diagnostic is byte for byte the one the other two runtimes render, caret
and all; a fallback to the last consumed token would have pointed
somewhere else.

## The grammar is embedded, and embedded as TEXT

`../ini-grammar.jsonic` is the source of truth for every runtime, and
`ts/embed-grammar.js` copies it verbatim into `ts/src/ini.ts`,
`go/ini.go` and `src/lib.rs`. All three parse that text with their own
jsonic at load time: one grammar, three readers. Never hand-edit between
the `BEGIN/END EMBEDDED` markers; run `npm run embed` from `ts/`.
`grammar_test.rs` compares the embedded block against the file and
against the other two embeds.

Two adjustments are made to the parsed document before it is installed,
and both mirror what the other ports do in code: the `QUOTE_CHARS`
placeholder becomes the real quote characters, and the three `check`
hooks the canonical port installs through `options.config.modify`
callbacks are named as ordinary serialized `check` references. A third is
this port's alone: jsonic parses every number as a double, so `b: 1`
arrives as `1.0`, which the engine's integer fields refuse. `integralize`
rounds whole doubles back before the document is installed.

## The docs are gated

`README.md` is in the published set: no em dashes in prose, no first
person singular, no links to any `AGENTS.md`, no project history. This
file is internal and may be blunt.

## The README is doctested

`src/lib.rs` includes `README.md` as rustdoc under `#[cfg(doctest)]`, so
every `rust` fence in it runs on `cargo test --doc`. rustdoc runs each
fence as written, so a fence must be a complete program: wrap it in
`fn main() -> Result<(), Box<dyn std::error::Error>> { ... Ok(()) }`
rather than using `?` at the top level, and never use hidden `# ` lines,
which render as garbage on GitHub.
