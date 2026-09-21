# ci/

Staging area for GitHub Actions workflow changes.

This directory exists because session credentials cannot write
`.github/workflows/*` — see admin `DECISIONS.md` ADR-8. To change CI:

1. Put the intended workflow file in `workflows/`.
2. A maintainer promotes it with the admin `rollout/apply-ci-folders.sh`
   script.

## Pending

- **`workflows/docs.yml`** — the prose gate: Vale over the reader-facing
  pages at the levels set in `.vale.ini`, on the file list
  `ts/scripts/gated-docs.cjs` produces. See `docs/STYLE-GUIDE.md`.

  It needs no sibling checkouts and no secrets, and pins its own Vale
  version. Errors fail the job; warnings go to the run summary as a
  report. `make prose` runs the identical check locally, and the test
  suite already runs the other half of the gate
  (`ts/test/docs.test.js`), so promoting this adds the spelling and
  Google-convention arm rather than the whole gate.

- **`workflows/rust.yml`** - the Rust gate: `ci/rust/run.sh`, which is
  formatting, the build, the tests, the doctests, clippy with warnings
  denied, and a lockfile check. It runs on the MSRV toolchain named in
  `rs/Cargo.toml` rather than on stable, so a change needing a newer
  compiler fails here instead of passing and breaking for anyone
  honouring `rust-version`.

  It needs no secrets, but it does need sibling checkouts: the engine,
  the JSON core, jsonic, hoover and the fixture runner are all path
  dependencies on repositories beside this one, so the job clones each
  before running the script. Its `paths` filters list the grammar
  (`ini-grammar.jsonic`) and the embedder (`ts/embed-grammar.js`)
  alongside `rs/**`, because a grammar change reaches the Rust crate
  through them.
