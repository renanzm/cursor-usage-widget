const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

const form = document.getElementById("form");
const NUMBER_FIELDS = ["quotaUsd", "windowHours", "rateMax", "lowMax", "highMin", "dayMaxUsd", "pollSeconds"];
const COLOR_KEYS = ["accent", "low", "mid", "high"];
let defaults = null;

const input = (name) => form.elements.namedItem(name);
const quotaMode = () => form.querySelector('input[name="quotaMode"]:checked').value;

function syncQuotaMode() {
  input("quotaUsd").disabled = quotaMode() !== "manual";
}

function fill(s, { colorsOnly = false } = {}) {
  if (!colorsOnly) {
    form.querySelector(`input[name="quotaMode"][value="${s.quotaMode}"]`).checked = true;
    NUMBER_FIELDS.forEach((k) => (input(k).value = s[k]));
    syncQuotaMode();
  }
  COLOR_KEYS.forEach((k) => (input(`colors.${k}`).value = s.colors[k]));
}

function read() {
  const s = { quotaMode: quotaMode(), colors: {} };
  NUMBER_FIELDS.forEach((k) => (s[k] = input(k).valueAsNumber));
  COLOR_KEYS.forEach((k) => (s.colors[k] = input(`colors.${k}`).value));
  return s;
}

function showErrors(fields, message) {
  form.querySelectorAll(".field").forEach((el) => {
    el.classList.remove("invalid");
    el.querySelector(".err").textContent = "";
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
});

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  const s = read();
  // Campo vazio vira NaN, que o Rust não aceita como número.
  const missing = NUMBER_FIELDS.filter((k) => !Number.isFinite(s[k])).map((field) => ({
    field,
    message: "Informe um número.",
  }));
  if (missing.length) return showErrors(missing, "Corrija os campos destacados.");
  try {
    await invoke("save_settings", { settings: s });
    await getCurrentWindow().close();
  } catch (err) {
    showErrors(err?.fields ?? [], err?.message ?? String(err));
  }
});

document.getElementById("cancel").addEventListener("click", () => getCurrentWindow().close());
document.getElementById("reset-colors").addEventListener("click", () => fill(defaults, { colorsOnly: true }));
document.getElementById("reset-all").addEventListener("click", () => {
  fill(defaults);
  showErrors([], "");
});

Promise.all([invoke("get_settings"), invoke("default_settings")]).then(([s, d]) => {
  defaults = d;
  fill(s);
});
