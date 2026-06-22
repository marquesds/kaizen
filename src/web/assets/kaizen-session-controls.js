const $ = selector => document.querySelector(selector);
const DEBOUNCE_MS = 250;
let onChange = () => {};
let searchTimer;
let page = { offset: 0, limit: 30, next_offset: null, filtered_total: 0 };

export function bindSessionControls(change) {
  onChange = change;
  $("#session-search").addEventListener("input", debounceSearch);
  $("#session-previous").addEventListener("click", previousPage);
  $("#session-next").addEventListener("click", nextPage);
}

export function renderSessionControls(meta) {
  page = normalizePage(meta);
  $("#session-previous").disabled = page.offset === 0;
  $("#session-next").disabled = page.next_offset === null;
  $("#session-page-status").textContent = pageStatus(page);
}

function debounceSearch(event) {
  window.clearTimeout(searchTimer);
  const query = event.target.value.trim();
  searchTimer = window.setTimeout(() => onChange({ query, offset: 0 }), DEBOUNCE_MS);
}

function previousPage() {
  const offset = Math.max(0, page.offset - page.limit);
  onChange({ query: searchQuery(), offset });
}

function nextPage() {
  if (page.next_offset === null) return;
  onChange({ query: searchQuery(), offset: page.next_offset });
}

function searchQuery() {
  return $("#session-search").value.trim();
}

function normalizePage(meta = {}) {
  return {
    filtered_total: Number(meta.filtered_total) || 0,
    offset: Number(meta.offset) || 0,
    limit: Math.max(1, Number(meta.limit) || 30),
    next_offset: Number.isInteger(meta.next_offset) ? meta.next_offset : null,
  };
}

function pageStatus(meta) {
  if (!meta.filtered_total) return "No result pages";
  const first = meta.offset + 1;
  const last = Math.min(meta.offset + meta.limit, meta.filtered_total);
  return `Results ${first}-${last} of ${meta.filtered_total}`;
}
