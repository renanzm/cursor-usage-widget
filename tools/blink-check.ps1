# Mede o brilho médio de uma região da tela várias vezes (detecta animação de piscar). Uso: -X -Y -W -H
param([int]$X = 1767, [int]$Y = 665, [int]$W = 34, [int]$H = 50)
Add-Type -AssemblyName System.Drawing
1..8 | ForEach-Object {
  $bmp = New-Object System.Drawing.Bitmap $W, $H
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($X, $Y, 0, 0, $bmp.Size)
  $sum = 0; $n = 0
  for ($x = 0; $x -lt $W; $x += 2) { for ($y = 0; $y -lt $H; $y += 2) { $c = $bmp.GetPixel($x, $y); $sum += ($c.R + $c.G + $c.B) / 3; $n++ } }
  "{0}: brilho medio {1:N1}" -f $_, ($sum / $n)
  $g.Dispose(); $bmp.Dispose()
  Start-Sleep -Milliseconds 330
}

