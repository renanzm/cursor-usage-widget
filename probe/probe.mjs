// Sonda: lê o token local do Cursor e consulta endpoints de uso. Nunca imprime o token.
import { DatabaseSync } from "node:sqlite";

const db = new DatabaseSync(process.env.APPDATA + "/Cursor/User/globalStorage/state.vscdb", { readOnly: true });
const get = (k) => db.prepare("SELECT value FROM ItemTable WHERE key=?").get(k)?.value;
const token = get("cursorAuth/accessToken");
const userId = String(get("cursorAuth/stripeMembershipAuthId")).split("|").pop();
const team = JSON.parse(get("cursorAuth/cachedTeam") ?? "{}");
console.log("userId:", userId, "| teamId:", team.teamId);

const variants = {
  cookie: { Cookie: `WorkosCursorSessionToken=${userId}%3A%3A${token}`, Origin: "https://cursor.com", Referer: "https://cursor.com/dashboard" },
  bearer: { Authorization: `Bearer ${token}` },
};

const redact = (s) => s.split(token).join("<TOKEN>");

async function tryReq(variant, name, url, init = {}) {
  try {
    const r = await fetch(url, { ...init, headers: { "Content-Type": "application/json", ...variants[variant] } });
    const t = redact(await r.text());
    console.log(`\n== [${variant}] ${name} -> ${r.status}\n${t.slice(0, 1200)}`);
  } catch (e) {
    console.log(`\n== [${variant}] ${name} ERRO`, e.message);
  }
}

const now = Date.now();
for (const v of Object.keys(variants)) {
  await tryReq(v, "usage-summary", "https://cursor.com/api/usage-summary");
  await tryReq(v, "usage", `https://cursor.com/api/usage?user=${userId}`);
  await tryReq(v, "aggregated", "https://cursor.com/api/dashboard/get-aggregated-usage-events", {
    method: "POST",
    body: JSON.stringify({ teamId: team.teamId, startDate: now - 3 * 3600e3, endDate: now }),
  });
}
