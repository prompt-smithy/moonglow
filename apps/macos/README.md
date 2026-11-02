# Moonglow

A macOS menu-bar app for external-display hardware brightness and scheduled colour temperature.

The app includes a native menu-bar popup and a settings window for the temperature schedule,
startup behavior, and displays. Its light or dark appearance follows macOS automatically. See [`docs/PORTING.md`](docs/PORTING.md)
for implementation status and platform details.

## Download

[Download from GitHub Releases](https://github.com/prompt-smithy/Night_Mode_Controller-MacOS/releases).

The packaging script produces `Moonglow-macOS-universal.dmg` and `Moonglow-macOS-universal.zip`:
a universal (`arm64` and `x86_64`) build for macOS 12 or later. Published test builds are
ad-hoc signed and not notarized; on first launch, Control-click the app and choose **Open** if
macOS blocks it.

## Layout

| Path | Purpose |
| --- | --- |
| `Package.swift` | SwiftPM manifest. Two targets: `MoonglowKit` and `Moonglow`. |
| `Sources/MoonglowKit/` | Platform logic: models, settings, schedule math, light/dark appearance colors, view model, service protocols. No UI framework dependency. |
| `Sources/Moonglow/` | macOS app shell: entry point, status item, popover, settings window, SwiftUI views, native bridges. |
| `Tests/MoonglowKitTests/` | Swift Testing suite for everything in `MoonglowKit`. |
| `Packaging/` | `Info.plist` and `build-app.sh` (app bundle, DMG, ZIP). |

The app icon is generated from the 1024 px render
[`resources/Moonglow-option-e-amber-noring.png`](../../resources/Moonglow-option-e-amber-noring.png)
of the SVG logo master
[`resources/Moonglow-option-e-amber-noring.svg`](../../resources/Moonglow-option-e-amber-noring.svg)
at the repository root, and the menu-bar button shows that same raster bundled as
`Sources/Moonglow/Resources/TrayIcon.png`
([`StatusIconFactory`](Sources/Moonglow/Shell/StatusIconFactory.swift)) in every appearance.


## Requirements

- macOS 12 or later (deployment target).
- **Full Xcode** (27.0 verified). A Command Line Tools-only install is not sufficient on this
  machine — its SwiftPM, compiler, and SDK disagree with each other. See `docs/PORTING.md`.

## Commands

```bash
swift build                  # build the app and MoonglowKit
swift test                   # run the complete test suite
swift run                    # run from the SwiftPM build (login-item registration requires a packaged app)
bash Packaging/build-app.sh  # build the app bundle, DMG, and ZIP
```

If plain `swift build` hangs, a stuck SwiftPM plugin process (observed from the VS Code
Swift extension) is holding the `.build` lock. Build and test with a scratch path instead:

```bash
swift build --scratch-path .build-run
swift test  --scratch-path .build-run
```

The packaging script creates:

- `artifacts/Moonglow.app`
- `../../release/<VERSION>/macOs/Moonglow-macOS-universal.dmg`
- `../../release/<VERSION>/macOs/Moonglow-macOS-universal.zip`

It builds a universal (`arm64` and `x86_64`) app for macOS 12 or later. Hardware DDC/CI
brightness targets the main display, and only when it is an external panel: on a MacBook whose
main display is the built-in panel, or for any display that is not main, the brightness slider
is unavailable and the Displays section says why. When DDC/CI cannot be used at all, brightness
falls back to software gamma dimming. Software dimming changes image output, not the display's
backlight, and requires CoreGraphics gamma support. Login-item management requires macOS 13 or
later. The app bundle is ad-hoc signed for local testing; it is not
Developer ID signed or notarized. Users may need to Control-click the app and choose **Open** on
first launch.

To preserve the normal outputs when making a separate verification build, set alternate output
directories:

```bash
MOONGLOW_ARTIFACTS_DIR="$PWD/artifacts/test-build" \
MOONGLOW_RELEASE_DIR="$PWD/release/test-build" \
bash Packaging/build-app.sh
```

If every `swift` command fails with a `dyld` symbol error, an "SDK is not supported by the
compiler" error, or a Swift Testing `__SourceBounds` mismatch, the active developer directory
is broken. Check `xcode-select -p` and install or repair Xcode rather than working around it.

## Diagnostics

Brightness routing, DDC probe outcomes, and display enumeration are logged to
`~/Library/Logs/Moonglow/Moonglow.log` (rotated at 512 KB, three backups kept).
The Displays section in Settings shows the path and reveals it in Finder.

## Architecture

`AppDelegate` is the composition root and is deliberately hand-rolled — there is no DI
container, matching the original design.

- A single `MainViewModel` instance backs both the popup and the settings window, so slider
  and automatic-mode state stay in sync without a second settings path.
- Platform behaviour is behind `DisplayService`, `BrightnessService`, and
  `ColorTemperatureService`. `MoonglowKit` never imports SwiftUI; the app target supplies
  automatic appearance handling, `LoginItemService`, and `HotKeyService`.
- Global hot keys adjust brightness (±5) and colour temperature (±100 K) from anywhere:
  Ctrl+Option with ↑/↓/←/→ by default, re-recordable or clearable per action in Settings.
  A temperature key leaves automatic mode so the change sticks. Registration uses Carbon,
  so no Accessibility permission is needed.
- View state uses Combine's `ObservableObject` and `@Published`, keeping the deployment target at
  macOS 12 while supporting both Intel and Apple Silicon.
- Colour temperature and software brightness share CoreGraphics gamma transfer tables. Hardware
  brightness uses the private CoreDisplay `IOAVService*` API on Apple Silicon and IOKit framebuffer
  I2C on Intel, with gamma dimming as the fallback when DDC/CI is unavailable.

The private API is isolated in `Sources/MoonglowKit/Services/NativeBridge.swift`. The app is
distributed directly rather than through the Mac App Store because of that dependency. The current
packaging script applies only an ad-hoc signature; public distribution still requires Developer ID
signing and notarization.

Startup is a login item registered through `SMAppService.mainApp`, which registers *the running
bundle*: the toggle in the settings window does nothing when Moonglow is launched
straight from `.build` rather than from `Moonglow.app`. The stored setting is only
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
- The popup is a native AppKit menu with system styling. Brightness and color temperature use the
  same triangle-headed slider, and the temperature track shows the warm-to-cool gradient. The
  Settings window switches automatically between neutral light and dark appearances with macOS.
- Displays are always available in the settings window; schedule controls are followed by Startup
  and Displays.
- A display capability is never reported unless the backend can actually apply it.
- ColourSync gamma state is restored on quit.
- Settings live at `~/Library/Application Support/Moonglow/settings.json`.
