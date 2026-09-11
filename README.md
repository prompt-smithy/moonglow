# Amber

A lightweight Windows system-tray app (WPF, .NET 10) for controlling **display brightness** and **screen color temperature**, with an automatic time-of-day warm/cool schedule.

- **Assembly:** `Amber` &nbsp;•&nbsp; **Product:** Amber
- **Platform:** Windows only (`net10.0-windows`, WPF)
- **Runs as:** a tray icon with a quick-control popup and a full settings window

---

## Features

- **Brightness control** for every controllable display:
  - Internal laptop panels via **WMI** (`WmiMonitorBrightness`).
  - External monitors via **DDC/CI** (`dxva2`), when the monitor supports it.
  - A safety floor keeps the screen from going fully black.
- **Color temperature** (warm ↔ cool) applied to the whole desktop:
  - Preferred: **Magnification full-screen color effect** (reaches very warm tones).
  - Fallback: **GPU gamma ramp**.
- **Automatic schedule** – gradually shifts color temperature between a day (cool) and night (warm) value across configurable **morning** and **evening** transition windows.
- **Themed UI** – six light and six dark palettes that **follow the Windows light/dark mode** automatically.
- **Time pickers** – hour / minute / AM–PM selectors (12- or 24-hour), scroll a field to change it.
- **Run at sign-in** toggle (per-user startup registration).
- **Single instance** – a second launch simply focuses the existing one.

---

## Requirements

- Windows 10/11
- [.NET 10 SDK](https://dotnet.microsoft.com/) (to build) or the .NET 10 Desktop Runtime (to run a published build)

---

## Build & run

```powershell
# from the repository root
dotnet build                     # Debug build
dotnet run                       # build + launch

dotnet build -c Release          # optimized build (recommended for daily use)
```

The built executable is at:

```
bin\Debug\net10.0-windows\Amber.exe
bin\Release\net10.0-windows\Amber.exe
```

> **Tip:** run the **Release** build for daily use — it uses roughly half the memory of the Debug build. If you enable *Run at sign-in* while running the Debug `.exe`, that Debug path is what gets registered; re-toggle it from the Release build to point startup at the leaner binary.

---

## Package for distribution

A single command builds **all four** distributable `Amber.exe` files — a portable executable and an installer, each in a self-contained (no .NET required) and a framework-dependent (needs the .NET runtime) flavor.

```powershell
# from the repository root
dotnet build Amber.csproj -t:PackageAll
```

> The explicit project name is required while the folder contains generated `*_wpftmp.csproj` temp files. You can also run the script directly: `.\build.ps1`. Override the target runtime with `-p:PackageRuntime=win-arm64` (or pass `-Runtime win-arm64` to `build.ps1`).

**Requirements:** the .NET 10 SDK, plus [Inno Setup 6](https://jrsoftware.org/isdl.php) for the installers. If Inno Setup isn't installed, the script attempts to install it via `winget`; if that fails, the two portable builds still complete and the installer steps are skipped with a warning.

All four outputs share the same file name, each in its own folder under `bin\`:

| Artifact | Folder | .NET runtime |
| --- | --- | --- |
| Portable executable | `bin\Portable-SelfContained\Amber.exe` | bundled (none needed) |
| Portable executable | `bin\Portable-FrameworkDependent\Amber.exe` | required |
| Installer | `bin\Installer-SelfContained\Amber.exe` | bundled (none needed) |
| Installer | `bin\Installer-FrameworkDependent\Amber.exe` | required |

---

## Usage

- **Left-click the tray icon** to open the quick-control popup: brightness slider, color-temperature slider, an *Automatic (time based)* toggle, and a **Settings** button (which opens the full window and dismisses the popup).
- **Right-click the tray icon** for a menu with *Settings…* and *Exit*.
- **Settings window** lets you configure:
  - Light-mode and dark-mode theme.
  - Detected displays (with a *Refresh displays* button).
  - Automatic color temperature: day/night targets, morning/evening transition times, and 12/24-hour clock format.
  - *Run Amber when I sign in*.
- Click anywhere on a slider track to jump straight to that value.

### Settings storage

User settings are saved as JSON at:

```
%APPDATA%\Amber\settings.json
```

Times are stored canonically as 24-hour `HH:mm`. Startup registration uses the per-user
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` key (value name `Amber`).

---

## Project structure

```
App.xaml / App.xaml.cs      Application entry: composition, tray icon, single-instance guard, system-event hooks
app.manifest                Application manifest

Infrastructure/             Cross-cutting helpers
  ObservableObject.cs         INotifyPropertyChanged base
  RelayCommand.cs             ICommand implementation
  StartupManager.cs           Run-at-sign-in registry entry
  IconFactory.cs              Tray icon bitmap

Models/                     Plain data
  AppSettings.cs              Persisted user configuration
  AppTheme.cs                 A named color palette
  MonitorInfo.cs              A physical display and its capabilities

Native/
  NativeMethods.cs            All P/Invoke: monitor enumeration, DDC/CI, gamma, Magnification, DWM, window styling

Services/                   Application logic
  BrightnessService.cs        WMI + DDC/CI brightness (async, coalesced)
  ColorTemperatureService.cs  Magnification / gamma color temperature
  MonitorService.cs           Enumerates displays and holds DDC handles
  ScheduleService.cs          Time-of-day color-temperature schedule
  SettingsService.cs          Load/save settings JSON
  ThemeService.cs             Applies themes, follows Windows light/dark mode
  ThemeCatalog.cs             The built-in palettes

ViewModels/
  MainViewModel.cs            Shared view model for the popup and settings window

Views/
  MainWindow.xaml(.cs)        Full settings window
  TrayPopup.xaml(.cs)         Quick-control tray popup
  TimePicker.xaml(.cs)        Reusable hour/minute/AM–PM control
```

For the technical details of how each mechanism works, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

---

## Notes & limitations

- **DDC/CI** must be supported and enabled on an external monitor for brightness control to work there; some monitors and connections (e.g. certain KVMs) don't expose it.
- **Internal panels** rely on the OEM's WMI brightness support.
- The color-temperature effect is applied to the **entire desktop**, not per-monitor, when using the Magnification path.
- Windows-only by design (uses WMI, DDC/CI, GDI gamma, and the Magnification API).
