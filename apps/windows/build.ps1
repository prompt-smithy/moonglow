<#
.SYNOPSIS
    Builds the distributable artifacts of Moonglow with a single command.

.DESCRIPTION
    Moonglow targets .NET Framework 4.8. The runtime is a Windows component rather than
    something we ship - it is preinstalled on Windows 10 1903+ and Windows 11, and can be
    installed on Windows 7 SP1 and later. The portable launcher is one file, but it still
    requires the .NET Framework runtime on the target PC.

    Produces two distributable artifacts under bin\:

        bin\Moonglow_Portable.exe      (1) single-file self-extracting portable launcher
        bin\Installer\Moonglow.exe    (2) single setup executable produced by Inno Setup

    The portable launcher embeds the published app, extracts it to a temporary directory,
    runs Moonglow, and removes the extracted files when Moonglow exits.
    The installer is compiled by Inno Setup (ISCC.exe); if Inno Setup is not found the script
    attempts to install it via winget, and if that fails the portable build still succeeds and
    the installer step is skipped with a warning.

.EXAMPLE
    .\build.ps1
#>
[CmdletBinding()]
param([switch]$SkipInstaller)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root      = $PSScriptRoot
$project   = Join-Path $root 'Moonglow.csproj'
$issScript = Join-Path $root 'installer\Moonglow.iss'
$binDir    = Join-Path $root 'bin'
$appExe    = 'Moonglow.exe'
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $root '../..'))
$releaseVersion = (Get-Content (Join-Path $repoRoot 'VERSION') -Raw).Trim()
$versionDir = Join-Path $repoRoot "release/$releaseVersion"
$releaseDir = Join-Path $versionDir 'windows'
foreach ($platform in @('linux', 'windows', 'macOs')) {
    New-Item -ItemType Directory -Force -Path (Join-Path $versionDir $platform) | Out-Null
}

# Output folders.
$portableDir  = Join-Path $binDir 'Portable'
$portableExe  = Join-Path $binDir 'Moonglow_Portable.exe'
$installerDir = Join-Path $binDir 'Installer'
$stagingDir   = Join-Path $binDir '_staging'
$launcherProject = Join-Path $root 'tools\PortableLauncher\PortableLauncher.csproj'

# Left over from the .NET 10 era; removed so bin\ is not confusing after upgrading.
$legacyDirs = @(
    (Join-Path $binDir 'Portable-SelfContained'),
    (Join-Path $binDir 'Portable-FrameworkDependent'),
    (Join-Path $binDir 'Installer-SelfContained'),
    (Join-Path $binDir 'Installer-FrameworkDependent'),
    (Join-Path $binDir '_staging')
)

function Write-Step([string]$message) {
    Write-Host ''
    Write-Host "==> $message" -ForegroundColor Cyan
}

function Invoke-Checked([string]$file, [string[]]$arguments) {
    & $file @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed ($LASTEXITCODE): $file $($arguments -join ' ')"
    }
}

function Get-AppVersion {
    $content = Get-Content $project -Raw
    if ($content -match '<Version>\s*([^<]+?)\s*</Version>') { return $Matches[1] }
    return $releaseVersion
}

