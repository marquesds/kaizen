import { AUTO_REFRESH_MS, chooseProject, decodeOutput, projectPaths } from "./kaizen-state.js";
import { bindRawReport, setRawReport } from "./kaizen-raw.js";
import { createTransport } from "./kaizen-transport.js";
import {
  renderProjects,
  renderReport,
  setBusy,
  setConnection,
  setJourney,
  showManual,
} from "./kaizen-render.js";
const $ = selector => document.querySelector(selector);
const params = new URLSearchParams(location.search);
const token = params.get("token") || localStorage.kaizenToken || "";
const requestedWorkspace = params.get("workspace") || "";
const state = {
  seq: 0,
  pending: new Map(),
  projects: [],
  workspace: "",
  selected: "",
  snapshotPending: false,
  refreshTimer: 0,
  lastRefresh: 0,
};
const transport = createTransport({
  url: socketUrl,
  onOpen: connected,
  onDisconnect: disconnected,
  onAuthFailure: () => showAuth("Authorization failed. Reopen Kaizen from daemon output."),
  onMessage: receive,
});
document.addEventListener("DOMContentLoaded", start);
function start() {
  bindControls();
  $("#manual-path").value = requestedWorkspace;
  if (!token) return showAuth("Authorization required. Reopen Kaizen from daemon output.");
  localStorage.kaizenToken = token;
  setConnection("Connecting", "neutral");
  setJourney("neutral", "Connecting", "Opening a secure local connection.");
  transport.connect();
}
function bindControls() {
  bindRawReport();
  $("#refresh-report").addEventListener("click", () => requestSnapshot(true));
  $("#project-select").addEventListener("change", event => activateProject(event.target.value));
  $("#session-rows").addEventListener("click", selectSession);
  $("#manual-form").addEventListener("submit", openManualPath);
  document.addEventListener("visibilitychange", visibilityChanged);
}
function socketUrl() {
  const scheme = location.protocol === "https:" ? "wss" : "ws";
  return `${scheme}://${location.host}/ws?token=${encodeURIComponent(token)}`;
}
function connected() {
  setConnection("Connected", "ready");
  discoverProjects();
}
function disconnected() {
  state.snapshotPending = false;
  setBusy(false);
  clearRefresh();
  setConnection("Reconnecting", "danger");
  setJourney("error", "Connection lost", "Trying the secure local connection again.");
}
function discoverProjects() {
  setBusy(true);
  setJourney("neutral", "Finding project", "Checking current and recently observed projects.");
  sendCall("kaizen_sessions_list", {
    all_workspaces: true,
    json: true,
    limit: 50,
  }, "projects");
}
function sendCall(tool, args, purpose) {
  const id = `call-${++state.seq}`;
  state.pending.set(id, purpose);
  if (!transport.send({ type: "call", id, tool, args })) fail("Local connection is not ready.");
}
function receive(raw) {
  let message;
  try {
    message = JSON.parse(raw);
  } catch {
    return fail("Unreadable response from local Kaizen server.");
  }
  if (message.type === "result") return receiveResult(message);
  if (message.type === "visualization_snapshot") return receiveSnapshot(message);
  if (message.type === "error") return fail(message.error || "Request failed.");
}
function receiveResult(message) {
  const purpose = state.pending.get(message.id);
  state.pending.delete(message.id);
  if (purpose === "projects") receiveProjects(decodeOutput(message.output));
}
function receiveProjects(value) {
  const fallback = requestedWorkspace || (!value?.count ? localStorage.kaizenWorkspace || "" : "");
  state.projects = projectPaths(value, fallback);
  const selected = chooseProject(value, state.projects, requestedWorkspace);
  if (!selected) return noProjects();
  renderProjects(state.projects, selected);
  activateProject(selected);
}
function noProjects() {
  setBusy(false);
  renderProjects([], "");
  showManual();
  setJourney("neutral", "No project found", "Use a project path to begin observing local sessions.");
}
function activateProject(workspace) {
  if (!workspace) return;
  state.workspace = workspace;
  state.selected = "";
  localStorage.kaizenWorkspace = workspace;
  $("#manual-path").value = workspace;
  renderProjects(state.projects.includes(workspace) ? state.projects : [workspace, ...state.projects], workspace);
  requestSnapshot(true);
}
function requestSnapshot(announce) {
  if (!state.workspace || state.snapshotPending) return;
  if (!transport.isOpen()) return fail("Local connection is not ready.");
  clearRefresh();
  state.snapshotPending = true;
  setBusy(true);
  if (announce) setJourney("neutral", "Loading observations", "Reading recent local telemetry.");
  transport.send(snapshotRequest());
}
function snapshotRequest() {
  return {
    type: "visualization_snapshot",
    id: `snapshot-${++state.seq}`,
    workspace: state.workspace,
    selected_session_id: state.selected || null,
  };
}
function receiveSnapshot(message) {
  const report = message.report || {};
  state.snapshotPending = false;
  state.selected = report.selected?.session?.id || report.sessions?.[0]?.id || "";
  state.lastRefresh = Date.now();
  setBusy(false);
  setRawReport(report);
  renderReport(report);
  report.sessions?.length ? ready(report) : empty(report);
  scheduleRefresh();
}
function ready(report) {
  const at = new Date(report.generated_at_ms || Date.now()).toLocaleTimeString();
  const visible = report.sessions?.length || 0;
  const total = report.totals?.session_count || visible;
  const scope = visible === total ? `${total}` : `${visible} of ${total}`;
  setJourney("ready", "Observations current", `Showing ${scope} recent sessions. Updated ${at}.`);
}
function empty(report) {
  const project = report.workspace?.split("/").filter(Boolean).at(-1) || "this project";
  setJourney("neutral", "No sessions yet", `No captured agent work for ${project}.`);
}
function selectSession(event) {
  const button = event.target.closest("button[data-session-id]");
  if (!button) return;
  state.selected = button.dataset.sessionId;
  requestSnapshot(true);
}
function openManualPath(event) {
  event.preventDefault();
  const path = $("#manual-path").value.trim();
  if (!path) return fail("Project path is required.");
  activateProject(path);
}
function visibilityChanged() {
  clearRefresh();
  if (document.hidden || !state.workspace) return;
  const remaining = AUTO_REFRESH_MS - (Date.now() - state.lastRefresh);
  remaining <= 0 ? requestSnapshot(false) : scheduleRefresh(remaining);
}
function scheduleRefresh(delay = AUTO_REFRESH_MS) {
  clearRefresh();
  if (document.hidden || !state.workspace || !transport.isOpen()) return;
  state.refreshTimer = setTimeout(() => requestSnapshot(false), Math.max(1_000, delay));
}
function clearRefresh() {
  clearTimeout(state.refreshTimer);
  state.refreshTimer = 0;
}
function fail(message) {
  state.snapshotPending = false;
  setBusy(false);
  setJourney("error", "Could not load observations", message);
  showManual(state.workspace);
  scheduleRefresh();
}
function showAuth(message) {
  setBusy(false);
  setConnection("Authorization required", "danger");
  setJourney("auth", "Authorization required", message);
}
