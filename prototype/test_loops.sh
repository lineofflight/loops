#!/usr/bin/env bash
set -euo pipefail

BIN="$(cd "$(dirname "$0")" && pwd)/loops"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

export STATE_FILE="$TMP/state.json"
cat > "$STATE_FILE" <<'JSON'
{
  "t8k": { "herdr.space": "sp1" },
  "rig/t3": {}
}
JSON

MOCK_HC="$TMP/hc"
cat > "$MOCK_HC" <<'PY'
#!/usr/bin/env python3
import json, os, sys

state_file = os.environ["STATE_FILE"]
with open(state_file, "r", encoding="utf-8") as f:
    state = json.load(f)

args = sys.argv[1:]
if args and args[0] == "get":
    args = [a for a in args[1:] if a != "--json"]
    term = args[0]
    if term not in state:
        sys.stderr.write(f'error: no terminal "{term}"\n')
        sys.exit(3)
    print(json.dumps({"address": term, "state": "live", "labels": state[term]}))
    sys.exit(0)

if args and args[0] == "label":
    args = [a for a in args[1:] if a != "--json"]
    term = args[0]
    if term not in state:
        sys.stderr.write(f'error: no terminal "{term}"\n')
        sys.exit(3)
    for kv in args[1:]:
        k, v = kv.split("=", 1)
        if v == "":
            state[term].pop(k, None)
        else:
            state[term][k] = v
    with open(state_file, "w", encoding="utf-8") as f:
        json.dump(state, f)
    print(json.dumps({"address": term, "state": "live", "labels": state[term]}))
    sys.exit(0)

sys.stderr.write(f"unexpected hc args: {args}\n")
sys.exit(1)
PY
chmod +x "$MOCK_HC"

export HC_BIN="$MOCK_HC"

fail() {
  echo "FAIL: $1" >&2
  exit 1
}

# 1. Missing HC_TERMINAL_ID without --terminal lists the workspace's own, empty
set +e
err_out="$(env -u HC_TERMINAL_ID "$BIN" list 2>&1)"
rc=$?
set -e
[[ "$rc" -eq 0 && -z "$err_out" ]] || fail "expected an empty list without HC_TERMINAL_ID, got $rc ($err_out)"

export HC_TERMINAL_ID="t8k"

# 2. Empty list outputs nothing, --json outputs []
[[ "$("$BIN" list)" == "" ]] || fail "expected empty list stdout"
[[ "$("$BIN" list --json)" == "[]" ]] || fail "expected [] for empty --json"

# 3. Add items with quotes, =, unicode, and newlines; deduplicate case-insensitively
"$BIN" add "Download Hans's homework?" >/dev/null
"$BIN" add 'Positioning: tier=paid, "pro", Raphaëlstraat #1265' >/dev/null
"$BIN" add "download hans's homework?" >/dev/null
"$BIN" add $'Line one\n  line two  ' >/dev/null

json_out="$("$BIN" list --json)"
python3 - "$json_out" <<'PY'
import json, sys
items = json.loads(sys.argv[1])
expected = [
    "Download Hans's homework?",
    'Positioning: tier=paid, "pro", Raphaëlstraat #1265',
    "Line one line two",
]
assert items == expected, f"got {items!r}, expected {expected!r}"
PY

list_out="$("$BIN" list)"
[[ "$list_out" == *"1. Download Hans's homework?"* ]] || fail "missing item 1 in list: $list_out"
[[ "$list_out" == *"2. Positioning: tier=paid, \"pro\", Raphaëlstraat #1265"* ]] || fail "missing item 2 in list"
[[ "$list_out" == *"3. Line one line two"* ]] || fail "missing item 3 in list"

# 4. Done by 1-based index
"$BIN" done 1 >/dev/null
python3 - "$("$BIN" list --json)" <<'PY'
import json, sys
items = json.loads(sys.argv[1])
assert len(items) == 2 and items[0].startswith("Positioning:"), items
PY

# 5. Done by case-insensitive substring
"$BIN" done "raphaëlstraat" >/dev/null
[[ "$("$BIN" list --json)" == '["Line one line two"]' ]] || fail "substring done failed"

# 6. Removing last item deletes herdr.loops label completely
"$BIN" done 1 >/dev/null
python3 - "$STATE_FILE" <<'PY'
import json, sys
state = json.load(open(sys.argv[1]))
assert "herdr.loops" not in state["t8k"], f"expected herdr.loops unset, got {state['t8k']}"
PY

# 7. Corrupt label recovers cleanly
python3 - "$STATE_FILE" <<'PY'
import json, sys
state = json.load(open(sys.argv[1]))
state["t8k"]["herdr.loops"] = "{not-valid-json"
json.dump(state, open(sys.argv[1], "w"))
PY
[[ "$("$BIN" list)" == "" ]] || fail "corrupt label should degrade to empty list"
"$BIN" add "Fresh item" >/dev/null
[[ "$("$BIN" list --json)" == '["Fresh item"]' ]] || fail "corrupt label recovery on add failed"

# 8. --terminal flag and clear
"$BIN" --terminal rig/t3 add "Remote loop" >/dev/null
[[ "$("$BIN" --terminal rig/t3 list --json)" == '["Remote loop"]' ]] || fail "--terminal add/list failed"
"$BIN" clear >/dev/null
python3 - "$STATE_FILE" <<'PY'
import json, sys
state = json.load(open(sys.argv[1]))
assert "herdr.loops" not in state["t8k"], state["t8k"]
remote = json.loads(state["rig/t3"]["herdr.loops"])
assert len(remote) == 1 and remote[0]["t"] == "Remote loop" and isinstance(remote[0]["at"], int), remote
PY

echo "PASS: all loops CLI tests passed"
