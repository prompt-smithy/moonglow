[CmdletBinding()]
param(
    [string]$SourcePath = (Join-Path $PSScriptRoot '..\Assets\Moonglow.png'),
    [string]$SmallSourcePath = (Join-Path $PSScriptRoot '..\Assets\Moonglow-small.png'),
    [string]$OutputPath = (Join-Path $PSScriptRoot '..\Assets\Moonglow.ico')
)

# Rebuilds Assets\Moonglow.ico from the packaged logo art. tools/generate-app-icons.py is the
# canonical generator for every platform; this script exists so the icon can be rebuilt on
# Windows without Python. Keep the two in step: frames below 128 px are written as BMP data,
# which every icon reader can decode, and larger frames as PNG so the file stays small.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

function New-IconBitmap([System.Drawing.Image]$source, [int]$size) {
    $bitmap = [System.Drawing.Bitmap]::new($size, $size)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.Clear([System.Drawing.Color]::Transparent)
        $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $graphics.DrawImage($source, 0, 0, $size, $size)
        Write-Output -NoEnumerate $bitmap
    }
    finally {
        $graphics.Dispose()
    }
}

function New-PngFrame([System.Drawing.Bitmap]$bitmap) {
    $stream = [System.IO.MemoryStream]::new()
    try {
        $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
        Write-Output -NoEnumerate $stream.ToArray()
    }
    finally {
        $stream.Dispose()
    }
}

function New-BmpFrame([System.Drawing.Bitmap]$bitmap) {
    $size = $bitmap.Width
    $maskStride = [int](([math]::Floor(($size + 31) / 32)) * 4)
    $pixels = [byte[]]::new($size * $size * 4)
    $mask = [byte[]]::new($maskStride * $size)
    $cursor = 0
    for ($row = 0; $row -lt $size; $row++) {
        $y = $size - 1 - $row
        $maskOffset = $row * $maskStride
        for ($x = 0; $x -lt $size; $x++) {
            $color = $bitmap.GetPixel($x, $y)
            $pixels[$cursor++] = $color.B
            $pixels[$cursor++] = $color.G
            $pixels[$cursor++] = $color.R
            $pixels[$cursor++] = $color.A
            if ($color.A -eq 0) {
                $index = $maskOffset + [int][math]::Floor($x / 8)
                $mask[$index] = $mask[$index] -bor (0x80 -shr ($x % 8))
            }
        }
    }

    $frame = [System.IO.MemoryStream]::new()
    $writer = [System.IO.BinaryWriter]::new($frame)
    try {
        $writer.Write([uint32]40)
        $writer.Write([int32]$size)
        $writer.Write([int32]($size * 2))
        $writer.Write([uint16]1)
        $writer.Write([uint16]32)
        $writer.Write([uint32]0)
        $writer.Write([uint32]($pixels.Length + $mask.Length))
        $writer.Write([int32]0)
        $writer.Write([int32]0)
        $writer.Write([uint32]0)
        $writer.Write([uint32]0)
        $writer.Write($pixels)
        $writer.Write($mask)
        $writer.Flush()
        Write-Output -NoEnumerate $frame.ToArray()
    }
    finally {
        $writer.Dispose()
    }
}

$sizes = @(16, 24, 32, 48, 64, 128, 256)
$full = [System.Drawing.Image]::FromFile((Resolve-Path $SourcePath))
$small = [System.Drawing.Image]::FromFile((Resolve-Path $SmallSourcePath))
try {
    $frames = @(
        foreach ($size in $sizes) {
            # Below 33 px the eight rays fall under a pixel; use the crescent-only art.
            $source = if ($size -le 32) { $small } else { $full }
            $bitmap = New-IconBitmap $source $size
            try {
                if ($size -ge 128) { New-PngFrame $bitmap } else { New-BmpFrame $bitmap }
            }
            finally {
                $bitmap.Dispose()
            }
        }
    )
}
finally {
    $full.Dispose()
    $small.Dispose()
}

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
