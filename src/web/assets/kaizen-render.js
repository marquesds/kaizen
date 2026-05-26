export function renderOutput(target, output, renderer = "detail") {
  if (!target) return;
  target.classList.remove("empty");
  const value = output?.kind === "json" ? output.value : output?.value;
  const mode = renderer === "summary" ? "cards" : renderer;
  target.replaceChildren();
  if (mode === "toast") return target.append(card("Done", stringify(value || "Saved")));
  if (mode === "table") return target.append(table(rows(value)));
  if (mode === "tree") return target.append(tree(value));
  if (mode === "cards" || mode === "metrics" || mode === "report") return target.append(...cards(value));
  if (mode === "markdown" || mode === "live") return target.append(pre(stringify(value)));
  target.append(detail(value));
}

function rows(value) {
  if (Array.isArray(value)) return value;
  if (!value || typeof value !== "object") return [{ value }];
  const found = Object.values(value).find(Array.isArray);
  return found || Object.entries(value).map(([key, val]) => ({ key, value: val }));
}

function table(items) {
  if (!items.length) return empty("No rows.");
  const columns = [...new Set(items.flatMap(item => Object.keys(flat(item))).slice(0, 8))];
  const el = document.createElement("table");
  el.append(thead(columns), tbody(items, columns));
  return el;
}

function thead(columns) {
  const head = document.createElement("thead");
  const row = document.createElement("tr");
  columns.forEach(col => row.append(cell("th", col)));
  head.append(row);
  return head;
}

function tbody(items, columns) {
  const body = document.createElement("tbody");
  items.forEach(item => {
    const row = document.createElement("tr");
    const data = flat(item);
    columns.forEach(col => row.append(cell("td", stringify(data[col]))));
    body.append(row);
  });
  return body;
}

function cards(value) {
  const items = rows(value);
  if (!items.length) return [empty("Nothing to show yet.")];
  return items.slice(0, 12).map((item, index) => card(titleFor(item, index), stringify(item)));
}

function detail(value) {
  if (!value || typeof value !== "object") return pre(stringify(value));
  const dl = document.createElement("dl");
  Object.entries(flat(value)).slice(0, 24).forEach(([key, val]) => {
    dl.append(Object.assign(document.createElement("dt"), { textContent: key }));
    dl.append(Object.assign(document.createElement("dd"), { textContent: stringify(val) }));
  });
  return dl;
}

function tree(value) {
  const box = document.createElement("div");
  box.className = "tree";
  treeRows(value).forEach((row, depth) => {
    const span = document.createElement("span");
    span.style.setProperty("--depth", row.depth ?? depth);
    span.textContent = row.label || stringify(row);
    box.append(span);
  });
  return box;
}

function treeRows(value) {
  if (Array.isArray(value)) return value;
  if (typeof value === "string") return value.split("\n").filter(Boolean).map(label => ({ label }));
  return rows(value);
}

function card(title, text) {
  const article = document.createElement("article");
  article.className = "mini-card";
  article.append(Object.assign(document.createElement("b"), { textContent: title }));
  article.append(Object.assign(document.createElement("p"), { textContent: text }));
  return article;
}

function pre(text) {
  const node = document.createElement("pre");
  node.className = "output";
  node.textContent = text;
  return node;
}

function empty(text) {
  const node = document.createElement("p");
  node.className = "empty";
  node.textContent = text;
  return node;
}

function cell(tag, text) {
  return Object.assign(document.createElement(tag), { textContent: text });
}

function flat(value, prefix = "") {
  if (!value || typeof value !== "object" || Array.isArray(value)) return { [prefix || "value"]: value };
  return Object.entries(value).reduce((out, [key, val]) => {
    const name = prefix ? `${prefix}.${key}` : key;
    if (val && typeof val === "object" && !Array.isArray(val)) return { ...out, ...flat(val, name) };
    return { ...out, [name]: val };
  }, {});
}

function titleFor(item, index) {
  return item.title || item.name || item.id || item.key || `Item ${index + 1}`;
}

function stringify(value) {
  if (value === undefined || value === null) return "";
  return typeof value === "string" ? value : JSON.stringify(value, null, 2);
}
