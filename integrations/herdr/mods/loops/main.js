// Loops mod for Herdr: renders either the top tab bar (with ∞ Loops button and
// live count next to Files and Changes, when props.tabs is true) or the Loops
// drawer inside the shared right panel with closing-arc age indicators and
// resting age -> hover × swap.

import { View, div, svg } from "gpui-kit";
import { v_flex, h_flex, ScrollHandle } from "gpui-base";
import { terminals, agents, panes, spaces, focused, theme, hc, run, props, checking } from "herdr";
import { resolveCurrentTerminal, partitionLoops, formatAge, ageStage } from "./logic.js";
import { renderTabsBar } from "./tabs.js";

function loopsDigest(allTerminals) {
  let out = "";
  for (const t of allTerminals) {
    if (t.state === "live" && t.labels?.["herdr.loops"]) {
      out += `${t.address}:${t.labels["herdr.loops"]}|`;
    }
  }
  return out;
}

function stageColor(t, stage) {
  if (stage === 1) return t.dim;
  if (stage === 2) return t.muted;
  if (stage === 3) return t.ansi3 ?? t.working ?? t.muted;
  return t.ansi1 ?? t.blocked ?? t.foreground;
}

export default class Loops extends View {
  init(_props, cx) {
    this.optimistic = new Map();
    this.lastDigest = loopsDigest(terminals());
    this.hoveredKey = null;
    this.pressed = null;
    this.scroll = ScrollHandle.new();
    try { this.panels = JSON.parse(localStorage.getItem("panels") ?? "{}"); } catch { this.panels = {}; }
    if (!this.panels || typeof this.panels !== "object" || Array.isArray(this.panels)) this.panels = {};
    try { this.firstSeen = JSON.parse(localStorage.getItem("loops.firstSeen") ?? "{}"); } catch { this.firstSeen = {}; }
    if (!this.firstSeen || typeof this.firstSeen !== "object" || Array.isArray(this.firstSeen)) this.firstSeen = {};
    this.swipe = { acc: 0, steps: 0, at: 0, last: 0 };
    if (checking()) return;

    cx.timer.every(400, (tickCx) => {
      const next = loopsDigest(terminals());
      if (next !== this.lastDigest) {
        this.lastDigest = next;
        tickCx.notify();
      }
    });
    cx.timer.every(30000, (tickCx) => tickCx.notify());
  }

  resolveCreatedAtMs(address, text, atSec, nowMs) {
    if (typeof atSec === "number" && atSec > 0) {
      return atSec * 1000;
    }
    const key = `${address}\u0000${text.toLowerCase()}`;
    const existing = this.firstSeen[key];
    if (typeof existing === "number" && existing > 0) {
      return existing;
    }
    this.firstSeen[key] = nowMs;
    if (!checking()) {
      try { localStorage.setItem("loops.firstSeen", JSON.stringify(this.firstSeen)); } catch {}
    }
    return nowMs;
  }

  wheel(e) {
    if (e.delta_lines) {
      if (e.delta_lines.y) run(e.delta_lines.y > 0 ? "tab.prev" : "tab.next");
      return;
    }
    const s = this.swipe, now = Date.now();
    if (e.touch_phase === "started" || now - s.at > 250 || e.delta.y * s.acc < 0) Object.assign(s, { acc: 0, steps: 0 });
    s.at = now;
    s.acc += e.delta.y;
    const need = s.steps ? 60 : 20;
    if (Math.abs(s.acc) < need || now - s.last < 100) return;
    s.acc -= Math.sign(s.acc) * need;
    s.steps++;
    s.last = now;
    run(e.delta.y > 0 ? "tab.prev" : "tab.next");
  }

  syncPanel(space, panel, body, enabled) {
    if (!space || !panel) return;
    const now = panel.hidden ? null : body?.active ?? null;
    if (space.id !== this.space) {
      const first = this.space === undefined;
      this.space = space.id; this.pending = null;
      if (!(first && !(space.id in this.panels))) {
        const stored = this.panels[space.id] ?? null, want = enabled.includes(stored) ? stored : null;
        if (want !== now) {
          this.pending = { want, until: Date.now() + 1000 };
          if (want) { run("layout.select", { stack: "panel-body", child: want }); run("layout.show", { id: "panel" }); }
          else run("layout.hide", { id: "panel" });
          return;
        }
      }
    }
    if (this.pending) {
      if (this.pending.want !== now && Date.now() < this.pending.until) return;
      this.pending = null;
    }
    if (this.panels[space.id] === now) return;
    this.panels[space.id] = now;
    localStorage.setItem("panels", JSON.stringify(this.panels));
  }

