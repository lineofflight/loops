// Loops for Herdr, on any layout, stock main included, with no layout of its
// own: "<n> loops" in the middle of the status bar, set as the status bar
// sets its own words, for the focused session's open loops; nothing at all at
// none. It sits fixed in the status bar's column and follows it as the
// window, zoom or layout changes. A click opens the list; while it shows, a
// click does nothing.
//
// The list opens where the command palette does, as the palette looks and
// works: Herdr's popup frame, its search box (focused, filtering the loops as
// you type) and its rows (closing ring, text, age turning into × under the
// pointer), the first selected, ↑/↓ moving the selection. Enter or a click on
// a row closes the list and pastes that loop into the terminal's prompt,
// without sending it; ⌘⌫ (Ctrl+Backspace elsewhere) dismisses the selected
// loop, as × does, and the list stays open until none are left. Esc or a
// click outside closes it. The status bar's words stay while it shows.
// `loops.toggle` opens or closes it too, for a key binding in keys.json.
//
// It starts once per layout, from the palette's "Loops" (mod.open), and comes
// back with that layout and on each start. One mod, four copies: what
// mod.open opens, which opens the kept window and goes; the kept window,
// empty and 0 by 0, which places the words and opens the list; the words;
// and the list. Herdr opens a keyed window
// where it was last dropped, whatever `at` says, so the words' window has no
// key: the kept window puts it where it belongs each time.

import { View, div, svg } from "gpui-kit";
import { v_flex, h_flex, InputState, Input, ScrollHandle } from "gpui-base";
import { terminals, agents, panes, spaces, focused, theme, hc, run, props, settings, checking, layout, window, openOverlay, closeOverlay, nextCommand } from "herdr";
import { resolveCurrentTerminal, partitionLoops, formatAge, ageStage } from "./logic.js";

const MOD = "loops";
// The window that places the words: kept, so it comes back on each start
// (and when its layout shows again), with no frame and nothing in it.
const KEEPER = { mod: MOD, kind: "window", key: MOD, keep: true, frame: "none", props: { kept: true } };
// The words, in the status bar's type (workspace-status: 11.8px, in a
// bar with a 1px top border), in the middle of the status bar (28px where the
// layout doesn't say), or this far above the window's bottom where it is
// hidden. Herdr makes no window smaller than 48px either way, and lays a
// smaller one's content out at its top left, so the words' window is at
// least that big, kept inside Herdr's, the words placed in it where they
// belong.
const WORDS = { size: 11.8, width: 96 };
const STATUS = 28;
const BOTTOM = 8;
const SMALLEST = 48;
// The list, as the command palette: its width and place (centered across the
// window, this far down), padding (less below, where the last row's own
// height already leaves room), search box height, row height, and as many
// rows as it shows before scrolling.
const LIST = { width: 620, top: 60, pad: 18, bottom: 12, search: 36, row: 32, shown: 10 };
// How often the words' place is checked against the window and layout.
const FOLLOW = 500;
// A press on the words this soon after the list closed is the one that closed it.
const JUST_CLOSED = 400;

// The kept windows, by the name each gives the words and list it opens; and
// loops dismissed but not yet gone from their label, shared so the words'
// count drops at once too.
const keepers = new Map();
let named = 0;
const optimistic = new Map();
let dismissed = 0;

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

function zoom() {
  return Number(settings()?.zoom) || 1;
}

const plural = (n) => `${n} ${n === 1 ? "loop" : "loops"}`;

// The path from the layout's root to the first node `match` accepts.
function pathTo(node, match, trail = []) {
  if (!node || typeof node !== "object") return null;
  const here = [...trail, node];
  if (match(node)) return here;
  for (const child of node.children ?? []) {
    const found = pathTo(child, match, here);
    if (found) return found;
  }
  return null;
}

// How far in a node at the end of `path` sits from the window's left and
// right: across each row on the way, the fixed-width blocks shown before and
// after it, with their dividers.
function insets(path) {
  let left = 0;
  let right = 0;
  for (let i = 0; i < path.length - 1; i++) {
    const row = path[i];
    if (row.type !== "row") continue;
    const divider = row.divider?.width ?? 0;
    let before = true;
    for (const sibling of row.children ?? []) {
      if (sibling === path[i + 1]) {
        before = false;
        continue;
      }
      if (sibling.hidden || typeof sibling.size?.px !== "number") continue;
      if (before) left += sibling.size.px + divider;
      else right += sibling.size.px + divider;
    }
  }
  return { left, right };
}

