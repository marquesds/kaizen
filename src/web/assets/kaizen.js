const $ = (sel, root = document) => root.querySelector(sel);
const state = { ws: null, seq: 0, selected: null, history: [], reconnect: 0 };
document.addEventListener("DOMContentLoaded", () => {
  bindControls();
  bindDeveloper();
  restoreWorkspace();
  connect();
});
function bindControls() {
  $("#snapshot-form").addEventListener("submit", event => {
    event.preventDefault();
    requestSnapshot();
  });
  $("#session-rows").addEventListener("click", selectSession);
}
function bindDeveloper() {
  $("#open-dev").addEventListener("click", () => $("#developer-drawer").showModal());
  $("#dev-filter").addEventListener("input", renderHistory);
}
function restoreWorkspace() {
  const params = new URLSearchParams(location.search);
  $("#workspace-input").value = params.get("workspace") || localStorage.kaizenWorkspace || "";
}
function connect() {
  const token = new URLSearchParams(location.search).get("token") || localStorage.kaizenToken || "";
  if (token) localStorage.kaizenToken = token;
  const scheme = location.protocol === "https:" ? "wss" : "ws";
  state.ws = new WebSocket(`${scheme}://${location.host}/ws?token=${encodeURIComponent(token)}`);
  state.ws.addEventListener("open", onOpen);
  state.ws.addEventListener("close", onClose);
  state.ws.addEventListener("message", event => receive(JSON.parse(event.data)));
}
function onOpen() {
  pill("connected", "");
  send({ type: "subscribe", id: "status" });
  if ($("#workspace-input").value.trim()) requestSnapshot();
}
function onClose() {
  pill("disconnected", "danger");
  clearTimeout(state.reconnect);
  state.reconnect = setTimeout(connect, 1600);
}
function requestSnapshot() {
  const workspace = $("#workspace-input").value.trim();
  if (!workspace) return showError("Workspace required.");
  const id = `v${++state.seq}`;
  const request = { type: "visualization_snapshot", id, workspace, selected_session_id: state.selected };
  localStorage.kaizenWorkspace = workspace;
  setBusy(true);
  if (!send(request)) return showError("WebSocket not connected.");
  record({ id, label: "visualization snapshot", status: "pending", workspace });
}
function receive(msg) {
  updateHistory(msg.id, msg);
  if (msg.type === "status") return receiveStatus(msg);
  if (msg.type === "visualization_snapshot") return receiveSnapshot(msg);
  if (msg.type === "error") return showError(msg.error || "Request failed.");
}
function receiveStatus(msg) {
  $("#tool-count").textContent = `MCP tools: ${(msg.tools || []).length}`;
}
function receiveSnapshot(msg) {
  setBusy(false);
  clearError();
  state.selected = msg.report?.selected?.session?.id || state.selected;
  renderReport(msg.report);
}
function selectSession(event) {
  const button = event.target.closest("button[data-session-id]");
  if (!button) return;
  state.selected = button.dataset.sessionId;
  $("#selected-session").textContent = shortId(state.selected);
  requestSnapshot();
}
function renderReport(report) {
  renderTotals(report.totals || {});
  renderActivity(report.activity?.day_bins || []);
  renderSessions(report.sessions || []);
  renderSelected(report.selected);
  renderQuality(report.quality || {});
  $("#report-status").textContent = `Snapshot generated for ${report.workspace || "workspace"}.`;
}
function renderTotals(totals) {
  $("#total-sessions").textContent = fmt(totals.session_count);
  $("#total-events").textContent = fmt(totals.event_count);
  $("#total-tokens").textContent = fmt(totals.tokens?.total);
  $("#total-cost").textContent = cost(totals.cost_usd_e6);
  $("#running-sessions").textContent = fmt(totals.running_count);
  $("#total-errors").textContent = fmt(totals.error_count);
}
function renderActivity(bins) {
  const active = bins.filter(bin => !bin.is_break || bin.event_count).slice(-48);
  $("#activity-bars").replaceChildren(...active.map(activityBar));
}
function activityBar(bin) {
  const li = el("li", "activity-bar");
  li.style.setProperty("--heat", Math.max(0.04, bin.heat || 0));
  li.append(el("span", "mono", time(bin.start_ms)), el("strong", "", fmt(bin.event_count)));
  li.title = `${fmt(bin.event_count)} events`;
  return li;
}
function renderSessions(sessions) {
  $("#session-rows").replaceChildren(...sessions.map(sessionRow));
  $("#session-empty").hidden = sessions.length > 0;
}
function sessionRow(session) {
  const row = document.createElement("tr");
  const button = el("button", "link-btn", "Inspect");
  button.type = "button";
  button.dataset.sessionId = session.id;
  row.replaceChildren(td(shortId(session.id)), td(session.status), td(fmt(session.event_count)), td(fmt(session.tokens?.total)), td(button));
  return row;
}
function renderSelected(selected) {
  const target = $("#selected-detail");
  if (!selected) return emptySelected(target);
  const items = selected.events.slice(-12).map(eventLine);
  target.classList.remove("empty");
  target.replaceChildren(el("p", "meta", selected.session.id), ...items);
}
function emptySelected(target) {
  target.classList.add("empty");
  target.replaceChildren(text("No session selected."));
}
function eventLine(event) {
  const line = el("div", "event-line");
  line.append(el("span", "mono", time(event.ts_ms)), el("strong", "", event.kind), text(event.tool || ""));
  return line;
}
function renderQuality(quality) {
  const rows = [["Token coverage", pct(quality.token_coverage_pct)], ["Cost coverage", pct(quality.cost_coverage_pct)], ["Partial cost sessions", fmt(quality.partial_cost_sessions)]];
  $("#quality-list").replaceChildren(...rows.flatMap(metricRow));
  $("#quality-warnings").replaceChildren(...(quality.warnings || []).map(warn));
}
function metricRow([name, value]) {
  return [el("dt", "", name), el("dd", "", value)];
}
function warn(message) {
  return el("li", "", message);
}
function showError(message) {
  setBusy(false);
  $("#report-status").textContent = message;
  $("#report-status").setAttribute("role", "alert");
}
function clearError() {
  $("#report-status").removeAttribute("role");
}
function setBusy(on) {
  $("#refresh-report").setAttribute("aria-busy", String(on));
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
  const rows = state.history.filter(row => JSON.stringify(row).toLowerCase().includes(term));
  $("#dev-list").replaceChildren(...rows.map(historyButton));
}
function historyButton(row) {
  const btn = el("button", "btn", `${row.at} ${row.label || row.status}`);
  btn.type = "button";
  btn.addEventListener("click", () => $("#dev-raw").textContent = JSON.stringify(row, null, 2));
  return btn;
}
function td(value) {
  const cell = document.createElement("td");
  value instanceof Node ? cell.append(value) : cell.textContent = value;
  return cell;
}
const send = value => state.ws?.readyState === WebSocket.OPEN ? (state.ws.send(JSON.stringify(value)), true) : false;
const pill = (textValue, tone) => ($("#socket-pill").textContent = textValue, $("#socket-pill").className = `pill ${tone || ""}`);
const fmt = value => Number(value || 0).toLocaleString();
const cost = value => `$${(Number(value || 0) / 1_000_000).toFixed(4)}`;
const pct = value => `${Number(value || 0).toFixed(0)}%`;
const shortId = id => id ? id.slice(0, 12) : "-";
const time = ms => ms ? new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : "-";
const text = value => document.createTextNode(value);
const el = (tag, className, value) => Object.assign(document.createElement(tag), { className, textContent: value || "" });
