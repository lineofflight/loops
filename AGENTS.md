# loops

Standalone open loops ledger for coding agents, following the same product shape as `~/code/voice`: a lightweight core binary that works in any coding agent or terminal, with optional UI integrations (`integrations/herdr/`, etc.) layered on top.

## Architecture & Design Context (graduated from `dotfiles`, 22 Nov 2026)

1. Core binary (`loops`, Rust, MIT):
   - CLI subcommands: `loops` / `loops list [--json]`, `loops add <text>...`, `loops done <idx-or-match>...`, `loops clear`.
   - Stdio MCP server: `loops mcp` (~2 MB RAM, no background HTTP daemon or SSH port-forwarding needed). Running over stdio lets `loops mcp` inherit the session id, `$HC_TERMINAL_ID` and `$PWD` directly from the parent agent's environment, and ships the open loops behavioral instructions in the MCP `initialize` `instructions` field so Claude Code, Antigravity, Codex, Cursor, or Zed learn the workflow without manual `AGENTS.md` edits.
   - Storage: a ledger belongs to the agent session, not to the Herdr terminal or the project (Hakan, 22 Nov 2026). The source of truth is one small file per session, `sessions/<id>.json` in the state directory, used the same way inside and outside Herdr by the CLI and `loops mcp`. Whenever `$HC_TERMINAL_ID` is set it is mirrored to `herdr.loops` via `hc label` (`[{"t":"<text>","at":<unix_sec>}]`, backwards-compatible with plain strings), so the drawer keeps working.
   - The label is also written by others: the drawer dismisses by rewriting it, and `loops --terminal <id> add` writes that terminal's label directly, whoever runs it. So a session writes a second label beside it, `herdr.loops.mirror`, `{"session":"<id>","loops":[...]}`: whose ledger the label mirrors, and the loops as that session left them. Each read applies what changed on the label since (`ledger::merge`): under the session's own mirror, removals and additions carry over; under another session's, only what was added since, a hand-off, joins, the rest leaves the label and the session takes the mirror; with no mirror, all of the label is a hand-off and is adopted. Outside Herdr it is the file alone, and its changes hold on return. Don't simplify this to "the label wins": a session that worked outside Herdr and came back would lose that work.
   - A fresh session starts empty (Hakan, 23 Nov 2026): one started in a terminal where an earlier session left loops takes none of them, only what was handed over since, even before its first call. The earlier session's file is never touched, so resuming it, there or anywhere, brings its loops back.
   - The mirror lives on the terminal, not in the session's file, because it describes that terminal's label: a file that moved to another terminal, or was swept, no longer knows what it left. The file keeps only `terminal`, the one it last mirrored to, so that on a move the old label is cleared, if it is still the session's and as it left it.
   - No session id: the terminal's label itself inside Herdr, the workspace's file outside. There is no `--workspace` flag.
   - Stale files: one goes when its ledger empties; any other is removed once unused for 30 days (reads touch it), swept whenever a session writes its own file.
   - Session ids, as verified on 22 Nov 2026:
     - Claude Code 2.1.330 gives `CLAUDE_CODE_SESSION_ID` to shell commands and to the stdio MCP servers it starts, the same after `claude --resume`. Nothing in `initialize`; `tools/call` carries only a tool use id. `/clear` gives shell commands a new id but leaves the MCP server running with the old one.
     - Antigravity 1.5.14 gives `ANTIGRAVITY_CONVERSATION_ID` to shell commands, the same after `agy --conversation <id>`. Its MCP servers inherit its environment, `HC_TERMINAL_ID` included, but no session id, and it starts them with `server/discover`, which names none. Every `tools/call` carries `_meta["antigravity.google/conversation_id"]` and `_meta["antigravity.google/root_conversation_id"]`, the same id for the main agent, the same after a resume (verified 23 Nov 2026). `loops mcp` reads the root one per call, so a subagent's loops land in the ledger of the conversation the user started. `mcp_config.json` interpolates nothing in `args` or `env`.
     - Codex 0.167 gives `CODEX_THREAD_ID` to shell commands. Its MCP servers get a scrubbed environment, without the id or `HC_TERMINAL_ID`, but every `tools/call` carries `_meta.threadId`, the same id, the same after `codex exec resume`, which `loops mcp` reads per call (verified with the registered server, 23 Nov 2026). `env_vars = ["HC_TERMINAL_ID"]` under `[mcp_servers.loops]` in `~/.codex/config.toml` passes the terminal on, and the mirror then works. `codex exec` refuses a call that needs approval, as every call of these tools does, since they carry no annotations (with `readOnlyHint` on `list` and `destructiveHint: false` on `add` it let those two through); `default_tools_approval_mode = "approve"` in the same table lifts that. Unchecked in the TUI, which presumably asks each time.
     - An agent started from another agent's shell inherits that one's session id variable. A call's `_meta` wins over the environment, so `loops mcp` is right under Codex and Antigravity; the command line takes the first of `store::SESSION_VARS` set.

