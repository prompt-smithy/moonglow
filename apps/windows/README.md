<p align="center">
  <img src="../../resources/Moonglow-option-e-amber-noring.png" alt="Moonglow logo" width="160" height="160">
</p>

<h1 align="center">Moonglow</h1>

<p align="center">
  A lightweight Windows tray app for display brightness and color temperature.
</p>

Moonglow keeps display controls close at hand without adding another window to your desktop. Adjust internal and supported external monitor brightness, warm the whole desktop, or let a daily schedule transition automatically between day and night temperatures.

## Features

- Controls internal-panel brightness through WMI and supported external monitors through DDC/CI.
- Applies whole-desktop color temperatures from 1200 K to 6500 K.
- Smoothly transitions between configurable day and night temperatures.
- Shares settings between a compact tray popup and the full settings window.
- Follows Windows light/dark mode with a clean greyscale interface.
- Refreshes connected displays and can run automatically at sign-in.
- Runs as a single instance and removes its display tint on exit.

## Requirements

- Windows 7 SP1 or later (32-bit or 64-bit); Windows 10 or Windows 11 recommended
- The [.NET Framework 4.8 runtime](https://dotnet.microsoft.com/download/dotnet-framework/net48)
- A DDC/CI-capable external monitor for external brightness control

Windows 10 1903 and later and Windows 11 already include .NET Framework 4.8, so most systems need nothing extra. On older systems the app still works, but dark title-bar tinting and the widest color-temperature range require Windows 10 or Windows 11.

## Install

Choose the download that matches your system:

| Download | Type | System requirements |
| --- | --- | --- |
| [Moonglow Installer](https://github.com/prompt-smithy/moonglow/releases) | Installer | Windows 7 SP1 or later with the .NET Framework 4.8 runtime |
| [Moonglow Portable](https://github.com/prompt-smithy/moonglow/releases) | Portable | Windows 7 SP1 or later with the .NET Framework 4.8 runtime |

Use an installer for a standard Windows installation, or a portable build to run Moonglow without installing it.

Moonglow runs in the system tray. Open the tray icon for quick controls or use **Settings** for scheduling, time format, and startup options. The tray, window, executable, and installer use the same amber logo as the macOS app in every Windows light/dark mode.

## Limitations

On Windows 8 and later, Moonglow applies a fullscreen colour matrix through the compositor. A single display worker fades toward the latest target in small, paced steps. It keeps that same backend throughout the temperature range, avoiding the brief double tint caused by switching between gamma and composition mid-drag.

Windows 7 uses hardware gamma ramps. Driver support varies, and Windows can silently ignore warmer ramps even when `SetDeviceGammaRamp` reports success. Moonglow saves and restores the original ramps on this legacy path.

After waking from sleep while locked, Moonglow reapplies its colour transform because Windows may reset display state. The Windows password prompt and elevated consent prompts use a separate *secure desktop*, so the desktop tint is not guaranteed on those screens.

Moonglow's transform combines with other Windows colour adjustments, including **Night Light** and **HDR** tone mapping. For a predictable temperature, turn those off while Moonglow controls the screen.

## Build from source

Build on Windows with the [.NET SDK](https://dotnet.microsoft.com/download/dotnet). Visual Studio is optional; the .NET Framework 4.8 reference assemblies are restored from NuGet automatically.

```powershell
# Build the application in Release configuration
dotnet build Moonglow.csproj -c Release

# Run the shared spec conformance suite
dotnet test Moonglow.Core.Tests/Moonglow.Core.Tests.csproj -c Release
dotnet test Moonglow.Platform.Tests/Moonglow.Platform.Tests.csproj -c Release

# Build both distributables
dotnet build Moonglow.csproj -t:PackageAll
```

Packaging scratch files live in `bin/Package/<VERSION>` so a running copy from an older build
does not block a new version's cleanup.

The package command collects distributables under repository-root `release/<VERSION>/windows`:

- `Moonglow_Portable.exe` — one portable launcher with the application and dependencies embedded. It extracts them while running and cleans up on exit. Start-at-login registers the stable launcher.
- `Moonglow-windows-portable.zip` — the application and its dependencies, ready to extract and run.
- `Moonglow-Setup.exe` — the Inno Setup installer, when Inno Setup is available.
- `SHA256SUMS` — checksums for the distributables.

Both artifacts require the .NET Framework 4.8 runtime on the target PC; the portable launcher is a single-file package, not a self-contained .NET runtime.

Packaging the installer requires [Inno Setup 6](https://jrsoftware.org/isdl.php). The build script checks common install locations and attempts to install it with `winget` if needed. If Inno Setup is unavailable, the portable launcher is still produced and the installer is skipped with a warning. See [Architecture](docs/ARCHITECTURE.md) for implementation details.

### Microsoft Store

The Store package uses MSIX. Pass the identity name, publisher, and publisher display name exactly
as shown on the app's Product identity page in Partner Center:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ..\..\tools\package-windows-store.ps1 `
  -PackageIdentityName "<Partner Center identity name>" `
  -PackagePublisher "<Partner Center publisher>" `
  -PackagePublisherDisplayName "<Partner Center publisher display name>"
```

The script writes `Moonglow-Store.msix` and `Moonglow-Store.msixupload` to the versioned release
folder. The GitHub Actions workflow named **Windows Store MSIX** builds the package on Windows; a
manual run with the Partner Center identity values uploads both files as a downloadable artifact.
The package uses a temporary certificate whose subject matches the publisher. Microsoft signs the
Store-distributed package after certification. Local packaging requires the Windows SDK's MakeAppx
and SignTool utilities. The package targets Windows 10 version 1903 or later and Windows 11.

## Settings

Settings are stored per user at `%APPDATA%\Moonglow\settings.json`. On first launch the app
imports an existing `%APPDATA%\Amber\settings.json` left by earlier versions.

## License

Licensed under the [MIT License](LICENSE).

## Privacy

See the [Moonglow Windows privacy policy](../../PRIVACY.md) for information about local
settings, diagnostic logs, and support reports.
