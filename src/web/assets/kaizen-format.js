export function count(value) {
  return Number(value || 0).toLocaleString();
}

export function money(value) {
  const dollars = Number(value || 0) / 1_000_000;
  return dollars < 0.01 && dollars > 0 ? "<$0.01" : `$${dollars.toFixed(2)}`;
}

export function dateTime(ms) {
  if (!ms) return "Unknown";
  return new Intl.DateTimeFormat([], {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(ms));
}

export function clock(ms) {
  if (!ms) return "Unknown time";
  return new Intl.DateTimeFormat([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(ms));
}

export function duration(start, end) {
  if (!start) return "Unknown";
  const elapsed = Math.max(0, Number(end || Date.now()) - Number(start));
  if (elapsed < 60_000) return `${Math.round(elapsed / 1_000)} sec`;
  if (elapsed < 3_600_000) return `${Math.round(elapsed / 60_000)} min`;
  return `${(elapsed / 3_600_000).toFixed(1)} hr`;
}

export function shortId(value) {
  return value ? String(value).slice(0, 12) : "Unknown";
}

export function label(value) {
  return String(value || "unknown")
    .replaceAll("_", " ")
    .replace(/\b\w/g, letter => letter.toUpperCase());
}

export function statusTone(status) {
  if (status === "errored" || status === "orphaned") return "danger";
  if (status === "active" || status === "done") return "ready";
  return "neutral";
}