// Herdr's window, in the px a mod is written in, with the status bar: its
// height (0 where hidden) and the middle of its column, which runs under the
// terminals and any open Files or Changes panel, right of the sidebar; the
// whole window where the layout has no status bar. Null where the window
// can't be measured.
function screen() {
  let size;
  try {
    size = globalThis.window.viewport_size();
  } catch {
    return null;
  }
  const z = zoom();
  const width = size.width / z;
  const height = size.height / z;
  let middle = width / 2;
  let status = STATUS;
  try {
    const bar = pathTo(layout(), (n) => n.id === "status" || n.content?.mod === "workspace-status");
    if (bar) {
      const node = bar[bar.length - 1];
      status = node.hidden ? 0 : typeof node.size?.px === "number" ? node.size.px : STATUS;
      const { left, right } = insets(bar);
      if (left + right < width) middle = left + (width - left - right) / 2;
    }
  } catch {}
  return { width, height, z, middle, status, statusTop: height - status };
}

export default class LoopsPill extends View {
  init(_props, cx) {
    this.hoveredKey = null;
    // The kept window's state: the words and list it opened, where it put
    // the words and for how many loops, and when the list last closed.
    this.wordsId = null;
    this.list = null;
    this.placed = null;
    this.listClosedAt = 0;
    // The words' own state: whether the press under way began with the list
    // showing.
    this.pressOpen = false;
    // The words' and the list's: the kept window that opened them.
    this.from = null;
    try { this.firstSeen = JSON.parse(localStorage.getItem("loops.firstSeen") ?? "{}"); } catch { this.firstSeen = {}; }
    if (!this.firstSeen || typeof this.firstSeen !== "object" || Array.isArray(this.firstSeen)) this.firstSeen = {};
    this.lastDigest = loopsDigest(terminals());
    this.lastDismissed = dismissed;
    if (checking()) return;

    const p = props() ?? {};
    // mod.open shows the overlay mod.json describes: this reopens as the kept
    // window, replacing one already open.
    if (!p.kept && !p.words && !p.popup) {
      this.bootstrap = true;
      cx.timer.after(0, () => {
        const id = openOverlay(KEEPER);
        if (p.overlay && p.overlay !== id) closeOverlay(p.overlay);
      });
      return;
    }
    if (p.words || p.popup) {
      this.from = keepers.get(p.words ?? p.popup) ?? null;
      // Its kept window gone, as when the mod was reloaded: nothing to show.
      if (!this.from) {
        cx.timer.after(0, () => closeOverlay(p.overlay));
        return;
      }
      // The list's search box, as the palette's: it takes the keyboard as the
      // list opens (and again a moment later, as the palette does).
      if (p.popup) {
        this.query = "";
        // The selected row, among those shown, and what was shown last.
        this.at = 0;
        this.shown = [];
        this.scroll = ScrollHandle.new();
        this.input = InputState.new({ placeholder: "Search open loops…" });
        this.input.on("change", (_e, changeCx) => {
          this.query = this.input.value();
          this.at = 0;
          changeCx.notify();
        });
        this.input.on("submit", (_e, submitCx) => this.paste(submitCx, this.shown[this.at]));
        this.handle = cx.focus_handle();
        this.input.focus();
        cx.timer.after(600, () => this.input?.focus());
      }
    } else {
      this.name = String(++named);
      keepers.set(this.name, this);
      // `loops.toggle` (a key binding, or `herdr-app do loops.toggle`) opens or closes the list.
      cx.spawn(async (loopCx) => {
        for (;;) {
          const command = await nextCommand();
          if (command === "toggle" || command === "open") this.toggle(command === "open");
          loopCx.notify();
        }
      });
      // The words now; back in place whenever the window or layout changes;
      // back if Herdr closed them; gone while there are no loops.
      cx.timer.after(0, () => this.place());
      cx.timer.every(FOLLOW, () => {
        const s = screen();
        if (!s) return;
        const count = this.loops(Date.now()).current.length;
        const moved = this.placedKey(s) !== this.placed?.key;
        const shown = Boolean(this.wordsId);
        if (moved || shown !== count > 0) this.place();
      });
    }

    if (!this.name) {
      cx.timer.every(400, (tickCx) => {
        const next = loopsDigest(terminals());
        if (next !== this.lastDigest || dismissed !== this.lastDismissed) {
          this.lastDigest = next;
          this.lastDismissed = dismissed;
          tickCx.notify();
        }
      });
      cx.timer.every(30000, (tickCx) => tickCx.notify());
    }
  }

