# Compila em release, instala em %LOCALAPPDATA%\CursorUsageWidget, ativa início com o Windows e abre o widget.
# Uso: powershell -File tools/install.ps1
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $env:LOCALAPPDATA "CursorUsageWidget"
$exeName = "cursor-usage-widget.exe"

Push-Location (Join-Path $root "src-tauri")
try { cargo build --release; if ($LASTEXITCODE -ne 0) { throw "cargo build falhou" } } finally { Pop-Location }

Get-Process -Name "cursor-usage-widget" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
New-Item -ItemType Directory -Force $dest | Out-Null
Copy-Item (Join-Path $root "src-tauri\target\release\$exeName") (Join-Path $dest $exeName) -Force

$runKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
Set-ItemProperty -Path $runKey -Name "CursorUsageWidget" -Value ('"' + (Join-Path $dest $exeName) + '"')

Start-Process (Join-Path $dest $exeName)
"Instalado em $dest e configurado para iniciar com o Windows."
