# ci/

The script the Rust gate runs, [`rust/run.sh`](rust/run.sh), and notes on
this repository's own CI workflows. `.github/workflows/rust.yml` calls
that script, and you can run it locally too.

The workflows themselves live in `.github/workflows/`. To change CI, edit
them there in a reviewed pull request: session credentials can push
workflow changes (admin `DECISIONS.md` ADR-8, as amended on 2026-09-24),
so staging a workflow here for a maintainer to promote is optional.

`ci.yml`, `crates-release.yml`, `release.yml`, `notify-status.yml`,
`scorecard.yml` and `deps-gate.yml` also have a template in admin
`rollout/workflows/ini__<file>`. Mirror a change there in the same change:
admin `scripts/verify.sh` compares the two, and
`rollout/apply-workflows.sh --apply` pushes the template's text back.
`clib.yml` and `clib-release.yml` are stamped from admin
`tasks/clib-template/`, so change the template and restamp.

Sessions still cannot push tags. Releases therefore go through
`workflow_dispatch`, and a workflow that runs only on a tag push needs a
maintainer to push that tag.

## Promoted

Both of these were staged here and now run from `.github/workflows/`:

- **`docs.yml`** — the prose gate: Vale over the reader-facing pages at
  the levels set in `.vale.ini`, on the file list
  `ts/scripts/gated-docs.cjs` produces. See `docs/STYLE-GUIDE.md`.

  It needs no sibling checkouts and no secrets, and pins its own Vale
  version. Errors fail the job; warnings go to the run summary as a
  report. `make prose` runs the identical check locally, and the test
  suite runs the other half of the gate (`ts/test/docs.test.js`).

- **`rust.yml`** - the Rust gate: `ci/rust/run.sh`, which is
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
