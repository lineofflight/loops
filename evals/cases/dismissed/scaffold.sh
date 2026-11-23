#!/bin/sh
# The ledger as the user left it: the history recorded the linter
# question, then the user dismissed it in the UI; the CI workflow stays.
set -eu
# The loops scripts/eval.sh puts in the plugin, keeping its state.
"$(dirname "$0")/../../bin/loops" add "Add a CI workflow" >/dev/null
