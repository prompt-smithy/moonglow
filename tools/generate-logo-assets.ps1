<#
.SYNOPSIS
    Generates every Moonglow logo asset from one geometry definition.

.DESCRIPTION
    The mark is a crescent moon. All three apps consume the files written here, so the artwork
    only ever needs editing in one place.

        Moonglow.png          two-tone master (black crescent with a white keyline) used for the
                              packaged app icons and the README art
        Moonglow.ico          Windows icon (exe, installer, portable launcher)


        linux/hicolor/**      freedesktop icon theme set installed by the Linux packages

    Usage: powershell -ExecutionPolicy Bypass -File tools/generate-logo-assets.ps1
#>
[CmdletBinding()]
param(
    [string]$ResourcesDir
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

# $PSScriptRoot is not available while parameters bind, so resolve the default here.
if (-not $ResourcesDir) { $ResourcesDir = Join-Path $PSScriptRoot '..\resources' }

# Design space: every asset is drawn in these 1024-unit coordinates and scaled down.
$Design = 1024.0
$CrescentRadius = 480.0     # radius of the outer rim
$CutAngleDeg = -45.0        # the crescent opens towards the upper right
$CutRadiusFactor = 1.05     # radius of the circle subtracted from the disc
$CrescentThickness = 0.60   # belly thickness as a fraction of the radius
$Supersample = 4

$LightColor = '#000000'

$KeylineColor = '#FFFFFF'
$KeylineDesign = 24.0       # two-tone keyline width, in design units

$IcoSizes = @(16, 24, 32, 48, 64, 128, 256)
$HicolorSizes = @(16, 24, 32, 48, 64, 128, 256, 512)

function Get-ArcPoints {
    param([double]$Cx, [double]$Cy, [double]$Radius, [double]$StartDeg, [double]$EndDeg, [double]$MidDeg)

    $norm = { param($a) (($a % 360.0) + 360.0) % 360.0 }
    $cwEnd = & $norm ($EndDeg - $StartDeg)
    $cwMid = & $norm ($MidDeg - $StartDeg)
    $sweep = if ($cwMid -le $cwEnd) { $cwEnd } else { $cwEnd - 360.0 }

    $points = New-Object System.Collections.ArrayList
    $steps = [Math]::Max(8, [int][Math]::Ceiling([Math]::Abs($sweep)))
    for ($i = 0; $i -le $steps; $i++) {
        $a = ($StartDeg + $sweep * $i / $steps) * [Math]::PI / 180.0
        [void]$points.Add([System.Drawing.PointF]::new(
            [single]($Cx + [Math]::Cos($a) * $Radius),
            [single]($Cy + [Math]::Sin($a) * $Radius)))
    }
    return , $points
}

function Get-CrescentPath {
    param([double]$Scale)

    $center = $Design / 2.0 * $Scale
    $R = $CrescentRadius * $Scale
    $rc = $R * $CutRadiusFactor
    $d = $rc - $R + $R * $CrescentThickness
    $theta = $CutAngleDeg * [Math]::PI / 180.0

    $a = ($d * $d + $R * $R - $rc * $rc) / (2.0 * $d)
    $h = [Math]::Sqrt([Math]::Max(0.0, $R * $R - $a * $a))
    $cutX = $center + [Math]::Cos($theta) * $d
    $cutY = $center + [Math]::Sin($theta) * $d

    $beta = [Math]::Atan2($h, $a) * 180.0 / [Math]::PI
    $gamma = [Math]::Atan2($h, $a - $d) * 180.0 / [Math]::PI

    $outer = Get-ArcPoints $center $center $R ($CutAngleDeg + $beta) ($CutAngleDeg - $beta) ($CutAngleDeg + 180.0)
    $inner = Get-ArcPoints $cutX $cutY $rc ($CutAngleDeg + $gamma) ($CutAngleDeg - $gamma) ($CutAngleDeg + 180.0)
    # Walk the inner rim back to the start so the outline stays continuous.
    $inner.Reverse()

    $points = New-Object System.Collections.ArrayList
    foreach ($p in $outer) { [void]$points.Add($p) }
    for ($i = 1; $i -lt $inner.Count - 1; $i++) { [void]$points.Add($inner[$i]) }

    # Centre the glyph's bounding box rather than the original disc, so the crescent sits
    # optically centred in the canvas.
    $minX = [double]::MaxValue; $maxX = [double]::MinValue
    $minY = [double]::MaxValue; $maxY = [double]::MinValue
    foreach ($p in $points) {
        if ($p.X -lt $minX) { $minX = $p.X }
        if ($p.X -gt $maxX) { $maxX = $p.X }
        if ($p.Y -lt $minY) { $minY = $p.Y }
        if ($p.Y -gt $maxY) { $maxY = $p.Y }
    }
    $shiftX = $center - ($minX + $maxX) / 2.0
    $shiftY = $center - ($minY + $maxY) / 2.0
    for ($i = 0; $i -lt $points.Count; $i++) {
        $points[$i] = [System.Drawing.PointF]::new(
            [single]($points[$i].X + $shiftX), [single]($points[$i].Y + $shiftY))
    }

    $path = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $path.AddPolygon([System.Drawing.PointF[]]$points.ToArray())
    $path.CloseFigure()
    return , $path
}

function New-MarkBitmap {
    param([int]$Size, [string]$Color, [string]$Keyline = '', [double]$KeylineWidth = 0)

    $big = $Size * $Supersample
    $scale = $big / $Design
    $keyWidth = if ($Keyline -and $KeylineWidth -le 0) { [Math]::Max($KeylineDesign, 1.15 * $Design / $Size) } else { $KeylineWidth }

    $hi = [System.Drawing.Bitmap]::new($big, $big, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($hi)
    $brush = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml($Color))
    $path = Get-CrescentPath $scale
    try {
        $g.Clear([System.Drawing.Color]::Transparent)
        $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality

        if ($Keyline -and $keyWidth -gt 0) {
            $keyPen = [System.Drawing.Pen]::new([System.Drawing.ColorTranslator]::FromHtml($Keyline), [single]($keyWidth * $scale))
            try {
                $keyPen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
                $g.DrawPath($keyPen, $path)
            }
            finally { $keyPen.Dispose() }
        }

        $g.FillPath($brush, $path)
    }
    finally {
        $path.Dispose(); $brush.Dispose(); $g.Dispose()
    }

    $out = [System.Drawing.Bitmap]::new($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g2 = [System.Drawing.Graphics]::FromImage($out)
    try {
        $g2.Clear([System.Drawing.Color]::Transparent)
        $g2.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $g2.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $g2.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
        $g2.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceCopy
        $g2.DrawImage($hi, [System.Drawing.Rectangle]::new(0, 0, $Size, $Size))
    }
    finally { $g2.Dispose(); $hi.Dispose() }
    return $out
}

function Get-PngBytes {
    param([System.Drawing.Bitmap]$Bitmap)

    $stream = [System.IO.MemoryStream]::new()
    try {
        $Bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
        Write-Output -NoEnumerate $stream.ToArray()
    }
    finally { $stream.Dispose() }
}

function Save-Png {
    param([System.Drawing.Bitmap]$Bitmap, [string]$Path)

    $dir = Split-Path -Parent $Path
    if ($dir) { [System.IO.Directory]::CreateDirectory($dir) | Out-Null }
    $Bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
}

function Save-Ico {
    param([string]$Path)

    $frames = @($IcoSizes | ForEach-Object {
            $bitmap = New-MarkBitmap $_ $LightColor $KeylineColor 0
            try { Get-PngBytes $bitmap } finally { $bitmap.Dispose() }
        })

    $file = [System.IO.File]::Create($Path)
    $writer = [System.IO.BinaryWriter]::new($file)
    try {
        $writer.Write([uint16]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]$frames.Count)

        $offset = 6 + (16 * $frames.Count)
        for ($index = 0; $index -lt $frames.Count; $index++) {
            $size = $IcoSizes[$index]
            $dimension = if ($size -eq 256) { 0 } else { $size }
            $writer.Write([byte]$dimension)
            $writer.Write([byte]$dimension)
            $writer.Write([byte]0)   # palette colours
            $writer.Write([byte]0)   # reserved
            $writer.Write([uint16]1) # colour planes
            $writer.Write([uint16]32)
            $writer.Write([uint32]$frames[$index].Length)
            $writer.Write([uint32]$offset)
            $offset += $frames[$index].Length
        }
        foreach ($frame in $frames) { $writer.Write($frame) }
    }
    finally {
        $writer.Dispose()
        $file.Dispose()
    }
}

[System.IO.Directory]::CreateDirectory($ResourcesDir) | Out-Null

$master = New-MarkBitmap 1024 $LightColor $KeylineColor 0
try { Save-Png $master (Join-Path $ResourcesDir 'Moonglow.png') } finally { $master.Dispose() }







Save-Ico (Join-Path $ResourcesDir 'Moonglow.ico')

foreach ($size in $HicolorSizes) {
    $bitmap = New-MarkBitmap $size $LightColor $KeylineColor 0
    try {
        Save-Png $bitmap (Join-Path $ResourcesDir "linux/hicolor/${size}x${size}/apps/moonglow.png")
    }
    finally { $bitmap.Dispose() }
}

Write-Host "Generated logo assets in $ResourcesDir"
