<#
.SYNOPSIS
    Builds every distributable flavor of Amber with a single command.

.DESCRIPTION
    Produces four artifacts, all named Amber.exe, each in its own folder
    under bin\:

        bin\Portable-SelfContained\Amber.exe        (1) portable, no .NET required
        bin\Portable-FrameworkDependent\Amber.exe   (2) portable, needs .NET runtime
        bin\Installer-SelfContained\Amber.exe       (3) installer, no .NET required
        bin\Installer-FrameworkDependent\Amber.exe  (4) installer, needs .NET runtime

    Portable builds are single-file executables produced by `dotnet publish`.
    Installers are single setup.exe files produced by Inno Setup (ISCC.exe). If Inno
    Setup is not found the script attempts to install it via winget; if that fails the
    portable builds still succeed and the installer steps are skipped with a warning.

.PARAMETER Runtime
    The .NET runtime identifier to target. Defaults to win-x64.

.EXAMPLE
    .\build.ps1
#>
[CmdletBinding()]
param(
    [string]$Runtime = 'win-x64'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root       = $PSScriptRoot
$project    = Join-Path $root 'Amber.csproj'
$issScript  = Join-Path $root 'installer\Amber.iss'
$binDir     = Join-Path $root 'bin'
$stagingDir = Join-Path $binDir '_staging'
$appExe     = 'Amber.exe'

# Output folders (same file name, different folders).
$portableScDir  = Join-Path $binDir 'Portable-SelfContained'
$portableFdDir  = Join-Path $binDir 'Portable-FrameworkDependent'
$installerScDir = Join-Path $binDir 'Installer-SelfContained'
$installerFdDir = Join-Path $binDir 'Installer-FrameworkDependent'

# Non-single-file publish folders used only as installer sources.
$stageScDir = Join-Path $stagingDir 'app-SelfContained'
$stageFdDir = Join-Path $stagingDir 'app-FrameworkDependent'

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
    return '1.0.0'
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
Write-Host "Amber packaging - version $version, runtime $Runtime" -ForegroundColor Green

# ---------------------------------------------------------------------------
# Clean previous artifacts.
# ---------------------------------------------------------------------------
Write-Step 'Cleaning previous artifacts'
foreach ($dir in @($portableScDir, $portableFdDir, $installerScDir, $installerFdDir, $stagingDir)) {
    if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
}

# ---------------------------------------------------------------------------
# 1. Portable, self-contained (no .NET framework needed) - single file.
# ---------------------------------------------------------------------------
Write-Step '(1/4) Portable - self-contained single file'
Invoke-Checked 'dotnet' @(
    'publish', $project,
    '-c', 'Release',
    '-r', $Runtime,
    '--self-contained', 'true',
    '-p:PublishSingleFile=true',
    '-p:IncludeNativeLibrariesForSelfExtract=true',
    '-p:EnableCompressionInSingleFile=true',
    '-p:DebugType=none',
    '-p:DebugSymbols=false',
    '-o', $portableScDir
)

# ---------------------------------------------------------------------------
# 2. Portable, framework-dependent (needs .NET runtime) - single file.
# ---------------------------------------------------------------------------
Write-Step '(2/4) Portable - framework-dependent single file'
Invoke-Checked 'dotnet' @(
    'publish', $project,
    '-c', 'Release',
    '-r', $Runtime,
    '--self-contained', 'false',
    '-p:PublishSingleFile=true',
    '-p:DebugType=none',
    '-p:DebugSymbols=false',
    '-o', $portableFdDir
)

# ---------------------------------------------------------------------------
# Stage plain (multi-file) publishes to feed the installers.
# ---------------------------------------------------------------------------
Write-Step 'Staging application files for installers'
Invoke-Checked 'dotnet' @(
    'publish', $project,
    '-c', 'Release',
    '-r', $Runtime,
    '--self-contained', 'true',
    '-p:PublishSingleFile=false',
    '-p:DebugType=none',
    '-p:DebugSymbols=false',
    '-o', $stageScDir
)
Invoke-Checked 'dotnet' @(
    'publish', $project,
    '-c', 'Release',
    '-r', $Runtime,
    '--self-contained', 'false',
    '-p:PublishSingleFile=false',
    '-p:DebugType=none',
    '-p:DebugSymbols=false',
    '-o', $stageFdDir
)

# ---------------------------------------------------------------------------
# 3 & 4. Installers via Inno Setup.
# ---------------------------------------------------------------------------
$iscc = Resolve-Iscc
if (-not $iscc) {
    Write-Warning 'Inno Setup (ISCC.exe) was not found and could not be installed automatically.'
    Write-Warning 'Portable builds completed. To build the installers, install Inno Setup 6'
    Write-Warning '  (https://jrsoftware.org/isdl.php) and re-run this script.'
} else {
    Write-Host "    Using Inno Setup compiler: $iscc" -ForegroundColor DarkGray

    Write-Step '(3/4) Installer - self-contained'
    New-Item -ItemType Directory -Force -Path $installerScDir | Out-Null
    Invoke-Checked $iscc @(
        "/DSourceDir=$stageScDir",
        "/DOutputDir=$installerScDir",
        "/DAppVersion=$version",
        $issScript
    )

    Write-Step '(4/4) Installer - framework-dependent'
    New-Item -ItemType Directory -Force -Path $installerFdDir | Out-Null
    Invoke-Checked $iscc @(
        "/DSourceDir=$stageFdDir",
        "/DOutputDir=$installerFdDir",
        "/DAppVersion=$version",
        '/DFrameworkDependent=1',
        $issScript
    )
}

# ---------------------------------------------------------------------------
# Clean up staging and report.
# ---------------------------------------------------------------------------
if (Test-Path $stagingDir) { Remove-Item $stagingDir -Recurse -Force }

Write-Step 'Build summary'
$outputs = [ordered]@{
    'Portable (self-contained)'      = Join-Path $portableScDir  $appExe
    'Portable (framework-dependent)' = Join-Path $portableFdDir  $appExe
    'Installer (self-contained)'     = Join-Path $installerScDir $appExe
    'Installer (framework-dependent)'= Join-Path $installerFdDir $appExe
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

if ($report | Where-Object { $_.Built -eq 'no' }) {
    Write-Warning 'One or more artifacts were not produced (see warnings above).'
    exit 1
}
Write-Host 'All four artifacts built successfully.' -ForegroundColor Green
