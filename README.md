# loops

Open loops ledger for coding agents, with optional UI integrations (starting with Herdr).

Keeps unanswered questions, pending choices, and parked ideas from getting buried when a conversation pivots.

## Install

```
cargo install --git https://github.com/lineofflight/loops
```

## Usage

```
loops [--terminal <id>] [list [--json] | add <text>... | done <idx-or-match>... | clear | mcp]
```

- `loops` or `loops list` prints the open loops, numbered. `--json` prints their texts as a JSON array.
- `loops add <text>...` records each text on one line, skipping blanks and ones already there whatever their case.
- `loops done <idx-or-match>...` closes loops by their number, or every loop containing the text, whatever its case.
- `loops clear` closes them all.
- `loops mcp` serves the same operations as MCP tools (`list`, `add`, `done`, `clear`) over stdio, and sends the open loops instructions in `initialize`, so an agent learns when to use them without edits to its `AGENTS.md`.

## Where open loops live

A ledger belongs to the agent session. Exit a session and resume it in the same Herdr terminal, in another one, or outside Herdr, and it has the same open loops. A new session starts with none, whatever the last one in its terminal left.

The session's ledger is one small file, `sessions/<session id>.json` under `$XDG_STATE_HOME/loops`, else `~/.local/state/loops`, used the same way by the command line and `loops mcp`, inside Herdr and out. The id is the one the agent gives the commands it runs: `CLAUDE_CODE_SESSION_ID`, `ANTIGRAVITY_CONVERSATION_ID` or `CODEX_THREAD_ID`. `loops mcp` inherits it from the agent that starts it, as Claude Code's does, or reads it beside each call, in `_meta`, as Codex (`threadId`) and Antigravity (`antigravity.google/root_conversation_id`) send it.

Inside Herdr (`$HC_TERMINAL_ID` set) the file is mirrored to the terminal's `herdr.loops` label through `hc label`, as `[{"t":"<text>","at":<unix_sec>}]`, which Herdr's Loops drawer shows. Plain strings still read. The drawer dismisses a loop by rewriting the label, and another session hands one over with `loops --terminal <id> add`, which writes that terminal's label itself, whoever runs it. So a session writes a second label beside it, `herdr.loops.mirror`, as `{"session":"<id>","loops":[...]}`: whose ledger the label mirrors and the loops as that session left them. Every read applies what changed on the label since:

- The session's own mirror: what left the label leaves the ledger, and what joined it joins.
- Another session's mirror: a fresh session starts empty. What the earlier session left is dropped from the label, not from that session's file, so resuming it brings its loops back. Only what was added since, a hand-off, joins the ledger.
- No mirror: all of the label was handed over, and is adopted. A hand-off made before a session's first call survives this way.
- Another terminal than last time: the file and what was handed over there are joined, without duplicates, and mirrored. The old terminal's label is cleared if it is still the session's and as it left it.
- Outside Herdr: the file alone. What changes there holds when the session is back in a terminal.

With no session id, a person in a plain shell or an agent that gives none, the ledger is the terminal's label itself inside Herdr. Outside Herdr it is a local file for the workspace, the nearest folder up holding `.git`, else the current one, beside the `sessions` folder.

A file goes when its ledger empties. One left by a session that ended with loops open goes once unused for 30 days, reads counting as use: each time a session writes its file, the stale ones beside it are removed.

Known gaps:

- Claude Code's `/clear` starts a new session id without restarting its MCP servers, so `loops mcp` stays on the earlier session's ledger until Claude Code is next started.
- Codex gives its MCP servers no `HC_TERMINAL_ID` unless configured to pass it on, so under it `loops mcp` keeps the session's file without mirroring it. See below.

`HC_BIN` points at another `hc`, for tests.

## Adding it to an agent

```
claude mcp add loops -- loops mcp
```

Any agent that runs stdio MCP servers takes the same command, `loops mcp`. Codex needs two more lines in `~/.codex/config.toml`, one to pass the Herdr terminal on to the server, one to approve its tools, which `codex exec` otherwise refuses to call:

```toml
[mcp_servers.loops]
command = "loops"
args = ["mcp"]
env_vars = ["HC_TERMINAL_ID"]
default_tools_approval_mode = "approve"
```

## Development

```
cargo test
cargo clippy --all-targets -- -D warnings
node --test tests/test_herdr_logic.mjs
```

`cargo test` also runs the prototype's `prototype/test_loops.sh` against the binary, unchanged but for its first case: outside Herdr the prototype exited 2.

`scripts/eval.sh` runs the multi-turn evals of the open loops instructions through `claude plugin eval`. They are real Claude Code sessions and spend tokens; `scripts/eval.sh --case question-pivot --runs 1` runs one case once.

## Layout

- `src/`: the `loops` binary. `ledger.rs` is the entries and what `add` and `done` do to them, `store.rs` where they live, `mcp.rs` the MCP server and its instructions, `main.rs` the command line.
- `tests/`: the command line (`cli.rs`, the prototype's cases ported), a session's ledger as it moves between terminals and out of Herdr (`session.rs`), the MCP server over real stdio (`mcp.rs`), the prototype's own script (`prototype.rs`), and Node unit tests for the Herdr mod logic (`test_herdr_logic.mjs`).
- `evals/`: the eval cases (`cases/`), each a resumed session's history, the next turn and its graders, and `histories.py`, which writes the histories.
- `scripts/eval.sh`: runs the evals against this binary.
- `integrations/herdr/`: Herdr UI mod (`mods/loops/`, `layouts/loops.json`) adding a monochrome `∞ Loops` toggle next to `Files` and `Changes` in the top tab bar and a `Loops` drawer with 4-stage closing age rings and resting-age -> `×` hover swap.
- `docs/`: the one-page site at <https://lineofflight.dev/loops/>, served by GitHub Pages.
- `prototype/`: the Python prototype CLI (`loops`) and shell test suite (`test_loops.sh`) from `dotfiles`, the reference for the binary's behavior.

## License

MIT
