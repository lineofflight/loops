#!/bin/sh
# The ledger as the history left it: the ticket the user was to file.
set -eu
# The loops scripts/eval.sh puts in the plugin, keeping its state.
"$(dirname "$0")/../../bin/loops" add "Affiliate 403s: file a support ticket" >/dev/null
