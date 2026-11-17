# Moonglow code review

Reviewer: Claude (Fable 5.2) · Date: 2026-11-16 · Branch: `codex/macos-sandbox-prototype` (uncommitted working tree on top of `f8b2305`)

## Scope and method

The working tree holds one large uncommitted change (58 files, +2715/−255): the **dual hardware/software brightness** feature from `docs/plans/dual-brightness-control.md`, the **restore-brightness recovery shortcut**, and the **macOS App Sandbox prototype** build flavour. I read that diff in full on all three platforms, plus the surrounding services it depends on (macOS `NativeBridge`, `IntelDDC`, `SettingsStore`, `ScheduleService`; Windows `ColorTemperatureService`, `BrightnessService`, `MonitorService`, `SettingsService`, `App.xaml.cs`; Linux `state.rs`, `traits.rs`, `ddc.rs`, `displays.rs`, `x11.rs`, `wlr.rs`, `shortcuts*.rs`, `settings_ui.rs`).

No code was changed by this review.

### What I ran

| Check | Result |
| --- | --- |
| `python3 spec/validate.py` | Passes (meta-schema check skipped: `jsonschema` not installed here) |
| `swift test` (apps/macos) | **Fails**: 8 tests, 14 issues, all in `MainViewModelTests` |
| `cargo test -p moonglow-core` | **Fails**: 8 tests in `state::tests`; `spec_conformance` passes |
| `dotnet test Moonglow.Core.Tests` | 20 pass on net8.0; the net48 run needs Mono, not available on this Mac |
| `cargo check -p moonglow` (GTK app), `Moonglow.Platform.Tests` (WPF) | Not runnable on macOS. Linux GUI and Windows platform findings below are from reading the code, not from running it |

### Summary

| ID | Severity | Platform | Finding |
| --- | --- | --- | --- |
| T1 | Blocker | macOS, Linux | 16 unit tests fail; CI will be red |
| H1 | High | all | Saved brightness is overwritten with 100 whenever the software lock engages (every launch on macOS with default settings) |
| H2 | High | macOS, Linux | "All displays or nothing" disables hardware brightness for the most common setups |
| H3 | High | all, worst on Windows | Hardware backlight is forced to 100% repeatedly when Moonglow is not managing it |
| H4 | High | Windows | 20 s probe timer never stops in mixed setups or with hardware disabled |
| H5 | High | Linux | New DDC-connector mapping loses hardware brightness on NVIDIA and most X11 drivers |
| H6 | High | macOS | Modal "set a recovery shortcut" alert at every launch for default users |
| M1 | Medium | all | Split-mode hot keys can never move the hardware backlight |
| M2 | Medium | all | Opening the split panel, or registering the shortcut, changes real brightness |
| M3 | Medium | Windows | UI thread takes the display worker's lock on every brightness property read |
| M4 | Medium | Windows | Legacy gamma path: one refusing monitor now blocks colour temperature everywhere and flickers |
| M5 | Medium | Windows | `DetectInternalPanel` publishes a transient "no hardware" state from a worker thread |
| M6 | Medium | Linux | wlr gamma probe now fails entirely if any one output lacks gamma |
| M7 | Medium | Linux | Desktops with no shortcut backend lose software brightness permanently |
| M8 | Medium | macOS | Menu shows "Setup required" instead of the percentage while hardware works |
| M9 | Medium | spec | New behaviour is not pinned by vectors; two of three conformance suites ignore the new fields |
| M10 | Medium | docs | `system.md` describes a different product |
| L1–L14 | Low | various | Cleanup and smaller defects, listed below |

The three High items H1–H3 share one root cause and are best fixed together; see "Design change (decided 2026-11-16)" before starting on them individually. That section records the owner's decision to replace the mandatory recovery-shortcut gate with a 50% minimum for software dimming.

---

## T1. The branch fails its own test suites

**macOS** (`apps/macos/Tests/MoonglowKitTests/MainViewModelTests.swift`), failing tests:

- "Capability flags and status text follow the services" (lines 187, 193)
- "Brightness hot keys step within 1...100" (414, 419)
- "Brightness is clamped to 1...100 and written through" (79, 83)
- "Software brightness is used when hardware DDC is unavailable" (46, 48)
- "applyInitialState pushes persisted values to the hardware" (611)
- "Display refresh restores manual temperature and brightness" (163, 164)
- "Brightness hot keys are inert without support" (429, 430)
- "Late hardware triggers a second enumeration and applies the persisted brightness" (332)