  // The words and list go with their kept window, so a reload or a layout
  // switch leaves none; the kept window hears when either goes.
  async close() {
    if (this.name) {
      if (this.list) closeOverlay(this.list);
      if (this.wordsId) closeOverlay(this.wordsId);
      this.list = null;
      this.wordsId = null;
      keepers.delete(this.name);
      return;
    }
    const p = props() ?? {};
    // The search box gives the keyboard back, to the terminal, and goes.
    if (this.input) {
      try { this.input.blur(); } catch {}
      try { this.input.release(); } catch {}
      this.input = null;
    }
    if (this.scroll) {
      try { this.scroll.release(); } catch {}
      this.scroll = null;
    }
    const keeper = this.from;
    if (!keeper || !keepers.has(keeper.name)) return;
    if (p.popup && keeper.list === p.overlay) {
      keeper.list = null;
      keeper.listClosedAt = Date.now();
    }
    if (p.words && keeper.wordsId === p.overlay) keeper.wordsId = null;
  }

  placedKey(s) {
    return [s.width, s.height, s.z, s.middle, s.status].join(",");
  }

  // Opens the words in the middle of the status bar (or just above the
  // window's bottom where it is hidden), in place of the ones open; with no
  // loops, only closes those, so nothing sits there.
  place() {
    const s = screen();
    if (!s) return;
    const count = this.loops(Date.now()).current.length;
    const previous = this.wordsId;
    this.placed = { key: this.placedKey(s) };
    if (count === 0) {
      this.wordsId = null;
      if (previous) closeOverlay(previous);
      return;
    }
    // The bar the words sit in: the status bar itself, or one as tall above
    // the window's bottom.
    const bar = s.status > 0 ? { top: s.statusTop, height: s.status } : { top: s.height - BOTTOM - STATUS, height: STATUS };
    const width = Math.max(WORDS.width, SMALLEST);
    const height = Math.max(bar.height, SMALLEST);
    const center = Math.min(Math.max(s.middle, width / 2), s.width - width / 2);
    const windowTop = Math.min(Math.max(bar.top + bar.height / 2 - height / 2, 0), s.height - height);
    this.wordsId = openOverlay({
      mod: MOD,
      kind: "window",
      frame: "none",
      width,
      height,
      at: { x: Math.round((center - width / 2) * s.z), y: Math.round(windowTop * s.z) },
      props: { words: this.name, inset: bar.top - windowTop, bar: bar.height },
    });
    if (previous) closeOverlay(previous);
  }

  resolveCreatedAtMs(address, text, atSec, nowMs) {
    if (typeof atSec === "number" && atSec > 0) return atSec * 1000;
    const key = `${address}\u0000${text.toLowerCase()}`;
    const existing = this.firstSeen[key];
    if (typeof existing === "number" && existing > 0) return existing;
    this.firstSeen[key] = nowMs;
    if (!checking()) {
      try { localStorage.setItem("loops.firstSeen", JSON.stringify(this.firstSeen)); } catch {}
    }
    return nowMs;
  }

  // The focused session's loops, each with its age.
  loops(now) {
    const allPanes = panes();
    const currentTerminal = resolveCurrentTerminal(focused(), spaces(), allPanes);
    const found = partitionLoops(terminals(), agents(), allPanes, currentTerminal, optimistic, now);
    for (const item of found.current) {
      item.createdMs = this.resolveCreatedAtMs(item.address, item.text, item.at, now);
      item.ageMs = Math.max(0, now - item.createdMs);
      item.stage = ageStage(item.ageMs);
    }
    return found;
  }