function Resolve-Iscc {
    $cmd = Get-Command 'iscc.exe' -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }

    $candidates = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles        'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA         'Programs\Inno Setup 6\ISCC.exe')
    )
    foreach ($c in $candidates) {
        if ($c -and (Test-Path $c)) { return $c }
    }

    # Not installed - try winget.
    if (Get-Command 'winget' -ErrorAction SilentlyContinue) {
        Write-Host '    Inno Setup not found; attempting installation via winget...' -ForegroundColor Yellow
        winget install --exact --id JRSoftware.InnoSetup `
            --accept-source-agreements --accept-package-agreements --silent | Out-Null

        foreach ($c in $candidates) {
            if ($c -and (Test-Path $c)) { return $c }
        }
        $cmd = Get-Command 'iscc.exe' -ErrorAction SilentlyContinue
        if ($cmd) { return $cmd.Source }
    }

    return $null
}

$version = Get-AppVersion
if ($version -ne $releaseVersion) { throw "Moonglow.csproj version must match root VERSION ($releaseVersion)." }
Write-Host "Moonglow packaging - version $version (.NET Framework 4.8)" -ForegroundColor Green

# ---------------------------------------------------------------------------
# Clean previous artifacts.
# ---------------------------------------------------------------------------
Write-Step 'Cleaning previous artifacts'
foreach ($dir in @($portableDir, $installerDir) + $legacyDirs) {
    if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
}
foreach ($legacyFile in @(
    (Join-Path $binDir 'Moonglow_Portable.exe'),
    (Join-Path $binDir 'Moonglow.exe'),
    (Join-Path $installerDir 'Moonglow.exe')
)) {
    if (Test-Path $legacyFile) { Remove-Item $legacyFile -Force }
}

# ---------------------------------------------------------------------------
# 1. Portable: the app plus its dependencies, ready to copy and run.
# ---------------------------------------------------------------------------
Write-Step '(1/2) Portable build'
Invoke-Checked 'dotnet' @(
    'publish', $project,
    '-c', 'Release',
    '--nologo',
    "-p:Version=$version",
    '-p:DebugType=none',
    '-p:EnableWindowsTargeting=true',
    '-p:DebugSymbols=false',
    '-o', $portableDir
)

New-Item -ItemType Directory -Force -Path $stagingDir | Out-Null
$launcherOutput = Join-Path $stagingDir 'Launcher'
$payloadPath = (Resolve-Path $portableDir).Path
Invoke-Checked 'dotnet' @(
    'build', $launcherProject,
    '-c', 'Release',
    '--nologo',
    "-p:PayloadDir=$payloadPath",
    "-p:Version=$version",
    "-p:OutputPath=$launcherOutput"
)
$builtLauncher = Join-Path $launcherOutput 'Moonglow_Portable.exe'
if (-not (Test-Path $builtLauncher)) { throw "Portable launcher was not produced at $builtLauncher" }
Copy-Item $builtLauncher $portableExe -Force

# ---------------------------------------------------------------------------
# 2. Installer via Inno Setup, wrapping the portable build.
# ---------------------------------------------------------------------------
$iscc = $null
if (-not $SkipInstaller -and [System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT) {
    $iscc = Resolve-Iscc
}
if (-not $iscc) {
    Write-Warning 'Inno Setup (ISCC.exe) was not found and could not be installed automatically.'
    Write-Warning 'The portable build completed. To build the installer, install Inno Setup 6'
    Write-Warning '  (https://jrsoftware.org/isdl.php) and re-run this script.'
} else {
    Write-Host "    Using Inno Setup compiler: $iscc" -ForegroundColor DarkGray

    Write-Step '(2/2) Installer'
    New-Item -ItemType Directory -Force -Path $installerDir | Out-Null
    Invoke-Checked $iscc @(
        "/DSourceDir=$portableDir",
        "/DOutputDir=$installerDir",
        "/DAppVersion=$version",
        $issScript
    )
}

# Collect distributables in the shared versioned release tree.
Copy-Item $portableExe (Join-Path $releaseDir 'Moonglow_Portable.exe') -Force
$portableZip = Join-Path $releaseDir 'Moonglow-windows-portable.zip'
Compress-Archive -Path (Join-Path $portableDir '*') -DestinationPath $portableZip -Force
$installer = Join-Path $installerDir $appExe
if (Test-Path $installer) { Copy-Item $installer (Join-Path $releaseDir 'Moonglow-Setup.exe') -Force }
elseif (Test-Path (Join-Path $releaseDir 'Moonglow-Setup.exe')) { Remove-Item (Join-Path $releaseDir 'Moonglow-Setup.exe') }
$checksums = Get-ChildItem $releaseDir -File | Where-Object { $_.Name -ne 'SHA256SUMS' } | Sort-Object Name | ForEach-Object {
    $hash = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $($_.Name)"
}
[System.IO.File]::WriteAllLines((Join-Path $releaseDir 'SHA256SUMS'), [string[]]$checksums, [System.Text.UTF8Encoding]::new($false))

Write-Step 'Build summary'
$outputs = [ordered]@{
    'Portable (single file)' = $portableExe
    'Installer' = Join-Path $installerDir $appExe
}
$report = foreach ($name in $outputs.Keys) {
    $path = $outputs[$name]
    $exists = Test-Path $path
    [pscustomobject]@{
        Artifact = $name
        Built    = if ($exists) { 'yes' } else { 'no' }
        'Size(MB)' = if ($exists) { [math]::Round((Get-Item $path).Length / 1MB, 1) } else { '-' }
        Path     = $path.Replace("$root\", '')
    }
}
$report | Format-Table -AutoSize

if (-not (Test-Path $portableExe)) {
    Write-Warning 'The portable build was not produced.'
    exit 1
}

if (Test-Path (Join-Path $installerDir $appExe)) {
    Write-Host 'Both artifacts built successfully.' -ForegroundColor Green
} else {
    Write-Host 'Portable build created; the installer was skipped.' -ForegroundColor Yellow
}



