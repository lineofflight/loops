#!/bin/sh
# Run the eval of the open loops instructions, evals/cases, with `claude
# plugin eval`, against `loops mcp` built from here. Each case resumes a
# session from its history.jsonl, so its prompt is the turn under test.
# The server keeps workspace ledgers in a throwaway state directory, one
# per run's workspace, and never touches Herdr: HC_TERMINAL_ID is unset
# and blanked for it, and so are the agents' session ids, since a
# scaffold seeds a run's ledger before its session has one.
#
# Usage: scripts/eval.sh [claude plugin eval options...]
#
#   Three runs a case on sonnet. Options given are passed on and win:
#   --case <glob>, --runs 1, --model opus.
#
# Results land in target/evals/<timestamp>/: report.html and the JSON
# result. Exits with the eval's code: 0 when every case meets the
# threshold, 1 when one falls short.
#
# LOOPS_EVAL_OUT sets the results directory.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
out=${LOOPS_EVAL_OUT:-"$root/target/evals/$(date -u +%Y-%m-%dT%H-%M-%SZ)"}
work=$(mktemp -d "${TMPDIR:-/tmp}/loops-eval.XXXXXX")
work=$(cd "$work" && pwd -P)

# shellcheck disable=SC2329 # run by the trap
cleanup() {
  rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

unset HC_TERMINAL_ID
mkdir -p "$out" "$work/state"
cargo build --quiet --manifest-path "$root/Cargo.toml" --bin loops

# `claude plugin eval` tests a plugin, so the cases go in a throwaway one
# whose only server is this binary. Named loops, with a server named
# loops, its tools are mcp__plugin_loops_loops__*, as the graders expect.
# Server and scaffolds both run its bin/loops, which keeps its ledgers in
# the throwaway state and never reaches the user's Herdr terminal: a
# scaffold gets none of this script's environment, and finds it beside
# itself. Run as no agent session, both keep the run's workspace's ledger.
mkdir -p "$work/plugin/.claude-plugin" "$work/plugin/bin"
printf '{"name": "loops", "version": "0.0.0"}\n' >"$work/plugin/.claude-plugin/plugin.json"
cat >"$work/plugin/bin/loops" <<EOF
#!/bin/sh
exec env -u CLAUDE_CODE_SESSION_ID -u ANTIGRAVITY_CONVERSATION_ID -u CODEX_THREAD_ID \\
  XDG_STATE_HOME='$work/state' HC_TERMINAL_ID= HC_BIN=false '$root/target/debug/loops' "\$@"
EOF
chmod +x "$work/plugin/bin/loops"
printf '{"mcpServers": {"loops": {"command": "%s", "args": ["mcp"]}}}\n' \
  "$work/plugin/bin/loops" >"$work/plugin/.mcp.json"
cp -R "$root/evals/cases" "$work/plugin/evals"

# The cases are this repo's own, so their scaffold scripts may run. Every
# grader is a regex or a tool count, so no judge model is needed.
# --allow-tools takes a list, so it comes last.
tool=mcp__plugin_loops_loops_
status=0
claude plugin eval "$work/plugin" \
  --trust-plugin --mocks off --scaffold --no-publish \
  --runs 3 --concurrency 3 --ablation none --model sonnet --threshold 1 \
  --output-dir "$out" --report "$out/report.html" \
  "$@" \
  --allow-tools "${tool}_list" "${tool}_add" "${tool}_done" "${tool}_clear" \
  Read Edit || status=$?

# The exit code says only whether the eval reached its threshold. The
# result file says more, so it decides too: the eval passes only when it
# ran at least one case, was not cut short, and every run of every case
# passed every grader without an error. It prints a pass table.
verdict=0
python3 -I "$root/evals/verdict.py" "$out/aggregate-result.json" || verdict=$?
if [ "$status" -eq 0 ] && [ "$verdict" -ne 0 ]; then
  status=$verdict
fi
echo "Results: $out"
exit "$status"
