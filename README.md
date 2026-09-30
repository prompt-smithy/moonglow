# Night Mode Controller

A macOS menu-bar app for external-display hardware brightness and colour temperature.

**Status: early Swift port.** The pure application logic is ported and tested, the settings window
and the menu-bar popup are in place, and the native display backends are implemented. See
[`docs/PORTING.md`](docs/PORTING.md).

## Layout

| Path | Purpose |
| --- | --- |
| `Package.swift` | SwiftPM manifest. Two targets: `AmberKit` and `Amber`. |
| `Sources/AmberKit/` | Platform logic: models, settings, schedule math, theme catalog, view model, service protocols. No UI framework dependency. |
| `Sources/Amber/` | macOS app shell: entry point, status item, popover, settings window, SwiftUI views, native bridges. |
| `Tests/AmberKitTests/` | Swift Testing suite for everything in `AmberKit`. |
| `Packaging/` | `Info.plist` and `build-app.sh` (app bundle, DMG, ZIP). |
| `Resources/` | App icon source (`Amber.png`) used to generate the app bundle icon. |
| `legacy-csharp/` | The original .NET 10 / Avalonia implementation, kept as a read-only reference for the port. |

## Requirements

- macOS 14 or later (deployment target).
- **Full Xcode** (27.0 verified). A Command Line Tools-only install is not sufficient on this
  machine — its SwiftPM, compiler, and SDK disagree with each other. See `docs/PORTING.md`.

## Commands

```bash
swift build                 # build both targets
swift test                  # run the AmberKit test suite (86 tests, 8 suites)
bash Packaging/build-app.sh # build artifacts/Night Mode Controller.app, plus a DMG and ZIP in release/
```

If every `swift` command fails with a `dyld` symbol error, an "SDK is not supported by the
compiler" error, or a Swift Testing `__SourceBounds` mismatch, the active developer directory
is broken. Check `xcode-select -p` and install or repair Xcode rather than working around it.

## Architecture

`AppDelegate` is the composition root and is deliberately hand-rolled — there is no DI
container, matching the original design.

- A single `MainViewModel` instance backs both the popup and the settings window, so slider
  and automatic-mode state stay in sync without a second settings path.
- Platform behaviour is behind `DisplayService`, `BrightnessService`, and
  `ColorTemperatureService`. `AmberKit` never imports SwiftUI; the app target supplies
  `ThemeApplying` and `LoginItemService`.
- View state uses the `Observation` framework (`@Observable`), which is why the deployment
  target is macOS 14.
- Colour temperature uses CoreGraphics gamma transfer tables. Hardware brightness uses
  DDC/CI over the **private** CoreDisplay `IOAVService*` API on Apple Silicon.

The private API is isolated in `Sources/AmberKit/Services/NativeBridge.swift`. Night Mode
Controller is distributed directly (Developer ID + notarization), not through the Mac App
Store, because of that dependency.

Startup is a login item registered through `SMAppService.mainApp`, which registers *the running
bundle*: the toggle in the settings window does nothing when Night Mode Controller is launched
straight from `.build` rather than from `Night Mode Controller.app`. The stored setting is only
the user's intent — the real registration lives in the system and can be changed outside the
app — so a change the system
refuses is not recorded, and a login item added by hand in System Settings is adopted rather than
switched off.

## Invariants

- Brightness stays in `1...100` percent; colour temperature in `900...6600` K, in 100 K steps.
- Defaults: manual brightness 80%, manual temperature 6500 K, day 6500 K, night 1200 K,
  automatic mode on, schedule 06:00–07:00 and 17:30–18:30.
- Time is always twelve-hour. Morning and evening are set with separate hour, minute, and AM/PM
  drop-downs, and the stored value is always `HH:mm`, so the display can never move a scheduled
  time.
- Day and night targets are edited as one two-handle bar, never as two independent values:
  the range keeps `night <= day` and snaps to the 100 K step.
- Controls are drawn from the palette, including drop-downs. Native menus and pop-up buttons
  are avoided because they paint their bezel and selected row with the system accent, which
  reads as system blue in every palette.
- A display capability is never reported unless the backend can actually apply it.
- ColourSync gamma state is restored on quit.
- Settings remain at `~/Library/Application Support/Amber/settings.json` for compatibility with existing installs.
