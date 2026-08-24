# The four rank mangos that MangoConfig draws next to a player's name, built
# from the app icon so the badge and the logo can never drift apart.
#
#   powershell -File tools/mango-badges.ps1
#
# The fruit is taken on its own - the sparkles above it are noise at eight
# pixels tall - and averaged inside its own pixel blocks rather than resampled
# across them, which is what keeps the edges crisp instead of smeared. Mango+
# is the artwork's own colours; the other three are the same shading walked
# along a two-leg colour ramp, so a gold mango is still a lit mango.
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$srcPath = Join-Path $root "src\renderer\assets\icon.png"
$fontDir = Join-Path $root "mod\src\main\resources\assets\mangoconfig\textures\font"
$webDir  = Join-Path $root "server\web"
$size = 16

# rank -> (shadow, body, highlight). Mango+ is missing on purpose: it keeps
# the artwork it came from.
$ramps = [ordered]@{
  grey = @( @(56,54,52),  @(166,161,153), @(255,255,255) )
  gold = @( @(92,54,2),   @(255,196,40),  @(255,248,205) )
  blue = @( @(10,38,74),  @(78,163,255),  @(222,242,255) )
}

$bmp = New-Object System.Drawing.Bitmap($srcPath)
$w = $bmp.Width; $h = $bmp.Height
$rect = New-Object System.Drawing.Rectangle(0, 0, $w, $h)
$data = $bmp.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$stride = $data.Stride
$bytes = New-Object byte[] ($stride * $h)
[System.Runtime.InteropServices.Marshal]::Copy($data.Scan0, $bytes, 0, $bytes.Length)
$bmp.UnlockBits($data)
$bmp.Dispose()

function Alpha($x, $y) { return $bytes[$y * $stride + $x * 4 + 3] }

# Where the artwork is at all.
$minX = $w; $maxX = 0; $minY = $h; $maxY = 0
for ($y = 0; $y -lt $h; $y++) {
  for ($x = 0; $x -lt $w; $x++) {
    if ((Alpha $x $y) -gt 8) {
      if ($x -lt $minX) { $minX = $x }; if ($x -gt $maxX) { $maxX = $x }
      if ($y -lt $minY) { $minY = $y }; if ($y -gt $maxY) { $maxY = $y }
    }
  }
}
$fullW = $maxX - $minX + 1

# The sparkles are narrow and the fruit is not: the fruit starts at the first
# row carrying one unbroken run of opaque pixels across nearly half the width.
# It has to be one run - two sparkles either side of the gap would otherwise
# look just as wide as a fruit.
$topY = $minY
for ($y = $minY; $y -le $maxY; $y++) {
  $run = 0; $longest = 0
  for ($x = $minX; $x -le $maxX; $x++) {
    if ((Alpha $x $y) -gt 8) { $run++; if ($run -gt $longest) { $longest = $run } } else { $run = 0 }
  }
  if ($longest -gt ($fullW * 0.45)) { $topY = $y; break }
}

# Then climb back up over the stem and the leaf, which are narrow but attached.
# The gap of clear rows above them is where the sparkles begin.
$bandFrom = [int]($minX + $fullW * 0.2); $bandTo = [int]($maxX - $fullW * 0.2)
# Bounded, because the tallest sparkle stands directly over the stem with no
# clear row between them: a stem is short, so a long climb is the sparkle.
$stop = $topY - [int](($maxY - $topY) * 0.16)
while ($topY -gt $minY -and $topY -gt $stop) {
  $any = $false
  for ($x = $bandFrom; $x -le $bandTo; $x++) { if ((Alpha $x ($topY - 1)) -gt 8) { $any = $true; break } }
  if (-not $any) { break }
  $topY--
}
$cropX = $minX; $cropY = $topY; $cropW = $fullW; $cropH = $maxY - $topY + 1
"fruit crop: ${cropW}x${cropH} at $cropX,$cropY of ${w}x${h}"

