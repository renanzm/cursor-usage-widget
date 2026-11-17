import { readFileSync } from "node:fs";
const raw = readFileSync(new URL("../.env", import.meta.url), "utf8");
const line = raw.split(/\r?\n/).find((l) => l.startsWith("CURSOR_COOKIE"));
const v = line.slice(line.indexOf("=") + 1);
console.log({
  tamanho: v.length,
  aspas: /^["']/.test(v),
  comecaCom: v.slice(0, 5),
  temPct3A: v.includes("%3A%3A"),
  temDoisPontos: v.includes("::"),
  partesPonto: v.split(".").length,
  temEspaco: /\s/.test(v.trim()),
  temPontoVirgula: v.includes(";"),
  temIgual: v.slice(0, 40).includes("="),
});
