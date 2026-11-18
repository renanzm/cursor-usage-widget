const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const root = $("w");
const usd = (v, d = 2) =>
  v
    .toLocaleString("pt-BR", { style: "currency", currency: "USD", minimumFractionDigits: d, maximumFractionDigits: d })
    .replace("US$", "US$ ")
    .replace(/\s+/g, " ");
const LEVEL_LABEL = { low: "Baixo", mid: "Médio", high: "Alto" };

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

// ---------- estado ----------
let snap = null;
let expanded = false;
let leaveTimer = null;

const cycleColor = (pct) =>
  pct >= 90 ? "#ff453a" : pct >= 75 ? "#ff6a00" : "#ff9f0a";

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
  drawTicks(s.lowMax, s.highMin, s.rateMax);
  const f = Math.max(0, Math.min(1, s.ratePerHour / s.rateMax));
  const dash = `${(f * 100).toFixed(2)} 100`;
  $("fill").style.strokeDasharray = dash;
  $("fill-glow").style.strokeDasharray = dash;
  $("needle").style.transform = `rotate(${f * 180 - 90}deg)`;
  $("rate").textContent = usd(s.ratePerHour);
  $("level-chip").textContent = LEVEL_LABEL[s.level];
  $("window").textContent = usd(s.windowUsd);

  // ciclo
  $("cycle-used").textContent = usd(s.cycleUsedUsd);
  $("cycle-limit").textContent = "de " + usd(s.cycleLimitUsd, 0);
  $("cycle-pct").textContent = pct.toFixed(1).replace(".", ",") + "%";
  $("cycle-fill").style.width = Math.min(pct, 100) + "%";

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
      proj.textContent = `Projeção ${usd(p, 0)} · ${days}d restantes`;
      if (p > s.cycleLimitUsd) proj.classList.add("warn");
    }
  }

  // mensagens / cookie
  // No estado "auth" a instrução já explica o que fazer; só mostra a mensagem se a sessão expirou.
  const showMsg = s.message && (s.status === "error" || (s.status === "auth" && s.message.includes("expirada")));
  $("msg").hidden = !showMsg;
  $("msg").textContent = s.message ?? "";
  $("cookie-form").hidden = s.status !== "auth";

  $("clock").textContent = new Date(s.updatedAtMs).toLocaleTimeString("pt-BR", {
    hour: "2-digit", minute: "2-digit", second: "2-digit",
  });
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
setInterval(() => snap && render(snap), 60_000);
