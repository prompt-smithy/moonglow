# Moonglow

Moonglow shifts your screen colour temperature toward amber in the evening and back to
neutral in the morning, on Windows, Linux, and macOS.

This repository is a **monorepo of three independent native apps**, not a shared codebase.
Each platform is written in the language that fits it — C#/WPF, Rust, and Swift — so the
apps share a product, a behavioural contract, and a release process, not source files.

| | Language | Package | Notes |
|---|---|---|---|
| [`apps/windows`](apps/windows) | C# / .NET Framework 4.8 / WPF | `Moonglow.csproj` | Tray app, Inno Setup installer |
| [`apps/linux`](apps/linux) | Rust (eframe) | `moonglow` + `moonglow-core` | Desktop app, global shortcuts, deb/rpm/AUR |
| [`apps/macos`](apps/macos) | Swift / SwiftUI | `Moonglow` + `MoonglowKit` | Menu bar app, DMG/ZIP |

## Why a monorepo

The three apps were ports of each other and had silently drifted: different default night
temperatures, different schedule windows, different settings keys. Nothing in the
repository structure made that drift visible, so nothing caught it.

Keeping them together does not let them share code, but it does let them share:

- **One behavioural contract** — [`spec/`](spec) holds a JSON Schema and golden vectors that
  all three implementations must satisfy. This is the source of truth for defaults,
  schedule semantics, and temperature mapping.
- **One issue tracker and one place to look** — a bug report for "night mode starts an hour
  late" is one issue with three possible implementations, not three parallel trackers.
- **One CI config** — path-filtered workflows so a change to the spec runs every platform's
  conformance suite.

## The spec

[`spec/README.md`](spec/README.md) is the contract. It documents the canonical settings
schema, the drift that already exists between platforms, and the conformance matrix.

```bash
# Golden vectors live here
spec/vectors/kelvin.json
spec/vectors/schedule.json
spec/vectors/temperature-range.json
spec/vectors/settings.json
```

All three implementations load these vectors in their test suites, so a change to the
contract that is not mirrored in an implementation fails CI.

## Building

Release artifacts live at the repository root, grouped by version and operating system:

```text
release/
  1.0.1/
    linux/    moonglow_1.0.1_amd64.deb, moonglow-1.0.1-linux-amd64.tar.gz, SHA256SUMS
    windows/  Moonglow_Portable.exe, Moonglow-windows-portable.zip, SHA256SUMS
    macOs/    Moonglow-macOS-universal.dmg, Moonglow-macOS-universal.zip, SHA256SUMS
```

`VERSION` selects the release version. Keep the native version in the Linux Cargo workspace,
Windows project, and macOS packaging plist aligned with it; the release builder checks these
values. Builds preserve other version folders. Windows native builds also include
`Moonglow-Setup.exe` when Inno Setup is installed.

```bash
python3 tools/build-release.py                   # package the current OS
python3 tools/build-release.py --platform linux  # select an OS
python3 tools/build-release.py --all             # all three on a configured Mac
```

The Mac build host can cross-build the Linux amd64 binary and Windows portable artifacts.
For that, install PowerShell, the .NET SDK, Zig, and cargo-zigbuild, then add the Linux Rust target:

```bash
brew install powershell dotnet zig
cargo install cargo-zigbuild --locked
rustup target add x86_64-unknown-linux-gnu
```

Linux archives contain the executable, desktop entry, icons, manual, and udev rule. The `.deb`
installs these files; for a portable archive, extract it and run `usr/bin/moonglow` from its
versioned directory. Linux cross-builds target glibc 2.31 or later. Windows portable builds
require .NET Framework 4.8. macOS artifacts are universal Intel/Apple Silicon. The automated CI
artifacts are ad-hoc signed for testing; public macOS releases are Developer ID signed, and the
release DMG is notarized and stapled. See the [macOS README](apps/macos/README.md) for download and
packaging details.

The **Release artifacts** workflow builds and tests on all three native runners. Run it manually
or push a tag matching `VERSION` (for example `v1.0.1`); its `moonglow-release` artifact contains
the same version/OS directory tree. It does not publish a GitHub release.

Each native app has its own build command. The root release builder orchestrates packaging and supports the cross-builds above.

### Windows

Requires Windows and the .NET Framework 4.8 developer pack.

```powershell
cd apps/windows
dotnet build Moonglow.csproj -c Release
dotnet test Moonglow.Core.Tests/Moonglow.Core.Tests.csproj -c Release
dotnet build Moonglow.csproj -t:PackageAll   # installer
```

### Linux

Requires a Rust toolchain and the usual Wayland/X11 development headers (see
[`apps/linux/README.md`](apps/linux/README.md)).

```bash
cd apps/linux
cargo run --release -p moonglow
cargo test --workspace
cargo clippy --workspace --all-targets
```

### macOS

Requires a Swift 6 toolchain on macOS.

```bash
cd apps/macos
swift build
swift test
bash Packaging/build-app.sh   # app bundle, DMG, and ZIP
```

## Repository layout

```
apps/windows/     C# / WPF application, net48 core library + tests, and installer
apps/linux/       Rust workspace: moonglow-core (logic) and moonglow (GUI)
apps/macos/       Swift package: MoonglowKit (logic) and Moonglow (SwiftUI app)
resources/        Shared logo artwork: amber-tile master SVG/PNG (black crescent
                  with rays), Windows ICO, and the Linux hicolor set
spec/             Settings schema and golden vectors shared by all apps
.github/          Path-filtered CI for each platform
```

The mark is a crescent moon with rays on an amber tile. The master artwork is
`resources/Moonglow-option-e-amber-noring.svg` (with its 1024 px PNG render beside it), and
every app shows that same mark in both light and dark modes. Note that
`tools/generate-logo-assets.ps1` still draws the previous two-tone mark — do not re-run it
until it is updated for the amber tile.

## Licence

MIT — see [LICENSE](LICENSE).
