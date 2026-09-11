# Requirements and Compatibility

## End users

- macOS 12 or later.
- Apple Silicon for the current `osx-arm64` package.
- No separately installed .NET runtime; release bundles are self-contained.
- Permission to open an ad-hoc-signed, non-notarized application for test builds.

Intel Macs are not currently packaged. An `osx-x64` or universal release requires an additional publish and bundle strategy.

## Developers

- .NET 10 SDK.
- Git.
- Avalonia dependencies restored from NuGet.
- macOS is required for final app-bundle, ICNS, codesign, ZIP, and DMG generation.
- macOS is the supported platform for development, runtime testing, and release validation.

No separate Xcode workload is required for the current P/Invoke-based CoreGraphics implementation. Xcode command-line tools may be required by the host for standard macOS signing utilities.

## Hardware support

### Color temperature

Color temperature requires a display with a writable CoreGraphics gamma table. It may be unavailable or overridden when using HDR, virtual displays, mirroring, AirPlay, unusual adapters, or other color-management applications. macOS can also reset gamma tables during display transitions, sleep, or wake.

### Brightness

Native brightness control is not currently implemented. macOS does not expose one stable public API that covers both built-in and external displays:

- Built-in panel approaches commonly rely on private frameworks.
- External displays generally require DDC/CI through IOKit.
- DDC/CI behavior depends on the monitor, cable, dock, adapter, and macOS release.

A future implementation must be tested on physical hardware and report support per display.

## Distribution status

The package script applies an ad-hoc signature for Apple Silicon integrity. It does not provide Developer ID trust or Apple notarization. Testers may need to Control-click Amber and choose **Open** on first launch.

Public distribution requires Apple Developer Program membership, a Developer ID Application certificate, hardened runtime configuration, reviewed entitlements, notarization, stapling, and validation on each supported OS and architecture.