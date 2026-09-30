# Porting status

Tracking the Swift port against `legacy-csharp/`. The .NET implementation is the reference
for behaviour; where the two disagree on *structure*, the Swift version is authoritative.

## Build status

Verified under Xcode 27.0:

| Command | Result |
| --- | --- |
| `swift build` | Passes |
| `swift test` | Passes — 86 tests in 8 suites |
| `bash Packaging/build-app.sh` | Passes — produces `Night Mode Controller.app`, DMG, and ZIP |
| App launch | Stays running with the stub backends; no crash reports |

## Done

| Area | Swift | Reference |
| --- | --- | --- |
| Settings model, defaults, ranges, lenient decoding | `Sources/AmberKit/Models/AppSettings.swift` | `Models/AppSettings.cs` |
| Time-of-day parsing and formatting | `Models/TimeOfDay.swift` | `ParseTime` in `AppSettings.cs` |
| Display model and capability summary | `Models/DisplayInfo.swift` | `Models/DisplayInfo.cs` |
| Settings load/save, normalization | `Services/SettingsStore.swift` | `Services/SettingsService.cs` |
| Kelvin to RGB scale, gamma ramp construction | `Services/KelvinConversion.swift` | `MacColorTemperatureService` |
| MCCS luminance packet and checksum | `Services/DDCPacket.swift` | `MacDdcBrightnessService.BuildWritePacket` |
| Schedule evaluation and the 10 s runner | `Services/ScheduleService.swift` | `Services/ScheduleService.cs` |
| Ten palettes, ordering, control accents | `Services/ThemeCatalog.swift` | `Services/ThemeCatalog.cs` |
| Service protocols | `Services/ServiceProtocols.swift` | `Services/DisplayServices.cs` |
| View model: clamping, persistence, capability gates, callbacks | `ViewModels/MainViewModel.swift` | `ViewModels/MainViewModel.cs` |
| App shell: accessory policy, native status menu, settings-window lifecycle | `Sources/Amber/Shell/` | `App.axaml.cs`, `QuickControlsWindow.axaml.cs` |
| Theme application from system appearance | `Sources/Amber/Theme/ThemeService.swift` | `Services/ThemeService.cs` |
| Hex-to-colour conversion | `Sources/Amber/Theme/Color+Hex.swift` | Avalonia brushes |
| Twelve-hour clock conversion and formatting | `Sources/AmberKit/Models/TwelveHourTime.swift` | `ClockIdentifier` mapping in `MainViewModel.cs` |
| Colour temperature window as one value | `Sources/AmberKit/Models/TemperatureRange.swift` | day/night target clamping in `AppSettings.cs` |
| Settings window, palette-drawn drop-down pickers, two-handle range bar | `Sources/Amber/Views/SettingsView.swift`, `Sources/Amber/Views/TimeOfDayPicker.swift`, `Sources/Amber/Views/ThemedDropDown.swift`, `Sources/Amber/Views/TemperatureRangeSlider.swift` | `Views/MainWindow.axaml` |
| Native backends: display enumeration, gamma colour temperature, DDC/CI brightness | `Sources/AmberKit/Services/NativeBridge.swift` | `MacDisplayService.cs`, `MacColorTemperatureService.cs`, `MacDdcBrightnessService.cs`, `MacNative.cs` |
| Native menu-bar controls: brightness, colour temperature, automatic toggle | `Sources/Amber/Shell/AppDelegate.swift`, `Sources/Amber/Shell/MenuSliderRow.swift` | `Views/QuickControlsWindow.axaml` |
| Unit tests | `Tests/AmberKitTests/` | `Amber MacOS.Tests/` |

**Deliberately not ported:** legacy settings import (`DisplayController/settings.json`) and
legacy dark theme-name migration. This port starts fresh, so `SettingsStore` has no migration
path and the old JSON is not read.

## Beyond the port (macOS only)

Behaviour the .NET original had no equivalent for. These have no reference file to check
against, so the reasoning is recorded here instead.

### Launch at login

The app registers itself as a login item so it can start with the Mac, with a toggle in the
settings window. `LoginItemService` (`Services/ServiceProtocols.swift`) is the seam;
`Sources/Amber/Shell/LoginItemService.swift` implements it with `SMAppService.mainApp`.

