import { renderDetail } from "./kaizen-detail.js";
import { count, label, money, topCommands } from "./kaizen-format.js";
import { renderSessions } from "./kaizen-sessions.js";

const $ = selector => document.querySelector(selector);

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

export function renderReport(report, query = "") {
  renderTotals(report?.totals || {});
  renderInsights(report);
  renderSessions(report, query);
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

function renderTotals(totals) {
  $("#total-sessions").textContent = count(totals.session_count);
  $("#active-sessions").textContent = count(totals.running_count);
  $("#total-errors").textContent = count(totals.error_count);
  $("#total-cost").textContent = money(totals.cost_usd_e6);
}
