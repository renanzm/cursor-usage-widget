const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const root = $("w");
const { t } = I18N;
const usd = (v, d = 2) =>
  v
    .toLocaleString(I18N.locale, { style: "currency", currency: "USD", minimumFractionDigits: d, maximumFractionDigits: d })
    .replace("US$", "US$ ")
    .replace(/\s+/g, " ");
const num = (v, d) => v.toLocaleString(I18N.locale, { minimumFractionDigits: d, maximumFractionDigits: d });
const hoursLabel = (h) => (Number.isInteger(h) ? String(h) : num(h, 1)) + "h";

function applyColors(c) {
  const st = document.documentElement.style;
  st.setProperty("--orange", c.accent);
  st.setProperty("--lvl-low", c.low);
  st.setProperty("--lvl-mid", c.mid);
  st.setProperty("--lvl-high", c.high);
}

// ---------- velocímetro (arco único com degradê) ----------
const CX = 100, CY = 100, R = 78;
const polar = (f, r) => {
  const th = Math.PI * (1 - f);
  return [CX + r * Math.cos(th), CY - r * Math.sin(th)];
};
function drawTicks(lowMax, highMin, max) {
  $("ticks").innerHTML = [lowMax, highMin]
    .map((v) => {
      const [x0, y0] = polar(v / max, R + 8);
      const [x1, y1] = polar(v / max, R + 14);
      return `<line x1="${x0}" y1="${y0}" x2="${x1}" y2="${y1}"/>`;
    })
    .join("");
  $("scale-max").textContent = max % 1 === 0 ? max : max.toFixed(1);
}

// ---------- equalizador semanal (seg–sex) ----------
const SEGMENTS = 10;
const eqCols = [0, 1, 2, 3, 4].map(() => {
  const col = document.createElement("div");
  col.className = "col future";
  const stack = document.createElement("div");
  stack.className = "stack";
  const segs = [];
  for (let i = SEGMENTS - 1; i >= 0; i--) {
    const seg = document.createElement("i");
    seg.className = "seg";
    // verde (embaixo) -> amarelo -> vermelho (em cima)
    seg.style.setProperty("--c", `hsl(${Math.round(140 - i * 15)} 80% 55%)`);
    stack.appendChild(seg);
    segs[i] = seg;
  }
  const lbl = document.createElement("span");
  lbl.className = "lbl";
  const val = document.createElement("span");
  val.className = "val";
  col.append(stack, lbl, val);
  $("eq").appendChild(col);
  return { col, segs, val, lbl };
});

function renderWeek(s) {
  const week = s.week_usd ?? s.weekUsd ?? [];
  const max = s.dayMaxUsd || 100;
  let total = 0;
  const days = t("days");
  eqCols.forEach(({ col, segs, val, lbl }, i) => {
    lbl.textContent = days[i];
    const v = week[i];
    const has = typeof v === "number";
    if (has) total += v;
    const over = has && v > max;
    const lit = !has ? 0 : Math.min(SEGMENTS, Math.ceil(v / max * SEGMENTS - 1e-9));
    col.classList.toggle("future", !has);
    col.classList.toggle("over", over);
    col.classList.toggle("today", s.weekToday === i);
    segs.forEach((seg, k) => seg.classList.toggle("on", k < lit));
    val.textContent = has ? "$" + Math.round(v) : "";
  });
  $("week-total").textContent = week.some((v) => typeof v === "number") ? usd(total, 0) : "";
}

// ---------- limites do plano Claude (5h + semana) ----------
function fmtReset(ms) {
  const d = ms - Date.now();
  if (d <= 0) return t("resetDone");
  const h = Math.floor(d / 3600000), m = Math.floor((d % 3600000) / 60000);
  const rel = h >= 24 ? `${Math.floor(h / 24)}d ${h % 24}h` : h ? `${h}h ${m}min` : `${m}min`;
  return t("resetsIn", { t: rel });
}
// Depois do reset a leitura antiga não vale mais: `null` até o Claude Code gravar de novo.
const validLimit = (pct, resets) =>
  typeof pct === "number" && !(resets && resets < Date.now()) ? Math.max(0, pct) : null;

const SOURCE_LABEL = { claude: "Claude Code", codex: "Codex", cursor: "Cursor" };
const shortLabel = (s) => s.limit5hLabel ?? "5h";
// Janela longa: "week" (padrão) ou um período como "30d" no Codex.
const longIsWeek = (s) => !s.limit7dLabel || s.limit7dLabel === "week";

// Plano por assinatura (Claude Pro/Max, ChatGPT/Codex): o limite mais apertado entre a janela curta e a longa.
function tightestLimit(s) {
  const opts = [
    { label: shortLabel(s), pct: validLimit(s.limit5hPct, s.limit5hResetsMs) },
    { label: longIsWeek(s) ? t("tabWeek") : s.limit7dLabel, pct: validLimit(s.limit7dPct, s.limit7dResetsMs) },
  ].filter((o) => o.pct !== null);
  return opts.length ? opts.reduce((a, b) => (b.pct > a.pct ? b : a)) : null;
}

