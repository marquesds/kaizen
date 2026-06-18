export const AUTO_REFRESH_MS = 20_000;

export function decodeOutput(output) {
  const value = output?.value;
  if (typeof value !== "string") return value || {};
  try {
    return JSON.parse(value);
  } catch {
    return {};
  }
}

export function projectPaths(value, explicit = "") {
  const sessions = Array.isArray(value?.sessions) ? value.sessions : [];
  const recent = sessions.map(session => session.workspace);
  const listed = Array.isArray(value?.workspaces) ? value.workspaces : [];
  const scoped = String(value?.workspace || "").startsWith("machine:")
    ? []
    : [value?.workspace];
  return unique([explicit, ...recent, ...listed, ...scoped]);
}

export function chooseProject(value, projects, explicit = "") {
  if (explicit) return explicit;
  const recent = value?.sessions?.[0]?.workspace;
  return recent || projects[0] || "";
}

function unique(values) {
  return [...new Set(values.filter(value => typeof value === "string" && value.trim()))];
}
