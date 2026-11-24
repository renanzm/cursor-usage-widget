# Status line do Claude Code: grava os limites (5h e semanal) em ~/.claude/widget-limits.json
# para o widget ler, e imprime um resumo curto. Configurado em ~/.claude/settings.json.
$ErrorActionPreference = "SilentlyContinue"
$raw = [Console]::In.ReadToEnd()
$j = $raw | ConvertFrom-Json
$rl = $j.rate_limits
if ($rl) {
    $out = [ordered]@{
        updated_at_ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
        rate_limits   = $rl
    }
    $path = Join-Path $env:USERPROFILE ".claude\widget-limits.json"
    $tmp = "$path.tmp"
    ($out | ConvertTo-Json -Depth 6) | Set-Content -Path $tmp -Encoding utf8
    Move-Item -Force $tmp $path
    $parts = @()
    if ($rl.five_hour) { $parts += ("5h {0:N0}%" -f $rl.five_hour.used_percentage) }
    if ($rl.seven_day) { $parts += ("7d {0:N0}%" -f $rl.seven_day.used_percentage) }
    [Console]::Out.Write($parts -join " | ")
}