  dismiss(cx, address, raw, entries, index) {
    const now = Date.now();
    const next = entries.filter((_, i) => i !== index);
    this.optimistic.set(address, { raw, entries: next, until: now + 3000 });
    this.hoveredKey = null;
    cx.notify();
    if (checking()) return;
    const serialized = next.map((e) => (e.at ? { t: e.text, at: e.at } : { t: e.text }));
    const arg = serialized.length ? `herdr.loops=${JSON.stringify(serialized)}` : "herdr.loops=";
    cx.spawn(async (innerCx) => {
      try {
        await hc(["label", address, arg]);
      } catch {}
      innerCx.notify();
    });
  }

  dismissButton(t, id, address, raw, entries, index) {
    return div()
      .id(id)
      .w(26)
      .h(18)
      .flex_none()
      .flex()
      .items_center()
      .justify_end()
      .pr(2)
      .rounded(5)
      .cursor_pointer()
      .text_color(t.muted)
      .hover((s) => s.text_color(t.foreground))
      .on_mouse_down("left", (_e, cx) => cx.stop_propagation())
      .on_click((_e, cx) => {
        cx.stop_propagation();
        this.dismiss(cx, address, raw, entries, index);
      })
      .child(svg("mods/pane/close.svg").size(11));
  }

  render() {
    if (props()?.tabs) {
      return renderTabsBar(this);
    }

    const now = Date.now();
    const t = theme();
    const allTerminals = terminals();
    const allSpaces = spaces();
    const allPanes = panes();
    const allAgents = agents();
    const currentTerminal = resolveCurrentTerminal(focused(), allSpaces, allPanes);
    const { current, currentRaw, currentEntries } = partitionLoops(
      allTerminals,
      allAgents,
      allPanes,
      currentTerminal,
      this.optimistic,
      now
    );

    return v_flex()
      .id("loops")
      .size_full()
      .min_h(0)
      .min_w(0)
      .rounded(10)
      .overflow_hidden()
      .bg(t.background)
      .border_1()
      .border_color(t.border)
      .text_color(t.foreground)
      .text_size(13)
      .child(
        h_flex()
          .id("loops-header")
          .h(42)
          .flex_none()
          .px(14)
          .gap(8)
          .items_center()
          .border_b(1)
          .border_color(t.border)
          .child(svg("mods/loops/infinity.svg").size(15).flex_none().text_color(t.muted))
          .child(div().font_semibold().child("Loops"))
          .child(
            div()
              .px(5)
              .py(1)
              .rounded(4)
              .border_1()
              .border_color(t.border)
              .text_size(10)
              .text_color(t.muted)
              .child(String(current.length))
          )
      )
      .child(
        v_flex()
          .id("loops-list")
          .flex_1()
          .min_h(0)
          .min_w(0)
          .overflow_y_scrollbar()
          .p(6)
          .gap(1)
          .when(current.length === 0, (el) =>
            el.child(div().p(8).text_size(12).text_color(t.dim).child("No open loops in this session"))
          )
          .when(current.length > 0, (el) =>
            el.children(
              current.map((item) => {
                const rowKey = `${item.address}:${item.index}`;
                const isHovered = this.hoveredKey === rowKey;
                const createdMs = this.resolveCreatedAtMs(item.address, item.text, item.at, now);
                const ageMs = Math.max(0, now - createdMs);
                const ageText = formatAge(ageMs);
                const stage = ageStage(ageMs);
                const arcColor = stageColor(t, stage);

                return h_flex()
                  .id(`loop-cur-${item.address}-${item.index}`)
                  .w_full()
                  .min_w(0)
                  .h(28)
                  .px(8)
                  .gap(8)
                  .flex_none()
                  .items_center()
                  .rounded(7)
                  .hover((s) => s.bg(t.hover))
                  .on_hover((on, cx) => {
                    if (on) this.hoveredKey = rowKey;
                    else if (this.hoveredKey === rowKey) this.hoveredKey = null;
                    cx.notify();
                  })
                  .tooltip(`${ageText} · ${item.text}`)
                  .child(
                    svg(`mods/loops/arc-${stage}.svg`)
                      .size(12)
                      .flex_none()
                      .text_color(arcColor)
                  )
                  .child(
                    div()
                      .flex_1()
                      .min_w(0)
                      .truncate()
                      .text_size(12.5)
                      .text_color(t.foreground)
                      .child(item.text)
                  )
                  .when(!isHovered, (row) =>
                    row.child(
                      div()
                        .w(26)
                        .flex_none()
                        .text_right()
                        .text_size(11)
                        .text_color(stage >= 3 ? arcColor : t.dim)
                        .child(ageText)
                    )
                  )
                  .when(isHovered, (row) =>
                    row.child(
                      this.dismissButton(
                        t,
                        `dismiss-cur-${item.address}-${item.index}`,
                        item.address,
                        currentRaw,
                        currentEntries,
                        item.index
                      )
                    )
                  );
              })
            )
          )
      );
  }
}
