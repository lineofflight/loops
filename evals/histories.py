#!/usr/bin/env python3
"""Writes each case's history.jsonl: the turns before the one under test,
as a Claude Code session transcript that `claude plugin eval` resumes
(`context.history_file`), so the case's prompt is the next turn.

The calls to the loops server in a history never reached a real one: the
ledger a run starts with is the scaffold's, empty unless it seeds one.
That is how `dismissed` has a loop the user dismissed in the UI.

Run it after changing a history: python3 evals/histories.py
"""

import json
import pathlib
import uuid

CASES = pathlib.Path(__file__).resolve().parent / "cases"
NS = uuid.UUID("8f9e2c1a-5b7d-4e3f-a1c2-d3e4f5a6b7c8")
TOOL = "mcp__plugin_loops_loops__"

LINT_ASK = "I want to set up linting for this repo."
LINT_QUESTION = (
    "Happy to. Which would you like: ESLint with Prettier, the usual pairing "
    "with the most plugins, or Biome, one fast tool that lints and formats?"
)
LINT_LOOP = "Linter: ESLint+Prettier or Biome?"
STATUS_ASK = "Quick one first: what does HTTP status 418 mean?"
STATUS_ANSWER = (
    "418 is \"I'm a teapot\", from the 1998 April Fools' RFC 2324 (HTCPCP). "
    "It isn't a real error, though some servers use it to turn away requests "
    "they don't want to serve."
)

# Each case's turns before its prompt: ("user", text), ("assistant", text),
# or ("call", tool, input, result) for a call to the loops server.
HISTORIES = {
    "question-pivot": [
        ("user", LINT_ASK),
        ("assistant", LINT_QUESTION),
    ],
    "parked-idea": [
        ("user", "We should add evals for the date parser at some point."),
        (
            "assistant",
            "Agreed, it has a few edge cases worth pinning down: time zones, "
            "two-digit years and ISO week dates. Want me to sketch a plan for "
            "them?",
        ),
    ],
    "answered-later": [
        ("user", LINT_ASK),
        ("assistant", LINT_QUESTION),
        ("user", STATUS_ASK),
        ("call", "list", {}, "No open loops."),
        ("call", "add", {"items": [LINT_LOOP]}, f"1. {LINT_LOOP}\n"),
        ("assistant", STATUS_ANSWER),
    ],
    "dismissed": [
        ("user", LINT_ASK),
        ("assistant", LINT_QUESTION),
        ("user", STATUS_ASK),
        ("call", "list", {}, "1. Add a CI workflow\n"),
        (
            "call",
            "add",
            {"items": [LINT_LOOP]},
            f"1. Add a CI workflow\n2. {LINT_LOOP}\n",
        ),
        ("assistant", STATUS_ANSWER),
    ],
    "external-wait": [
        (
            "user",
            "File a ticket with the affiliate network's support about the 403s "
            "on our new API key. The failing request IDs are in the sync log.",
        ),
        (
            "assistant",
            "Filed: ticket #48213 with their support, with the three failing "
            "request IDs and the 403 response bodies attached. They answer by "
            "email, usually within two business days, so the key stays blocked "
            "until they reply.",
        ),
    ],
    "unconfirmed-action": [
        (
            "user",
            "Three things: how do I undo my last commit but keep the changes, "
            "what's a good name for a branch that fixes the date parser's time "
            "zones, and draft a short WhatsApp to Sam saying I'll be 20 minutes "
            "late to dinner.",
        ),
        (
            "assistant",
            "1. `git reset --soft HEAD~1` undoes the commit and keeps its "
            "changes staged; `git reset HEAD~1` keeps them unstaged.\n\n"
            "2. `fix-parser-time-zones`.\n\n"
            "3. For Sam:\n\n"
            "Hey Sam, running about 20 minutes late tonight, sorry! Order "
            "without me if you're hungry.",
        ),
    ],
    "footer": [
        ("user", LINT_ASK),
        (
            "assistant",
            f"{LINT_QUESTION}\n\nStill open:\n- Linter: ESLint+Prettier or Biome?",
        ),
    ],
}


def transcript(case, turns):
    session = str(uuid.uuid5(NS, case))
    common = {
        "isSidechain": False,
        "userType": "external",
        "cwd": "/workspace",
        "sessionId": session,
        "version": "2.1.330",
    }
    lines = []
    parent = None
    second = 0

    def emit(kind, message, **extra):
        nonlocal parent, second
        second += 1
        uid = str(uuid.uuid5(NS, f"{case}/{len(lines)}"))
        lines.append(
            {
                "parentUuid": parent,
                "type": kind,
                "message": message,
                "uuid": uid,
                "timestamp": f"2026-11-22T09:00:{second:02d}.000Z",
                **common,
                **extra,
            }
        )
        parent = uid
        return uid

    def assistant(block, stop):
        return {
            "model": "claude-sonnet-5-5",
            "id": f"msg_{uuid.uuid5(NS, f'{case}/msg/{len(lines)}').hex}",
            "type": "message",
            "role": "assistant",
            "content": [block],
            "stop_reason": stop,
            "stop_sequence": None,
            "usage": {"input_tokens": 0, "output_tokens": 0},
        }

    for turn in turns:
        if turn[0] == "user":
            emit("user", {"role": "user", "content": turn[1]})
        elif turn[0] == "assistant":
            emit("assistant", assistant({"type": "text", "text": turn[1]}, "end_turn"))
        else:
            _, tool, args, result = turn
            call = f"toolu_{uuid.uuid5(NS, f'{case}/call/{len(lines)}').hex[:24]}"
            block = {"type": "tool_use", "id": call, "name": TOOL + tool, "input": args}
            source = emit("assistant", assistant(block, "tool_use"))
            content = [{"type": "text", "text": result}]
            emit(
                "user",
                {
                    "role": "user",
                    "content": [
                        {"type": "tool_result", "tool_use_id": call, "content": content}
                    ],
                },
                toolUseResult=content,
                sourceToolAssistantUUID=source,
            )
    return "".join(json.dumps(line, ensure_ascii=False) + "\n" for line in lines)


def main():
    for case, turns in HISTORIES.items():
        path = CASES / case / "history.jsonl"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(transcript(case, turns), encoding="utf-8")
        print(path.relative_to(CASES.parent.parent))


if __name__ == "__main__":
    main()
