# Teste manual do fluxo de colar o cookie: clica no campo, cola (Ctrl+V) o CURSOR_COOKIE do .env e clica em Salvar.
# Pressupõe o widget aberto, sem cookie salvo, e o mouse livre. Não imprime o cookie.
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System; using System.Runtime.InteropServices;
public class M { [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, UIntPtr e); }
"@
function Click($x, $y) {
  [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point($x, $y)
  Start-Sleep -Milliseconds 300
  [M]::mouse_event(0x02, 0, 0, 0, [UIntPtr]::Zero); [M]::mouse_event(0x04, 0, 0, 0, [UIntPtr]::Zero)
}
$line = (Get-Content (Join-Path $PSScriptRoot "..\.env") | Where-Object { $_ -like "CURSOR_COOKIE=*" })
Set-Clipboard -Value $line.Substring("CURSOR_COOKIE=".Length)

[System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point(900, 540); Start-Sleep 1
[System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point(1900, 440); Start-Sleep 2   # expande
Click 1750 585                                   # campo
[System.Windows.Forms.SendKeys]::SendWait("^v"); Start-Sleep -Milliseconds 500
& powershell -File (Join-Path $PSScriptRoot "shot.ps1") -Out "$env:TEMP\cuw\e2e1.png" -Y 350 -H 380
Click 1868 585                                   # Salvar
Start-Sleep 4
& powershell -File (Join-Path $PSScriptRoot "shot.ps1") -Out "$env:TEMP\cuw\e2e2.png" -Y 350 -H 380
Set-Clipboard -Value " "
