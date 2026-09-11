# Build, Test, and Release

Commands below run from the repository root unless stated otherwise.

## Restore

```bash
dotnet restore Amber.MacOS.slnx
```

## Build

```bash
dotnet build "Amber MacOS/Amber.MacOS.csproj" --configuration Debug
dotnet build "Amber MacOS/Amber.MacOS.csproj" --configuration Release
```

Outputs are under `Amber MacOS/bin/<Configuration>/net10.0/` and use the `Amber` assembly name.

## Test

```bash
dotnet test Amber.MacOS.slnx --configuration Release
```

Tests do not invoke CoreGraphics and can run on Windows or macOS.

## Run for development

On macOS:

```bash
dotnet run --project "Amber MacOS/Amber.MacOS.csproj"
```

Running from source requires the .NET 10 SDK. End-user release bundles do not require .NET.

## Regenerate icons

The committed PNG and ICO are generated from an Amber sun-mark script on Windows PowerShell:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
& ".\Amber MacOS\Assets\Generate-Icons.ps1"
```

The script creates `Amber.png` at 1024×1024 and a six-resolution `Amber.ico`. The macOS packaging script derives `Amber.icns` with `sips` and `iconutil`.

## Cross-publish Apple Silicon

This validates restore, compilation, native apphost generation, and Amber output naming from Windows or macOS:

```bash
dotnet publish "Amber MacOS/Amber.MacOS.csproj" \
  --configuration Release \
  --runtime osx-arm64 \
  --self-contained true \
  -p:PublishSingleFile=true \
  --output artifacts/publish-osx-arm64
```

Cross-publishing does not create or validate a macOS `.app`, `.icns`, signature, or DMG.

## Build distributable files

Run on macOS:

```bash
bash "Amber MacOS/Packaging/build-macos.sh"
```

The script creates:

```text
release/Amber-macOS-arm64.dmg
release/Amber-macOS-arm64.zip
```

It publishes `osx-arm64`, constructs `Amber.app`, embeds `Amber.icns`, applies and verifies an ad-hoc signature, creates a bundle-preserving ZIP, and creates a compressed DMG with an Applications shortcut.

## Obtain a test installer

Run the macOS packaging script on an Apple Silicon Mac, then use `release/Amber-macOS-arm64.dmg` for installation testing or sharing with another tester.

## Install a test build

1. Open `Amber-macOS-arm64.dmg`.
2. Drag Amber to Applications.
3. Eject the DMG.
4. Control-click Amber in Applications and choose **Open**.
5. Confirm the Gatekeeper prompt.

The extra first-launch step is expected until Developer ID signing and notarization are configured.

## Release verification checklist

- Run the complete Release test suite.
- Confirm the apphost is named `Amber`.
- Confirm `Info.plist` names Amber, bundle ID `com.amber.macos`, and `Amber.icns`.
- Run `codesign --verify --deep --strict "Amber.app"` on macOS.
- Mount the DMG and verify drag-to-Applications installation.
- Open Amber and verify tray icon, window icon, light/dark themes, settings persistence, and color reset on quit.
- Test color temperature on each target display and HDR configuration.
- Do not claim brightness support until a physical-hardware backend passes validation.