The setting records *intent* only — the real registration lives in the system's login items,
which the user can change outside Night Mode Controller — so it is treated as two pieces of state that can
disagree:

- `launchAtLogin` is the stored preference and is what the toggle binds to.
- `launchAtLoginStatus` is read live from the service, and the caption shows it rather than
  deriving from the setting.

Consequences worth keeping. A registration the system refuses is **not** recorded, so the toggle
falls back to the truth instead of claiming a registration that was never made; the stored
preference is kept only when it was already on disk, and the caption then says it could not be
registered. A registration the user added by hand in System Settings is **adopted** at launch
rather than switched off or reported as off — this is not hypothetical, it was the state on the
development machine. Approval-pending is tested before "enabled" in the status, because a request
the user has not approved is not running and claiming otherwise is a lie they cannot act on.

The service reports failure rather than hiding it: `SMAppService.mainApp` registers the running
bundle, so under `swift run` — or any binary launched out of `.build` — there is no bundle to
register and every call fails. Test through `artifacts/Night Mode Controller.app`, not the bare binary.

### Native menu-bar controls

The popup is an AppKit `NSMenu`, following the native menu presentation used by Display Source
Switch. Its controls use system appearance and are independent of the selected palette; only the
Settings window is themed. Brightness and colour temperature use standard AppKit sliders embedded
in menu rows. Software brightness and colour temperature share the same gamma ramp, so adjusting
one does not overwrite the other.

## Not started

### 1. Settings view

`SettingsView` is SwiftUI and retains the app's selected palette. The popup is a native AppKit menu
and deliberately does not apply that palette.

**Clock format.** The .NET `TimePicker` took a `ClockIdentifier` (`24HourClock` /
`12HourClock`). AppKit has no equivalent — 24-hour display is a locale property — so the
conversions live in `Models/TwelveHourTime.swift` and the picker in
`Views/TimeOfDayPicker.swift`, which wraps AppKit's `NSDatePicker`.

Divergences from the .NET original:

- Night Mode Controller is **twelve-hour only**. The `Use24HourClock` setting and its toggle are gone
  entirely: `AppSettings` no longer has the key (older files simply drop it, since the decoder
  ignores unknown keys) and AM/PM is always available.
- The time and the appearance pickers are ``ThemedDropDown``s drawn from the palette rather than
  built on a system control. Two system controls were tried and rejected for the same reason —
  each paints itself with the *system* accent, which reads as system blue inside a palette:
  - A native `Picker` menu paints its bezel and its selected row with the system accent, and
    `NSMenu` highlighting follows the macOS accent setting, so `.tint` cannot repaint it.
  - `NSDatePicker` — the control System Settings uses for the Night Shift schedule — paints the
    clicked segment through `NSDatePickerCell`, which never attaches the window's field editor.
    Verified by rendering the field (the clicked segment is a system-blue pill) and by
    `currentEditor()` staying `nil` after a synthesised click. `selectedTextAttributes` therefore
    cannot reach it, the focus ring and control colours do not affect it, and there is no editable
    switch to suppress it. Replacing the control is the only fix.
- Sliders and the automatic-mode switches take the palette's control accent through `.tint`. Without
  it a switch's on-state is painted with the *system* accent (blue in every palette). Verified by
  rendering both states: `NSSwitch` does honour `.tint` on this macOS.
- The day and night targets are one two-handle bar (`Views/TemperatureRangeSlider.swift`)
  instead of two sliders. `Models/TemperatureRange.swift` owns ordering and snapping, and
  `MainViewModel.setTemperatureRange` writes both endpoints in a single pass, so a drag cannot
  persist settings or re-evaluate the schedule twice per update.

