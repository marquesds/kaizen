const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
const state = { ws: null, seq: 0, route: routeName(), connected: false };
const titles = {
  dashboard: ["Live metrics", "Realtime session, tool, cost, token, eval, feedback, and review data."],
  "session-detail": ["Session browser", "Live-tail session metadata, event search, span tree, and feedback."],
  analysis: ["Analysis workbench", "Retro, metrics, cases, rules, alerts, and review queue controls."],
  experiments: ["Experiments", "Create, run, tag, report, conclude, and archive local experiments."],
  settings: ["Settings", "Workspace setup, sync, ingest, and tool capability checks."]
};

document.addEventListener("DOMContentLoaded", () => {
  bindRoutes();
  bindClicks();
  showRoute(state.route);
  connect();
});

function routeName() {
  const raw = location.pathname.replace("/", "") || "dashboard";
  return ["dashboard", "session-detail", "analysis", "experiments", "settings"].includes(raw) ? raw : "dashboard";
}

function bindRoutes() {
  $$("[data-route]").forEach(a => a.addEventListener("click", event => {
    event.preventDefault();
    history.pushState(null, "", a.getAttribute("href"));
    showRoute(routeName());
  }));
  window.addEventListener("popstate", () => showRoute(routeName()));
}

function bindClicks() {
  $$("[data-tool]").forEach(btn => btn.addEventListener("click", () => call(btn.dataset.tool, json(btn.dataset.args), outputFor(btn))));
  $$("[data-tool-submit]").forEach(btn => btn.addEventListener("click", event => {
    event.preventDefault();
    const form = btn.closest("form");
    call(btn.dataset.toolSubmit, argsFor(form, btn), outputFor(form));
  }));
}

function connect() {
  const token = new URLSearchParams(location.search).get("token") || localStorage.kaizenToken || "";
  if (token) localStorage.kaizenToken = token;
  const scheme = location.protocol === "https:" ? "wss" : "ws";
  state.ws = new WebSocket(`${scheme}://${location.host}/ws?token=${encodeURIComponent(token)}`);
  state.ws.addEventListener("open", () => {
    state.connected = true;
    pill("connected", "");
    send({ type: "subscribe", id: "status" });
    call("kaizen_summary", { json: true }, $("#stream-feed"));
  });
  state.ws.addEventListener("close", () => {
    state.connected = false;
    pill("disconnected", "danger");
    setTimeout(connect, 1600);
  });
  state.ws.addEventListener("message", event => receive(JSON.parse(event.data)));
}

function send(value) {
  if (state.ws?.readyState === WebSocket.OPEN) state.ws.send(JSON.stringify(value));
}

function call(tool, args, target) {
  const id = `w${++state.seq}`;
  busy(tool, true);
  send({ type: "call", id, tool, args: withWorkspace(args) });
  if (target) target.dataset.pending = id;
}

function receive(msg) {
  if (msg.type === "status") return addFeed("status", "daemon", `${msg.tools.length} web tools available`);
  if (msg.type === "pong") return;
  busy(msg.tool, false);
  if (msg.type === "error") return showOutput(msg.id, `error: ${msg.error}`);
  showOutput(msg.id, renderOutput(msg.output));
  updateDashboard(msg.tool, msg.output);
}

function argsFor(form, btn) {
  const data = Object.fromEntries(new FormData(form).entries());
  const extra = json(btn.dataset.extra);
  const tool = btn.dataset.toolSubmit;
  if (tool === "kaizen_session_show" || tool === "get_session_span_tree") return { id: data.id, ...extra };
  if (tool === "mcp/search_sessions") return { query: data.query || "tool:bash", limit: 20, ...extra };
  if (tool === "kaizen_query") return { expr: data.query || "tool:bash", limit: 20, ...extra };
  if (tool === "kaizen_annotate_session") return { session_id: data.id, score: Number(data.score), label: data.label, ...extra };
  if (tool === "kaizen_retro" || tool === "kaizen_alerts_check") return { days: Number(data.days || 7), ...extra };
  if (tool === "kaizen_cases_create") return { session_id: data.session || data.id, reason: data.reason || "web", label: "manual", ...extra };
  if (tool === "kaizen_cases_show" || tool === "kaizen_cases_archive") return { id: data.id, ...extra };
  if (tool.startsWith("kaizen_rules_")) return ruleArgs(tool, data, extra);
  if (tool.startsWith("kaizen_review_")) return { id: data.id, ...extra };
  if (tool === "kaizen_exp_new") return { name: data.name, ...extra };
  if (tool === "kaizen_exp_tag") return { id: data.id, session: data.session, ...extra };
  if (tool.startsWith("kaizen_exp_")) return { id: data.id, ...extra };
  if (tool === "kaizen_ingest_hook") return { payload: data.payload, ...extra };
  return extra;
}

function ruleArgs(tool, data, extra) {
  if (tool === "kaizen_rules_create") return { name: data.name || "web-rule", filter: data.filter, action: "queue_review", message: "web review", ...extra };
  if (tool === "kaizen_rules_run") return { dry_run: true, ...extra };
  if (tool === "kaizen_rules_enable" || tool === "kaizen_rules_disable") return { id: data.id, ...extra };
  return extra;
}

function withWorkspace(args) {
  const workspace = $("input[name=workspace]")?.value?.trim();
  return workspace ? { ...args, workspace } : args;
}

function showRoute(route) {
  state.route = route;
  $$(".screen").forEach(screen => screen.hidden = screen.id !== route);
  $$("[data-route]").forEach(a => a.setAttribute("aria-current", a.dataset.route === route ? "page" : "false"));
  $("#screen-title").textContent = titles[route][0];
  $("#screen-copy").textContent = titles[route][1];
}

function outputFor(node) {
  const screen = node.closest(".screen")?.id || "dashboard";
  return $(`#${screen}-output`) || $("#stream-feed");
}

function showOutput(id, text) {
  const target = $$("[data-pending]").find(node => node.dataset.pending === id) || outputFor($(`#${state.route}`));
  if (target.tagName === "UL") addFeed("result", "tool", text.slice(0, 180));
  else target.textContent = text;
}

function updateDashboard(tool, output) {
  if (tool !== "kaizen_summary" || output.kind !== "json") return;
  const value = output.value;
  $("#metric-sessions").textContent = value.stats?.sessions ?? value.sessions ?? "-";
  $("#metric-cost").textContent = value.cost_usd == null ? "-" : `$${Number(value.cost_usd).toFixed(2)}`;
  $("#metric-tokens").textContent = value.stats?.tokens_total?.toLocaleString?.() ?? "-";
  addFeed("summary", "kaizen", "summary refreshed");
}

function renderOutput(output) {
  if (!output) return "";
  return output.kind === "json" ? JSON.stringify(output.value, null, 2) : output.value;
}

function addFeed(kind, source, text) {
  const feed = $("#stream-feed");
  const li = document.createElement("li");
  li.className = "is-new";
  li.innerHTML = `<span class="mono">${new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span><span></span><span class="chip"></span>`;
  li.children[1].textContent = text;
  li.children[2].textContent = source || kind;
  feed?.prepend(li);
  while (feed?.children.length > 8) feed.lastElementChild.remove();
}

function busy(tool, on) {
  $$(`[data-tool="${tool}"],[data-tool-submit="${tool}"]`).forEach(btn => btn.setAttribute("aria-busy", String(on)));
}

function pill(text, tone) {
  const node = $("#socket-pill");
  node.textContent = text;
  node.className = `pill ${tone || ""}`;
}

function json(text) {
  if (!text) return {};
  try { return JSON.parse(text); } catch { return {}; }
}
