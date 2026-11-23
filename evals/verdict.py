#!/usr/bin/env python3
"""Prints a pass table from a `claude plugin eval` result file, and exits
0 only when the eval ran at least one case, was not cut short, and every
run of every case passed every grader without an error; 1 otherwise.

Usage: python3 evals/verdict.py <aggregate-result.json>
"""

import json
import sys


def main(path):
    try:
        with open(path, encoding="utf-8") as f:
            result = json.load(f)
    except (OSError, ValueError) as e:
        print(f"verdict: no result to read at {path}: {e}", file=sys.stderr)
        return 1

    problems = []
    if result.get("partial"):
        problems.append("the eval was cut short, so its result is partial")
    cases = result.get("cases") or []
    if not cases:
        problems.append("no case ran")

    rows = [("CASE", "PASSED", "SCORE", "FAILING")]
    for case in cases:
        runs = [run for arm in (case.get("arms") or {}).values() for run in arm]
        passed = sum(1 for run in runs if run.get("passed") and not run.get("error"))
        score = sum(run.get("score") or 0 for run in runs) / len(runs) if runs else 0
        failing = {}
        for run in runs:
            if run.get("error"):
                failing["error"] = failing.get("error", 0) + 1
            for grader in run.get("graders") or []:
                if not grader.get("passed"):
                    failing[grader["name"]] = failing.get(grader["name"], 0) + 1
        notes = ", ".join(f"{name} x{n}" for name, n in sorted(failing.items()))
        rows.append((case["name"], f"{passed}/{len(runs)}", f"{score:.2f}", notes))
        if not runs or passed < len(runs):
            problems.append(f"{case['name']}: {passed} of {len(runs)} runs passed")

    widths = [max(len(row[i]) for row in rows) for i in range(3)]
    for row in rows:
        print("  ".join(cell.ljust(width) for cell, width in zip(row, widths)), row[3])
    print(f"cost ${result.get('costUsd', 0):.2f}", flush=True)
    for problem in problems:
        print(f"verdict: {problem}", file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]) if len(sys.argv) == 2 else 2)