function renderLimit(key, pct, resets) {
  const v = validLimit(pct, resets);
  const has = v !== null;
  const p = has ? v : 0;
  const box = $(`lim${key}-pct`).closest(".lim");
  box.classList.toggle("warn", has && p >= 75 && p < 90);
  box.classList.toggle("crit", has && p >= 90);
  $(`lim${key}-pct`).textContent = has ? Math.round(p) + "%" : "--";
  $(`lim${key}-fill`).style.width = Math.min(p, 100) + "%";
  $(`lim${key}-reset`).textContent = typeof resets === "number" ? fmtReset(resets) : "";
}
function renderLimits(s) {
  // O Cursor não tem limites de janela: some com o bloco.
  const show = s.source !== "cursor";
  $("limits").hidden = !show;
  $("lim-divider").hidden = !show;
  if (!show) return;
  const app = SOURCE_LABEL[s.source] ?? "Claude Code";
  $("lim5-label").textContent = t("session") + " · " + shortLabel(s);
  $("lim7-label").textContent = longIsWeek(s) ? t("week") : t("period") + " · " + s.limit7dLabel;
  renderLimit("5", s.limit5hPct, s.limit5hResetsMs);
  renderLimit("7", s.limit7dPct, s.limit7dResetsMs);
  // O plano do Codex pode ter uma janela só: esconde a outra quando já há leitura.
  const partial = s.source === "codex" && s.limitsUpdatedMs;
  $("lim5-pct").closest(".lim").hidden = partial && s.limit5hPct == null;
  $("lim7-pct").closest(".lim").hidden = partial && s.limit7dPct == null;
  const note = $("lim-note");
  if (s.limitsUpdatedMs) {
    const min = Math.floor((Date.now() - s.limitsUpdatedMs) / 60000);
    note.hidden = min < 10;
    note.textContent = t("lastRead", { t: min < 60 ? min + " min" : Math.floor(min / 60) + " h", app });
  } else {
    note.hidden = false;
    note.textContent = t("waiting", { app });
  }
}

// ---------- estado ----------
let snap = null;
let expanded = false;
let leaveTimer = null;

const cycleColor = (pct) =>
  pct >= 90 ? "#ff453a" : pct >= 75 ? "#ff6a00" : "var(--orange)";

function render(s) {
  snap = s;
  root.dataset.state = s.status;
  root.dataset.level = s.status === "auth" ? "high" : s.level;

  const pct = s.cycleLimitUsd > 0 ? (s.cycleUsedUsd / s.cycleLimitUsd) * 100 : 0;
  root.style.setProperty("--cycle", cycleColor(pct));

  // aba: com limites do plano, mostra o mais apertado (5h ou semana); sem eles, o ciclo em US$.
  const lim = s.status === "auth" ? null : tightestLimit(s);
  if (lim) {
    $("tab-amount").textContent = Math.round(lim.pct) + "%";
    $("tab-label").textContent = lim.label;
    $("tab-fill").style.height = Math.min(lim.pct, 100) + "%";
    $("tab-fill").style.background =
      lim.pct >= 90 ? "var(--lvl-high)" : lim.pct >= 75 ? "var(--lvl-mid)" : "var(--lvl-low)";
  } else {
    $("tab-amount").textContent = s.status === "auth" ? "!" : "$" + Math.round(s.cycleUsedUsd);
    $("tab-label").textContent = t("tabCycle");
    $("tab-fill").style.height = Math.min(pct, 100) + "%";
    $("tab-fill").style.background = "";
  }

  const badge = $("source-badge");
  badge.hidden = !SOURCE_LABEL[s.source];
  badge.textContent = SOURCE_LABEL[s.source] ?? "";
  $("tab-src").textContent = SOURCE_LABEL[s.source]?.split(" ")[0] ?? "";
  root.dataset.source = s.source ?? "";
  root.title = SOURCE_LABEL[s.source] ? t("sourceTitle", { s: SOURCE_LABEL[s.source] }) : "";

  const wh = hoursLabel(s.windowHours ?? 3);
  $("window-label").textContent = wh;
  $("window-hours").textContent = wh;

  // velocímetro
  drawTicks(s.lowMax, s.highMin, s.rateMax);
  const f = Math.max(0, Math.min(1, s.ratePerHour / s.rateMax));
  const dash = `${(f * 100).toFixed(2)} 100`;
  $("fill").style.strokeDasharray = dash;
  $("fill-glow").style.strokeDasharray = dash;
  $("needle").style.transform = `rotate(${f * 180 - 90}deg)`;
  $("rate").textContent = usd(s.ratePerHour);
  $("level-chip").textContent = t("levels")[s.level] ?? "--";
  $("window").textContent = usd(s.windowUsd);

  // ciclo
  $("cycle-used").textContent = usd(s.cycleUsedUsd);
  $("cycle-limit").textContent = t("of", { v: usd(s.cycleLimitUsd, 0) });
  $("cycle-pct").textContent = num(pct, 1) + "%";
  $("cycle-fill").style.width = Math.min(pct, 100) + "%";

  renderWeek(s);
  renderLimits(s);

  // projeção linear até o fim do ciclo
  const proj = $("projection");
  proj.classList.remove("warn");
  proj.textContent = "";
  if (s.cycleStart && s.cycleEnd) {
    const t0 = Date.parse(s.cycleStart), t1 = Date.parse(s.cycleEnd), now = Date.now();
    const frac = (now - t0) / (t1 - t0);
    if (frac > 0.02 && s.cycleUsedUsd > 0) {
      const p = s.cycleUsedUsd / frac;
      const days = Math.max(0, Math.ceil((t1 - now) / 86400000));
      proj.textContent = t("projection", { v: usd(p, 0), d: days });
      if (p > s.cycleLimitUsd) proj.classList.add("warn");
    }
  }

  // mensagens / cookie
  // No estado "auth" a instrução já explica o que fazer; só mostra a mensagem se a sessão expirou.
  const showMsg = s.message && (s.status === "error" || (s.status === "auth" && /expir/i.test(s.message)));
  $("msg").hidden = !showMsg;
  $("msg").textContent = s.message ?? "";
  $("cookie-form").hidden = s.status !== "auth";

  $("clock").textContent = new Date(s.updatedAtMs).toLocaleTimeString(I18N.locale, {
    hour: "2-digit", minute: "2-digit", second: "2-digit", hourCycle: "h23", // sem AM/PM: o cabeçalho é estreito
  });
  refit();
}

