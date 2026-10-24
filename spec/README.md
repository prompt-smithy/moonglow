# Shared spec

The three apps in `apps/` are separate implementations in three languages, so there is no
shared source code. What they *do* share is behaviour, and this directory is the contract
for it: one settings shape, one colour-temperature curve, one schedule, and golden vectors
that every platform is tested against.

```
spec/
├── settings.schema.json          canonical persisted settings shape
├── validate.py                   self-check: schema is valid, vectors are consistent
├── vectors/
│   ├── kelvin.json               kelvin → normalized RGB channel scale
│   ├── schedule.json             schedule evaluation at fixed times
│   └── temperature-range.json    snapping and clamping
└── learnings/
    └── windows-popup-color-slider.md   post-merge Windows slider bugs and the
                                        rules that stop them coming back
```

## Why this exists

Each port claims to be derived from another:

- `apps/macos/Sources/MoonglowKit/Models/AppSettings.swift` — "Ported from `Models/AppSettings.cs`"
- `apps/linux/crates/moonglow-core/src/settings.rs` — "Ported from `Models/AppSettings.swift`"

That provenance is circular, so the schema below is the target rather than whichever port
happened to be read first. The defaults are now reconciled across all three implementations.

| Setting | Canonical (schema) | Windows | Linux |
| --- | --- | --- | --- |
| `nightColorTempK` | 1200 | 1200 | 1200 |
| `manualBrightness` | 80 | 80 | 80 |
| `morningStart` / `morningEnd` | 06:00 / 07:00 | 06:00 / 07:00 | 06:00 / 07:00 |
| `eveningStart` / `eveningEnd` | 17:30 / 18:30 | 17:30 / 18:30 | 17:30 / 18:30 |
| `launchAtLogin` key | present | present | present |

Ranges, the 100 K step, the five-state schedule model, lenient decoding, and the
schedule-ordering clamp also agree everywhere.

Global shortcuts share the four setting keys but not their representation: macOS
stores `{keyCode, modifiers}`, while Linux stores a readable `"Ctrl+Alt+Up"`
string. Both are documented by the `hotKey` definition in the schema, are
platform-specific, and are not portable between operating systems. `null`
disables one action without disturbing the others.

## Conformance

Each platform ships a test that loads these files and asserts against them. A change to the
contract lands here first, then in the three implementations.

| Platform | Test |
| --- | --- |
| macOS | `apps/macos/Tests/MoonglowKitTests/SpecConformanceTests.swift` |
| Linux | `apps/linux/crates/moonglow-core/tests/spec_conformance.rs` |
| Windows | `apps/windows/Moonglow.Core.Tests/SpecConformanceTests.cs` |

Tolerances: kelvin channel scales are compared to `1e-6` because `log`/`pow` may differ in
the last bits between runtimes. Schedule results are integers and must match exactly.

## Learnings

`learnings/` records hard-won, platform-specific lessons that the vectors do
not cover — e.g. why the Windows slider keeps its own gradient ramp instead of
copying the macOS one, and why two app copies must never tint one screen.
Read the relevant note before porting UI values or hardware behaviour between
platforms; each note names the test that enforces it.

## Validating

`validate.py` checks that the schema is a valid JSON Schema and that the vectors are
internally consistent — outputs on the step grid, inside the configured range, ordered,
no duplicate inputs. It does not check any implementation; that is what the conformance
tests are for.

```bash
python3 spec/validate.py     # needs the `jsonschema` package for the meta-schema check
```

CI runs it on any change under `spec/`, and the platform workflows run their own
conformance suite on the same trigger.

## Windows layout

`apps/windows` keeps its pure logic in a `net48` class library, `Moonglow.Core`
(`AppSettings`, `ScheduleCalculator`, `KelvinConversion`, `TemperatureRange`), which the WPF
app references. `Moonglow.Core.Tests` is the xUnit project that loads these vectors; the WPF
layer holds only the native and UI code, so the conformance suite runs without a desktop
session or display hardware.
