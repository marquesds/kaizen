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

export function statusLabel(status) {
  return status === "orphaned" ? "No completion" : label(status);
}

export function statusExplanation(status) {
  if (status === "orphaned") return "No completion event received for 30+ minutes.";
  return "";
}

const SHELL_TOOLS = new Set(["bash", "shell", "exec_command", "run_terminal_cmd", "terminal"]);
const SHELL_WRAPPERS = new Set(["command", "env", "exec", "nohup", "sudo", "time"]);
const SHELL_NOISE = new Set(["cd", "export", "set", "source"]);
const QUOTED_ARGUMENT = /'(?:\\.|[^'\\])*'|"(?:\\.|[^"\\])*"/g;

export function topCommands(events) {
  const counts = (events || []).filter(shellCall).flatMap(commandNames).reduce(addCommand, new Map());
  return [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).slice(0, 3);
}

function shellCall(event) {
  return SHELL_TOOLS.has(String(event.tool || "").toLowerCase()) && event.payload?.summary;
}

function commandNames(event) {
  const shell = event.payload.summary.replace(QUOTED_ARGUMENT, "");
  return shell.split(/&&|\|\||[;|\n]/).map(commandName).filter(Boolean);
}

function commandName(segment) {
  const words = segment.trim().split(/\s+/);
  const word = words.find(item => item && !item.includes("=") && !item.startsWith("-") && !SHELL_WRAPPERS.has(item));
  const command = word?.replace(/^['"]|['"]$/g, "").split("/").pop() || "";
  return SHELL_NOISE.has(command) ? "" : command;
}

function addCommand(counts, command) {
  counts.set(command, (counts.get(command) || 0) + 1);
  return counts;
}
