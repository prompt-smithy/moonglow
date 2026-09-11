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

- Windows 10 or Windows 11
- A DDC/CI-capable external monitor for external brightness control
- The .NET 10 Desktop Runtime when using a framework-dependent build

## Install

Download an installer or portable build from [GitHub Releases](../../releases). Self-contained builds do not require a separate .NET installation.

Amber runs in the system tray. Open the tray icon for quick controls or use **Settings** for scheduling, themes, time format, and startup options.

## Build from source

Install the [.NET 10 SDK](https://dotnet.microsoft.com/download/dotnet/10.0), then run:

```powershell
dotnet build Amber.csproj
```

To create portable and installer distributions:

```powershell
dotnet build Amber.csproj -t:PackageAll
```

Packaging installers requires [Inno Setup 6](https://jrsoftware.org/isdl.php). See [Architecture](docs/ARCHITECTURE.md) for implementation details.

## Settings

Settings are stored per user at `%APPDATA%\Amber\settings.json`.

## License

Licensed under the [MIT License](LICENSE).