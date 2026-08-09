#!/usr/bin/env bash
#
# Documented shell entry point for the third-party INI conformance corpus.
#
# The real implementation is scripts/fetch-ini-corpus.js -- it is Node so
# that it runs on any platform, where a bash script would not.
#
# Read scripts/build-ini-corpus.js for why INI has no authoritative suite,
# and for the provenance of every document, label and expected value.
#
# This is a REGENERATION and AUDIT tool, not a test prerequisite. The
# upstream checkouts (test/corpus/upstream/) are gitignored and never
# committed; the assembled manifest (test/corpus/ini-corpus.json) IS
# committed, so the suites run with no network and cannot skip. Running
# this rewrites the committed manifest -- review the diff.
#
# Usage:  ./scripts/fetch-ini-corpus.sh [--force]

set -euo pipefail
exec node "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/fetch-ini-corpus.js" "$@"
