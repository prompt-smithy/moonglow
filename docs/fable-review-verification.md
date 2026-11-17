# Fable review verification

Audit completed 2026-11-17 against commit `41810a0` and
[`fable_review.md`](../fable_review.md). That commit did not resolve every numbered
finding: its portable tests passed, but the Linux app, sandbox tests and net48
brightness records still had build/test failures, and several runtime paths retained
reviewed defects. This follow-up fixes those gaps and records the remaining platform
boundaries below.

## Numbered findings

| Finding | Verified result |
| --- | --- |
| T1 | Swift, Rust workspace, Windows portable tests, sandbox tests and production builds pass. Fixed Linux app compile errors, sandbox default/test isolation, net48 record compatibility and strict Clippy failures. |
| H1 | Saved brightness survives pending/recreated shortcut registration. Software-only/split values below 50 are retained; pending deep unified preferences are retained through recovery loss and return. Only explicit restore writes full saved brightness. |
| H2 | Applied the review's minimum regression-preserving option: macOS/Linux keep hardware control on capable displays in mixed setups. Their unified mixed route controls those backlights; split Digital Shade can dim all gamma-capable displays. macOS still targets the primary external display. Windows retains complete-coverage routing. Full per-display gamma routing is a separate extension. |
| H3 | Never-owned hardware is not reset. Owned hardware is released once when its route ends; lost support clears ownership. Explicit restore takes precedence over a concurrent capability transition. |
| H4 | Windows startup probing stops when any hardware is found, hardware is disabled, or the 15-attempt budget is exhausted. |
| H5 | Linux matches responding I2C buses, DRM connectors and RandR outputs by EDID. Retained conservative count fallback, card-qualified identity and the final mux adapter. Added fixtures for missing DDC links, driver spelling differences, duplicate card suffixes and disabled panels. |
| H6 | No automatic recovery modal at launch. Recovery information remains available on demand and explains the 50% floor. |
| M1 | Split hotkeys cascade software upward before hardware, and hardware downward before software. |
| M2 | Disclosure captures the current route. Returning from independent split values uses their effective product; engine/floor transitions re-express the last output, subject to the safety floor and retained pending preferences. Shared vectors cover these conversions. |
| M3 | Windows software capability reads use the cached volatile field. Worker capability changes notify the UI asynchronously. A stalled-driver regression verifies that reading capability stays responsive. |
| M4 | Legacy Windows gamma keeps temperature on accepting monitors, removes partial software dimming, captures original ramps once and goes idle. Tests cover a refusing monitor with and without dimming. |
| M5 | WMI detection computes locally and publishes under the state lock. A blocked-query test verifies that support remains visible throughout refresh. |
| M6 | wlroots colour support requires any valid gamma output; uniform dimming requires every output. Failed controls retain stable callback indices and do not disable temperature on working outputs. |
| M7 | Backend-less Linux sessions can dim to 50%. An IPC restore command acknowledges an external compositor binding for deeper dimming; that acknowledgement is effective only while the application shortcut backend is unavailable. |
| M8 | macOS percentage readouts remain available; there is no “Setup required” replacement. |
| M9 | All three suites consume the routing/conversion and settings vectors. Added floor/transition cases and recovery priority, invalid-value and clearing cases. The validator now checks calculated outputs and transitions, not only ranges. |
| M10 | `system.md`, both architecture diagrams, shared contract and customer documentation describe the actual implementation. The old design plan is marked historical. |
| L1 | The duplicate macOS capability branches are removed. |
| L2 | Removed the unused Swift/Rust software-fallback gate APIs and recovery override arguments. State routing uses the pure brightness plan. |
| L3 | Windows `DefaultFor` returns nullable defaults, including no recovery shortcut. |
| L4 | Linux status preserves the DDC permission detail even when software dimming works. |
| L5 | Intel DDC resolves the current main display at probe time, retires changed targets and retries with a fresh connection after repeated failures. Intel production build passes. |
| L6 | Apple Silicon opens an external `DCPAVServiceProxy` matching the intended display's valid EDID, requires a unique match, retires a changed main-display target and publishes support after probing. Fake-transport tests cover identity and target changes. |
| L7 | The sandbox gamma probe uses a temporary settings store and cleans it up. Sandbox neutral first-launch defaults are injected by the shell, leaving core defaults canonical. |
| L8 | GTK model synchronization updates the expander under signal suppression. The native regression changes split mode externally and refreshes labels without a reentrant borrow. |
| L9 | Brightness information uses the supported libadwaita message dialog. |
| L10 | The Windows unified slider has a 50% marker, the split header reflects its engine levels, and software slider minima follow recovery capability. |
| L11 | WMI query objects are disposed in detection, read and write paths. |
| L12 | Linux sockets use a private owned runtime directory or private per-user temporary directory. Unsafe directory permissions, directory symlinks and lock-file symlinks are rejected and tested. |
| L13 | macOS combined display brightness capability is derived from hardware or software support. |
| L14 | The root photo is removed, Linux is documented as GTK4/libadwaita, and the resource/design files are tracked. Sandbox scratch output is ignored. |

The restore action is available while software is dimmed, including without a registered
shortcut. Help text and sliders consistently expose 50% as the default software minimum.
The sandbox flavour is built/tested in macOS CI; its entitlements are applied when signing
the app bundle. Linux CI installs native GTK dependencies and exercises the GTK controls
under Xvfb.

## Validation

| Check | Result |
| --- | --- |
| Standard macOS `swift test` | 153 tests pass; production app built |
| `MOONGLOW_APP_SANDBOXED=1 swift test` with isolated scratch path | 148 tests pass; sandbox app built; hardware-only suite excluded |
| Intel `swift build --triple x86_64-apple-macosx12.0` | Pass |
| Linux container `cargo test --workspace --no-fail-fast` | 177 tests pass; two desktop-dependent tests ignored by default |
| Native GTK control test under Xvfb/D-Bus | Pass, including split synchronization and software minimum |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass |
| `cargo fmt --all --check` | Pass |
| Windows net8 core tests | 21 pass |
| Windows portable platform tests | 32 pass, exercising production workers through fake native APIs |
| net48 WPF app and net48 core-test builds | Pass, zero warnings/errors |
| Spec validation with `jsonschema` installed | Pass; deliberate output/transition corruption is rejected |
| Workflow YAML, packaging shell syntax, `git diff --check` | Pass |

Validation used macOS and a Linux container. Windows WPF was compiled, but Windows UI/native
monitor operations and net48 test execution require a Windows/Mono runtime. The live GNOME
ownership test remains unrun because there is no GNOME desktop session; the GTK test was
run explicitly under Xvfb. Physical multi-monitor DDC/gamma behaviour still needs device
smoke testing. The optional perceptual scale and further hardware-write optimisation from
the review are product improvements rather than changes to the approved linear mapping.
