# Moonglow

A macOS menu-bar app for external-display hardware brightness and scheduled colour temperature.

The app includes a native menu-bar popup and a settings window for the temperature schedule,
startup behavior, and displays. Its light or dark appearance follows macOS automatically. See [`docs/PORTING.md`](docs/PORTING.md)
for implementation status and platform details.

## Download

[Download from GitHub Releases](https://github.com/prompt-smithy/moonglow/releases).

The packaging script produces `Moonglow-macOS-universal.dmg` and `Moonglow-macOS-universal.zip`:
a universal (`arm64` and `x86_64`) build for macOS 12 or later. Public releases are signed with
Developer ID. The app is notarized and stapled before the DMG and ZIP are built, so both carry
the ticket. The ZIP contains the signed app that the Homebrew cask installs.

Install with Homebrew:

```bash
brew install --cask prompt-smithy/apps/moonglow
```

After publishing a notarized direct release, update the version and ZIP checksum in the
[Moonglow cask](https://github.com/prompt-smithy/homebrew-apps/blob/main/Casks/moonglow.rb).

## Layout

| Path | Purpose |
| --- | --- |
| `Package.swift` | SwiftPM manifest. Two targets: `MoonglowKit` and `Moonglow`. |
| `Sources/MoonglowKit/` | Platform logic: models, settings, schedule math, light/dark appearance colors, view model, service protocols. No UI framework dependency. |
| `Sources/Moonglow/` | macOS app shell: entry point, status item, popover, settings window, SwiftUI views, native bridges. |
| `Tests/MoonglowKitTests/` | Swift Testing suite for everything in `MoonglowKit`. |
| `Packaging/` | `Info.plist`, `build-app.sh` (direct build: app bundle, DMG, ZIP), `build-mas.sh` (Mac App Store `.pkg`), `build-sandbox-prototype.sh`, and `AppStoreSandbox.entitlements`. |

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
bash Packaging/build-app.sh  # direct / Homebrew build: app bundle, DMG, and ZIP
bash Packaging/build-mas.sh  # Mac App Store build: signed .app and .pkg (needs Apple signing identities)
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

It builds a universal (`arm64` and `x86_64`) app for macOS 12 or later. If a Developer ID
Application certificate is installed in the login keychain, the script detects and uses it
automatically. Set `MOONGLOW_SIGNING_IDENTITY` to choose a specific identity. Set
`MOONGLOW_NOTARIZE=1` and `MOONGLOW_NOTARY_KEYCHAIN_PROFILE=<profile>` after configuring an
`xcrun notarytool` keychain profile to notarize and staple the DMG.

### Build flavors

Moonglow's macOS packaging produces two builds from this codebase.

| Build | Command | Hardware (DDC/CI) | Sandbox | Distribution |
| --- | --- | --- | --- | --- |
| Direct / Homebrew | `build-app.sh` | Yes | No | Developer ID, notarized DMG and ZIP |
| Mac App Store | `build-mas.sh` | No (software only) | Yes | Apple Distribution `.app` in a signed `.pkg` |

`MOONGLOW_APP_SANDBOXED=1` selects the Mac App Store feature set. It excludes all private API code and
hides every hardware control. `MoonglowKit.BuildFlavor` exposes the choice to the UI.

- `bash Packaging/build-app.sh` builds the full version for direct download and the Homebrew tap.
  It uses one unified brightness slider by default, with optional split Backlight and Digital Shade
  sliders. Hardware brightness remains available on the primary external display in mixed
  setups. Digital Shade can dim all gamma-capable displays down to 50%; a registered recovery
  shortcut unlocks lower levels.
- `bash Packaging/build-mas.sh` builds the Mac App Store submission. It runs `build-app.sh` in
  software-only release mode and embeds the provisioning profile. It checks that the profile
  matches the selected team and bundle ID, then signs the app using the profile's application
  identifier and the App Sandbox entitlement. It verifies the signature and entitlements before
  creating `release/<VERSION>/macOs/Moonglow-macOS-AppStore.pkg` with the Store installer identity.
  Set `MOONGLOW_TEAM_ID` and `MOONGLOW_MAS_PROVISION_PROFILE` before running. The script finds
  Store signing identities for that team, or accepts `MOONGLOW_MAS_APP_IDENTITY` and
  `MOONGLOW_MAS_INSTALLER_IDENTITY` overrides. Validate the package with `xcrun altool --validate-app`
  or Transporter. Software brightness is available down to 50%. A registered restore-to-100%
  shortcut unlocks lower levels.
- `bash Packaging/build-sandbox-prototype.sh` builds a local sandboxed software-only build named
  "Moonglow Sandbox Prototype" for testing. It is the same feature set as the Mac App Store build
  and is not submitted.

**Store builds need a new `CFBundleVersion` each upload.** Bump `CFBundleVersion` in
`Packaging/Info.plist` before every Mac App Store upload. The build script and
`tools/build-release.py` read it, and App Store Connect rejects a build number it has already seen.
`CFBundleShortVersionString` must match `VERSION`.

Settings opens the privacy policy in the browser. Once the updated policy is on the public
`main` branch, enter its URL in App Store Connect. [Privacy policy](https://github.com/prompt-smithy/moonglow/blob/main/PRIVACY.md)

For a local ad-hoc build, explicitly clear the identity:

```bash
MOONGLOW_SIGNING_IDENTITY='' bash Packaging/build-app.sh
```

The app targets macOS 12 or later. Hardware DDC/CI brightness targets the main display only when
it is an external panel. A mixed setup keeps that hardware control; Digital Shade in split controls
dims all gamma-capable displays. The unified 50% hardware/software handoff is used when every
active display supports hardware brightness. Backlights that Moonglow has never managed are left
untouched. Disabling an owned hardware route releases it once to 100%. Software dimming changes
the image rather than the physical backlight and has a 50% floor until a recovery shortcut is
registered. Both engines can be switched independently in Settings. Login-item management requires macOS 13 or
later; on macOS 12 the Launch at login toggle is replaced by a note. Without a Developer ID identity, the build script uses an ad-hoc signature and does not
notarize the app. For public releases, use a Developer ID identity and enable notarization as
described above.

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

The direct build uses a private API isolated in `Sources/MoonglowKit/Services/NativeBridge.swift`,
so it is distributed directly and through Homebrew rather than the Mac App Store. The Mac App Store
build compiles that path out entirely, so it contains no private API and no hardware controls. Public direct-download releases are Developer ID signed,
and the release DMG is notarized and stapled. Builds made without a Developer ID identity use an
ad-hoc signature for local testing.

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
- The Mac App Store build always uses a unified brightness slider and exposes only Digital Shade
  controls. CI checks the sandboxed release for private hardware APIs, the gamma debug probe, and
  links to other builds or source code.
- Each Mac App Store upload needs a higher `CFBundleVersion` in `Packaging/Info.plist`. Notarized direct builds staple the ticket into the `.app` before the ZIP and DMG are created.
- ColourSync gamma state is restored on quit.
- Settings live at `~/Library/Application Support/Moonglow/settings.json`. In the Mac App Store build, which is sandboxed, the same path resolves inside `~/Library/Containers/com.prompt-smithy.Moonglow/Data/`.
