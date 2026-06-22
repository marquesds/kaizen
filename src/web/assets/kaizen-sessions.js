import { renderSessionControls } from "./kaizen-session-controls.js";
import { count, dateTime, label, money, shortId, statusLabel, statusTone } from "./kaizen-format.js";

const $ = selector => document.querySelector(selector);
const MAX_SESSION_ROWS = 30;
let renderedSessions = "";

export function renderSessions(report, query) {
  const sessions = (report?.sessions || []).slice(0, MAX_SESSION_ROWS);
  const selectedId = report?.selected?.session?.id;
  const signature = sessionSignature(sessions);
  if (signature !== renderedSessions) replaceSessionRows(sessions, selectedId, signature);
  markSelected(selectedId);
  renderResultState(sessions, report?.session_page, query);
}

function renderResultState(sessions, page = {}, query = "") {
  const total = Number(page.filtered_total) || 0;
  const empty = $("#session-empty");
  empty.hidden = sessions.length > 0;
  empty.textContent = emptyMessage(query);
  $("#session-count-note").textContent = resultCount(total, query);
  renderSessionControls(page);
}

function emptyMessage(query) {
  return query
    ? "No matching sessions. Try a different search."
    : "No captured sessions yet. Run an agent in this project, then refresh.";
}

function resultCount(total, query) {
  if (!total) return query ? "No matching sessions" : "No sessions";
  return query ? `${count(total)} matching sessions` : `${count(total)} sessions`;
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

function sessionRow(session, selectedId) {
  const row = document.createElement("tr");
  const values = [identity(session), session.model || "Unknown", dateTime(session.started_at_ms),
    status(session), money(session.cost_usd_e6), count(session.error_count), inspect(session, selectedId)];
  row.append(...values.map(cell));
  return row;
}

function identity(session) {
  const box = document.createElement("div");
  box.append(strong(label(session.agent)));
  box.append(element("span", "session-id", shortId(session.id)));
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