2. Herdr integration (`integrations/herdr/`):
   - `mods/loops/` (`main.js`, `tabs.js`, `logic.js`, `infinity.svg`, `arc-1.svg`..`arc-4.svg`): monochrome `∞ Loops` toggle with live count next to `Files` and `Changes` in the top bar; opens inside Herdr's shared `panel` drawer.
   - Scoped strictly to the current session (`Other agents` intentionally omitted because Herdr's left sidebar already lists `Agents`).
   - Single-line truncated rows with full text in hover tooltip, 4-stage closing arc ring on the left (`<1h`, `<24h`, `1-3d` amber, `3d+` coral), and resting age (`15m`, `4h`, `3d`) on the right that swaps in-place to `×` on hover.
   - Installed on this machine and rig (cutover from `~/code/dotfiles` done 23 Nov 2026):
     - `~/.local/bin/loops` is a copy of `target/release/loops`; rebuild and reinstall after changes.
     - `loops mcp` is registered user-wide in Claude Code, in `~/.gemini/config/mcp_config.json` for Antigravity and in `~/.codex/config.toml` for Codex, there with `env_vars = ["HC_TERMINAL_ID"]` and `approval_mode = "approve"` for the `list`, `add` and `done` tools (`clear` still asks).
     - `~/.config/herdr-app/mods/loops` symlinks to `integrations/herdr/mods/loops`; `~/.config/herdr-app/layouts/loops.json` is a plain copy of `integrations/herdr/layouts/loops.json`.
     - rig has the same, minus Herdr: built there from a clone in `~/code/loops`, so updating it means pulling, rebuilding and reinstalling.

3. Prompt lesson & multi-turn `evals/` suite (modeled on `~/code/voice/evals/`):
   - Real-world dogfooding showed out-of-band tool calls get skipped during fast back-and-forth unless the instruction has an explicit start-of-turn trigger covering both agent questions and user-parked ideas:
     > "Never add a newly received ask in the turn it was requested. At the start of each new turn, if the user's message skipped past an unanswered question, pending choice, or parked idea from the previous turn, record it immediately before starting on the new ask. Check current loops before updating so items dismissed in the UI stay dropped."
   - Build deterministic multi-turn eval cases covering:
     1. Agent-question pivot (turn 1 agent asks A/B -> turn 2 user pivots -> `loops add` called on turn 2, never turn 1).
     2. User-parked-idea pivot ("we should add evals" followed immediately by an unrelated UI tweak -> parked idea recorded).
     3. No premature add on turn 1.
     4. Resolution (`loops done` when answered on a later turn).
     5. Respect external UI dismissal (`loops list` checked; dismissed item is not re-added).
     6. Zero inline `"Still open"` footer in chat replies when `loops` is active.

## Before you finish

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (which also runs `prototype/test_loops.sh` against the binary, unchanged but for its first case, which expected exit 2 outside Herdr, and `tests/packaging.sh`, the install scripts and the formula, offline) and `node --test tests/test_herdr_logic.mjs`, all clean. Run `shellcheck` on a shell script you change.
- Tests never reach the user's Herdr or ledgers: they unset `HC_TERMINAL_ID` and the agents' session ids (`store::SESSION_VARS`, which your own shell has set), point `HC_BIN` at a mock `hc`, and point `HOME` at a temporary directory with the `XDG_*` variables removed. `tests/packaging.sh` moves `HOME` itself, once it has found Ruby, and installs only into its own temporary directories, from a `file://` release.

## Releasing

Bump `version` in `Cargo.toml`, then push a matching `v*` tag, only when the user asks. `.github/workflows/release.yml` checks on Linux and macOS, builds `loops-<tag>-<target>.tar.gz` for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, publishes the release with `SHA256SUMS` and `loops.rb` (`scripts/render-formula.sh` from `packaging/homebrew/loops.rb`), and commits the formula to `lineofflight/homebrew-tap` when the `HOMEBREW_TAP_TOKEN` secret is set, skipping it otherwise. `docs/install.sh`, served at `https://lineofflight.dev/loops/install.sh`, installs from the same release.

## Evals

`scripts/eval.sh` runs `evals/cases/` through `claude plugin eval`: fresh Claude Code sessions with only a throwaway plugin named loops, whose server is `loops mcp` built from here, keeping its ledgers in a throwaway state directory and never in Herdr. The tools are `mcp__plugin_loops_loops__list`, `add`, `done` and `clear`.

- Each case but `fresh-ask` resumes a session from its `history.jsonl` (`context.history_file`), so its prompt is the turn under test and the trace holds only that turn. `evals/histories.py` writes the histories; run it after changing one. A history's calls to the loops server never reached a real one: a run's ledger is what its scaffold seeds, through the plugin's `bin/loops`, since a scaffold gets none of the script's environment. That wrapper unsets the session ids: a scaffold runs before its session has one, so scaffold and server both keep the run's workspace's ledger.
- The cases, one per item above: `question-pivot` (1), `parked-idea` (2), `fresh-ask` (3), `answered-later` (4), `dismissed` (5), `footer` (6). Every case also has `no-footer`, which fails on a footer or list (a line opening with "Still open", "Open loops" or "Carried"), not on a sentence that mentions a loop. All graders are regexes or tool counts, so no judge model runs.
- A count grader with `max` states `min` too: left out, it defaults to 1, so a "never called" grader fails when nothing was called. `tests/evals.rs` checks this, and that every case is whole, without running anything.
- Run sessions see no user-level config: the harness gives each its own home and config directory, so `~/.claude/CLAUDE.md` and the user's plugins never load. Only the server's instructions mention open loops.
- Run it after changing `INSTRUCTIONS` or a tool's description in `src/mcp.rs`, and only when the user asks: runs are real Claude Code sessions and spend tokens. 3 runs a case on sonnet, about $0.65 for the suite; `--runs`, `--model` and `--keep-temp` pass through. `--case <glob>` takes one name; `--tag <tag>`, repeated, picks several. Results land in `target/evals/<timestamp>/`.
- The exit code is 0 only when `evals/verdict.py` agrees: at least one case ran, the result is whole, and every run of every case passed every grader without an error. It prints the pass table.
- A failing grader is a finding about the instructions. Fix the instructions, never the grader, unless the grader is wrong. `--keep-temp` keeps each run's `trace.jsonl` to read why.