# One output pixel per block of the artwork, fitted square without stretching.
$cell = [Math]::Max($cropW, $cropH) / [double]$size
$offX = $cropX + ($cropW - $cell * $size) / 2.0
$offY = $cropY + ($cropH - $cell * $size) / 2.0
$base = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
for ($oy = 0; $oy -lt $size; $oy++) {
  for ($ox = 0; $ox -lt $size; $ox++) {
    $x0 = [int][Math]::Floor($offX + $ox * $cell); $x1 = [int][Math]::Floor($offX + ($ox + 1) * $cell) - 1
    $y0 = [int][Math]::Floor($offY + $oy * $cell); $y1 = [int][Math]::Floor($offY + ($oy + 1) * $cell) - 1
    $sr = 0.0; $sg = 0.0; $sb = 0.0; $sa = 0.0; $n = 0; $weight = 0.0
    for ($y = $y0; $y -le $y1; $y++) {
      if ($y -lt 0 -or $y -ge $h) { continue }
      for ($x = $x0; $x -le $x1; $x++) {
        if ($x -lt 0 -or $x -ge $w) { continue }
        $i = $y * $stride + $x * 4
        $a = $bytes[$i + 3]
        # Colour averaged weighted by alpha, so a transparent neighbour cannot
        # drag an edge towards black; alpha averaged on its own.
        $sr += $bytes[$i + 2] * $a; $sg += $bytes[$i + 1] * $a; $sb += $bytes[$i] * $a
        $weight += $a; $sa += $a; $n++
      }
    }
    if ($n -eq 0 -or $weight -le 0) { continue }
    $base.SetPixel($ox, $oy, [System.Drawing.Color]::FromArgb(
      [int][Math]::Round($sa / $n),
      [int][Math]::Round($sr / $weight),
      [int][Math]::Round($sg / $weight),
      [int][Math]::Round($sb / $weight)))
  }
}

function Save-Both([System.Drawing.Bitmap]$img, [string]$fontName, [string]$webName) {
  $img.Save((Join-Path $fontDir $fontName), [System.Drawing.Imaging.ImageFormat]::Png)
  $img.Save((Join-Path $webDir $webName), [System.Drawing.Imaging.ImageFormat]::Png)
}

Save-Both $base "mango_plus.png" "mango-mangoplus.png"

# The brightness the artwork actually uses, so a ramp spends its whole range
# on the fruit instead of bunching up in the middle.
$lo = 1.0; $hi = 0.0
for ($y = 0; $y -lt $size; $y++) { for ($x = 0; $x -lt $size; $x++) {
  $c = $base.GetPixel($x, $y); if ($c.A -lt 16) { continue }
  $l = (0.299 * $c.R + 0.587 * $c.G + 0.114 * $c.B) / 255.0
  if ($l -lt $lo) { $lo = $l }; if ($l -gt $hi) { $hi = $l }
} }
if ($hi -le $lo) { $hi = $lo + 0.001 }

$webNames = @{ grey = "mango-member.png"; gold = "mango-owner.png"; blue = "mango-support.png" }
foreach ($name in $ramps.Keys) {
  $ramp = $ramps[$name]
  $img = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  for ($y = 0; $y -lt $size; $y++) { for ($x = 0; $x -lt $size; $x++) {
    $c = $base.GetPixel($x, $y); if ($c.A -eq 0) { continue }
    $l = (0.299 * $c.R + 0.587 * $c.G + 0.114 * $c.B) / 255.0
    $t = [Math]::Max(0.0, [Math]::Min(1.0, ($l - $lo) / ($hi - $lo)))
    if ($t -lt 0.5) { $p = $ramp[0]; $q = $ramp[1]; $k = $t * 2 } else { $p = $ramp[1]; $q = $ramp[2]; $k = ($t - 0.5) * 2 }
    $img.SetPixel($x, $y, [System.Drawing.Color]::FromArgb($c.A,
      [int][Math]::Round($p[0] + ($q[0] - $p[0]) * $k),
      [int][Math]::Round($p[1] + ($q[1] - $p[1]) * $k),
      [int][Math]::Round($p[2] + ($q[2] - $p[2]) * $k)))
  } }
  Save-Both $img "mango_$name.png" $webNames[$name]
  $img.Dispose()
}
$base.Dispose()
"wrote mango_plus/grey/gold/blue to $fontDir and $webDir"
