import { renderDetail } from "./kaizen-detail.js";
import { count, dateTime, label, money, shortId, statusLabel, statusTone, topCommands } from "./kaizen-format.js";

const $ = selector => document.querySelector(selector);
const MAX_SESSION_ROWS = 30;
let renderedSessions = "";

export function setConnection(text, tone = "neutral") {
  const target = $("#connection-state");
  target.textContent = text;
  target.dataset.tone = tone;
}

export function setJourney(tone, title, message) {
  const isError = tone === "error" || tone === "auth";
  $("#journey-state").dataset.tone = tone;
  $("#journey-state-title").textContent = title;
  $("#journey-status").hidden = isError;
  $("#journey-error").hidden = !isError;
  (isError ? $("#journey-error") : $("#journey-status")).textContent = message;
}

export function setBusy(busy) {
  $("#observe-screen").setAttribute("aria-busy", String(busy));
  $("#refresh-report").disabled = busy || !$("#project-select").value;
}

export function renderProjects(projects, selected) {
  const select = $("#project-select");
  select.replaceChildren(...projects.map(path => option(path, path === selected)));
  select.disabled = projects.length === 0;
  if (selected) select.value = selected;
  setBusy(false);
}

export function showManual(path = "") {
  $("#manual-workspace").open = true;
  $("#manual-path").value = path;
}

export function renderReport(report) {
  renderTotals(report?.totals || {});
  renderInsights(report);
  renderSessions(
    report?.sessions || [],
    report?.selected?.session?.id,
    report?.totals?.session_count,
  );
  renderDetail(report);
}

function renderInsights(report) {
  const sessions = report?.sessions || [];
  const attention = sessions.filter(row => ["errored", "orphaned"].includes(row.status)).length;
  const quality = report?.quality || {};
  renderToolInsight(report, sessions);
  setInsight("attention", attention ? `${count(attention)} need attention` : "No recent warnings", `${count(sessions.length)} checked; includes errors and missing completion events`);
  setInsight("coverage", `${percent(quality.token_coverage_pct)} token coverage`, `${percent(quality.cost_coverage_pct)} cost coverage`);
}

function renderToolInsight(report, sessions) {
  const [tool, calls] = topTool(sessions);
  const commands = topCommands(report?.selected?.events || []);
  const names = commands.map(([name]) => name).join(" · ");
  const total = commands.reduce((sum, [, value]) => sum + value, 0);
  const note = names ? `${count(total)} calls across top commands in selected session; ${label(tool)} leads visible tools` : `${count(calls)} calls in visible sessions`;
  setInsight("tools", names || (tool ? `${label(tool)} leads` : "No tool calls yet"), note);
}

function topTool(sessions) {
  const counts = sessions.flatMap(row => row.top_tools || []).reduce(addTool, new Map());
  return [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))[0] || ["", 0];
}

function addTool(counts, [tool, calls]) {
  counts.set(tool, (counts.get(tool) || 0) + calls);
  return counts;
}

function setInsight(id, title, note) {
  $(`#insight-${id}`).textContent = title;
  $(`#insight-${id}-note`).textContent = note;
}

function percent(value) {
  return `${Math.round(Number(value) || 0)}%`;
}

function renderTotals(totals) {
  $("#total-sessions").textContent = count(totals.session_count);
  $("#active-sessions").textContent = count(totals.running_count);
  $("#total-errors").textContent = count(totals.error_count);
  $("#total-cost").textContent = money(totals.cost_usd_e6);
}

function renderSessions(sessions, selectedId, totalCount) {
  const visible = sessions.slice(0, MAX_SESSION_ROWS);
  const total = totalCount || sessions.length;
  const signature = sessionSignature(visible);
  if (signature !== renderedSessions) replaceSessionRows(visible, selectedId, signature);
  markSelected(selectedId);
  $("#session-empty").hidden = sessions.length > 0;
  $("#session-count-note").textContent = sessionCountNote(visible.length, total);
}

function replaceSessionRows(sessions, selectedId, signature) {
  $("#session-rows").replaceChildren(...sessions.map(row => sessionRow(row, selectedId)));
  renderedSessions = signature;
}

function markSelected(selectedId) {
  $("#session-rows").querySelectorAll("button[data-session-id]").forEach(button =>
    button.setAttribute("aria-current", String(button.dataset.sessionId === selectedId)));
}

function sessionSignature(sessions) {
  return JSON.stringify(sessions.map(session => [
    session.id, session.agent, session.model, session.started_at_ms, session.status,
    session.status_reason, session.cost_usd_e6, session.error_count,
  ]));
}

function sessionCountNote(visible, total) {
  if (!total) return "No sessions";
  if (visible === total) return `${count(total)} newest`;
  return `${count(visible)} of ${count(total)} newest`;
}

function sessionRow(session, selectedId) {
  const row = document.createElement("tr");
  row.append(
    cell(identity(session)),
    cell(session.model || "Unknown"),
    cell(dateTime(session.started_at_ms)),
    cell(status(session)),
    cell(money(session.cost_usd_e6)),
    cell(count(session.error_count)),
    cell(inspect(session, selectedId)),
  );
  return row;
}

function identity(session) {
  const box = document.createElement("div");
  box.append(strong(label(session.agent)));
  const id = element("span", "session-id", shortId(session.id));
  box.append(id);
  return box;
}

function status(session) {
  const node = element("span", "status-label", statusLabel(session.status));
  node.dataset.tone = statusTone(session.status);
  node.title = session.status_reason || "";
  return node;
}

function inspect(session, selectedId) {
  const button = element("button", "inspect-button", "Inspect");
  button.type = "button";
  button.dataset.sessionId = session.id;
  button.setAttribute("aria-current", String(session.id === selectedId));
  button.setAttribute("aria-label", `Inspect ${session.agent} session ${shortId(session.id)}`);
  return button;
}

function option(path, selected) {
  return Object.assign(document.createElement("option"), {
    value: path,
    textContent: projectName(path),
    selected,
  });
}

function projectName(path) {
  const name = path.split("/").filter(Boolean).at(-1) || path;
  return `${name} - ${path}`;
}

function cell(value) {
  const node = document.createElement("td");
  value instanceof Node ? node.append(value) : node.textContent = value;
  return node;
}

function strong(text) {
  return Object.assign(document.createElement("strong"), { textContent: text });
}

function element(tag, className, text) {
  return Object.assign(document.createElement(tag), { className, textContent: text });
}
