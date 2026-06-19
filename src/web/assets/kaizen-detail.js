import { clock, count, dateTime, duration, label, money, shortId, statusExplanation, statusLabel } from "./kaizen-format.js";

const $ = selector => document.querySelector(selector);
const MAX_ITEMS = 40;

export function renderDetail(report) {
  const detail = report?.selected;
  const summary = report?.sessions?.find(row => row.id === detail?.session?.id);
  if (!detail) return renderEmpty();
  $("#selected-session").textContent = shortId(detail.session.id);
  $("#detail-facts").replaceChildren(...facts(detail.session, summary));
  renderPrompt(detail.prompt);
  renderEvents(detail.events || []);
  renderSpans(detail.spans || []);
  renderSimple("#detail-files", detail.files || [], "No files recorded.");
  renderTools(summary?.top_tools || []);
}

function renderEmpty() {
  $("#selected-session").textContent = "No session selected";
  $("#detail-facts").replaceChildren(...fact("Status", "Waiting for data"));
  renderPrompt(null);
  renderSimple("#detail-events", [], "No events available.");
  renderSimple("#detail-spans", [], "No spans available.");
  renderSimple("#detail-files", [], "No files recorded.");
  renderSimple("#detail-tools", [], "No tools recorded.");
}

function facts(session, summary) {
  const status = summary?.status || String(session.status).toLowerCase();
  return [
    ...fact("Agent", label(session.agent)),
    ...fact("Model", session.model || "Unknown"),
    ...fact("Started", dateTime(session.started_at_ms)),
    ...fact("Duration", duration(session.started_at_ms, session.ended_at_ms)),
    ...fact("Status", statusLabel(status)),
    ...(statusExplanation(status) ? fact("Status note", statusExplanation(status)) : []),
    ...fact("Cost", money(summary?.cost_usd_e6)),
    ...fact("Errors", count(summary?.error_count)),
  ];
}

function renderPrompt(prompt) {
  $("#detail-prompt").textContent = prompt || "Prompt unavailable for this session.";
}

function fact(name, value) {
  return [node("dt", name), node("dd", value)];
}

function renderEvents(events) {
  const recent = events.slice(-MAX_ITEMS).reverse();
  renderList("#detail-events", recent, eventRow, "No events available.");
}

function eventRow(event) {
  const row = node("li");
  row.append(node("time", clock(event.ts_ms)), node("strong", label(event.kind)));
  row.append(node("span", event.tool || label(event.source)));
  if (event.payload?.summary) row.append(node("code", event.payload.summary));
  return row;
}

function renderSpans(spans) {
  const rows = flatten(spans).slice(0, MAX_ITEMS);
  renderList("#detail-spans", rows, spanRow, "No spans available.");
}

function spanRow(entry) {
  const span = entry.node.span || {};
  const row = node("li");
  row.className = "span-row";
  row.style.setProperty("--depth", entry.depth);
  row.append(node("strong", span.tool || "Unknown tool"));
  const status = String(span.status || "").toLowerCase() === "orphaned" ? "No result event" : label(span.status);
  row.append(node("span", `${status} | ${span.lead_time_ms || 0} ms`));
  return row;
}

function flatten(nodes, depth = 0) {
  return nodes.flatMap(item => [
    { node: item, depth },
    ...flatten(item.children || [], depth + 1),
  ]);
}

function renderTools(tools) {
  const rows = tools.map(([tool, calls]) => `${tool}: ${count(calls)} calls`);
  renderSimple("#detail-tools", rows, "No tools recorded.");
}

function renderSimple(selector, values, emptyText) {
  renderList(selector, values.slice(0, MAX_ITEMS), value => node("li", value), emptyText);
}

function renderList(selector, values, render, emptyText) {
  const target = $(selector);
  const rows = values.length ? values.map(render) : [empty(emptyText)];
  target.replaceChildren(...rows);
}

function empty(text) {
  const row = node("li", text);
  row.className = "empty-note";
  return row;
}

function node(tag, text = "") {
  return Object.assign(document.createElement(tag), { textContent: text });
}
