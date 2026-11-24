# Gera as imagens do README (docs/images) com dados fictícios, sem rodar o app.
# Copia a UI para uma pasta temporária, troca a ponte do Tauri por um simulado e fotografa com o Edge/Chrome headless.
# Uso: powershell -ExecutionPolicy Bypass -File tools\readme-shots.ps1
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root "docs\images"
$tmp = Join-Path ([IO.Path]::GetTempPath()) "cuw-readme-shots"
New-Item -ItemType Directory -Force $out, $tmp | Out-Null
Copy-Item (Join-Path $root "ui\*") $tmp -Force

$browser = @(
  "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
  "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe",
  "$env:ProgramFiles\Google\Chrome\Application\chrome.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $browser) { throw "Edge ou Chrome não encontrado." }

# Ponte simulada: ?theme=dark|light e ?open=1 (painel aberto).
$mock = @'
<script>
(() => {
  const q = new URLSearchParams(location.search), now = Date.now(), H = 3600000;
  const snap = {
    status: "ok", source: "claude", message: null,
    cycleUsedUsd: 312.4, cycleLimitUsd: 1000,
    cycleStart: new Date(new Date().getFullYear(), new Date().getMonth(), 1).toISOString(),
    cycleEnd: new Date(new Date().getFullYear(), new Date().getMonth() + 1, 1).toISOString(),
    windowUsd: 22.2, windowEvents: 41, ratePerHour: 7.4, windowHours: 3, level: "mid",
    lowMax: 6, highMin: 13, rateMax: 20, weekUsd: [46, 88, 61, null, null], weekToday: 2, dayMaxUsd: 100,
    updatedAtMs: now, limit5hPct: 42, limit5hResetsMs: now + 2.6 * H, limit7dPct: 63, limit7dResetsMs: now + 3.4 * 24 * H,
    limitsUpdatedMs: now,
  };
  const settings = { theme: q.get("theme") || "dark", side: "right",
    colors: { accent: "#ff9f0a", low: "#30d158", mid: "#ffd60a", high: "#ff453a" } };
  window.__TAURI__ = {
    core: { invoke: async (cmd) => ({ get_snapshot: snap, get_settings: settings, get_lang: q.get("lang") || "en" })[cmd] },
    event: { listen: async () => {} },
  };
  if (q.get("open")) addEventListener("load", () => setTimeout(() => document.getElementById("w").classList.add("expanded"), 50));
  // O headless tem largura mínima de janela: fixa o widget no tamanho real e o script recorta depois.
  const [w, h] = (q.get("size") || "56,128").split(",");
  document.write(`<style>html,body{width:auto;height:auto}.widget{width:${w}px!important;height:${h}px!important}</style>`);
})();
</script>
'@
$index = Join-Path $tmp "index.html"
(Get-Content $index -Raw -Encoding utf8) -replace '<script src="app.js">', ($mock + '<script src="app.js">') |
  Set-Content $index -Encoding utf8

Add-Type -AssemblyName System.Drawing
$scale = 2
$url = ([Uri]$index).AbsoluteUri
# Inglês para o README.md, português para o README.pt-BR.md (sufixo -pt).
foreach ($lang in "en", "pt") {
foreach ($theme in "dark", "light") {
  foreach ($shot in @(@{ name = "closed"; w = 56; h = 128; q = "" }, @{ name = "open"; w = 272; h = 620; q = "&open=1" })) {
    $suffix = if ($lang -eq "pt") { "-pt" } else { "" }
    $file = Join-Path $out "widget-$($shot.name)-$theme$suffix.png"
    $raw = Join-Path $tmp "raw.png"
    # O navegador escreve progresso no stderr: Start-Process evita que o PowerShell trate isso como erro.
    Start-Process -FilePath $browser -Wait -WindowStyle Hidden -ArgumentList @(
      "--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=$scale",
      "--default-background-color=00000000", "--virtual-time-budget=3000",
      "--window-size=800,800", "--screenshot=`"$raw`"",
      "`"$url`?lang=$lang&theme=$theme&size=$($shot.w),$($shot.h)$($shot.q)`""
    )
    $img = [Drawing.Bitmap]::FromFile($raw)
    $crop = $img.Clone([Drawing.Rectangle]::new(0, 0, $shot.w * $scale, $shot.h * $scale), $img.PixelFormat)
    $img.Dispose()
    $crop.Save($file, [Drawing.Imaging.ImageFormat]::Png)
    $crop.Dispose()
    Write-Host "ok  $file"
  }
}
}
