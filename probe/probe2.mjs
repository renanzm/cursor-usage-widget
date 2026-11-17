import { DatabaseSync } from "node:sqlite";
const db = new DatabaseSync(process.env.APPDATA + "/Cursor/User/globalStorage/state.vscdb", { readOnly: true });
const get = (k) => db.prepare("SELECT value FROM ItemTable WHERE key=?").get(k)?.value;
const token = get("cursorAuth/accessToken");
const team = JSON.parse(get("cursorAuth/cachedTeam") ?? "{}");
const redact = (s) => s.split(token).join("<TOKEN>");

async function rpc(path, body = {}) {
  try {
    const r = await fetch("https://api2.cursor.sh/" + path, {
      method: "POST",
      headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json", "Connect-Protocol-Version": "1" },
      body: JSON.stringify(body),
    });
    console.log(`\n== ${path} -> ${r.status}\n${redact(await r.text()).slice(0, 1500)}`);
  } catch (e) { console.log(`\n== ${path} ERRO`, e.message); }
}

const now = Date.now();
await rpc("aiserver.v1.DashboardService/GetCurrentPeriodUsage");
await rpc("aiserver.v1.DashboardService/GetAggregatedUsageEvents", { teamId: team.teamId, startDate: String(now - 3 * 3600e3), endDate: String(now) });
await rpc("aiserver.v1.DashboardService/GetFilteredUsageEvents", { teamId: team.teamId, startDate: String(now - 3 * 3600e3), endDate: String(now), page: 1, pageSize: 5 });
await rpc("aiserver.v1.DashboardService/GetTeamSpend", { teamId: team.teamId });
await rpc("aiserver.v1.DashboardService/GetHardLimit", { teamId: team.teamId });