  toggle(openOnly = false) {
    if (this.list) {
      if (!openOnly) this.closeList();
      return;
    }
    this.openList();
  }

  closeList() {
    if (!this.list) return;
    const list = this.list;
    this.list = null;
    this.listClosedAt = Date.now();
    closeOverlay(list);
  }

  // Opens the list where the command palette opens: a popup in Herdr's frame,
  // as wide as the palette where the window has room, centered across it,
  // LIST.top down.
  openList() {
    const s = screen();
    if (!s || this.list) return;
    const width = Math.floor(Math.min(LIST.width, s.width - 16));
    this.list = openOverlay({
      mod: MOD,
      kind: "popup",
      width,
      at: { x: Math.round(((s.width - width) / 2) * s.z), y: Math.round(LIST.top * s.z) },
      props: { popup: this.name },
    });
  }

  // Closes the list and puts `item`'s text in the terminal's prompt, by way of
  // the clipboard and Herdr's own paste, so it isn't sent. The loop stays.
  paste(cx, item) {
    if (!item || !this.from) return;
    cx.write_to_clipboard(item.text);
    try { this.input?.blur(); } catch {}
    this.from.closeList();
    run("terminal.paste");
  }

  // Dismisses a loop, at once on screen and then in its label; the list
  // closes once the session has none left.
  dismiss(cx, address, raw, entries, index) {
    const now = Date.now();
    const next = entries.filter((_, i) => i !== index);
    optimistic.set(address, { raw, entries: next, until: now + 3000 });
    dismissed++;
    this.hoveredKey = null;
    cx.notify();
    if (next.length === 0 && (props() ?? {}).popup) this.from?.closeList();
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

  // The words, set as the status bar sets its "commands": a bar as tall as
  // the status bar with its 1px top border's room, the words in its middle,
  // dim, with no hover, cursor or tooltip. They stay put: every
  // press on their window is stopped, so Herdr never drags it. A click opens
  // the list; one while it shows (or that just closed it, as a press outside
  // a popup does) does nothing.
  renderWords(t, current) {
    const keeper = this.from;
    const p = props() ?? {};
    return div()
      .id("words-room")
      .size_full()
      .flex()
      .justify_center()
      .items_start()
      .pt(p.inset ?? 0)
      .on_mouse_down("left", (_e, cx) => cx.stop_propagation())
      .child(
        h_flex()
          .id("words-bar")
          .h(p.bar ?? STATUS)
          .pt(1)
          .items_center()
          .child(
            div()
              .id("loops-words")
              .text_size(WORDS.size)
              .text_color(t.dim)
              .on_mouse_down("left", () => {
                this.pressOpen = Boolean(keeper.list) || Date.now() - keeper.listClosedAt < JUST_CLOSED;
              })
              .on_click(() => {
                const open = this.pressOpen;
                this.pressOpen = false;
                if (!open) keeper.openList();
              })
              .child(plural(current.length))
          )
      );
  }

  // The list, as the command palette lays out its own: padding, its search
  // box, and rows of its height and type, muted until selected or under the
  // pointer, scrolling past as many rows as it shows; the loops whose text
  // holds what was typed, in any case. Its keys, the search box focused or
  // not: ↑/↓ (round at the ends, kept in view), ⌘⌫ to dismiss the selected
  // loop, Esc to close; Enter is the search box's submit.
  renderList(t, current, currentRaw, currentEntries) {
    const query = (this.query ?? "").trim().toLowerCase();
    const shown = query ? current.filter((item) => item.text.toLowerCase().includes(query)) : current;
    this.at = Math.min(this.at ?? 0, Math.max(0, shown.length - 1));
    this.shown = shown;
    const rows = shown.map((item, i) => {
      const rowKey = `${item.address}:${item.index}`;
      const isHovered = this.hoveredKey === rowKey;
      const on = i === this.at;
      const ageText = formatAge(item.ageMs);
      const arcColor = stageColor(t, item.stage);
      return h_flex()
        .id(`loop-cur-${item.address}-${item.index}`)
        .h(LIST.row)
        .flex_none()
        .mx(6)
        .px(10)
        .gap(10)
        .items_center()
        .rounded(7)
        .cursor_pointer()
        .text_size(12.8)
        .when(on, (el) => el.bg(t.selected))
        .hover((s) => s.bg(t.hover))
        .on_hover((over, cx) => {
          if (over) this.hoveredKey = rowKey;
          else if (this.hoveredKey === rowKey) this.hoveredKey = null;
          cx.notify();
        })
        .on_click((_e, cx) => this.paste(cx, item))
        .tooltip(`${ageText} · ${item.text}`)
        .child(svg(`mods/loops/arc-${item.stage}.svg`).size(12).flex_none().text_color(arcColor))
        .child(div().flex_1().min_w(0).truncate().text_color(on || isHovered ? t.foreground : t.muted).child(item.text))
        .when(!isHovered, (row) =>
          row.child(div().w(26).flex_none().text_right().text_size(11.5).text_color(item.stage >= 3 ? arcColor : t.dim).child(ageText))
        )
        .when(isHovered, (row) =>
          row.child(this.dismissButton(t, `dismiss-cur-${item.address}-${item.index}`, item.address, currentRaw, currentEntries, item.index))
        );
    });
    const empty = current.length === 0 ? "No open loops in this session" : `Nothing matches "${this.query.trim()}".`;
    return v_flex()
      .id("loops")
      .when(Boolean(this.handle), (el) => el.track_focus(this.handle))
      .w_full()
      .min_h(0)
      .pt(LIST.pad)
      .px(LIST.pad)
      .pb(LIST.bottom)
      .gap(10)
      .bg(t.popover)
      .on_key_down((e, cx) => {
        const count = this.shown.length;
        if (e.key === "escape") {
          cx.stop_propagation();
          this.from?.closeList();
        } else if ((e.key === "down" || e.key === "up") && count) {
          cx.stop_propagation();
          this.at = (this.at + (e.key === "down" ? 1 : count - 1)) % count;
          this.scroll?.scroll_to_item(this.at);
          cx.notify();
        } else if (e.key === "backspace" && (e.modifiers?.platform || (window()?.platform !== "macos" && e.modifiers?.control)) && count) {
          // ⌘⌫ on macOS, Ctrl+Backspace elsewhere; Backspace alone edits the search.
          cx.stop_propagation();
          const item = this.shown[this.at];
          this.dismiss(cx, item.address, currentRaw, currentEntries, item.index);
        }
      })
      .child(
        h_flex()
          .h(LIST.search)
          .flex_none()
          .px(10)
          .gap(10)
          .items_center()
          .rounded(9)
          .border_1()
          .border_color(t.accent)
          .bg(t.background)
          .child(svg("mods/workspace-chrome/search.svg").size(16).text_color(t.dim))
          .when(Boolean(this.input), (el) => el.child(Input.new(this.input).flex_1().h_full().text_size(12.8).text_color(t.foreground)))
          .when(!this.input, (el) => el.child(div().flex_1().text_size(12.8).text_color(t.dim).child("Search open loops…")))
          .child(div().text_size(11).text_color(t.dim).child("Esc"))
      )
      .child(
        rows.length
          ? v_flex()
              .id("loops-rows")
              .when(rows.length > LIST.shown, (el) => el.h(LIST.shown * LIST.row).overflow_y_scrollbar().when(Boolean(this.scroll), (s) => s.track_scroll(this.scroll)))
              .children(rows)
          : div().px(16).py(14).text_size(13).text_color(t.dim).child(empty)
      );
  }

  render() {
    const p = props() ?? {};
    // The kept window, and what mod.open opens, are empty.
    if (this.bootstrap || this.name || ((p.words || p.popup) && !this.from)) return div();
    const t = theme();
    const { current, currentRaw, currentEntries } = this.loops(Date.now());
    if (p.popup) return this.renderList(t, current, currentRaw, currentEntries);
    if (current.length === 0) return div();
    if (p.words) return this.renderWords(t, current);
    // A check renders this: the words, as they show.
    return div().text_size(WORDS.size).text_color(t.dim).child(plural(current.length));
  }
}
