# Night Mode Controller

A macOS menu-bar app for external-display hardware brightness and scheduled colour temperature.

The app includes a neutral, macOS-style menu-bar popup and a settings window for the temperature
schedule, startup behavior, displays, and appearance. See [`docs/PORTING.md`](docs/PORTING.md)
for implementation status and platform details.

## Download

[Download the macOS installer (Apple Silicon)](https://github.com/prompt-smithy/Night_Mode_Controller-MacOS/releases/download/v2/Night-Mode-Controller-macOS-arm64.dmg.zip)

The current download is for Apple Silicon and requires macOS 14 or later. The packaging script
builds a universal app for macOS 12 or later. This test build is ad-hoc signed and not
notarized; on first launch, Control-click the app and choose **Open** if macOS blocks it.

## Layout

| Path | Purpose |
| --- | --- |
| `Package.swift` | SwiftPM manifest. Two targets: `AmberKit` and `Amber`. |
| `Sources/AmberKit/` | Platform logic: models, settings, schedule math, theme catalog, view model, service protocols. No UI framework dependency. |
| `Sources/Amber/` | macOS app shell: entry point, status item, popover, settings window, SwiftUI views, native bridges. |
| `Tests/AmberKitTests/` | Swift Testing suite for everything in `AmberKit`. |
| `Packaging/` | `Info.plist` and `build-app.sh` (app bundle, DMG, ZIP). |
| `Resources/` | App icon source (`Amber.png`) used to generate the app bundle icon. |

## Requirements

- macOS 12 or later (deployment target).
- **Full Xcode** (27.0 verified). A Command Line Tools-only install is not sufficient on this
  machine — its SwiftPM, compiler, and SDK disagree with each other. See `docs/PORTING.md`.

## Commands

```bash
swift build                  # build the app and AmberKit
swift test                   # run the complete test suite
swift run                    # run from the SwiftPM build (login-item registration requires a packaged app)
bash Packaging/build-app.sh  # build the app bundle, DMG, and ZIP
```

The packaging script creates:

- `artifacts/Night Mode Controller.app`
- `release/Night-Mode-Controller-macOS-universal.dmg`
- `release/Night-Mode-Controller-macOS-universal.zip`

It builds a universal (`arm64` and `x86_64`) app for macOS 12 or later. On Intel, the app probes
the primary external display for DDC/CI brightness and falls back to software gamma dimming when
DDC/CI is unavailable. Software dimming changes image output, not the display's backlight, and
requires CoreGraphics gamma support. Login-item management requires macOS 13 or later. The app
bundle is ad-hoc signed for local testing; it is not
Developer ID signed or notarized. Users may need to Control-click the app and choose **Open** on
first launch.

To preserve the normal outputs when making a separate verification build, set alternate output
directories:

```bash
AMBER_ARTIFACTS_DIR="$PWD/artifacts/test-build" \
AMBER_RELEASE_DIR="$PWD/release/test-build" \
bash Packaging/build-app.sh
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
- View state uses Combine's `ObservableObject` and `@Published`, keeping the deployment target at
  macOS 12 while supporting both Intel and Apple Silicon.
- Colour temperature and software brightness share CoreGraphics gamma transfer tables. Hardware
  brightness uses the private CoreDisplay `IOAVService*` API on Apple Silicon and IOKit framebuffer
  I2C on Intel, with gamma dimming as the fallback when DDC/CI is unavailable.

The private API is isolated in `Sources/AmberKit/Services/NativeBridge.swift`. The app is
distributed directly rather than through the Mac App Store because of that dependency. The current
packaging script applies only an ad-hoc signature; public distribution still requires Developer ID
signing and notarization.

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
  automatic mode on, schedule 06:00–07:00 and 17:30–18:30, Light Green and Dark Green palettes.
- Existing settings that contain the original Light Orange/Dark Orange default pair migrate to
  Light Green/Dark Green. Other saved palette selections are preserved.
- Time is always twelve-hour. Morning and evening are set with separate hour, minute, and AM/PM
  drop-downs, and the stored value is always `HH:mm`, so the display can never move a scheduled
  time.
- Day and night targets are edited as one two-handle bar, never as two independent values:
  the range keeps `night <= day` and snaps to the 100 K step.
- The popup is a native AppKit menu with system styling and no app palette applied. Its brightness
  and temperature sliders use standard system controls; the Settings window continues to use the
  selected light and dark palettes.
- Displays and Appearance are always visible in the settings window; schedule controls are followed
  by Startup, Displays, and Appearance.
- A display capability is never reported unless the backend can actually apply it.
- ColourSync gamma state is restored on quit.
- Settings remain at `~/Library/Application Support/Amber/settings.json` for compatibility with existing installs.