**Linux** (`apps/linux/crates/moonglow-core/src/state.rs`, `state::tests`): `apply_initial_state_pushes_values`, `brightness_clamps_to_range`, `brightness_needs_service_and_capable_display`, `brightness_status_carries_hardware_detail`, `capability_flags_follow_services`, `display_change_reprobes_and_reapplies_brightness`, `fallback_uses_software_when_hardware_unsupported`, `setters_persist_but_do_not_drive_without_displays`.

These are not all stale expectations. Sort them into two groups before touching them:

1. **Genuine regressions the tests caught.** "Late hardware … applies the persisted brightness" and "Brightness hot keys are inert without support" fail because of H1 (the saved value is rewritten to 100). `setters_persist_but_do_not_drive_without_displays` fails because `set_brightness` now refuses to persist without a capable display, which was a documented behaviour. `brightness_status_carries_hardware_detail` fails because the new early returns in `brightness_status` (state.rs:172) hide the "permission denied" detail that tells a Linux user to fix their i2c group membership. Fix the code for these.
2. **Expectations that legitimately changed.** The fakes record software writes of 100 alongside hardware writes (`[70, 100]` vs `[70]`), and status strings changed. Update these, but split the fakes so hardware and software writes are recorded separately; a single `applied` list cannot express the new contract.

Also add new tests (see "Tests to add"); none of the new routing logic is covered on any platform.

---

## High severity

### H1. The safety lock destroys the saved brightness instead of only withholding it

All three platforms respond to "no registered recovery shortcut and no hardware" by writing `manualBrightness = 100` (and `manualSoftwareBrightness = 100`) into settings and persisting it. Locking the software path only requires *applying* 100; the apply functions already do that on their own.

Where:

- macOS: `MainViewModel.updateHotKeys()` (`MainViewModel.swift:633-643`)
- Windows: `ForceSoftwareBrightnessSafe()` (`MainViewModel.cs:413`), called from `SetRestoreBrightnessHotkeyRegistered` (:353) and `PublishDisplays` (:804)
- Linux: `set_brightness_recovery_shortcut_registered` (`state.rs:411`), `set_shortcut` (:628), `apply_initial_state` (:865)

Concrete failures:

- **macOS, every launch, default settings.** `init` calls `refreshDisplays()` then `updateHotKeys()`. The Apple Silicon and Intel DDC probes are asynchronous by design (`NativeBridge.swift:414-419`), so `isHardwareSupported` is still false, and `restoreBrightnessShortcut` defaults to nil. The condition at line 633 is true, so the saved brightness becomes 100. When the probe completes, `applyInitialState()` pushes 100 to the monitor. A user who set 40% yesterday gets 100% at every login.
- **Windows.** The same happens when a DDC monitor answers late at sign-in (the case the probe timer exists for) and in any mixed-display setup.
- **Linux, portal backend (KDE, Hyprland).** `portal.rs:74` calls `set_brightness_recovery_registration(state, &bindings, false)` before every session re-creation, including the first one at startup and after editing *any* shortcut. That wipes the saved software level each time. This contradicts the comment in `apply_initial_state`, which says the saved level is kept while the key is being registered.
- **All.** Re-recording the recovery shortcut to a different combination resets brightness to 100.

Fix: never mutate saved values in the lock path. The only place that should write 100 into settings is the explicit restore action.

```swift
// macOS MainViewModel.updateHotKeys()
registeredHotKeys = hotKeyService.updateHotKeys([...])
applyBrightnessSettings()   // canSoftware is false without a registered key, so this already writes 100
```

```csharp
// Windows: delete ForceSoftwareBrightnessSafe(); both call sites become
ApplyBrightnessSettings();
NotifyBrightnessProperties();
```

```rust
// Linux state.rs
pub fn set_brightness_recovery_shortcut_registered(&mut self, registered: bool) {
    self.brightness_recovery_shortcut_registered =
        registered && self.settings.restore_brightness_shortcut.is_some();
    self.apply_brightness_safely();
}
```

Remove the equivalent blocks in `set_shortcut` and `apply_initial_state`. Windows and Linux already present 100 in the getter while locked (`Brightness` / `brightness()`); do the same on macOS so the slider does not show a value that is not on screen.

Regression tests: launch with saved 40 and a late hardware probe, assert settings still hold 40 and hardware receives 40. Launch with saved 40, software-only, shortcut configured but not yet registered: assert 100 applied, 40 retained, then 40 applied after registration.

### H2. "All displays or nothing" turns hardware brightness off for the common cases

The plan requires hardware control only when *every* active display supports it. Combined with how each platform classifies displays, that excludes most real setups:

