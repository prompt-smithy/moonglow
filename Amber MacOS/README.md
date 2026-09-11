# Amber for macOS

Amber is an Avalonia UI menu-bar app for display brightness and color temperature. The current implementation includes persisted settings, automatic color-temperature scheduling, active-display discovery, and CoreGraphics gamma-table color control.

Brightness control is capability-gated and currently disabled while the public macOS/IOKit implementation is validated on physical hardware. The app does not call private DisplayServices APIs.

## Documentation

- [Functionality](docs/FUNCTIONALITY.md)
- [Requirements and compatibility](docs/REQUIREMENTS.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Build, test, and release](docs/BUILD.md)
- [Amber project agent](../.github/agents/Amber.agent.md)

## Build and test

```bash
dotnet restore ../Amber.MacOS.slnx
dotnet test ../Amber.MacOS.slnx
dotnet run --project Amber.MacOS.csproj
```

Settings are stored at `~/Library/Application Support/Amber/settings.json` on macOS. Existing settings from the former application directory are imported once.

## Package Apple Silicon

Run on macOS:

```bash
bash "Packaging/build-macos.sh"
```

The outputs are:

- `release/Amber-macOS-arm64.dmg` - drag-to-Applications installer image.
- `release/Amber-macOS-arm64.zip` - compressed application bundle.

Both packages are self-contained and do not require the .NET runtime.

The current builds use an ad-hoc signature for Apple Silicon compatibility but are not Developer ID signed or notarized. On the test Mac, open the DMG, drag **Amber** to **Applications**, then Control-click the application and choose **Open** to approve the first launch. Developer ID signing and notarization will remove this extra Gatekeeper step.