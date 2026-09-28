<p align="center">
  <img src="Assets/Amber.png" alt="Amber logo" width="160" height="160">
</p>

<h1 align="center">Amber</h1>

<p align="center">
  A lightweight Windows tray app for display brightness and color temperature.
</p>

Amber keeps display controls close at hand without adding another window to your desktop. Adjust internal and supported external monitor brightness, warm the whole desktop, or let a daily schedule transition automatically between day and night temperatures.

## Features

- Controls internal-panel brightness through WMI and supported external monitors through DDC/CI.
- Applies whole-desktop color temperatures from 1200 K to 6500 K.
- Smoothly transitions between configurable day and night temperatures.
- Shares settings between a compact tray popup and the full settings window.
- Follows Windows light/dark mode and includes selectable themes.
- Supports 12-hour and 24-hour schedule entry.
- Refreshes connected displays and can run automatically at sign-in.
- Runs as a single instance and restores neutral color on exit.

## Requirements

- Windows 7 SP1 or later (32-bit or 64-bit); Windows 10 or Windows 11 recommended
- The [.NET Framework 4.8 runtime](https://dotnet.microsoft.com/download/dotnet-framework/net48)
- A DDC/CI-capable external monitor for external brightness control

Windows 10 1903 and later and Windows 11 already include .NET Framework 4.8, so most systems need nothing extra. On older systems the app still works, but dark title-bar tinting and the widest color-temperature range require Windows 10 or Windows 11.

## Install

Choose the download that matches your system:

| Download | Type | System requirements |
| --- | --- | --- |
| [Amber Installer](https://github.com/prompt-smithy/Amber/releases/download/v1/Amber_Installer.exe) | Installer | Windows 7 SP1 or later with the .NET Framework 4.8 runtime |
| [Amber Portable](https://github.com/prompt-smithy/Amber/releases/download/v1/Amber_Portable.exe) | Portable | Windows 7 SP1 or later with the .NET Framework 4.8 runtime |

Use an installer for a standard Windows installation, or a portable build to run Amber without installing it.

Amber runs in the system tray. Open the tray icon for quick controls or use **Settings** for scheduling, themes, time format, and startup options.

## Limitations

Amber cannot apply color-temperature settings to the Windows lock screen. If this is inconvenient, configure Windows Night Light separately to provide a color-temperature effect on the lock screen. You can still use Amber for brightness control and quick color-temperature adjustments on the regular desktop alongside Windows Night Light.

## Build from source

Install the [.NET SDK](https://dotnet.microsoft.com/download/dotnet) (or Visual Studio 2022), then run:

```powershell
dotnet build Amber.csproj
```

The project targets .NET Framework 4.8. Its reference assemblies come from NuGet, so no separate Developer Pack is needed.

To create the portable build and the installer:

```powershell
dotnet build Amber.csproj -t:PackageAll
```

Packaging the installer requires [Inno Setup 6](https://jrsoftware.org/isdl.php). See [Architecture](docs/ARCHITECTURE.md) for implementation details.

## Settings

Settings are stored per user at `%APPDATA%\Amber\settings.json`.

## License

Licensed under the [MIT License](LICENSE).