// ---------- expandir/recolher ----------
const PANEL_W = 272;
let panelH = null;

// Altura que o painel precisa (o bloco de limites some no Cursor, a nota some quando a leitura é recente).
// Mede fora da tela, já na largura do painel, mesmo com a janela ainda recolhida.
function neededHeight() {
  const p = document.querySelector(".panel");
  const prev = p.style.cssText;
  p.style.cssText = `display:flex;visibility:hidden;width:${PANEL_W - 1}px;right:auto;bottom:auto;height:auto`;
  const h = p.offsetHeight;
  p.style.cssText = prev;
  return Math.ceil(h) + 2; // bordas de cima e de baixo do widget
}

async function expand() {
  clearTimeout(leaveTimer);
  if (expanded) return;
  expanded = true;
  panelH = neededHeight();
  await invoke("set_expanded", { expanded: true, height: panelH });
  root.classList.add("expanded");
}

// Conteúdo mudou com o painel aberto: ajusta a altura sem fechar.
function refit() {
  if (!expanded) return;
  const h = neededHeight();
  if (h !== panelH) {
    panelH = h;
    invoke("set_expanded", { expanded: true, height: h });
  }
}
function collapseSoon() {
  clearTimeout(leaveTimer);
  leaveTimer = setTimeout(async () => {
    if (document.activeElement?.tagName === "INPUT") return;
    root.classList.remove("expanded");
    expanded = false;
    await invoke("set_expanded", { expanded: false });
  }, 350);
}
document.addEventListener("mouseenter", expand);
document.addEventListener("mouseleave", collapseSoon);
root.addEventListener("mouseenter", expand);
root.addEventListener("dblclick", () => invoke("refresh_now"));
$("open-settings").addEventListener("click", () => invoke("open_settings"));
$("open-settings").addEventListener("dblclick", (e) => e.stopPropagation());

$("cookie-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const v = $("cookie-input").value;
  if (!v.trim()) return;
  try {
    await invoke("set_cookie", { cookie: v });
    $("cookie-input").value = "";
    $("cookie-input").blur();
    $("msg").hidden = true;
  } catch (err) {
    $("msg").hidden = false;
    $("msg").textContent = String(err);
  }
});

// ---------- inicialização ----------
listen("usage", (ev) => render(ev.payload));
const applySettings = async (s) => {
  applyColors(s.colors);
  document.documentElement.dataset.theme = s.theme ?? "dark";
  root.dataset.side = s.side ?? "right";
  I18N.set(await invoke("get_lang"));
  if (snap) render(snap);
};
listen("settings", (ev) => applySettings(ev.payload));
// Idioma antes do primeiro desenho, para não piscar no idioma errado.
invoke("get_settings")
  .then(applySettings)
  .then(() => invoke("get_snapshot"))
  .then(render);
setInterval(() => snap && render(snap), 60_000);
