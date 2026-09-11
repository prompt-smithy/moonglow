# Amber

Amber is a macOS menu-bar application for controlling screen color temperature on a schedule. It is built with .NET 10 and Avalonia UI and packaged as a self-contained Apple Silicon application.

![Amber sun icon](Amber%20MacOS/Assets/Amber.png)

## Features

- Active-display discovery through CoreGraphics.
- Manual color temperature from 900 K to 6600 K.
- Automatic morning and evening transitions in 100 K steps.
- Six light and six dark palettes that follow system appearance.
- Persistent JSON settings with legacy-setting migration.
- Menu-bar lifecycle with a reusable settings window.
- ColorSync gamma restoration on normal exit.
- Self-contained Apple Silicon app, DMG, and ZIP packaging.

Brightness controls are capability-aware but currently disabled. A native macOS brightness backend will remain unavailable until a supported implementation is validated on physical displays, docks, and adapters.

## Requirements

### Run a packaged build

- macOS 12 or later.
- Apple Silicon Mac for the current `osx-arm64` package.
- No separate .NET runtime installation.

Current test packages are ad-hoc signed rather than Developer ID signed or notarized. On first launch, Control-click Amber in Applications and choose **Open**.

### Build from source

- macOS.
- .NET 10 SDK.
- Git.

## Build and test

From the repository root:

```bash
dotnet restore Amber.MacOS.slnx
dotnet build "Amber MacOS/Amber.MacOS.csproj" --configuration Release
dotnet test Amber.MacOS.slnx --configuration Release
```

Run during development:

```bash
dotnet run --project "Amber MacOS/Amber.MacOS.csproj"
```

## Package

Build the Apple Silicon application bundle, DMG, and ZIP on macOS:

```bash
bash "Amber MacOS/Packaging/build-macos.sh"
```

Outputs:

```text
release/Amber-macOS-arm64.dmg
release/Amber-macOS-arm64.zip
```

The package is self-contained, so testers do not need the .NET runtime.

## Settings

Amber stores settings at:

```text
~/Library/Application Support/Amber/settings.json
```

## Documentation

- [Functionality](Amber%20MacOS/docs/FUNCTIONALITY.md)
- [Requirements and compatibility](Amber%20MacOS/docs/REQUIREMENTS.md)
- [Architecture](Amber%20MacOS/docs/ARCHITECTURE.md)
- [Build, test, and release](Amber%20MacOS/docs/BUILD.md)
- [Project agent](.github/agents/Amber.agent.md)

## Current limitations

- Native brightness control is not implemented.
- Intel Macs are not currently packaged.
- Gamma-table behavior can vary with HDR, sleep/wake, virtual displays, AirPlay, and other color-management software.
- Public distribution still requires Developer ID signing and notarization.

## License

Licensed under the [MIT License](LICENSE).