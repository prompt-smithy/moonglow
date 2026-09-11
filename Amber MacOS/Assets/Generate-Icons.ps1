param(
    [string]$OutputDirectory = $PSScriptRoot
)

Add-Type -AssemblyName System.Drawing

$sizes = @(16, 32, 48, 64, 128, 256)
$pngFrames = [System.Collections.Generic.List[byte[]]]::new()

function New-AmberSunPng([int]$Size) {
    $bitmap = [System.Drawing.Bitmap]::new($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.Clear([System.Drawing.Color]::Transparent)

    $center = $Size / 2.0
    $rayInner = $Size * 0.35
    $rayOuter = $Size * 0.45
    $rayWidth = [Math]::Max(1.5, $Size * 0.065)
    $rayPen = [System.Drawing.Pen]::new([System.Drawing.ColorTranslator]::FromHtml('#FFA000'), $rayWidth)
    $rayPen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $rayPen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round

    for ($angle = 0; $angle -lt 360; $angle += 45) {
        $radians = $angle * [Math]::PI / 180
        $graphics.DrawLine(
            $rayPen,
            [float]($center + [Math]::Cos($radians) * $rayInner),
            [float]($center + [Math]::Sin($radians) * $rayInner),
            [float]($center + [Math]::Cos($radians) * $rayOuter),
            [float]($center + [Math]::Sin($radians) * $rayOuter))
    }

    $diameter = $Size * 0.54
    $offset = ($Size - $diameter) / 2
    $sunBrush = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml('#FFC107'))
    $graphics.FillEllipse($sunBrush, [float]$offset, [float]$offset, [float]$diameter, [float]$diameter)

    $stream = [System.IO.MemoryStream]::new()
    $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    $bytes = $stream.ToArray()

    $stream.Dispose()
    $sunBrush.Dispose()
    $rayPen.Dispose()
    $graphics.Dispose()
    $bitmap.Dispose()
    return $bytes
}

New-Item -ItemType Directory -Force $OutputDirectory | Out-Null

foreach ($size in $sizes) {
    $pngFrames.Add((New-AmberSunPng $size))
}

[System.IO.File]::WriteAllBytes(
    (Join-Path $OutputDirectory 'Amber.png'),
    (New-AmberSunPng 1024))

$iconPath = Join-Path $OutputDirectory 'Amber.ico'
$stream = [System.IO.File]::Create($iconPath)
$writer = [System.IO.BinaryWriter]::new($stream)
$writer.Write([uint16]0)
$writer.Write([uint16]1)
$writer.Write([uint16]$sizes.Count)

$offset = 6 + (16 * $sizes.Count)
for ($index = 0; $index -lt $sizes.Count; $index++) {
    $size = $sizes[$index]
    $frame = $pngFrames[$index]
    $writer.Write([byte]$(if ($size -ge 256) { 0 } else { $size }))
    $writer.Write([byte]$(if ($size -ge 256) { 0 } else { $size }))
    $writer.Write([byte]0)
    $writer.Write([byte]0)
    $writer.Write([uint16]1)
    $writer.Write([uint16]32)
    $writer.Write([uint32]$frame.Length)
    $writer.Write([uint32]$offset)
    $offset += $frame.Length
}

foreach ($frame in $pngFrames) {
    $writer.Write($frame)
}

$writer.Dispose()
$stream.Dispose()
Write-Host "Created Amber.png and Amber.ico in $OutputDirectory"