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
6. Connect tray commands and create the settings window.
7. Apply persisted state and start the scheduler.

The window intercepts close, hides itself, and remains reusable. Explicit exit stops the timer and restores display color state.

## Layers

### Models

- `AppSettings` defines persisted values and parsed schedule times.
- `AppTheme` defines one named palette.
- `DisplayInfo` exposes native ID, label, primary status, and control capabilities.

### View model

`MainViewModel` owns UI state, commands, persistence triggers, range clamping, display refresh, and schedule notifications. It depends on interfaces for platform operations.

### Services

- `SettingsService`: JSON load/save, legacy import, value normalization, and theme-name migration.
- `ThemeService`: applies selected palette brushes and responds to system appearance changes.
- `ScheduleService`: platform-neutral schedule calculation and a 10-second timer.
- `MacDisplayService`: active-display enumeration and gamma capability probing.
- `MacColorTemperatureService`: Kelvin conversion, gamma-table construction, apply, and reset.
- `UnsupportedBrightnessService`: explicit placeholder until a validated macOS backend exists.

### Native boundary

`MacNative` contains all CoreGraphics imports:

| Function | Purpose |
| --- | --- |
| `CGGetActiveDisplayList` | Enumerate active display IDs |
| `CGMainDisplayID` | Identify the primary display |
| `CGDisplayGammaTableCapacity` | Detect gamma-table capability and size |
| `CGSetDisplayTransferByTable` | Apply RGB transfer tables |
| `CGDisplayRestoreColorSyncSettings` | Restore system color state |

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