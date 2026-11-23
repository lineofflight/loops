// Pure helpers for parsing, aging, and partitioning Herdr open loops.

export function parseLoopEntries(raw) {
  if (!raw || typeof raw !== "string") return [];
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return [];
  }
  if (!Array.isArray(parsed)) return [];
  const out = [];
  for (const item of parsed) {
    if (typeof item === "string") {
      const cleaned = item.replace(/\s+/g, " ").trim();
      if (cleaned) out.push({ text: cleaned, at: null });
    } else if (item && typeof item === "object" && typeof item.t === "string") {
      const cleaned = item.t.replace(/\s+/g, " ").trim();
      if (cleaned) {
        const at = typeof item.at === "number" && item.at > 0 ? Math.floor(item.at) : null;
        out.push({ text: cleaned, at });
      }
    }
  }
  return out;
}

export function parseLoops(raw) {
  return parseLoopEntries(raw).map((e) => e.text);
}

export function formatAge(ms) {
  const safe = Math.max(0, ms);
  const m = Math.floor(safe / 60000);
  if (m < 1) return "<1m";
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h`;
  const d = Math.floor(h / 24);
  return d < 14 ? `${d}d` : `${Math.floor(d / 7)}w`;
}

export function ageStage(ms) {
  const safe = Math.max(0, ms);
  if (safe < 3600 * 1000) return 1;
  if (safe < 24 * 3600 * 1000) return 2;
  if (safe < 3 * 24 * 3600 * 1000) return 3;
  return 4;
}

export function resolveCurrentTerminal(focusedObj, spacesList, panesList) {
  if (focusedObj?.shown) return focusedObj.shown;
  if (focusedObj?.terminal) return focusedObj.terminal;
  if (focusedObj?.agent) return focusedObj.agent;

  const activeTabId =
    focusedObj?.tab ??
    spacesList?.find((s) => s.active)?.tabs?.find((t) => t.active)?.id ??
    null;

  if (!activeTabId || !Array.isArray(panesList)) return null;
  const tabPanes = panesList.filter((p) => p.tab === activeTabId && p.terminal);
  const agentPane = tabPanes.find((p) => p.program?.agent);
  return (agentPane ?? tabPanes[0])?.terminal ?? null;
}

function formatAgentLabel(agent, terminal) {
  if (agent) {
    const space = agent.space_name || "";
    const tab = agent.tab_name || "";
    const prog = agent.program?.name || "";
    const detail = tab && tab !== prog ? tab : "";
    if (space && detail) return `${space} · ${detail}`;
    if (space) return space;
    if (detail) return detail;
  }
  return terminal?.labels?.where || terminal?.name || terminal?.title || terminal?.address || "";
}

export function partitionLoops(terminalsList, agentsList, panesList, currentTerminal, optimisticMap, now = Date.now()) {
  const agentByAddr = new Map((agentsList ?? []).map((a) => [a.address, a]));
  const paneByTerm = new Map();
  for (const p of panesList ?? []) {
    if (p.terminal && !paneByTerm.has(p.terminal)) paneByTerm.set(p.terminal, p);
  }

  const current = [];
  let currentRaw = "";
  let currentEntries = [];
  const others = [];
  let total = 0;

  for (const t of terminalsList ?? []) {
    if (t.state !== "live") continue;
    const raw = t.labels?.["herdr.loops"] ?? "";
    let entries;
    const opt = optimisticMap?.get(t.address);
    if (opt && opt.raw === raw && now < opt.until) {
      entries = opt.entries ?? (opt.loops ?? []).map((text) => ({ text, at: null }));
    } else {
      if (opt) optimisticMap.delete(t.address);
      entries = parseLoopEntries(raw);
    }
    if (!entries.length) continue;

    const loops = entries.map((e) => e.text);
    total += entries.length;
    if (t.address === currentTerminal) {
      currentRaw = raw;
      currentEntries = entries;
      // Newest first: last stored entry on top. index stays the stored position.
      for (let i = entries.length - 1; i >= 0; i--) {
        current.push({
          address: t.address,
          index: i,
          text: entries[i].text,
          at: entries[i].at,
        });
      }
    } else {
      const ag = agentByAddr.get(t.address);
      const pn = paneByTerm.get(t.address);
      const machine = t.address.includes("/") ? t.address.split("/")[0] : null;
      others.push({
        address: t.address,
        pane: ag?.pane ?? pn?.pane ?? null,
        program: ag?.program ?? pn?.program ?? t.program ?? null,
        label: formatAgentLabel(ag, t),
        machine,
        raw,
        loops,
        entries,
      });
    }
  }

  return { current, currentRaw, currentLoops: currentEntries.map((e) => e.text), currentEntries, others, total };
}
