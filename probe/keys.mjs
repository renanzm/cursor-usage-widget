import { DatabaseSync } from "node:sqlite";
const db = new DatabaseSync(process.env.APPDATA + "/Cursor/User/globalStorage/state.vscdb", { readOnly: true });
const g = (k) => db.prepare("SELECT value FROM ItemTable WHERE key=?").get(k)?.value;
const t = g("cursorAuth/accessToken");
// Só descreve a forma, sem revelar conteúdo
console.log("começa com:", JSON.stringify(t.slice(0, 5)), "| tem '.':", t.includes("."), "| '::':", t.includes("::"), "| '%3A':", t.includes("%3A"), "| '|':", t.includes("|"));
console.log("iguais refresh==access:", t === g("cursorAuth/refreshToken"));
console.log("membership:", g("cursorAuth/stripeMembershipType"), "| team:", g("cursorAuth/cachedTeam"));
console.log("authId prefix:", String(g("cursorAuth/stripeMembershipAuthId")).slice(0, 10));
