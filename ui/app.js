const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const root = $("w");
const usd = (v, d = 2) =>
  v.toLocaleString("pt-BR", { style: "currency", currency: "USD", minimumFractionDigits: d, maximumFractionDigits: d }).replace("US$", "US$ ").replace(/\s+/g, " ");
const LEVEL_LABEL = { low: "Baixo", mid: "Médio", high: "Alto" };

// ---------- velocímetro ----------
const CX = 100, CY = 100, R = 78;
const pt = (f) => {
  const th = Math.PI * (1 - f);
  return [CX + R * Math.cos(th), CY - R * Math.sin(th)];
};
const arc = (f0, f1, color) => {
  const [x0, y0] = pt(f0), [x1, y1] = pt(f1);
  return `<path d="M${x0} ${y0} A${R} ${R} 0 0 1 ${x1} ${y1}" stroke="${color}" opacity=".9"/>`;
};
function drawArcs(lowMax, highMin) {
  const max = highMin * 2, g = 0.012;
  const a = lowMax / max, b = highMin / max;
  $("arcs").innerHTML =
    arc(0, a - g, "var(--low)") + arc(a + g, b - g, "var(--mid)") + arc(b + g, 1, "var(--high)");
}

// ---------- estado ----------
let snap = null;
let expanded = false;
let leaveTimer = null;

function cycleColor(pct) {
  return pct >= 90 ? "var(--high)" : pct >= 75 ? "var(--mid)" : "var(--low)";
}

function render(s) {
  snap = s;
  root.dataset.state = s.status;
  root.dataset.level = s.status === "auth" ? "high" : s.level;

  const pct = s.cycleLimitUsd > 0 ? (s.cycleUsedUsd / s.cycleLimitUsd) * 100 : 0;
  root.style.setProperty("--cycle", cycleColor(pct));

  // aba
  $("tab-amount").textContent = s.status === "auth" ? "!" : "$" + Math.round(s.cycleUsedUsd);
  $("tab-fill").style.height = Math.min(pct, 100) + "%";

  // velocímetro
  drawArcs(s.lowMax, s.highMin);
  const f = Math.max(0, Math.min(1, s.ratePerHour / (s.highMin * 2)));
  $("needle").style.transform = `rotate(${f * 180 - 90}deg)`;
  $("rate").textContent = usd(s.ratePerHour);
  $("level-label").textContent = LEVEL_LABEL[s.level];
  $("window").textContent = usd(s.windowUsd);

  // ciclo
  $("cycle-amount").textContent = `${usd(s.cycleUsedUsd)} / ${usd(s.cycleLimitUsd, 0)}`;
  $("cycle-pct").textContent = pct.toFixed(1).replace(".", ",") + "%";
  $("cycle-fill").style.width = Math.min(pct, 100) + "%";

  // projeção linear até o fim do ciclo
  const proj = $("projection");
  proj.classList.remove("warn");
  if (s.cycleStart && s.cycleEnd) {
    const t0 = Date.parse(s.cycleStart), t1 = Date.parse(s.cycleEnd), now = Date.now();
    const frac = (now - t0) / (t1 - t0);
    if (frac > 0.02 && s.cycleUsedUsd > 0) {
      const p = s.cycleUsedUsd / frac;
      const days = Math.max(0, Math.ceil((t1 - now) / 86400000));
      proj.textContent = `Projeção: ${usd(p, 0)} no fim do ciclo · ${days}d restantes`;
      if (p > s.cycleLimitUsd) proj.classList.add("warn");
    } else {
      proj.textContent = "";
    }
  }

  // mensagens / cookie
  const needsCookie = s.status === "auth";
  $("msg").hidden = !(s.message && s.status !== "ok");
  $("msg").textContent = s.message ?? "";
  $("cookie-form").hidden = !needsCookie;

  const d = new Date(s.updatedAtMs);
  $("clock").textContent = d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

// ---------- expandir/recolher ----------
async function expand() {
  clearTimeout(leaveTimer);
  if (expanded) return;
  expanded = true;
  await invoke("set_expanded", { expanded: true });
  root.classList.add("expanded");
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
invoke("get_snapshot").then(render);
// reaplica a projeção/relógio mesmo sem novos dados
setInterval(() => snap && render(snap), 60_000);
