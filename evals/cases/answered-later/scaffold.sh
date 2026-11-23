#!/bin/sh
# The ledger as the history left it: the linter question recorded.
set -eu
# The loops scripts/eval.sh puts in the plugin, keeping its state.
"$(dirname "$0")/../../bin/loops" add "Linter: ESLint+Prettier or Biome?" >/dev/null