- The settings window is sized to the sheet's *measured* content height plus 8 pt of slack, rather
  than a fixed `680 × 780` like the .NET window, and it is 480 pt wide instead of 680. Width is set
  by the schedule rows: two 196 pt time fields plus the FROM/TO label need 456 pt before padding, so
  a narrower window squeezes the time selector. Two traps
  are worth knowing. First, assigning `contentViewController` makes AppKit size the window to the
  hosting view's first — much too short — fitting size, which was why the sheet opened with a
  scroll bar; the sheet therefore lays out and reports its height before the window is shown.
  The window must also be published to `AppDelegate.settingsWindow` *before* `contentViewController`
  is attached. Attaching it is what triggers the sheet's first height report, and that report is the
  only accurate measurement — the height does not change again afterwards, so one dropped on a nil
  window leaves the sheet stuck at its 320 pt minimum with a scroll bar.
  Second, the sheet measures slightly differently once the window changes size, so the height is
  applied **once**: following every report makes the window oscillate forever. The consequence is
  that expanding a collapsed section scrolls instead of growing the window. The `ScrollView`
  remains as the fallback.
- The header is the app name only — no subtitle, no platform badge — and Displays and Appearance
  start collapsed and expand on click, which keeps the sheet short. Both are reference material
  rather than everyday controls.

No display choice can move a scheduled time: the bound value is always the 24-hour `TimeOfDay`,
and persistence stays `HH:mm`. `TwelveHourTimeTests` round-trips all 1440 minutes, and
`TemperatureRangeTests` covers the fraction-to-kelvin maths and handle ordering.

### 2. Lifecycle parity — `Sources/Amber/Shell/HidingWindow.swift`

- Settings window hides on close and hides itself after 10 seconds without interaction, matching
  `MainWindow.axaml.cs`. The countdown is driven by an app-local event monitor rather than the
  window's own events: a drop-down opens a window of its own, so counting only the settings window's
  events would let it vanish mid-choice. Verified by opening the sheet and polling: still visible at
  9 s, hidden by 12 s.
- The popup is a standard `NSMenu` attached to the status item, so macOS owns its appearance and
  dismissal. It is not passed `ThemeService`; selecting a palette changes Settings only.
- No single-instance enforcement, matching the original. Login-item registration is now present;
  see **Beyond the port**.

### 3. Packaging

`Packaging/build-app.sh` does ad-hoc signing only. A release needs Developer ID signing with
the hardened runtime, notarization (`notarytool submit --wait`), and `stapler staple`.

## Toolchain

The build uses **full Xcode 27.0** (`/Applications/Xcode.app`), which supplies a
self-consistent Swift 6.4 compiler, the macOS 27.0 SDK, SwiftPM, and Swift Testing.

A Command Line Tools-only install on this machine was unusable and is not a supported setup:

- `swift-package` crashed with `dyld: Symbol not found: ... BuildServerProtocol ...` — the
  installed binary (June) was older than the frameworks it loaded (September).
- The bundled SDK was newer than the compiler (`SDK is not supported by the compiler`).
- The bundled Swift Testing framework expected `sourceBounds:` while its macro plugin emitted
  `sourceLocation:`.

Reinstalling Command Line Tools reproduced all three, because the package itself ships the
mismatched payload. If any of those errors reappear, the active developer directory is broken:
check `xcode-select -p` and install or repair Xcode. Do not work around it in code.

## Verification checklist

Requires hardware and a working toolchain; none of this can be automated.

What the backends *detect* has been confirmed on the development machine (Mac mini + DELL P2419H,
1920×1080): one display — ID 2, primary, gamma capacity > 0 — and the DDC/CI luminance probe
answered, so brightness and colour temperature both report as available. The items below are the
parts that still need a person looking at the screen.

`CGDisplayIsBuiltin` is **not** deprecated in the macOS 27.0 SDK — it carries only
`API_AVAILABLE(macos(10.2))` — so it is called directly rather than through the
`NSScreen.deviceDescription` workaround this document previously suggested.

- [ ] Gamma apply and reset on quit, across sleep/wake and display-mode changes.
- [ ] DDC brightness writes on an external display over HDMI/DisplayPort/USB-C.
- [ ] HDR behaviour with gamma tables applied.
- [ ] Light/dark switching across all ten palettes.
- [ ] All five schedule states, including both transitions.
- [ ] Settings round trip through `~/Library/Application Support/Amber/settings.json`.
- [ ] Menu-bar-only behaviour: no Dock icon, no app-switcher entry, no quit on window close.
