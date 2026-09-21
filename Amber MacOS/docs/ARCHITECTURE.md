# Architecture

## Stack

- .NET 10
- Avalonia UI 12
- CommunityToolkit.Mvvm
- CoreGraphics through C# P/Invoke
- xUnit for unit tests

Amber uses a small hand-composed MVVM architecture without a dependency-injection container.

## Composition and lifecycle

`App.OnFrameworkInitializationCompleted` is the composition root:

1. Set explicit shutdown so the app remains in the menu bar.
2. Load and normalize settings.
3. Apply the palette matching the current system appearance.
4. Select macOS or unsupported platform services.
5. Construct the scheduler and shared main view model.
6. Connect tray commands and create the quick-controls and settings windows.
7. Apply persisted state and start the scheduler.

Both windows intercept close, hide themselves, and remain reusable. The quick-controls window also hides when it loses focus. Explicit exit closes both windows, stops the timer, and restores display color state.

## Layers

### Models

- `AppSettings` defines persisted values and parsed schedule times.
- `AppTheme` defines one named palette.
- `DisplayInfo` exposes native ID, label, primary status, and control capabilities.

### View model

`MainViewModel` owns UI state, commands, persistence triggers, range clamping, display refresh, and schedule notifications. It depends on interfaces for platform operations.

The tray quick-controls window and main settings window share the same `MainViewModel`, so slider and automatic-mode state stay synchronized without a second settings path.

### Services

- `SettingsService`: JSON load/save, legacy import, value normalization, and theme-name migration.
- `ThemeService`: applies selected palette brushes and responds to system appearance changes.
- `ScheduleService`: platform-neutral schedule calculation and a 10-second timer.
- `MacDisplayService`: active-display enumeration and gamma capability probing.
- `MacDdcBrightnessService`: Apple Silicon DDC/CI capability probe, debounced MCCS luminance writes, and Core Foundation lifetime management.
- `MacColorTemperatureService`: Kelvin scaling, gamma-table construction, apply, and reset.
- `UnsupportedBrightnessService`: explicit fallback outside the supported macOS runtime.

### Native boundary

`MacNative` contains the CoreGraphics imports and the isolated private CoreDisplay DDC boundary:

| Function | Purpose |
| --- | --- |
| `CGGetActiveDisplayList` | Enumerate active display IDs |
| `CGMainDisplayID` | Identify the primary display |
| `CGDisplayGammaTableCapacity` | Detect gamma-table capability and size |
| `CGSetDisplayTransferByTable` | Apply RGB transfer tables |
| `CGDisplayRestoreColorSyncSettings` | Restore system color state |
| `IOAVServiceCreate` | Open the default Apple Silicon display service |
| `IOAVServiceReadI2C` | Probe DDC/CI luminance support |
| `IOAVServiceWriteI2C` | Send DDC/CI luminance queries and writes |
| `CFRelease` | Release the CoreDisplay service |

The `IOAVService` functions are private macOS APIs. Amber limits them to `MacDdcBrightnessService`; availability and physical DDC communication are checked before the UI reports support.

Non-macOS execution receives unsupported service implementations and does not load CoreGraphics.

## Threading

`ScheduleService` uses `System.Threading.Timer`. Native color application runs on its callback. Successful temperatures are posted to Avalonia's UI dispatcher before changing the displayed value.

Settings writes are synchronous, small, best-effort JSON writes initiated by user changes.

## Themes and resources

`ThemeCatalog` contains six ordered light and six ordered dark palettes. `ThemeService` writes brushes into application resources. Views use dynamic resources, so palette changes update existing controls without window recreation.

App-level resources contain Light Amber fallback values to avoid an unrelated color flash before settings load.

## Packaging

The packaging script publishes a self-contained single-file `osx-arm64` apphost, constructs `Amber.app`, creates `Amber.icns` from the committed 1024px PNG, applies an ad-hoc signature, creates a drag-to-Applications DMG, and creates a ZIP.

The complete app bundle must be built and validated locally on macOS.

## Test boundaries

Unit tests cover schedule states and interpolation, Kelvin conversion, settings migration and normalization, theme ordering, and required Amber colors. CoreGraphics calls, tray behavior, app focus, `.icns`, codesign, and real display behavior require macOS integration testing.