[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$PackageIdentityName,

    [Parameter(Mandatory = $true)]
    [string]$PackagePublisher,

    [Parameter(Mandatory = $true)]
    [string]$PackagePublisherDisplayName
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Resolve-WindowsSdkTool([string]$ToolName) {
    $sdkRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    if (-not (Test-Path $sdkRoot)) {
        throw "Windows SDK was not found at $sdkRoot. Install the Windows 10 SDK."
    }

    $sdkVersions = Get-ChildItem -LiteralPath $sdkRoot -Directory | Sort-Object Name -Descending
    foreach ($sdkVersion in $sdkVersions) {
        $candidate = Join-Path $sdkVersion.FullName "x64\$ToolName"
        if (Test-Path $candidate) { return $candidate }
    }

    throw "$ToolName was not found in the installed Windows SDK."
}

function New-StoreLogoAsset([string]$Source, [string]$Destination, [int]$Size) {
    $sourceImage = [System.Drawing.Image]::FromFile($Source)
    $bitmap = [System.Drawing.Bitmap]::new($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.Clear([System.Drawing.Color]::Transparent)
        $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
        $graphics.DrawImage($sourceImage, 0, 0, $Size, $Size)
        $bitmap.Save($Destination, [System.Drawing.Imaging.ImageFormat]::Png)
    }
    finally {
        $graphics.Dispose()
        $bitmap.Dispose()
        $sourceImage.Dispose()
    }
}

if ($PackageIdentityName -notmatch '^[A-Za-z0-9.\-]+$') {
    throw 'PackageIdentityName may contain only letters, digits, periods, and hyphens.'
}
if ([string]::IsNullOrWhiteSpace($PackagePublisher)) {
    throw 'PackagePublisher must exactly match the Publisher value assigned in Partner Center.'
}
if ([string]::IsNullOrWhiteSpace($PackagePublisherDisplayName)) {
    throw 'PackagePublisherDisplayName must exactly match the PublisherDisplayName value assigned in Partner Center.'
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$version = (Get-Content -LiteralPath (Join-Path $repoRoot 'VERSION') -Raw).Trim()
if ($version -notmatch '^(\d+)\.(\d+)\.(\d+)$') {
    throw "VERSION must be numeric major.minor.patch; found '$version'."
}
$packageVersion = "$version.0"

$windowsProject = Join-Path $repoRoot 'apps\windows\Moonglow.csproj'
$manifestTemplate = Join-Path $repoRoot 'apps\windows\StorePackage\Package.appxmanifest'
$sourceLogo = Join-Path $repoRoot 'resources\Moonglow-option-e-amber-noring.png'
$outputDir = Join-Path $repoRoot "release\$version\windows"
$publishDir = Join-Path ([System.IO.Path]::GetTempPath()) "moonglow-store-publish-$([guid]::NewGuid().ToString('N'))"
$stageDir = Join-Path ([System.IO.Path]::GetTempPath()) "moonglow-store-stage-$([guid]::NewGuid().ToString('N'))"
$uploadStage = Join-Path ([System.IO.Path]::GetTempPath()) "moonglow-store-upload-$([guid]::NewGuid().ToString('N'))"
$pfxPath = Join-Path ([System.IO.Path]::GetTempPath()) "moonglow-store-$([guid]::NewGuid().ToString('N')).pfx"
$msixPath = Join-Path $outputDir 'Moonglow-Store.msix'
$uploadPath = Join-Path $outputDir 'Moonglow-Store.msixupload'
$uploadZip = Join-Path ([System.IO.Path]::GetTempPath()) "moonglow-store-upload-$([guid]::NewGuid().ToString('N')).zip"
$cert = $null

try {
    Add-Type -AssemblyName System.Drawing
    New-Item -ItemType Directory -Force -Path $outputDir, $stageDir, $uploadStage | Out-Null

    Write-Host "Publishing Moonglow $version for MSIX..." -ForegroundColor Cyan
    & dotnet publish $windowsProject --configuration Release --nologo "-p:Version=$version" `
        -p:DebugType=none -p:DebugSymbols=false --output $publishDir
    if ($LASTEXITCODE -ne 0) { throw "dotnet publish failed with exit code $LASTEXITCODE." }

    Copy-Item -Path (Join-Path $publishDir '*') -Destination $stageDir -Recurse -Force
    $assetsDir = Join-Path $stageDir 'Assets'
    New-Item -ItemType Directory -Force -Path $assetsDir | Out-Null
    New-StoreLogoAsset $sourceLogo (Join-Path $assetsDir 'StoreLogo.png') 50
    New-StoreLogoAsset $sourceLogo (Join-Path $assetsDir 'Square44x44Logo.png') 44
    New-StoreLogoAsset $sourceLogo (Join-Path $assetsDir 'Square150x150Logo.png') 150

    [xml]$manifest = Get-Content -LiteralPath $manifestTemplate -Raw
    $identity = $manifest.SelectSingleNode("/*[local-name()='Package']/*[local-name()='Identity']")
    if ($null -eq $identity) { throw 'Package manifest is missing its Identity element.' }
    $identity.SetAttribute('Name', $PackageIdentityName)
    $identity.SetAttribute('Publisher', $PackagePublisher)
    $identity.SetAttribute('Version', $packageVersion)
    $publisherDisplayName = $manifest.SelectSingleNode("/*[local-name()='Package']/*[local-name()='Properties']/*[local-name()='PublisherDisplayName']")
    if ($null -eq $publisherDisplayName) { throw 'Package manifest is missing its PublisherDisplayName element.' }
    $publisherDisplayName.InnerText = $PackagePublisherDisplayName
    $manifest.Save((Join-Path $stageDir 'AppxManifest.xml'))

    $makeAppx = Resolve-WindowsSdkTool 'makeappx.exe'
    $signTool = Resolve-WindowsSdkTool 'signtool.exe'
    Remove-Item -LiteralPath $msixPath, $uploadPath -Force -ErrorAction SilentlyContinue
    & $makeAppx pack /v /h SHA256 /o /d $stageDir /p $msixPath
    if ($LASTEXITCODE -ne 0) { throw "MakeAppx failed with exit code $LASTEXITCODE." }

    # MakeAppx produces unsigned packages. A temporary certificate with the Store publisher
    # subject lets Partner Center validate the package identity; the Store re-signs it for users.
    $cert = New-SelfSignedCertificate `
        -Type Custom `
        -Subject $PackagePublisher `
        -KeyUsage DigitalSignature `
        -KeyAlgorithm RSA `
        -KeyLength 2048 `
        -HashAlgorithm SHA256 `
        -CertStoreLocation 'Cert:\CurrentUser\My' `
        -KeyExportPolicy Exportable `
        -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}') `
        -FriendlyName 'Moonglow temporary Microsoft Store package signer'
    $pfxPasswordText = [guid]::NewGuid().ToString('N')
    $pfxPassword = ConvertTo-SecureString -String $pfxPasswordText -AsPlainText -Force
    Export-PfxCertificate -Cert $cert -FilePath $pfxPath -Password $pfxPassword | Out-Null
    & $signTool sign /fd SHA256 /a /f $pfxPath /p $pfxPasswordText $msixPath
    if ($LASTEXITCODE -ne 0) { throw "SignTool failed with exit code $LASTEXITCODE." }

    Copy-Item -LiteralPath $msixPath -Destination $uploadStage
    Compress-Archive -Path (Join-Path $uploadStage '*') -DestinationPath $uploadZip -Force
    Move-Item -LiteralPath $uploadZip -Destination $uploadPath -Force

    $checksums = Get-ChildItem -LiteralPath $outputDir -File |
        Where-Object { $_.Name -ne 'SHA256SUMS' } |
        Sort-Object Name |
        ForEach-Object {
            $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            "$hash  $($_.Name)"
        }
    [System.IO.File]::WriteAllLines(
        (Join-Path $outputDir 'SHA256SUMS'),
        [string[]]$checksums,
        [System.Text.UTF8Encoding]::new($false))

    Write-Host "MSIX:       $msixPath" -ForegroundColor Green
    Write-Host "Store file: $uploadPath" -ForegroundColor Green
}
finally {
    if ($cert) {
        Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($cert.Thumbprint)" -Force -ErrorAction SilentlyContinue
    }
    Remove-Item -LiteralPath $pfxPath -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $uploadZip -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $publishDir, $stageDir, $uploadStage -Recurse -Force -ErrorAction SilentlyContinue
}
