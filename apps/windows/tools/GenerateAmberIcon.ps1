[CmdletBinding()]
param(
    [string]$OutputPath = (Join-Path $PSScriptRoot '..\Assets\Amber.ico')
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

function New-IconFrame([int]$size) {
    $bitmap = [System.Drawing.Bitmap]::new($size, $size)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.Clear([System.Drawing.Color]::Transparent)
        $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality

        $scale = $size / 256.0
        $center = $size / 2.0
        $amber = [System.Drawing.Color]::FromArgb(255, 255, 193, 7)
        $amberDark = [System.Drawing.Color]::FromArgb(255, 255, 160, 0)

        $pen = [System.Drawing.Pen]::new($amber, [Math]::Max(1.5, 14 * $scale))
        try {
            $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
            $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
            for ($index = 0; $index -lt 8; $index++) {
                $angle = $index * [Math]::PI / 4
                $inner = 82 * $scale
                $outer = 113 * $scale
                $graphics.DrawLine(
                    $pen,
                    [single]($center + [Math]::Cos($angle) * $inner),
                    [single]($center + [Math]::Sin($angle) * $inner),
                    [single]($center + [Math]::Cos($angle) * $outer),
                    [single]($center + [Math]::Sin($angle) * $outer))
            }
        }
        finally {
            $pen.Dispose()
        }

        $sunBounds = [System.Drawing.RectangleF]::new(
            [single]($center - 62 * $scale),
            [single]($center - 62 * $scale),
            [single](124 * $scale),
            [single](124 * $scale))
        $brush = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
            $sunBounds, $amber, $amberDark, 90.0)
        try {
            $graphics.FillEllipse($brush, $sunBounds)
        }
        finally {
            $brush.Dispose()
        }

        $stream = [System.IO.MemoryStream]::new()
        try {
            $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
            $bytes = $stream.ToArray()
            Write-Output -NoEnumerate $bytes
        }
        finally {
            $stream.Dispose()
        }
    }
    finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

$sizes = @(16, 24, 32, 48, 64, 128, 256)
$frames = @($sizes | ForEach-Object { New-IconFrame $_ })
$outputDirectory = Split-Path -Parent $OutputPath
[System.IO.Directory]::CreateDirectory($outputDirectory) | Out-Null

$file = [System.IO.File]::Create($OutputPath)
$writer = [System.IO.BinaryWriter]::new($file)
try {
    $writer.Write([uint16]0)
    $writer.Write([uint16]1)
    $writer.Write([uint16]$frames.Count)

    $offset = 6 + (16 * $frames.Count)
    for ($index = 0; $index -lt $frames.Count; $index++) {
        $size = $sizes[$index]
        $iconDimension = if ($size -eq 256) { 0 } else { $size }
        $writer.Write([byte]$iconDimension)
        $writer.Write([byte]$iconDimension)
        $writer.Write([byte]0)
        $writer.Write([byte]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]32)
        $writer.Write([uint32]$frames[$index].Length)
        $writer.Write([uint32]$offset)
        $offset += $frames[$index].Length
    }

    foreach ($frame in $frames) {
        $writer.Write($frame)
    }
}
finally {
    $writer.Dispose()
    $file.Dispose()
}

Write-Host "Generated $OutputPath"