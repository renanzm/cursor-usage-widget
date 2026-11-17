// Gera src-tauri/icons/icon.png e icon.ico (velocímetro estilizado). Uso: node tools/gen-icon.mjs
import { deflateSync, crc32 } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";

const N = 256;
const px = Buffer.alloc(N * N * 4);
const cx = 128, cy = 150;

const clamp = (v, a = 0, b = 1) => Math.max(a, Math.min(b, v));
const mix = (a, b, t) => a.map((v, i) => Math.round(v + (b[i] - v) * t));
const G = [52, 211, 153], Y = [250, 204, 21], R = [248, 113, 113];
const ramp = (t) => (t < 0.5 ? mix(G, Y, t * 2) : mix(Y, R, (t - 0.5) * 2));

function over(i, rgb, a) {
  const da = px[i + 3] / 255, oa = a + da * (1 - a);
  if (oa <= 0) return;
  for (let k = 0; k < 3; k++) px[i + k] = Math.round((rgb[k] * a + px[i + k] * da * (1 - a)) / oa);
  px[i + 3] = Math.round(oa * 255);
}
const distSeg = (x, y, x1, y1, x2, y2) => {
  const dx = x2 - x1, dy = y2 - y1, t = clamp(((x - x1) * dx + (y - y1) * dy) / (dx * dx + dy * dy));
  return Math.hypot(x - (x1 + t * dx), y - (y1 + t * dy));
};

const needleAng = Math.PI * (1 - 0.62);
const nx = cx + Math.cos(needleAng) * 70, ny = cy - Math.sin(needleAng) * 70;

for (let y = 0; y < N; y++) for (let x = 0; x < N; x++) {
  const i = (y * N + x) * 4;
  // fundo: quadrado arredondado
  const rx = Math.abs(x - 127.5) - (128 - 52), ry = Math.abs(y - 127.5) - (128 - 52);
  const sd = Math.hypot(Math.max(rx, 0), Math.max(ry, 0)) + Math.min(Math.max(rx, ry), 0) - 52;
  const bg = clamp(0.5 - sd);
  if (bg > 0) over(i, [20, 22, 28], bg);
  // arco
  const d = Math.hypot(x - cx, y - cy), ang = Math.atan2(cy - y, x - cx);
  if (y <= cy + 4 && ang >= -0.02) {
    const t = clamp(1 - ang / Math.PI);
    const cov = clamp(0.5 - Math.abs(d - 84) + 8) ;
    const edge = clamp(9 - Math.abs(d - 84) + 0.5);
    if (edge > 0) over(i, ramp(t), clamp(edge));
  }
  // ponteiro
  const nd = distSeg(x, y, cx, cy, nx, ny);
  const na = clamp(6.5 - nd + 0.5);
  if (na > 0) over(i, [240, 240, 245], na);
  const hub = clamp(13 - d + 0.5);
  if (hub > 0) over(i, [240, 240, 245], hub);
}

function png() {
  const raw = Buffer.alloc((N * 4 + 1) * N);
  for (let y = 0; y < N; y++) { raw[y * (N * 4 + 1)] = 0; px.copy(raw, y * (N * 4 + 1) + 1, y * N * 4, (y + 1) * N * 4); }
  const chunk = (type, data) => {
    const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
    const td = Buffer.concat([Buffer.from(type), data]);
    const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td) >>> 0);
    return Buffer.concat([len, td, crc]);
  };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(N, 0); ihdr.writeUInt32BE(N, 4); ihdr[8] = 8; ihdr[9] = 6;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}

const p = png();
const hdr = Buffer.alloc(6); hdr.writeUInt16LE(1, 2); hdr.writeUInt16LE(1, 4);
const ent = Buffer.alloc(16); ent[0] = 0; ent[1] = 0; ent.writeUInt16LE(1, 4); ent.writeUInt16LE(32, 6); ent.writeUInt32LE(p.length, 8); ent.writeUInt32LE(22, 12);
mkdirSync("src-tauri/icons", { recursive: true });
writeFileSync("src-tauri/icons/icon.png", p);
writeFileSync("src-tauri/icons/icon.ico", Buffer.concat([hdr, ent, p]));
console.log("ícones gerados");
