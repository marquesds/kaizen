import { renderOutput } from "/assets/kaizen-render.js";

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
const state = { ws: null, seq: 0, route: routeName(), pending: new Map(), history: [], features: [] };
document.addEventListener("DOMContentLoaded", () => {
  bindRoutes();
  bindActions();
  bindDeveloper();
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
    history.pushState(null, "", a.href);
    showRoute(routeName());
  }));
  window.addEventListener("popstate", () => showRoute(routeName()));
}
function bindActions() {
  $$("[data-feature]").forEach(node => node.addEventListener(node.form && node.type === "submit" ? "submit" : "click", event => {
    event.preventDefault();
    runFeature(event.submitter || node);
  }));
  $$("form").forEach(form => form.addEventListener("submit", event => {
    if (!event.submitter?.dataset.feature) return;
    event.preventDefault();
    runFeature(event.submitter);
  }));
}
function bindDeveloper() {
  $("#open-dev").addEventListener("click", () => $("#developer-drawer").showModal());
  $("#dev-filter").addEventListener("input", renderHistory);
}
function connect() {
  const token = new URLSearchParams(location.search).get("token") || localStorage.kaizenToken || "";
  if (token) localStorage.kaizenToken = token;
  const scheme = location.protocol === "https:" ? "wss" : "ws";
  state.ws = new WebSocket(`${scheme}://${location.host}/ws?token=${encodeURIComponent(token)}`);
  state.ws.addEventListener("open", () => {
    pill("connected", "");
    send({ type: "subscribe", id: "status" });
    call(featureButton("kaizen_summary"), { json: true });
  });
  state.ws.addEventListener("close", () => {
    pill("disconnected", "danger");
    setTimeout(connect, 1600);
  });
  state.ws.addEventListener("message", event => receive(JSON.parse(event.data)));
}
function runFeature(trigger) {
  const form = trigger.closest("form");
  const args = aliasArgs(trigger, { ...formArgs(form), ...json(trigger.dataset.args) });
  const missing = required(trigger).filter(name => args[name] === undefined);
  if (missing.length) return localError(trigger, `Missing ${missing.join(", ")}`);
  call(trigger, args);
}
function call(trigger, args) {
  const id = `w${++state.seq}`;
  const feature = trigger.dataset.feature;
  const meta = featureMeta(feature);
  const request = { type: "call", id, tool: feature, args: withWorkspace(args) };
  state.pending.set(id, { trigger, request, renderer: trigger.dataset.render || meta?.renderer || "detail" });
  busy(trigger, true);
  record({ id, feature, label: meta?.label || trigger.textContent.trim(), request, status: "pending" });
  send(request);
}
function receive(msg) {
  if (msg.type === "status") return receiveStatus(msg);
  if (msg.type === "pong") return;
  const pending = state.pending.get(msg.id);
  if (pending) busy(pending.trigger, false);
  updateHistory(msg.id, msg);
  if (msg.type === "error") return showError(pending?.trigger, msg.error);
  renderWorkflow(pending, msg.output);
}
function receiveStatus(msg) {
  state.features = msg.features || [];
  $("#metric-tools").textContent = state.features.length || "-";
  addFeed("status", "daemon", `${state.features.length} web features available`);
}
function renderWorkflow(pending, output) {
  const target = targetFor(pending?.trigger);
  renderOutput(target, output, pending?.renderer);
  updateDashboard(pending?.request.tool, output);
}
function formArgs(form) {
  if (!form) return {};
  const pairs = [...new FormData(form).entries()].map(([k, v]) => [k, normalize(k, v)]).filter(([, v]) => v !== undefined);
  return Object.fromEntries(pairs);
}
function normalize(key, value) {
  const text = `${value}`.trim();
  if (!text) return undefined;
  return ["days", "duration_days", "limit", "score", "target_pct"].includes(key) ? Number(text) : text;
}
function aliasArgs(trigger, args) {
  const aliases = json(trigger.dataset.alias);
  return Object.entries(aliases).reduce((next, [from, to]) => {
    if (next[from] !== undefined && next[to] === undefined) next[to] = next[from];
    return next;
  }, args);
}
function withWorkspace(args) {
  const workspace = $("#workspace-input")?.value?.trim();
  return workspace ? { ...args, workspace } : args;
}
function required(trigger) {
  return (trigger.dataset.required || featureMeta(trigger.dataset.feature)?.required_args?.join(",") || "").split(",").filter(Boolean);
}
function showRoute(route) {
  state.route = route;
  $$(".screen").forEach(screen => screen.hidden = screen.id !== route);
  $$("[data-route]").forEach(a => a.setAttribute("aria-current", a.dataset.route === route ? "page" : "false"));
}
function targetFor(trigger) {
  return $(`#${trigger?.dataset.target}`) || $(`#${trigger?.closest(".screen")?.id}-output`) || $("#live-feed");
}
function showError(trigger, text) {
  const target = targetFor(trigger);
  target.classList.remove("empty");
  target.textContent = text;
  target.setAttribute("role", "alert");
}
function localError(trigger, text) {
  showError(trigger, text);
  record({ id: "local", feature: trigger.dataset.feature, status: "error", response: { error: text } });
}
function updateDashboard(tool, output) {
  if (tool !== "kaizen_summary") return;
  const value = jsonValue(output);
  $("#metric-sessions").textContent = value.stats?.sessions ?? value.sessions ?? "-";
  $("#metric-cost").textContent = value.cost_usd == null ? "-" : `$${Number(value.cost_usd).toFixed(2)}`;
  $("#metric-tokens").textContent = value.stats?.tokens_total?.toLocaleString?.() ?? "-";
}
function addFeed(kind, source, text) {
  const li = document.createElement("li");
  li.className = "is-new";
  li.dataset.kind = kind;
  li.append(timeNode(), textNode(text), chip(source || kind));
  $("#live-feed")?.prepend(li);
}
function record(entry) {
  state.history.unshift({ at: new Date().toLocaleTimeString(), ...entry });
  state.history = state.history.slice(0, 80);
  renderHistory();
}
function updateHistory(id, response) {
  const item = state.history.find(row => row.id === id);
  if (item) Object.assign(item, { status: response.type, response });
  renderHistory();
}
function renderHistory() {
  const term = $("#dev-filter").value.toLowerCase();
  const list = $("#dev-list");
  list.replaceChildren(...state.history.filter(row => JSON.stringify(row).toLowerCase().includes(term)).map(historyButton));
}
function historyButton(row) {
  const btn = document.createElement("button");
  btn.className = "btn";
  btn.type = "button";
  btn.textContent = `${row.at} ${row.label || row.feature} ${row.status}`;
  btn.addEventListener("click", () => $("#dev-raw").textContent = JSON.stringify(row, null, 2));
  return btn;
}

const featureMeta = tool => state.features.find(feature => feature.tool === tool);
const featureButton = tool => $(`[data-feature="${tool}"]`);
const jsonValue = output => output?.kind === "json" ? output.value : {};
const send = value => state.ws?.readyState === WebSocket.OPEN && state.ws.send(JSON.stringify(value));
const busy = (node, on) => node?.setAttribute("aria-busy", String(on));
const pill = (text, tone) => ($("#socket-pill").textContent = text, $("#socket-pill").className = `pill ${tone || ""}`);
const json = text => { try { return text ? JSON.parse(text) : {}; } catch { return {}; } };
const timeNode = () => Object.assign(document.createElement("span"), { className: "mono", textContent: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) });
const textNode = text => Object.assign(document.createElement("span"), { textContent: text });
const chip = text => Object.assign(document.createElement("span"), { className: "chip", textContent: text });
