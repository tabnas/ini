#!/usr/bin/env bash
#
# Documented shell entry point for the third-party INI conformance corpus.
#
# The real implementation is scripts/fetch-ini-corpus.js -- it is Node so
# that it can also run as the ts/ package's npm `pretest` hook on the
# Windows CI runner, where a bash script would not.
#
# Read scripts/build-ini-corpus.js for why INI has no authoritative suite,
# and for the provenance of every document, label and expected value.
#
# NOTHING FETCHED HERE IS EVER COMMITTED. test/corpus/ is gitignored.
#
# Usage:  ./scripts/fetch-ini-corpus.sh [--force]

set -euo pipefail
exec node "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/fetch-ini-corpus.js" "$@"
