// Sonda com cookie da sessão web (lido de ../.env). Nunca imprime o cookie.
import { readFileSync } from "node:fs";

const env = Object.fromEntries(
  readFileSync(new URL("../.env", import.meta.url), "utf8")
    .split(/\r?\n/).filter((l) => l.includes("=")).map((l) => [l.slice(0, l.indexOf("=")).trim(), l.slice(l.indexOf("=") + 1).trim().replace(/^["']|["']$/g, "")])
);
let c = env.CURSOR_COOKIE; if (c.startsWith("ser_")) c = "u" + c;
if (!c) throw new Error("CURSOR_COOKIE ausente");
const cookie = c.includes("WorkosCursorSessionToken=") ? c : `WorkosCursorSessionToken=${c}`;
const redact = (s) => s.split(c).join("<COOKIE>");

const headers = { Cookie: cookie, Origin: "https://cursor.com", Referer: "https://cursor.com/dashboard", "Content-Type": "application/json" };
async function req(name, url, init = {}) {
  try {
    const r = await fetch(url, { headers, ...init });
    console.log(`\n== ${name} -> ${r.status}\n${redact(await r.text()).slice(0, 2500)}`);
  } catch (e) { console.log(`\n== ${name} ERRO`, e.message); }
}

const now = Date.now();
await req("auth/me", "https://cursor.com/api/auth/me");
await req("usage-summary", "https://cursor.com/api/usage-summary");
await req("teams", "https://cursor.com/api/dashboard/teams", { method: "POST", body: "{}" });
await req("aggregated(3h)", "https://cursor.com/api/dashboard/get-aggregated-usage-events", {
  method: "POST", body: JSON.stringify({ teamId: 6332594, startDate: now - 3 * 3600e3, endDate: now }),
});
await req("filtered(3h)", "https://cursor.com/api/dashboard/get-filtered-usage-events", {
  method: "POST", body: JSON.stringify({ teamId: 6332594, startDate: String(now - 3 * 3600e3), endDate: String(now), page: 1, pageSize: 3 }),
});

