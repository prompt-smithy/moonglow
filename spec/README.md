# Shared spec

The three apps in `apps/` are separate implementations in three languages, so there is no
shared source code. What they *do* share is behaviour, and this directory is the contract
for it: one settings shape, one colour-temperature curve, one schedule, and golden vectors
that every platform is tested against.

```
spec/
├── settings.schema.json          canonical persisted settings shape
├── validate.py                   self-check: schema is valid, vectors are consistent
└── vectors/
    ├── kelvin.json               kelvin → normalized RGB channel scale
    ├── schedule.json             schedule evaluation at fixed times
    └── temperature-range.json    snapping and clamping
```

## Why this exists

Each port claims to be derived from another:

- `apps/macos/Sources/MoonglowKit/Models/AppSettings.swift` — "Ported from `Models/AppSettings.cs`"
- `apps/linux/crates/moonglow-core/src/settings.rs` — "Ported from `Models/AppSettings.swift`"

That provenance is circular, and the constants have already drifted. Until the defaults are
reconciled, the schema defaults below are the target and the drift is listed as known
divergence.

| Setting | Canonical (schema) | Windows today | Linux today |
| --- | --- | --- | --- |
| `nightColorTempK` | 1200 | 3500 | 1200 |
| `manualBrightness` | 80 | 60 | 80 |
| `morningStart` / `morningEnd` | 06:00 / 07:00 | 07:00 / 09:00 | 06:00 / 07:00 |
| `eveningStart` / `eveningEnd` | 17:30 / 18:30 | 18:00 / 22:00 | 17:30 / 18:30 |
| `launchAtLogin` key | present | not persisted (read from registry) | present |

Ranges, the 100 K step, and the five-state schedule model already agree everywhere.

## Conformance

Each platform ships a test that loads these files and asserts against them. A change to the
contract lands here first, then in the three implementations.

| Platform | Test |
| --- | --- |
| macOS | `apps/macos/Tests/MoonglowKitTests/SpecConformanceTests.swift` |
| Linux | `apps/linux/crates/moonglow-core/tests/spec_conformance.rs` |
| Windows | not yet — the WPF project has no test project (see below) |

Tolerances: kelvin channel scales are compared to `1e-6` because `log`/`pow` may differ in
the last bits between runtimes. Schedule results are integers and must match exactly.

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

## Windows gap

`apps/windows` has no automated test project, so it cannot read the vectors yet. Closing the
gap means extracting the pure logic (`Models/AppSettings.cs`, `Services/ColorTemperatureService.cs`,
`Services/ScheduleService.cs`) into a `net48` class library that the WPF app references, then
adding an xUnit project against it. That is a refactor, not a rename, so it is deliberately
left as the next step.
