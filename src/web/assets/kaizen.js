const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
const state = { ws: null, seq: 0, route: routeName(), connected: false };

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
  if (msg.type === "status") {
    updateToolCount(msg.tools);
    return addFeed("status", "daemon", `${msg.tools.length} web tools available`);
  }
  if (msg.type === "pong") return;
  busy(msg.tool, false);
  if (msg.type === "error") return showOutput(msg.id, `error: ${msg.error}`);
  showOutput(msg.id, renderOutput(msg.output));
  updateDashboard(msg.tool, msg.output);
}

function argsFor(form, btn) {
  return aliasArgs(btn.dataset.toolSubmit, {
    ...formArgs(form),
    ...json(btn.dataset.extra)
  });
}

const numberFields = new Set(["days", "duration_days", "limit", "score", "target_pct"]);

function formArgs(form) {
  const pairs = [...new FormData(form).entries()].map(normalizedPair).filter(hasValue);
  return checkedArgs(form, Object.fromEntries(pairs));
}

function normalizedPair([key, value]) {
  return [key, normalizeValue(key, value)];
}

function hasValue([, value]) {
  return value !== undefined;
}

function normalizeValue(key, value) {
  const text = `${value}`.trim();
  if (text === "") return undefined;
  return numberFields.has(key) ? Number(text) : text;
}

function checkedArgs(form, args) {
  $$("input[type=checkbox]", form).forEach(input => setCheckboxArg(args, input));
  return args;
}

function setCheckboxArg(args, input) {
  if (!input.name || !input.checked) delete args[input.name];
  else args[input.name] = true;
}

function aliasArgs(tool, args) {
  if (tool === "kaizen_query") return renameArg(args, "query", "expr");
  if (tool === "kaizen_annotate_session") return renameArg(args, "id", "session_id");
  return args;
}

function renameArg(args, from, to) {
  if (args[from] === undefined || args[to] !== undefined) return args;
  const { [from]: value, ...rest } = args;
  return { ...rest, [to]: value };
}

function withWorkspace(args) {
  const workspace = $("input[name=workspace]")?.value?.trim();
  return workspace ? { ...args, workspace } : args;
}

function showRoute(route) {
  state.route = route;
  $$(".screen").forEach(screen => screen.hidden = screen.id !== route);
  $$("[data-route]").forEach(a => a.setAttribute("aria-current", a.dataset.route === route ? "page" : "false"));
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

function updateToolCount(tools) {
  $("#metric-tools").textContent = Array.isArray(tools) ? tools.length : "-";
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
