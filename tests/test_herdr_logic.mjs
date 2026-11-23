import test from "node:test";
import assert from "node:assert/strict";
import { parseLoops, parseLoopEntries, formatAge, ageStage, resolveCurrentTerminal, partitionLoops } from "../integrations/herdr/mods/loops/logic.js";

test("parseLoops and parseLoopEntries handle strings and {t, at} objects", () => {
  assert.deepEqual(parseLoops('[" First ", "", {"t":"Second","at":1700}, 42, null]'), ["First", "Second"]);
  assert.deepEqual(parseLoopEntries('[" First ", {"t":"Second","at":1700}]'), [
    { text: "First", at: null },
    { text: "Second", at: 1700 },
  ]);
  assert.deepEqual(parseLoops(undefined), []);
  assert.deepEqual(parseLoops(""), []);
  assert.deepEqual(parseLoops("{bad-json"), []);
  assert.deepEqual(parseLoops('{"not":"an-array"}'), []);
});

test("formatAge and ageStage format durations and bucket into 4 closing-arc stages", () => {
  assert.equal(formatAge(15_000), "<1m");
  assert.equal(formatAge(15 * 60_000), "15m");
  assert.equal(formatAge(3 * 3_600_000), "3h");
  assert.equal(formatAge(2 * 86_400_000), "2d");
  assert.equal(formatAge(21 * 86_400_000), "3w");

  assert.equal(ageStage(10 * 60_000), 1);
  assert.equal(ageStage(2 * 3_600_000), 2);
  assert.equal(ageStage(2 * 86_400_000), 3);
  assert.equal(ageStage(5 * 86_400_000), 4);
});

test("resolveCurrentTerminal falls back to active tab lead terminal when focused terminal is null", () => {
  const spaces = [
    { id: "sp1", active: false, tabs: [{ id: "tb1", active: true }] },
    { id: "sp2", active: true, tabs: [{ id: "tb2", active: false }, { id: "tb3", active: true }] },
  ];
  const panes = [
    { pane: "p1", tab: "tb3", terminal: "t1", program: { agent: false } },
    { pane: "p2", tab: "tb3", terminal: "t2", program: { agent: true } },
  ];

  assert.equal(resolveCurrentTerminal({ shown: "t9", terminal: "t2", tab: "tb3" }, spaces, panes), "t9");
  assert.equal(resolveCurrentTerminal({ shown: null, terminal: "t1", tab: "tb3" }, spaces, panes), "t1");
  assert.equal(resolveCurrentTerminal({ shown: null, terminal: null, agent: null, tab: "tb3" }, spaces, panes), "t2");
  assert.equal(resolveCurrentTerminal(null, spaces, panes), "t2");
});

test("partitionLoops separates current vs live others, ignores finished, and applies optimistic overrides", () => {
  const now = 1000;
  const terminals = [
    { address: "t8k", state: "live", labels: { "herdr.loops": '["Hans homework?"]' } },
    { address: "rig/t3", state: "live", labels: { "herdr.loops": '[{"t":"What you want & hours","at":500},"12-31 rule"]' } },
    { address: "t99", state: "finished", labels: { "herdr.loops": '["Should not appear"]' } },
    { address: "t50", state: "live", name: "shell-helper", labels: { where: "canape / 1", "herdr.loops": '["Check port"]' } },
  ];
  const agents = [
    { address: "t8k", pane: "p8", space_name: "personal-agent", tab_name: "2", program: { name: "Antigravity" } },
    { address: "rig/t3", pane: "rig/p3", space_name: "pulsify", tab_name: "Pulsify rig handoff", program: { name: "Claude Code" } },
  ];
  const panes = [
    { pane: "p8", terminal: "t8k" },
    { pane: "rig/p3", terminal: "rig/t3" },
  ];

  const res = partitionLoops(terminals, agents, panes, "rig/t3", new Map(), now);
  assert.equal(res.total, 4);
  assert.deepEqual(res.current, [
    { address: "rig/t3", index: 1, text: "12-31 rule", at: null },
    { address: "rig/t3", index: 0, text: "What you want & hours", at: 500 },
  ]);
  assert.deepEqual(res.currentEntries.map((e) => e.text), ["What you want & hours", "12-31 rule"]);
  for (const item of res.current) assert.equal(res.currentEntries[item.index].text, item.text);
  assert.equal(res.others.length, 2);

  const opt = new Map([
    ["rig/t3", { raw: '[{"t":"What you want & hours","at":500},"12-31 rule"]', entries: [{ text: "12-31 rule", at: null }], until: 2000 }],
  ]);
  const resOpt = partitionLoops(terminals, agents, panes, "rig/t3", opt, now);
  assert.equal(resOpt.total, 3);
  assert.deepEqual(resOpt.current, [{ address: "rig/t3", index: 0, text: "12-31 rule", at: null }]);
});
