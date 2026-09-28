<#
.SYNOPSIS
    Builds the distributable artifacts of Amber with a single command.

.DESCRIPTION
    Amber targets .NET Framework 4.8. The runtime is a Windows component rather than
    something we ship - it is preinstalled on Windows 10 1903+ and Windows 11, and can be
    installed on Windows 7 SP1 and later - so every build is framework-dependent and there
    is no self-contained or single-file flavor to produce.

    Produces two artifacts, both named Amber.exe, each in its own folder under bin\:

        bin\Portable\Amber.exe      (1) the whole app; copy the folder and run it
        bin\Installer\Amber.exe     (2) single setup executable produced by Inno Setup

    The portable build is a plain `dotnet publish` folder. The installer is compiled by
    Inno Setup (ISCC.exe); if Inno Setup is not found the script attempts to install it via
    winget, and if that fails the portable build still succeeds and the installer step is
    skipped with a warning.

.EXAMPLE
    .\build.ps1
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root      = $PSScriptRoot
$project   = Join-Path $root 'Amber.csproj'
$issScript = Join-Path $root 'installer\Amber.iss'
$binDir    = Join-Path $root 'bin'
$appExe    = 'Amber.exe'

# Output folders (same file name, different folders).
$portableDir  = Join-Path $binDir 'Portable'
$installerDir = Join-Path $binDir 'Installer'

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
Write-Host "Amber packaging - version $version (.NET Framework 4.8)" -ForegroundColor Green

# ---------------------------------------------------------------------------
# Clean previous artifacts.
# ---------------------------------------------------------------------------
Write-Step 'Cleaning previous artifacts'
foreach ($dir in @($portableDir, $installerDir) + $legacyDirs) {
    if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
}

# ---------------------------------------------------------------------------
# 1. Portable: the app plus its dependencies, ready to copy and run.
# ---------------------------------------------------------------------------
Write-Step '(1/2) Portable build'
Invoke-Checked 'dotnet' @(
    'publish', $project,
    '-c', 'Release',
    '--nologo',
    '-p:DebugType=none',
    '-p:DebugSymbols=false',
    '-o', $portableDir
)

# ---------------------------------------------------------------------------
# 2. Installer via Inno Setup, wrapping the portable build.
# ---------------------------------------------------------------------------
$iscc = Resolve-Iscc
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

Write-Step 'Build summary'
$outputs = [ordered]@{
    'Portable'  = Join-Path $portableDir  $appExe
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

if (-not (Test-Path (Join-Path $portableDir $appExe))) {
    Write-Warning 'The portable build was not produced.'
    exit 1
}

if (Test-Path (Join-Path $installerDir $appExe)) {
    Write-Host 'Both artifacts built successfully.' -ForegroundColor Green
} else {
    Write-Host 'Portable build created; the installer was skipped.' -ForegroundColor Yellow
}
