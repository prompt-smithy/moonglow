<#
.SYNOPSIS
    Resizes the amber logo master into Windows and Linux icon assets.
.DESCRIPTION
    Consumes resources/Moonglow-option-e-amber-noring.png, the same artwork as the macOS
    app and menu bar. The master PNG/SVG are never redrawn or overwritten.
    Use -WindowsOnly when updating just the Windows executable, launcher, and tray icon.
#>
[CmdletBinding()]
param(
    [string]$ResourcesDir,
    [switch]$WindowsOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Drawing
if (-not $ResourcesDir) { $ResourcesDir = Join-Path $PSScriptRoot '..\resources' }
$ResourcesDir = [System.IO.Path]::GetFullPath($ResourcesDir)
$sourcePath = Join-Path $PSScriptRoot '..\resources\Moonglow-option-e-amber-noring.png'
$source = [System.Drawing.Image]::FromFile($sourcePath)
$icoSizes = @(16, 24, 32, 48, 64, 128, 256)

function New-LogoBitmap([int]$Size) {
    $bitmap = [System.Drawing.Bitmap]::new($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.Clear([System.Drawing.Color]::Transparent)
        $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $graphics.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceCopy
        $graphics.DrawImage($source, [System.Drawing.Rectangle]::new(0, 0, $Size, $Size))
    }
    finally { $graphics.Dispose() }
    return $bitmap
}

try {
    [System.IO.Directory]::CreateDirectory($ResourcesDir) | Out-Null
    $frames = @($icoSizes | ForEach-Object {
        $bitmap = New-LogoBitmap $_
        $stream = [System.IO.MemoryStream]::new()
        try {
            $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
            Write-Output -NoEnumerate $stream.ToArray()
        }
        finally { $stream.Dispose(); $bitmap.Dispose() }
    })

    $writer = [System.IO.BinaryWriter]::new([System.IO.File]::Create((Join-Path $ResourcesDir 'Moonglow.ico')))
    try {
        $writer.Write([uint16]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]$frames.Count)
        $offset = 6 + 16 * $frames.Count
        for ($index = 0; $index -lt $frames.Count; $index++) {
            $dimension = if ($icoSizes[$index] -eq 256) { 0 } else { $icoSizes[$index] }
            $writer.Write([byte]$dimension)
            $writer.Write([byte]$dimension)
            $writer.Write([byte]0)
            $writer.Write([byte]0)
            $writer.Write([uint16]1)
            $writer.Write([uint16]32)
            $writer.Write([uint32]$frames[$index].Length)
            $writer.Write([uint32]$offset)
            $offset += $frames[$index].Length
        }
        foreach ($frame in $frames) { $writer.Write([byte[]]$frame) }
    }
    finally { $writer.Dispose() }

    if (-not $WindowsOnly) {
        foreach ($size in @(16, 24, 32, 48, 64, 128, 256, 512)) {
            $destination = Join-Path $ResourcesDir "linux\hicolor\${size}x${size}\apps\moonglow.png"
            [System.IO.Directory]::CreateDirectory((Split-Path -Parent $destination)) | Out-Null
            $bitmap = New-LogoBitmap $size
            try { $bitmap.Save($destination, [System.Drawing.Imaging.ImageFormat]::Png) }
            finally { $bitmap.Dispose() }
        }
    }
}
finally { $source.Dispose() }

Write-Host "Generated logo assets in $ResourcesDir"
