const { invoke } = window.__TAURI__.core;
const { t } = I18N;
const closeWindow = () => invoke("close_settings");

const form = document.getElementById("form");
const NUMBER_FIELDS = ["quotaUsd", "windowHours", "rateMax", "lowMax", "highMin", "dayMaxUsd", "pollSeconds"];
const COLOR_KEYS = ["accent", "low", "mid", "high"];
let defaults = null;
let sources = null;
let monitors = [];

const input = (name) => form.elements.namedItem(name);
const quotaMode = () => form.querySelector('input[name="quotaMode"]:checked').value;
const theme = () => form.querySelector('input[name="theme"]:checked')?.value ?? "dark";
// Prévia imediata nesta janela (tema e idioma); o widget só muda ao salvar.
const applyTheme = (th) => (document.documentElement.dataset.theme = th);
let savedTheme = "dark";

async function applyLanguage(language) {
  I18N.set(await invoke("resolve_lang", { language }));
  if (sources) showSources(sources);
  showMonitors();
}

function syncQuotaMode() {
  input("quotaUsd").disabled = quotaMode() !== "manual";
}

const SOURCE_NAMES = { claude: "Claude Code", codex: "Codex", cursor: "Cursor" };
const source = () => form.querySelector('input[name="source"]:checked')?.value ?? "auto";

// Marca o que existe nesta máquina; Claude/Codex sem logs ficam desabilitados.
function showSources(av) {
  const note = (k, text) => (form.querySelector(`[data-src-note="${k}"]`).textContent = text);
  note("claude", t(av.claude ? "found" : "notFound"));
  note("codex", t(av.codex ? "found" : "notFound"));
  note("cursor", t(av.cursor ? "cookieSaved" : "cookieNeeded"));
  form.querySelector('input[name="source"][value="claude"]').disabled = !av.claude;
  form.querySelector('input[name="source"][value="codex"]').disabled = !av.codex;
  const auto = av.claude ? "claude" : av.codex ? "codex" : "cursor";
  note("auto", t("uses", { s: SOURCE_NAMES[auto] }));
}

// Lista de monitores no idioma atual; um monitor salvo e desconectado continua na lista.
function showMonitors(saved = input("monitor").value) {
  const sel = input("monitor");
  sel.querySelectorAll("option:not([value=''])").forEach((o) => o.remove());
  for (const m of monitors) {
    const primary = m.primary ? " · " + (I18N.lang === "pt" ? "principal" : "primary") : "";
    sel.add(new Option(`Monitor ${m.number} · ${m.width}×${m.height}${primary}`, m.name));
  }
  if (saved && !monitors.some((m) => m.name === saved)) {
    sel.add(new Option(t("disconnected", { m: saved.replace(/^\\\\\.\\/, "") }), saved));
  }
  sel.value = saved ?? "";
}

function fill(s, { colorsOnly = false } = {}) {
  if (!colorsOnly) {
    showMonitors(s.monitor ?? "");
    form.querySelector(`input[name="side"][value="${s.side ?? "right"}"]`).checked = true;
    const radio = form.querySelector(`input[name="source"][value="${s.source ?? "auto"}"]`);
    (radio.disabled ? form.querySelector('input[name="source"][value="auto"]') : radio).checked = true;
    form.querySelector(`input[name="quotaMode"][value="${s.quotaMode}"]`).checked = true;
    form.querySelector(`input[name="theme"][value="${s.theme ?? "dark"}"]`).checked = true;
    input("language").value = s.language ?? "auto";
    applyTheme(s.theme ?? "dark");
    NUMBER_FIELDS.forEach((k) => (input(k).value = s[k]));
    syncQuotaMode();
  }
  COLOR_KEYS.forEach((k) => (input(`colors.${k}`).value = s.colors[k]));
}

function read() {
  const s = {
    source: source(),
    quotaMode: quotaMode(),
    theme: theme(),
    language: input("language").value,
    monitor: input("monitor").value,
    side: form.querySelector('input[name="side"]:checked')?.value ?? "right",
    colors: {},
  };
  NUMBER_FIELDS.forEach((k) => (s[k] = input(k).valueAsNumber));
  COLOR_KEYS.forEach((k) => (s.colors[k] = input(`colors.${k}`).value));
  return s;
}

function showErrors(fields, message) {
  form.querySelectorAll(".field").forEach((el) => {
    el.classList.remove("invalid");
    el.querySelector(".err")?.replaceChildren();
  });
  for (const { field, message: m } of fields) {
    const el = form.querySelector(`.field[data-field="${field}"]`);
    if (!el) continue;
    el.classList.add("invalid");
    el.querySelector(".err").textContent = m;
  }
  const msg = document.getElementById("form-msg");
  msg.hidden = !message;
  msg.textContent = message ?? "";
}

form.addEventListener("change", (e) => {
  if (e.target.name === "quotaMode") syncQuotaMode();
  if (e.target.name === "theme") applyTheme(theme());
  if (e.target.name === "language") applyLanguage(e.target.value);
});

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  const s = read();
  // Campo vazio vira NaN, que o Rust não aceita como número.
  const missing = NUMBER_FIELDS.filter((k) => !Number.isFinite(s[k])).map((field) => ({
    field,
    message: t("enterNumber"),
  }));
  if (missing.length) return showErrors(missing, t("fixFields"));
  try {
    await invoke("save_settings", { settings: s });
    await closeWindow();
  } catch (err) {
    showErrors(err?.fields ?? [], err?.message ?? String(err));
  }
});

document.getElementById("cancel").addEventListener("click", () => {
  applyTheme(savedTheme);
  closeWindow();
});
document.getElementById("reset-colors").addEventListener("click", () => fill(defaults, { colorsOnly: true }));
document.getElementById("reset-all").addEventListener("click", () => {
  fill(defaults);
  applyLanguage(defaults.language ?? "auto");
  showErrors([], "");
});

Promise.all([
  invoke("get_settings"),
  invoke("default_settings"),
  invoke("get_sources"),
  invoke("list_monitors"),
]).then(async ([s, d, av, mons]) => {
  defaults = d;
  sources = av;
  monitors = mons;
  savedTheme = s.theme ?? "dark";
  fill(s);
  await applyLanguage(s.language ?? "auto");
});
