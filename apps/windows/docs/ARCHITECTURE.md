# Architecture

Technical reference for **Moonglow** (`Moonglow`). For a user-facing overview see the [README](../README.md).

## Contents
- [Tech stack](#tech-stack)
- [Application lifecycle & composition](#application-lifecycle--composition)
- [MVVM structure](#mvvm-structure)
- [Brightness pipeline](#brightness-pipeline)
- [Color-temperature pipeline](#color-temperature-pipeline)
- [Scheduling](#scheduling)
- [Monitor enumeration & DDC handles](#monitor-enumeration--ddc-handles)
- [Theming](#theming)
- [Settings & startup](#settings--startup)
- [UI controls & the slider-binding pattern](#ui-controls--the-slider-binding-pattern)
- [Project layout & conformance tests](#project-layout--conformance-tests)
- [Native interop reference](#native-interop-reference)

---

## Tech stack

- **.NET Framework 4.8**, **WPF** (`net48`, `OutputType=WinExe`). Targeting the in-box framework
  keeps Moonglow runnable on older Windows versions: 4.8 is included with Windows 10 1903+ and
  Windows 11, and can be installed on Windows 7 SP1 through 8.1.
- **`Moonglow.Core`** – a `net48` class library holding the pure logic (settings, schedule,
  Kelvin conversion, temperature range). The WPF app references it, and the separate
  `Moonglow.Core.Tests` xUnit project tests it against the shared vectors in `spec/vectors`.
- **[Hardcodet.NotifyIcon.Wpf]** – system-tray icon, popup, and context menu.
- **System.Management** – WMI access for internal-panel brightness (a framework assembly on
  `net48` rather than a NuGet package).
- **System.Text.Json** – settings persistence, and the only NuGet runtime dependency besides the
  tray icon.
- No MVVM framework or DI container; composition is done by hand in `App`.

.NET Framework 4.8 implements .NET Standard 2.0, so two small shims stand in for things it
lacks: `MathUtil.Clamp` in `Moonglow.Core` (instead of `Math.Clamp`, which needs .NET Standard
2.1) and `IsExternalInit` in `Infrastructure` (which lets the models keep C# 9 `init`-only
setters).

[Hardcodet.NotifyIcon.Wpf]: https://github.com/hardcodet/wpf-notifyicon

---

## Application lifecycle & composition

`App.OnStartup` ([App.xaml.cs](../App.xaml.cs)) is the composition root:

1. **Single-instance guard** – a named `Mutex` (`Local\Moonglow.SingleInstance`); if it already exists, the second process shuts down immediately.
2. `ShutdownMode = OnExplicitShutdown` (the app has no main window; it lives in the tray).
3. Services are constructed in dependency order: `SettingsService` → `ThemeService` → `MonitorService` → `BrightnessService` → `ColorTemperatureService` → `ScheduleService` → `MainViewModel`.
4. A `TaskbarIcon` is created with the view model as `DataContext`, a `TrayPopup`, and a context menu.
5. `MainViewModel.ApplyInitialState()` applies the saved brightness and (manual) color temperature; `ScheduleService.Start()` begins the schedule.
6. System events are hooked: `SystemEvents.DisplaySettingsChanged` (re-enumerate monitors + re-tick the schedule) and `SystemEvents.UserPreferenceChanged` (re-apply the theme when Windows light/dark mode changes).

`OnExit` unhooks system events, resets the color effect, disposes the monitor handles, the tray icon, and the mutex.

---

## MVVM structure

- A single [`MainViewModel`](../ViewModels/MainViewModel.cs) backs **both** the tray popup and the settings window (same instance set as `DataContext` on each).
- [`ObservableObject`](../Infrastructure/ObservableObject.cs) provides `INotifyPropertyChanged` with a `SetProperty` helper; [`RelayCommand`](../Infrastructure/RelayCommand.cs) provides `ICommand`.
- The view model exposes brightness, color temperature, auto-mode, day/night targets, transition times, clock format, theme selections, and run-at-startup — each property persists to settings on change and triggers the relevant service.
- Navigation events (`OpenSettingsRequested`, `ExitRequested`) are raised by the view model and handled in `App`, which owns the windows and tray.

---

## Brightness pipeline

Implemented in [`BrightnessService`](../Services/BrightnessService.cs).

- **Internal panel** – WMI: query `WmiMonitorBrightnessMethods` and invoke `WmiSetBrightness(timeout, level)`; availability is detected at construction via `WmiMonitorBrightness`.
- **External monitors** – DDC/CI: `SetMonitorBrightness` on each display that reported brightness support during enumeration, scaled between the monitor's reported min/max.
- **Minimum floor** – values are clamped to `MinBrightnessPercent` (1%) so the panel never goes fully black.
- **Async + coalesced application** – `SetBrightness` records a target and starts a **single background worker** (`Task.Run`) that applies the *latest* requested value in a loop until it stops changing. This keeps slow WMI/DDC calls off the UI thread and collapses bursts from dragging the slider into "latest wins."
- **Thread safety** – the worker reaches DDC handles through `MonitorService.ForEachDdcMonitor(...)`, which holds the same lock used by `Refresh`/`Dispose`, so handles can't be destroyed mid-use.
- **Probe recovery** – a panel that is asleep or waking can fail its first DDC probe. Support is re-probed on popup open, after display changes, and every 20 seconds while unsupported, all on a worker thread. A brightness worker that has been stuck in a DDC transaction for 5 seconds is replaced by a fresh one, so a hung driver call cannot lock out later adjustments.

> Note: a laptop's internal panel is often also enumerated as a DDC monitor, so both the WMI and DDC paths may target it — they set the same value, so the result is consistent.

---

## Color-temperature pipeline

Implemented in [`ColorTemperatureService`](../Services/ColorTemperatureService.cs).

- Kelvin is converted to per-channel RGB scale factors by `KelvinConversion.RgbScale` in `Moonglow.Core`, using the **Tanner Helland** approximation normalized so ~6500 K is neutral.
- **GPU gamma LUT** (`SetDeviceGammaRamp`): Kelvin becomes per-channel RGB scale factors, then a 256-entry ramp is built for each channel and applied per monitor (falling back to the whole-desktop DC if per-monitor DCs are unavailable). The original ramps are captured and restored on exit and around unlock. Device contexts are cached for the lifetime of the display set.
- **Writes run on one dedicated background thread with latest-value-wins.** Slider events only record the target; dragging issues one driver write per settled value instead of one per pixel, and a slow driver can never block the UI thread. One thread (not pool workers) also keeps every Magnification call on the thread that initialized it, which the fullscreen effect requires.
- **Composition fallback.** When the driver exposes no GDI gamma ramp — Windows Night Light and HDR both take ownership of the colour pipeline — the same scale factors are applied through `MagSetFullscreenColorEffect`. The effect is cleared as soon as a gamma write succeeds, so the two transforms never stack, and identity/`MagUninitialize` runs on exit. Exact temperatures still require Night Light/HDR to be off; the fallback keeps the schedule working rather than silently doing nothing.
- **Driver support** – LUT application is driver-dependent. On one local machine, `SetDeviceGammaRamp` returned `true` without a visible change. The same LUT path is used on the desktop and after waking into a locked session; whether it affects the secure sign-in desktop depends on the display driver.
- `Reset()` restores the captured original ramps rather than writing an identity ramp, avoiding overwriting another color calibration.
- `App` re-applies on `SystemEvents.PowerModeChanged` (`Resume`), because resuming can reset display state. While locked, it retries for up to five seconds; on unlock it restores the original ramps and reapplies the selected temperature.

---

## Scheduling

Implemented in [`ScheduleService`](../Services/ScheduleService.cs).

- A `DispatcherTimer` ticks every **10 seconds** (plus explicit ticks on start, mode change, and display change).
- The pure evaluation lives in `ScheduleCalculator` (`Moonglow.Core/ScheduleCalculator.cs`), so the golden vectors can exercise it without a timer.
- The daily cycle is: **night (warm)** → *morning transition (cools up to day)* → **day (cool)** → *evening transition (warms down to night)* → **night (warm)**.
- Within a transition, the target is linearly interpolated between the day and night values and **snapped to 100 K steps**.
- Ticks are a no-op unless *Automatic* mode is enabled. When enabled, each applicable tick applies the computed temperature and raises `TemperatureApplied`, which updates the view model's displayed value.

---

## Monitor enumeration & DDC handles

Implemented in [`MonitorService`](../Services/MonitorService.cs).

- `Refresh()` enumerates displays with `EnumDisplayMonitors`; for each it resolves the physical monitor(s) via `GetPhysicalMonitorsFromHMONITOR` and probes `GetMonitorBrightness` to record DDC brightness support and min/max range.
- Physical-monitor handles are retained for the lifetime of the enumeration and released with `DestroyPhysicalMonitors` on the next `Refresh()` or on `Dispose()`.
- All handle mutation and use is guarded by a lock (`Refresh`, `Dispose`, and `ForEachDdcMonitor`) so the background brightness worker is safe against concurrent re-enumeration.
- `Refresh()` runs at startup (from the view model), on `DisplaySettingsChanged`, and from the *Refresh displays* button.

---

## Theming

- The app has exactly **two greyscale modes**: light and dark. There is no user-selectable palette catalog — controls use black, white, and shades of grey, while the color-temperature controls keep their warm-to-cool gradient.
- [`ThemeService`](../Services/ThemeService.cs) writes the active palette into **`Application.Current.Resources`** as `SolidColorBrush` values keyed `ThemeWindowBackground`, `ThemePrimaryText`, etc. UI uses `DynamicResource` against those keys, so a mode switch restyles everything live.
- Because the brushes live at **application scope**, control styles and `UserControl`s (e.g. `TimePicker`) resolve them too. Shared control styles (buttons, checkboxes, radios, sliders, scrollbars, **combo boxes**) are defined in [App.xaml](../App.xaml); window-specific styles live in the window.
- The app **follows Windows light/dark mode**: `IsWindowsLight()` reads the `Personalize` registry value, and `UserPreferenceChanged` re-applies the matching mode. The OS title bar is tinted to match via `DwmSetWindowAttribute`.
- The **logo is monochrome**, so it cannot be one baked bitmap: [`IconFactory`](../Infrastructure/IconFactory.cs) draws the crescent at runtime in black for the light theme and white for the dark one, and `OnThemeChanged` redraws the tray and window icons when the mode flips. Geometry mirrors `tools/generate-logo-assets.ps1`, which produces the packaged `.ico`.

---

## Settings & startup

- [`SettingsService`](../Services/SettingsService.cs) serializes [`AppSettings`](../Moonglow.Core/AppSettings.cs) to `%APPDATA%\Moonglow\settings.json` (indented JSON). Load and save are best-effort and swallow I/O errors.
- **Only malformed JSON** is moved aside as `settings.json.corrupt`; a transient read failure (antivirus, indexer, concurrent reader) leaves the file untouched so the next launch can read it, and a parseable `.corrupt` copy is used when the canonical file is missing. Saves go through a temporary file and `File.Replace`, so a reader never sees a half-written document.
- Times are stored as 24-hour `HH:mm`; `AppSettings` exposes parsed `TimeSpan` helpers for the scheduler.
- [`StartupManager`](../Infrastructure/StartupManager.cs) adds/removes the current executable path under `HKCU\...\CurrentVersion\Run` (value `Moonglow`).

---

## UI controls & the slider-binding pattern

**Important pattern.** A WPF `Slider` bound `TwoWay` to an auto-saving property, when hosted in a lazily-rendered container (the tray popup) or a settings window, can fire a spurious `ValueChanged` during startup/layout that **writes a stale value back to the source** — silently overwriting a saved setting.

All value sliders (brightness, and the three color-temperature sliders) therefore use:

- `Value="{Binding …, Mode=OneWay}"` — display only; the binding never writes back.
- A code-behind `ValueChanged` handler that forwards to the view model **only** when the change is genuine — guarded by `slider.IsLoaded` **and** `round(NewValue) != currentValue` (which also ignores the one-way echo).
- `IsMoveToPointEnabled="True"` for click-to-position.

Do **not** replace this with a mouse-event flag (`PreviewMouseDown`/`LostMouseCapture`): with `IsMoveToPointEnabled` the capture sequence can reset the flag before `ValueChanged`, dropping the click.

[`TimePicker`](../Views/TimePicker.xaml.cs) is a reusable `UserControl` with `Time` (`"HH:mm"`, two-way) and `Is24Hour` dependency properties. It renders hour/minute/AM–PM combo boxes, converts to/from the canonical string, hides AM/PM in 24-hour mode, and steps a field on mouse-wheel over a closed box.

---

## Project layout & conformance tests

```
Moonglow.csproj            WPF app: views, view models, native services, tray integration
Moonglow.Core/             net48 class library: AppSettings, ScheduleCalculator,
                           KelvinConversion, TemperatureRange, MathUtil
Moonglow.Core.Tests/       xUnit conformance suite that reads ../../../spec/vectors
tools/PortableLauncher/    single-file launcher used by the portable build
```

The pure types are the contract every platform shares, so they live outside the WPF assembly.
`Moonglow.Core.Tests` copies `spec/vectors/*.json` to its output and asserts the Kelvin curve
(`1e-6` tolerance), exact schedule results, snapping, and range ordering. Run it with:

```powershell
dotnet test Moonglow.Core.Tests/Moonglow.Core.Tests.csproj -c Release
```

The WPF project removes `Moonglow.Core/**` and `Moonglow.Core.Tests/**` from its default
compile glob; without that the SDK would compile the other projects' sources into the app.

---

## Native interop reference

All P/Invoke lives in [`NativeMethods`](../Native/NativeMethods.cs), grouped by concern:

| Area | Library | Key entry points |
| --- | --- | --- |
| Monitor enumeration | `user32` | `EnumDisplayMonitors`, `GetMonitorInfo` |
| DDC/CI brightness | `dxva2` | `GetNumberOfPhysicalMonitorsFromHMONITOR`, `GetPhysicalMonitorsFromHMONITOR`, `GetMonitorBrightness`, `SetMonitorBrightness`, `DestroyPhysicalMonitors` |
| Gamma control | `gdi32` | `CreateDC`, `GetDeviceGammaRamp`, `SetDeviceGammaRamp`, `DeleteDC` |
| Colour fallback | `Magnification` | `MagInitialize`, `MagSetFullscreenColorEffect`, `MagUninitialize` |
| Title-bar theming / styling | `dwmapi`, `user32` | `DwmSetWindowAttribute`, `GetWindowLong`/`SetWindowLong`, `SetWindowPos` |
| Internal-panel brightness | WMI (`System.Management`) | `WmiMonitorBrightness`, `WmiMonitorBrightnessMethods.WmiSetBrightness` |
