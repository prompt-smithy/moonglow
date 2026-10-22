<p align="center">
  <img src="Assets/Moonglow.png" alt="Moonglow logo" width="160" height="160">
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
- Runs as a single instance and restores the previous display gamma LUT on exit.

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

Moonglow runs in the system tray. Open the tray icon for quick controls or use **Settings** for scheduling, time format, and startup options.

## Limitations

Moonglow adjusts desktop colour temperature using a hardware gamma LUT (a 256-entry lookup table for each RGB channel). Gamma-LUT support varies by display driver; on at least one machine `SetDeviceGammaRamp` reported success without a visible change, so the effect is not guaranteed. Moonglow saves the existing ramp before changing it and restores it on exit.

After waking from sleep while locked, Moonglow reapplies its gamma LUT because Windows may reset display state. The Windows password prompt and elevated consent prompts use a separate *secure desktop*, so tinting those screens remains driver-dependent. If the LUT does not appear there, configure Windows Night Light, which uses a different OS-level path.

## Build from source

Build on Windows with the [.NET SDK](https://dotnet.microsoft.com/download/dotnet). Visual Studio is optional; the .NET Framework 4.8 reference assemblies are restored from NuGet automatically.

```powershell
# Build the application in Release configuration
dotnet build Moonglow.csproj -c Release

# Build both distributables
dotnet build Moonglow.csproj -t:PackageAll
```

The package command produces:

- `bin\Moonglow_Portable.exe` — one portable launcher file with the application and dependencies embedded. It extracts them to a temporary directory while running, then cleans up when Moonglow exits.
- `bin\Installer\Moonglow.exe` — the Inno Setup installer.

Both artifacts require the .NET Framework 4.8 runtime on the target PC; the portable launcher is a single-file package, not a self-contained .NET runtime.

Packaging the installer requires [Inno Setup 6](https://jrsoftware.org/isdl.php). The build script checks common install locations and attempts to install it with `winget` if needed. If Inno Setup is unavailable, the portable launcher is still produced and the installer is skipped with a warning. See [Architecture](docs/ARCHITECTURE.md) for implementation details.

## Settings

Settings are stored per user at `%APPDATA%\Moonglow\settings.json`. On first launch the app
imports an existing `%APPDATA%\Amber\settings.json` left by earlier versions.

## License

Licensed under the [MIT License](LICENSE).
