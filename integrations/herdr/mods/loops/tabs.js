import { div, svg, image } from "gpui-kit";
import { h_flex } from "gpui-base";
import { spaces, panes, terminals, focused, run, openOverlay, theme, window, shared, layout, settings } from "herdr";
import { parseLoops, resolveCurrentTerminal } from "./logic.js";

function find(node, id) {
  if (node?.id === id) return node;
  for (const child of node?.children ?? []) {
    const found = find(child, id);
    if (found) return found;
  }
  return null;
}

const clean = (s) => (s ?? "").replace(/^[^\p{L}\p{N}\p{Ps}\p{Pi}]+/u, "").replace(/^\[ [!.] \] /, "").trim();

function label(tab, lead) {
  const prog = lead?.program?.name;
  const base = tab.named ? tab.name : prog ?? "Terminal";
  if (!lead?.program?.agent) return base;
  const live = clean(lead.title);
  const path = /^[~/]|^[A-Za-z]:\\/.test((lead.title ?? "").replace(/^[^\p{L}\p{N}~/]+/u, ""));
  const telling = live && live !== lead.terminal && live !== prog && !path && !live.includes("@") && live.toLowerCase() !== lead.program.key;
  return (tab.name === prog || !tab.named) && telling ? live : base;
}

export function renderTabsBar(view) {
  const t = theme();
  const allSpaces = spaces();
  const allPanes = panes();
  const allTerminals = terminals();
  const space = allSpaces.find(p => p.active);
  const shown = space?.tabs ?? [];
  const active = shown.findIndex(tab => tab.active);
  const selected = shown[active]?.id;
  if (selected !== view.selected || active !== view.selectedIndex) {
    view.selected = selected;
    view.selectedIndex = active;
    if (active >= 0) view.scroll.scroll_to_item(active);
  }
  const stop = (_e, cx) => cx.stop_propagation();
  const button = (id, labelText, action) => div().id(id).size(26).flex_none().flex().items_center().justify_center().rounded(6).text_color(t.dim).hover(s => s.bg(t.hover).text_color(t.foreground))
    .on_mouse_down("left", stop).on_click(action).child(labelText);
  const item = (c, i) => {
    const lead = allPanes.find(p => p.tab === c.id && p.program?.agent) ?? allPanes.find(p => p.tab === c.id);
    const program = lead?.program;
    const name = label(c, lead);
    return h_flex().id(`tab-${c.id}`).h(29).px(10).gap(7).rounded(7).items_center().flex_none().text_size(12.8)
      .when(i <= active, el => el.on_size_change((_size, cx) => { view.scroll.scroll_to_item(active); cx.notify(); }))
      .border(1).border_color(c.active ? t.border : t.chrome).bg(c.active ? t.raised : t.chrome).text_color(c.active ? t.foreground : t.muted)
      .hover(s => s.bg(t.hover))
      .on_mouse_down("left", (e, cx) => { cx.stop_propagation(); view.pressed = i; if (e.click_count === 2) run("tab.rename", { id: c.id }); })
      .on_mouse_up("left", () => {
        const from = view.pressed; view.pressed = null;
        if (from !== null && from !== i) run("tab.move", { id: shown[from].id, index: space.tabs.findIndex(x => x.id === c.id) + 1 });
        else run("tab.select", { id: c.id });
      })
      .on_mouse_down("right", (_e, cx) => { cx.stop_propagation(); openOverlay({ mod: "menu", kind: "popup", width: 200, props: { tab: "tab", target: { id: c.id, space: space.id } } }); })
      .when(Boolean(program?.icon), el => el.child(program.tint ? svg(program.icon).size(14).text_color(t.muted) : image(program.icon).size(14)))
      .child(div().max_w(170).truncate().child(c.zoomed ? `${name} Z` : name))
      .when(c.status === "working", el => el.child(svg("mods/workspace-chrome/spinner.svg").size(12).flex_none().text_color(t.working).spin(900)))
      .when(c.status === "blocked" || c.status === "done", el => el.child(div().size(6).rounded(3).bg(t[c.status])))
      .when(c.active, el => el.child(div().id(`close-${c.id}`).text_color(t.dim).on_mouse_down("left", stop).on_click((_e, cx) => { cx.stop_propagation(); run("tab.close", { id: c.id }); }).child(svg("mods/pane/close.svg").size(12))));
  };
  const root = layout(), sidebar = find(root, "sidebar"), w = window();
  const lightsPadding = w?.platform === "macos" && w.titlebar === "none" && !w.fullscreen && root?.traffic_lights
    ? Math.max(10, (root.traffic_lights[0] + 64) / settings().zoom - (sidebar?.hidden ? 0 : sidebar?.size?.px ?? 0))
    : 10;
  const panel = find(root, "panel"), body = find(root, "panel-body");
  const baseEnabled = shared("workspace.panels") ?? ["files", "changes"];
  const enabled = baseEnabled.includes("loops") ? baseEnabled : [...baseEnabled, "loops"];
  view.syncPanel(space, panel, body, enabled);

  const currentAddr = resolveCurrentTerminal(focused(), allSpaces, allPanes);
  const currentTerm = currentAddr ? allTerminals.find(x => x.state === "live" && x.address === currentAddr) : null;
  const currentLoopCount = currentTerm ? parseLoops(currentTerm.labels?.["herdr.loops"]).length : 0;

  const panelButton = (id, text, iconPath, count = 0) => {
    const on = panel && !panel.hidden && body?.active === id;
    return h_flex().id(`panel-${id}`).h(26).px(8).gap(5).flex_none().items_center().rounded(6)
      .text_size(12).bg(on ? t.selected : t.chrome).text_color(on ? t.foreground : t.muted)
      .hover(s => s.bg(t.hover)).on_mouse_down("left", stop)
      .on_click(() => {
        if (on) run("layout.hide", { id: "panel" });
        else { run("layout.select", { stack: "panel-body", child: id }); run("layout.show", { id: "panel" }); }
      })
      .child(svg(iconPath).size(14))
      .child(text)
      .when(count > 0, el => el.child(div().text_size(11).text_color(on ? t.muted : t.dim).child(String(count))));
  };

  return h_flex().size_full().min_w(0).px(10).pl(lightsPadding).gap(3).items_center().bg(t.chrome).border_b(1).border_color(t.border)
    .on_mouse_up("left", () => { view.pressed = null; })
    .on_scroll_wheel(e => view.wheel(e))
    .child(h_flex().id("tab-scroll").flex_1().min_w(0).h_full().items_center().gap(3).overflow_x_scrollbar().track_scroll(view.scroll)
      .on_size_change((_size, cx) => { if (active >= 0) { view.scroll.scroll_to_item(active); cx.notify(); } })
      .children(shown.map(item)))
    .child(button("tab-new", "+", () => openOverlay({ mod: "new-menu", kind: "popup", width: 250, height: 512 })))
    .when(enabled.includes("files"), el => el.child(panelButton("files", "Files", "mods/files/file.svg")))
    .when(enabled.includes("changes"), el => el.child(panelButton("changes", "Changes", "mods/changes/changes.svg")))
    .child(panelButton("loops", "Loops", "mods/loops/infinity.svg", currentLoopCount))
    .when(w?.buttons, el => el.child(button("minimize", w.platform === "windows" ? svg("mods/tabs/win-min.svg").size(16) : "−", () => run("window.minimize")))
      .child(button("maximize", w.platform === "windows" ? svg(`mods/tabs/${w.maximized ? "win-restore" : "win-max"}.svg`).size(16) : "□", () => run("window.maximize")))
      .child(button("close-window", w.platform === "windows" ? svg("mods/tabs/win-close.svg").size(16) : svg("mods/pane/close.svg").size(13), () => run("window.close"))));
}
