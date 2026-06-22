export function initialState() {
  return {
    seq: 0,
    pending: new Map(),
    projects: [],
    workspace: "", selected: "", query: "",
    offset: 0, snapshotPending: "", refreshQueued: false,
  };
}

export function fallbackOffset(report, currentOffset) {
  if (report?.sessions?.length || currentOffset === 0) return null;
  const total = Number(report?.session_page?.filtered_total) || 0;
  const limit = Math.max(1, Number(report?.session_page?.limit) || 30);
  return total ? Math.floor((total - 1) / limit) * limit : 0;
}

export function reportJourney(report, query) {
  if (!report?.sessions?.length) return emptyJourney(report, query);
  const page = report.session_page || {};
  const first = (Number(page.offset) || 0) + 1;
  const last = first + report.sessions.length - 1;
  const total = Number(page.filtered_total) || report.sessions.length;
  const scope = query ? "matching sessions" : "sessions";
  const at = new Date(report.generated_at_ms || Date.now()).toLocaleTimeString();
  return ["ready", "Observations current", `Showing ${first}-${last} of ${total} ${scope}. Updated ${at}.`];
}

function emptyJourney(report, query) {
  if (query) return ["neutral", "No matching sessions", "Try a different search."];
  const project = report?.workspace?.split("/").filter(Boolean).at(-1) || "this project";
  return ["neutral", "No sessions yet", `No captured agent work for ${project}.`];
}
