# Amber for macOS: Role and Task

## Role

I am acting as the senior .NET desktop engineer for this port, with responsibility for:

- Avalonia UI application architecture and implementation.
- WPF-to-Avalonia migration while preserving the existing MVVM behavior.
- macOS display discovery, brightness control, and color-temperature integration through native APIs where required.
- macOS menu-bar lifecycle, startup behavior, settings storage, packaging, and validation.
- Keeping the Windows application unchanged unless a shared-code extraction is explicitly approved.

I will favor small, testable platform abstractions and reuse existing application logic where it is genuinely platform-neutral. Native macOS behavior will be isolated behind interfaces instead of leaking into view models or Avalonia views.

## Task

Create the Amber macOS menu-bar application in Avalonia UI with functionality comparable to the existing Windows/WPF application:

- Discover connected displays and report their controllable capabilities.
- Adjust display brightness with a safe minimum value.
- Adjust screen color temperature manually.
- Automatically transition between configurable daytime and nighttime color temperatures.
- Provide a compact menu-bar popup and a full settings window.
- Follow the macOS light/dark appearance and offer the existing theme choices where practical.
- Store user settings locally and optionally launch at sign-in.
- Run as a single application instance and cleanly restore color state on exit.
- Produce distributable macOS builds for Apple Silicon, with Intel support considered separately.

The new application will be created as a separate macOS/Avalonia project in this repository. The existing Windows/WPF project remains the behavioral reference.

## Proposed Technical Direction

- **Runtime:** .NET 10, subject to Avalonia package compatibility at implementation time.
- **UI:** Avalonia UI using XAML and the existing MVVM approach without introducing a framework or DI container unless it removes a concrete migration problem.
- **Menu bar:** Avalonia `TrayIcon` and native menu integration, with a compact popup/window where macOS permits it.
- **Native boundary:** C# interfaces backed by macOS implementations using CoreGraphics, IOKit, and other appropriate system frameworks.
- **Settings:** `System.Text.Json` under the macOS application-support directory.
- **Startup:** macOS login-item integration using a supported mechanism appropriate to the chosen packaging model.
- **Packaging:** Self-contained `osx-arm64` application bundle, followed by signing/notarization guidance. A universal or separate `osx-x64` build is a follow-up decision.

## Reuse Plan

### Reuse with little or no behavioral change

- `Models/AppSettings.cs`
- `Models/AppTheme.cs`
- `Infrastructure/ObservableObject.cs`
- `Infrastructure/RelayCommand.cs`
- Theme palette definitions from `Services/ThemeCatalog.cs`
- Schedule interpolation and 100 K stepping from `Services/ScheduleService.cs`
- Kelvin-to-RGB conversion from `Services/ColorTemperatureService.cs`
- Most settings and command behavior in `ViewModels/MainViewModel.cs`

### Adapt for Avalonia or cross-platform use

- Replace WPF `DispatcherTimer` in scheduling with an Avalonia timer or a platform-neutral periodic service.
- Replace WPF dependency properties, resources, styles, bindings, and window lifecycle APIs with Avalonia equivalents.
- Adapt `MainViewModel` to depend on brightness, color-temperature, theme, startup, and display interfaces.
- Store settings at `~/Library/Application Support/Amber/settings.json` and import the legacy file once.
- Replace Windows appearance detection with Avalonia/macOS appearance notifications.
- Rebuild `TimePicker`, tray popup, settings window, icon handling, and application composition for Avalonia.

### Replace with macOS implementations

- WMI internal-panel brightness.
- Windows DDC/CI calls in `dxva2`.
- Magnification API and GDI gamma ramps.
- Windows monitor enumeration and physical-monitor handle management.
- Registry-based startup registration.
- DWM title-bar styling and Windows system-event hooks.
- Hardcodet WPF tray integration.
- Inno Setup packaging.

## macOS Constraints and Validation Spikes

The platform-specific controls need early proof before the full UI is built:

1. **Internal display brightness:** validate a viable macOS API on current Apple Silicon hardware. Some commonly used display-control APIs are not public or stable across macOS releases.
2. **External display brightness:** validate DDC/CI through IOKit for the target monitor/connection combinations. Support varies by monitor, dock, adapter, and macOS version.
3. **Color temperature:** validate per-display gamma-table control through CoreGraphics. macOS does not expose Night Shift as a supported public API, and gamma changes may be limited or reset by the OS, HDR, display mode changes, sleep, or other color-management features.
4. **Menu-bar popup:** verify the Avalonia tray behavior and activation/focus semantics on macOS before reproducing the Windows popup interaction exactly.
5. **Permissions and distribution:** identify any entitlements, sandbox limitations, signing requirements, or notarization consequences before final packaging.

If a capability cannot be implemented reliably with supported APIs, the UI will expose that display as unsupported rather than claiming success. Any use of private or undocumented APIs will require explicit approval and will be isolated so it can be replaced.

## Implementation Phases

1. Scaffold the Avalonia macOS project and establish application lifecycle, MVVM composition, logging, and settings.
2. Extract or port platform-neutral models, infrastructure, scheduling, and color conversion with focused tests.
3. Implement display enumeration and run native brightness/color-temperature proof-of-concept checks.
4. Build the menu-bar controls, settings window, time picker, themes, and capability/error states.
5. Add login-at-startup behavior, display-change handling, single-instance behavior, and state restoration.
6. Validate on macOS hardware, then publish the application bundle and document signing/notarization steps.

## Initial Acceptance Criteria

- The app launches on a supported macOS version and remains available from the menu bar.
- Connected displays are listed with accurate brightness and color-control capability states.
- Supported displays respond to brightness changes without blocking the UI.
- Color temperature can be applied, scheduled, and restored to neutral on exit where supported.
- Settings survive restart and automatic transitions match the Windows app's timing behavior.
- Unsupported hardware or denied native operations produce clear status without crashing.
- Light/dark appearance, keyboard navigation, and primary controls work in both the popup and settings window.
- An `osx-arm64` app bundle can be produced from a documented command.
- Existing Windows project behavior and files are not regressed by the port.

## Scope Requiring Review

Before implementation begins, please confirm or amend these assumptions:

- Apple Silicon (`osx-arm64`) is the first release target.
- The first release is distributed outside the Mac App Store.
- Use of private/undocumented macOS APIs is not allowed unless separately approved after supported alternatives are tested.
- Feature parity means capability-aware behavior; unsupported display hardware may have brightness or color controls disabled.
- The macOS project should remain a sibling of the existing Windows project, not replace it.
