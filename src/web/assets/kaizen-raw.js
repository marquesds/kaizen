const $ = selector => document.querySelector(selector);
const EMPTY = "Open to format latest response.";

let latestReport = null;

export function bindRawReport() {
  $("#developer-raw").addEventListener("toggle", renderRawReport);
}

export function setRawReport(report) {
  latestReport = report;
  renderRawReport();
}

function renderRawReport() {
  const details = $("#developer-raw");
  $("#raw-json").textContent = details.open
    ? JSON.stringify(latestReport, null, 2)
    : EMPTY;
}