- **macOS.** `supportsHardwareBrightness` is `ddc && isPrimary && !isBuiltin` (`NativeBridge.swift:127`), so at most one display can ever be true. `allDisplaysSupportHardwareBrightness` (`MainViewModel.swift:94-98`) uses `allSatisfy`, so with two or more active displays it is always false. A MacBook with its lid open and an external monitor, or a Mac with two monitors, had DDC brightness before this change and has none now.
- **Linux.** Internal panels are hard-coded as not hardware-capable (`displays.rs:89`), so a laptop plus a DDC monitor never qualifies. A docked laptop with the lid closed does not qualify either, because `connected_connectors` reads `status == "connected"` (`displays.rs:127`) and eDP stays "connected" with the lid shut.
- **Windows** is fine here because the internal panel counts through WMI.

The fallback is software dimming, which is locked by default (no recovery shortcut). The net result for a default-settings upgrade on those setups: the slider is disabled, the saved value is reset (H1), and the external monitor is pushed to 100% (H3). `brightnessLimitation` (`MainViewModel.swift:192-199`) still advises "make an external display the main display", which no longer helps.

Fix options, in order of preference:

1. **Route per display** instead of all-or-nothing. Send hardware levels to displays that support them and software dimming only to the ones that do not. On macOS this means `CoreGraphicsColorTemperatureService` keeps a brightness scale per `CGDirectDisplayID` rather than one `softwareBrightness`; on Linux the X11 and wlr gamma services need the same per-output scale; on Windows the Magnification path is desktop-wide, so Windows can keep all-or-nothing there.
2. If uniform dimming must stay: apply the rule to **external displays only** and treat internal panels as their own path (Linux: logind `org.freedesktop.login1.Session.SetBrightness`, which needs no root; Linux lid state: read the connector's `enabled` file rather than `status`).
3. At minimum, do not regress existing users: when the hardware path existed before the upgrade and software is locked, keep driving the hardware-capable display as before.

This changes a stated requirement in the plan, so it needs a product decision; I recommend option 1.

### H3. Hardware backlight is forced to 100% whenever Moonglow is not managing it

Every branch that does not use hardware writes 100 to it: `MainViewModel.swift:681-683, 690-692, 710-712`; `MainViewModel.cs:370-371, 401-402, 407-408, 424-425`; `state.rs:431` onward. Disabling "Hardware Backlight" in settings also writes 100 first.

Consequences:

- **Windows laptop plus a non-DDC external (TV, projector).** `_brightness.IsSupported` is true because of the internal panel, `AllActiveDisplaysSupportHardware` is false, so `SetBrightness(100)` sets the laptop panel to 100% through WMI. `PublishDisplays` repeats this at startup, on every popup open, on display change, on resume, and every 20 seconds from the probe timer (H4). The user's own brightness keys and Windows adaptive brightness are overridden continuously.
- **macOS and Linux.** A monitor whose brightness the user set on its own buttons is pushed to 100% at launch and on every slider move, even though Moonglow is not supposed to be controlling it.
- Turning hardware control *off* is the user asking Moonglow to stop touching the backlight; jumping to 100% is the opposite.

Fix: treat hardware as unmanaged unless it is the active route. Track ownership so an undo happens at most once:

```swift
private var hardwareOwned = false      // true after Moonglow writes a managed level

private func releaseHardwareIfOwned() {
    guard hardwareOwned, brightnessService.isHardwareSupported else { return }
    brightnessService.setHardwareBrightness(percent: 100)
    hardwareOwned = false
}
```

Call `releaseHardwareIfOwned()` in the non-hardware branches instead of the unconditional write, and set `hardwareOwned = true` where a managed level is written. Better still, read the level once before the first managed write (DDC GetVCP 0x10 is already implemented for the probe; WMI `CurrentBrightness` exists in `GetInternalBrightness`) and restore that value rather than 100.

### H4. Windows: the support-probe timer never stops in mixed setups

`App.xaml.cs:190-195` changed the stop condition from `HasBrightnessSupport` to `HasHardwareBrightnessSupport`, which is false whenever one display lacks hardware control *or the user disabled hardware brightness*. In those states the 20-second timer runs for the whole session: a WMI query, `DestroyPhysicalMonitors` plus a DDC capability probe on every monitor, a rebuild of the `Displays` collection, `ReapplyColorTemperature()`, and (with H3) a hardware write. DDC probing visibly disturbs some monitors.

Fix: probe only while there is something to discover, and back off.

```csharp
_supportProbeTimer.Tick += (_, _) =>
{
    bool worthProbing = _settingsService.Settings.HardwareBrightnessEnabled
        && !_brightness.IsSupported;          // nothing found yet at all
    if (!worthProbing || ++_probeAttempts > 15) { _supportProbeTimer.Stop(); return; }
    _viewModel.RefreshDisplaysAsync();
};
```

`SystemEvents.DisplaySettingsChanged` and the popup-open refresh already cover later hot-plugs; restart the timer from those handlers if needed.

### H5. Linux: the new connector mapping silently drops working DDC

Before this change, hardware support was "any I2C bus answered the luminance probe". Now a display is hardware-capable only if its DRM connector's sysfs `ddc` symlink resolves to an answering bus (`ddc.rs:272-330`) and, on X11, only if the RandR output name equals the DRM connector suffix (`x11.rs:256`).

- **No `ddc` symlink.** The NVIDIA proprietary driver, DisplayLink docks, and DisplayPort connectors on older kernels do not expose it. `supported_connectors()` is then empty, `ddc_brightness_supported` becomes false (`displays.rs:75`), and the user loses hardware brightness even though the buses answer.
- **X11 name mismatch.** DRM names HDMI connectors `HDMI-A-1`; the modesetting DDX calls the same output `HDMI-1`, the Intel DDX `HDMI1`, amdgpu `HDMI-A-0` / `DisplayPort-0`, NVIDIA `DP-0`. Only some DisplayPort names coincide. The comment at `ddc.rs:268` assumes the suffixes match.
- Minor: `target.components().find(...)` takes the *first* `i2c-N` path component; for muxed adapters the bus is the last one.

Fix: match by EDID, which is stable across drivers. Read `/sys/class/drm/<connector>/edid`; read the EDID from each answering bus at I2C address 0x50 (128 bytes) during the probe; on X11 read the output's `EDID` RandR property. Key the capability set by an EDID hash (bytes 8–17: manufacturer, product, serial) and look displays up by that. Keep a conservative fallback: if no mapping can be made but the number of answering buses equals the number of connected external connectors, treat all externals as capable. Add unit tests with a fake sysfs tree for: no `ddc` link, `HDMI-A-1` vs `HDMI-1`, and two cards with the same suffix.

### H6. macOS: a modal alert at every launch

`AppDelegate.swift:176-181` shows `presentBrightnessRecoveryNotice()` when `requiresBrightnessRecoverySetup` is true, which is: software enabled (default) and gamma available (nearly always) and no registered shortcut (default nil). Every default-settings user of the full build, including those with working DDC who do not need software dimming, gets a focus-stealing modal (`NSApp.activate(ignoringOtherApps: true)`) each time the app starts, including at login. The doc comment on `restoreBrightnessShortcut` shows this was meant for the sandbox flavour.

Fix: show it at most once per install and only when brightness would otherwise be unavailable.

```swift
if viewModel.requiresBrightnessRecoverySetup,
   !viewModel.hardwareBrightnessControlEnabled,
   !installationStore.hasSeen(identifier: "recovery-notice-v1") {
    installationStore.markSeen(identifier: "recovery-notice-v1")
    DispatchQueue.main.async { [weak self] in self?.presentBrightnessRecoveryNotice() }
}
```

The info button in the menu already gives access to the same explanation on demand.

---

## Medium severity

### M1. Split-mode hot keys cannot reach the hardware backlight

`increaseBrightness`/`decreaseBrightness` (`MainViewModel.swift:510-534`), `NudgeBrightness` (`MainViewModel.cs:319`) and `increase_brightness`/`decrease_brightness` (`state.rs:363`) adjust software whenever software is enabled and fall to hardware only when it is not. With both engines on, "brightness up" at software 100 does nothing and the backlight is unreachable from the keyboard; "brightness down" dims only the image.

Fix: cascade in the same order as the unified slider.

```swift
func stepSplit(_ delta: Int) {
    if delta > 0 {
        if softwareBrightnessControlEnabled, softwareBrightness < 100 { softwareBrightness += delta }
        else if hardwareBrightnessControlEnabled { hardwareBrightness += delta }
    } else {
        if hardwareBrightnessControlEnabled, hardwareBrightness > 1 { hardwareBrightness += delta }
        else if softwareBrightnessControlEnabled { softwareBrightness += delta }
    }
}
```

### M2. Disclosure and registration change the actual brightness

- `splitBrightnessControls` is presented as a view preference ("Show split sliders in the menu by default", a chevron, an `Expander`), but it switches the routing. Expanding the panel instantly applies `manualHardwareBrightness`/`manualSoftwareBrightness` (defaults 80/100) in place of the unified mapping. A user at unified 20% (backlight 1%, shade about 40%) who clicks the chevron jumps to backlight 80%, shade 100%.
- The meaning of `manualBrightness` depends on which engines are usable. With hardware only, 80 means backlight 80%. Once the recovery shortcut registers, 80 means backlight 60%. Registering or losing the shortcut, or a display hot-plug, therefore moves the backlight with no slider movement.

Fix: convert on every mode transition so output is continuous.

- Entering split: compute the unified mapping's current hardware and software outputs and store them as the split values. Leaving split: invert (`hardware > 1` → `master = 50 + (hardware − 1) × 50 / 99`; otherwise `master = 1 + (software − 1) × 49 / 99`).
- When the usable-engine set changes, re-derive `manualBrightness` from the outputs that were on screen under the old set.

Put both conversions in the pure core next to the forward mapping and pin them with vectors (M9).

### M3. Windows: UI thread contends on the display worker's lock

`SupportsSoftwareBrightness` (`ColorTemperatureService.cs:63-70`) takes `_gammaGate`, the lock the worker holds across `MagSetFullscreenColorEffect` / `SetDeviceGammaRamp`. `HasSoftwareBrightnessSupport` is read several times per `ApplyBrightnessSettings` and by most of the 16 notifications in `NotifyBrightnessProperties`, on the UI thread, on every slider tick. The file's own header says driver calls can stall for seconds; when that happens the tray UI now freezes with them. On the legacy path each read also performs `GetDeviceGammaRamp` on every monitor.

Fix: publish the capability as a cached field.

```csharp
private volatile bool _supportsSoftwareBrightness = true;
public bool SupportsSoftwareBrightness => _supportsSoftwareBrightness;
// worker: after InvalidateOnWorker() and after each apply
_supportsSoftwareBrightness = _useMagnification || CanApplyGammaToAllMonitors();
```

Mark `_useMagnification` volatile or only touch it on the worker.

### M4. Windows legacy gamma path: one refusing monitor now blocks colour temperature for all

`ApplyToMonitors` (`ColorTemperatureService.cs:425-493`) was changed to roll back every monitor if any one fails, for the sake of uniform software brightness. But this function also carries colour temperature. On the gamma path (Windows 7, or Magnification unavailable), a single display that refuses ramps (USB/DisplayLink adapters, remote or virtual displays) now means no monitor is ever tinted. Because the loop never abandons a target, it retries every 50–250 ms: write to the good monitors, roll back, repeat. That is a visible flicker with no end. It also adds a `GetDeviceGammaRamp` per monitor per frame during fades.

Fix: keep colour temperature best-effort and make only the brightness component all-or-nothing. Build two ramps; if not every monitor can take a ramp, apply the temperature-only ramp to those that can and report software brightness as unsupported (which the M3 cached flag then exposes). Capture the "previous" ramp only on the first write per monitor, as before.

### M5. Windows: `DetectInternalPanel` exposes a transient "no hardware" state

`BrightnessService.cs:62-84` now resets `InternalPanelAvailable = false` and the count to 0 *before* running the WMI query, and `RefreshDisplaysAsync` runs it on a pool thread. For the duration of the query (hundreds of milliseconds, right after the popup opens) the UI thread sees no internal panel: the slider binding is disabled, a drag is dropped, or `ApplyBrightnessSettings` takes the "no hardware" branch. The fields are also unsynchronised across threads.

Fix: compute into locals and publish once.

```csharp
int count = 0;
try { /* query, count++ */ } catch { count = 0; }
lock (_stateGate) { ActiveWmiBrightnessDisplayCount = count; InternalPanelAvailable = count > 0; }
```

### M6. Linux: wlr gamma now fails closed for the whole session

`wlr.rs:160-166` returns an error when any output lacks a gamma control ("not every active output supports gamma control"). `WlrGammaService` is the colour-temperature backend, so one headless/virtual output, or one output whose gamma another client holds, now disables colour temperature on every output. Before, the capable outputs were tinted.

Fix: keep the service alive with the outputs that work, and expose the all-outputs fact separately for brightness:

```rust
let all_outputs_controlled = state.controls.len() == active_output_count;
// store it; supports_software_brightness() returns it; colour temperature ignores it
```

### M7. Linux: no shortcut backend means no software brightness, ever

When no backend is available, `shortcuts.rs:209-213` reports the recovery key as unregistered and software dimming stays locked. That is the normal situation on sway, river and other wlroots compositors without the GlobalShortcuts portal, which are exactly the sessions served by `WlrSoftwareBrightnessService`. Those users had software brightness before this change. The status text even tells them to bind the CLI commands in their compositor config, and `moonglow --restore-brightness` exists, but nothing lets them say they did.

Fix: add an explicit acknowledgement for backend-less sessions, for example a persisted `restoreBrightnessBoundExternally` flag set by a Preferences checkbox ("I have bound `moonglow --restore-brightness` to a key") or by the first successful IPC `restore-brightness` call, and treat it as satisfying the gate only when the backend is `Unavailable`. With the decided 50% software floor (design section below) these users get dimming down to 50% without any acknowledgement; the flag is then only needed to unlock the range below 50%.

### M8. macOS menu header says "Setup required" while hardware works

`MenuSliderRow.swift:88` shows "Setup required" whenever `requiresBrightnessRecoverySetup` is true, which includes users whose hardware slider is fully functional. They lose the percentage readout.

Fix: `else if viewModel.requiresBrightnessRecoverySetup && !viewModel.hardwareBrightnessControlEnabled`.

### M9. The new contract is not pinned by the spec

The repository's stated defence against drift is `spec/`. This change adds behaviour in three hand-written copies and pins almost none of it.

- `apps/windows/Moonglow.Core.Tests/SpecConformanceTests.cs` and `apps/linux/crates/moonglow-core/tests/spec_conformance.rs` were not updated; neither asserts the five new settings, so the new vectors only exercise macOS. (Plan step 4 lists the Linux file.)
- The unified-slider mapping exists as three implementations with three rounding styles (`.rounded()`, `Math.Round` with banker's rounding, integer `+25`/`+24`). I checked that they agree for all inputs today, including the one exact half at master = 75, but nothing enforces it.
- Routing (which engines are usable for a given display set, settings and shortcut state) has no vectors.
- `restoreBrightnessShortcut` normalisation is unpinned: it takes priority in duplicate removal on all platforms; an unparseable value becomes null on Windows.
- `spec/README.md` still says there are four shortcut keys and describes only macOS and Linux representations; it does not mention the brightness settings.
- Linux defines a default for the recovery action (`shortcut.rs:435`, Ctrl+Alt+Home) while the schema and settings say there is none.

Fix: add `spec/vectors/brightness-routing.json` with rows of `{ master | hardware/software, split, hardwareEnabled, softwareEnabled, allDisplaysHardware, softwareAvailable, recoveryRegistered } → { hardwarePercent | null (unmanaged), softwarePercent }`, including the conversions from M2. Implement it as one pure function per platform (`BrightnessPlan` in MoonglowKit, `Moonglow.Core`, `moonglow-core`) and load the vectors in all three conformance suites. On Windows this moves the logic out of the WPF view model into `Moonglow.Core`, where it can be tested without a desktop session. Extend `validate.py` to check the vector file's internal consistency.

### M10. `system.md` does not describe Moonglow

`AGENTS.md` makes `system.md` the architectural source of truth and requires it to be updated with any behaviour or contract change. Its current content describes a "Muxage Shell & Relay Bar", PTY multiplexing and a document relay engine; nothing in it relates to this repository, and the dual-brightness change did not touch it.

Fix: replace it with the real architecture: the three apps, `spec/` as the contract, and per platform the settings store → state/view model → {schedule, colour service, brightness router → hardware/software backends, hot-key service}, with a Mermaid diagram. The routing diagram in `docs/plans/dual-brightness-control.md` is a good starting point once H2 is decided.

---

## Low severity and cleanup

- **L1.** `brightnessControlEnabled` (`MainViewModel.swift:119-124`) has two identical branches.
- **L2.** Dead API after the refactor: `setBrightness(percent:allowSoftwareFallback:)` (`ServiceProtocols.swift:19, 45-51, 85-98`) and `set_brightness_with_software_fallback` (`traits.rs:46, 105`) are no longer called by the state layer. Remove them or route through them; two parallel routing paths will drift.
- **L3.** `Hotkey.DefaultFor(HotkeySlot.RestoreBrightness)` throws (`Hotkey.cs:44`) and `DefaultFor` has no callers. Delete the method, or return `string?`.
- **L4.** Linux `brightness_status` (`state.rs:172`) checks software support before the mixed-display case, so the message "At least one display lacks hardware brightness…" that macOS and Windows show can never appear, and the DDC permission detail is hidden (see T1).
- **L5.** macOS Intel: `IntelDDCBrightnessService(displayID: CGMainDisplayID())` (`AppDelegate.swift:102`) captures the main display at launch; after the user changes the main display, writes go to the old one while the UI describes the new one. Resolve the id at probe time. `applyPendingBrightness` (`IntelDDC.swift:283-286`) also has no recovery after repeated write failures, unlike the Apple Silicon path.
- **L6.** macOS Apple Silicon: `IOAVServiceCreate(nil)` (`NativeBridge.swift:355`) returns the default external service, not necessarily the main display's. With two externals the write can land on the other monitor. Creating services per display (`IOAVServiceCreateWithService` on the matching `DCPAVServiceProxy`) fixes this and is the prerequisite for per-display routing in H2.
- **L7.** `--sandbox-gamma-probe` overwrites the user's settings with defaults (`AppDelegate.swift:250`). Use a temporary store for the probe.
- **L8.** Linux `refresh_labels` calls `brightness_split_expander.set_expanded(split)` (`settings_ui.rs:1703`) while the model is mutably borrowed. If the value ever differs, the `expanded` notify handler runs `model.borrow_mut()` and panics. `push_to_widgets` already sets it under `suppressed`; drop the call from `refresh_labels`.
- **L9.** `gtk::MessageDialog` (`settings_ui.rs:1301`) is deprecated since GTK 4.10; use `adw::AlertDialog` (or `adw::MessageDialog` on older libadwaita).
- **L10.** Windows: the plan's 50% midpoint marker is not implemented in `TrayPopup.xaml`, and the header percentage (line 18) keeps showing the unified value while the split sliders are in use.
- **L11.** Windows: `ManagementObject` instances from the WMI queries are never disposed (`BrightnessService.cs:72, 97, 182`). Wrap each in `using`.
- **L12.** Linux IPC falls back to `/tmp/moonglow-<uid>.sock` when `XDG_RUNTIME_DIR` is unset (`ipc.rs:22-30`). Another local user can pre-create that path. Use a 0700 per-user directory and verify its owner before binding.
- **L13.** macOS `DisplayInfo.supportsBrightness` means "primary external" when DDC is up and "has gamma" when it is not (`NativeBridge.swift:123-125`), so the Displays list contradicts the new routing. Derive it as `supportsHardwareBrightness || supportsSoftwareBrightness`, as Linux now does.
- **L14.** Repository hygiene: `20261027_075028.jpg` (2.3 MB) is tracked at the root; the root `README.md` still lists Linux as "Rust (eframe)" though the UI is GTK4/libadwaita; `resources/logo-proposals/` and `docs/` are untracked and should be either committed or ignored deliberately.

---

## Design change (decided 2026-11-16): a 50% software floor instead of a mandatory hot key

H1, H2, H6, M7 and M8 all come from one decision: software dimming is unavailable until a global shortcut is registered. That gate is strict (registration can fail, is asynchronous on Linux, is impossible on some desktops) and its failure mode is "the feature the user had is gone".

The risk being guarded against is an unreadable screen. A floor guards against that directly.

**Decision from the project owner: the minimum for software dimming is 50%.**

- Without a registered recovery shortcut, software dimming is available but never goes below 50%.
- With a registered shortcut, the full 1–100% range is allowed, as now. *This second point is my reading of the decision, carried over from the original proposal. If 50% is meant as an unconditional floor, drop the shortcut condition below, and the registration tracking (`registeredHotKeys`, `SetRestoreBrightnessHotkeyRegistered`, `set_brightness_recovery_shortcut_registered`) can be removed as a gate and kept only as a convenience key.*
- Keep the menu's "Restore Brightness to 100%" item visible whenever software dimming is below 100, regardless of the shortcut.

### How to implement

1. **Spec constant.** Add `softwareBrightnessFloor: 50` to the contract: a named constant in each core (`AppSettings.softwareBrightnessFloor`, `AppSettings.SoftwareBrightnessFloor`, `AppSettings::SOFTWARE_BRIGHTNESS_FLOOR`) and a line in `spec/README.md`. Leave the schema range of `manualSoftwareBrightness` at 1–100; the floor is applied at routing time, so a saved lower value survives and takes effect again once the shortcut registers.

2. **Replace the boolean gate with a minimum** in the three apply functions (`applyBrightnessSettings`, `ApplyBrightnessSettings`, `apply_brightness_with_recovery_override`). Software is usable whenever it is enabled and supported; the shortcut only chooses the floor.

   ```swift
   let canSoftware = settings.softwareBrightnessEnabled && hasSoftwareBrightnessSupport
   let floor = hasBrightnessRecoveryShortcut ? 1 : AppSettings.softwareBrightnessFloor   // 50
   ```

3. **Mapping per mode** (with `floor` as above):

   | Mode | Slider value | Hardware | Software |
   | --- | --- | --- | --- |
   | Unified, both engines | master 50–100 | `1 + round((master − 50) × 99 / 50)` | 100 |
   | Unified, both engines | master 1–49 | 1 | `floor + round((master − 1) × (99 − floor) / 48)` |
   | Unified, software only | master | not written | `max(master, floor)` |
   | Unified, hardware only | master | master | 100 |
   | Split | each slider | `manualHardwareBrightness` | `max(manualSoftwareBrightness, floor)` |

   With `floor = 50` the lower half of the unified slider runs from software 99% at master 49 down to 50% at master 1. With `floor = 1` the second row should stay the existing formula `1 + round((master − 1) × 99 / 49)` so current behaviour with a registered shortcut is unchanged; branch on the floor rather than forcing one expression to cover both.

4. **Sliders show what is applied.** In software-only and split modes, set the control's minimum to the floor (macOS `TriangleSlider` range, WPF `Slider.Minimum`, GTK `Adjustment` lower bound) so the handle cannot sit below the applied value, and present `max(saved, floor)` in the getters. Hot-key steps clamp to the same bound.

5. **Remove the lock machinery this replaces.** Delete the "reset to 100" blocks from H1 outright (they have no purpose once software is never locked), the "Setup required" label (M8), and the launch alert (H6). `requiresBrightnessRecoverySetup` becomes an optional hint: "Register a restore shortcut to dim below 50%."

6. **Status text.** Replace the "Software brightness is locked…" strings on all three platforms with "Software dimming is limited to 50% until a restore shortcut is registered."

7. **Recovery override.** `restoreBrightnessToFull` keeps working as is; `allowSoftwareWithoutRecovery` is no longer needed and can be removed.

Effect on the other findings: existing software-only users on Linux and macOS keep working after the upgrade, nothing needs resetting, M7 (desktops with no shortcut backend) reduces to "cannot dim below 50%", and the shortcut becomes an unlock for deep dimming. Add the floor rows to the routing vectors from M9: at minimum master 1, 25 and 49 in both-engines mode, 30 in software-only mode, and split software 20, each with and without a registered shortcut.

## Other suggestions

1. **Migration note.** Whatever is decided for H2, the first launch after upgrade should not change what is on screen. Add a one-time migration that leaves `manualBrightness` meaning what it meant (hardware percent for hardware users, gamma percent for software users) and write the behaviour change into the release notes.
2. **Debounce hardware writes at the router.** The unified slider now issues a hardware write and a software write per tick; in the software half the hardware write is always "1". Skip writes whose value did not change (keep `lastHardware`/`lastSoftware` in the router). The macOS and Linux gamma services already cache; the DDC paths do not.
3. **Sandbox flavour in CI.** `MOONGLOW_APP_SANDBOXED` gates sizeable `#if` regions. Add `MOONGLOW_APP_SANDBOXED=1 swift build` (and `swift test`; confirm `AppleSiliconBrightnessServiceTests` is excluded under that flag) to `macos.yml` so the flavour cannot rot. Pass `--entitlements` only when signing the bundle; `build-app.sh` currently also applies them to the inner executable signature at line 160, which is redundant.
4. **Perceptual scale for the shade.** Gamma scaling is linear in signal, so the lower half of the slider changes perceived brightness much faster than the upper half. Mapping the software range through an approximate 2.2 power curve would make the unified slider feel continuous across the 50% handoff.

## Tests to add

- Router vectors (M9) on all three platforms, including the floor and the mode-transition conversions.
- H1: saved value survives lock/unlock, late probe, shortcut re-record, portal session re-creation.
- H3: no hardware write occurs when hardware is not the active route and was never owned.
- M1: split-mode key presses cascade software → hardware and back.
- Linux H5: fake sysfs trees (no `ddc` link, HDMI naming, duplicate suffixes on two cards, lid closed).
- Windows M4: with `useMagnification: false` and one refusing monitor, colour temperature still reaches the others and the loop goes idle.
- Windows M5: `HasHardwareBrightnessSupport` never reads false during a refresh when the panel is present.

## Suggested order of work

1. H1 (small, removes data loss) together with H6 and M8.
2. H3 and H4 (stop fighting the OS and the monitor).
3. Implement the 50% software floor (decided) inside a pure router with vectors (M9), which also resolves M1 and M2 and gives a place to fix T1 properly. H2 (per-display routing vs all-or-nothing) is still open and needs a decision before the router's display rules are final.
4. H5, M6, M7 on Linux; M3, M4, M5 on Windows.
5. Rewrite `system.md` (M10), update `spec/README.md` and the root README, then the Low items.
