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

Amber provides DDC/CI hardware brightness from `1..100%` for the primary external display on Apple Silicon. Startup sends a read-only MCCS luminance probe; the UI reports brightness support only when the CoreDisplay service accepts the probe. The connected Dell P2419H has been validated for DDC reads and writes.

The implementation uses the private CoreDisplay `IOAVServiceCreate`, `IOAVServiceReadI2C`, and `IOAVServiceWriteI2C` functions because macOS does not expose a stable public API for external-display DDC/CI. These functions may change or disappear in a future macOS release.

- Built-in display brightness is not supported by this backend.
- Independent control of multiple external displays is not implemented.
- DDC/CI behavior depends on the monitor, cable, dock, adapter, and macOS release.

Every target hardware configuration must be physically tested before support is claimed.

## Distribution status

The package script applies an ad-hoc signature for Apple Silicon integrity. It does not provide Developer ID trust or Apple notarization. Testers may need to Control-click Amber and choose **Open** on first launch.

Public distribution requires Apple Developer Program membership, a Developer ID Application certificate, hardened runtime configuration, reviewed entitlements, notarization, stapling, and validation on each supported OS and architecture.