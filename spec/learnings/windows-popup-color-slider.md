# Learning: Windows popup color slider (October 2026)

After the three repos were merged into this monorepo, the Windows popup color
slider broke in three independent ways. They looked like one bug ("the slider
feels wrong") but had three separate causes, found one at a time over several
rounds of device logs and a screen recording. This note records each cause, the
fix, and the guard that stops it coming back.

## 1. The track gradient must not be copied from macOS

**Symptom.** The number on the slider ran lowest-to-highest left to right, but
the felt warmth bottomed out in the middle and rose toward both ends.

**Cause.** The Windows `TemperatureRangeGradient` had been replaced with the
macOS ramp: two stops, `#FFA94D` → `#8FB8FF`
(`apps/macos/Sources/Moonglow/Views/TemperatureGradient.swift`). A two-stop
orange-to-blue interpolation passes through near-grey mid-track (measured
saturation ≈ 0.06 there, vs ≈ 0.6 at the ends), so the middle of the slider
reads as the weakest point even though the numbers are monotonic.

**Fix.** Restored the original Windows four-stop saturated ramp in
`apps/windows/App.xaml`, warm end first, day end last:

| Offset | Color     |
| ------ | --------- |
| 0      | `#E87925` |
| 0.48   | `#F0B479` |
| 0.72   | `#9BB9D7` |
| 1      | `#5E91C2` |

Every stop keeps saturation above 0.25, so hue — not brightness — carries the
direction along the whole track.

**Rule going forward.** Windows keeps its own four-stop ramp. Do not paste the
macOS two-stop pair (`#FFA94D` / `#8FB8FF`) into `App.xaml`, even though both
ramps nominally mean "warm to cool": the endpoints match, but the journeys
between them do not, and the grey middle breaks the control. This is enforced
by `TemperatureGradientTests.GradientStopsAreWarmToCoolWithoutGreyMiddle` in
`apps/windows/Moonglow.Core.Tests`, which pins the exact stops, rejects any
near-grey stop, rejects the macOS endpoint pair outright, and runs in CI on
every change under `apps/windows/**` or `spec/**` (see
`.github/workflows/windows.yml`).

## 2. The driver can refuse warm gamma ramps; the fallback must actually land

**Symptom.** The number tracked the thumb, but the screen tint did not: 6000 K
could show deep red, 2700 K near-neutral. With Night Light off.

**Cause (a).** The display driver deterministically rejects warm gamma ramps
(below roughly 3200–3300 K on the affected machine) while accepting cool ones,
so the GDI gamma path alone cannot serve the warm end.

**Cause (b).** The Magnification-API composition fallback, which exists for
exactly this case, failed with `ERROR_NOT_READY` on every worker thread except
the first one that initialized it. Pool-thread rotation therefore starved it.

**Fix** (`apps/windows/Services/ColorTemperatureService.cs`).
- All compositor calls run on one dedicated `ColorApply` thread, so every call
  happens where the runtime was initialized.
- The hardware ramp is restored only *after* the fallback takes over, so a
  refused fallback keeps the last tint instead of flashing neutral.
- Fallback updates are paced; a refused update rejoins a retry loop keyed by
  target value (a fresh value must never inherit an expired stall timer), with
  200 ms backoff while moving and 2 s once stalled, and never abandons a
  clicked value.
- A refused fallback *clear* rejoins the loop too, or the two transforms stack.
- A `%Temp%\Moonglow-color.log` trail records queued vs applied kelvin, the
  path taken (`gamma` / `magnification` / `pending`), PID and thread id.

**Rule going forward.** Any change to the apply pipeline must be validated
against a real drag log on hardware that rejects warm ramps: every settled
value lands via one path or the other, on one stable thread id, with no
`pending` that never resolves. The log format is the contract for that check.

## 3. Two copies tinting one screen look like one broken slider

**Symptom.** Same as §2 — number moves, tint disagrees — but only while a
second copy ran.

**Cause.** The Amber → Moonglow rename changed the single-instance mutex
(`Local\Amber.SingleInstance` → `Local\Moonglow.SingleInstance`), so a
pre-merge build and the merged build (or two same-generation builds, which a
mutex alone once failed to exclude in practice) could run side by side. Each
owned the gamma ramps on its own schedule ticks. The two popups and tray icons
are visually identical, so the extra copy is easy to miss.

**Fix** (`apps/windows/App.xaml.cs`, `Moonglow.Core/InstanceNames.cs`,
`Moonglow.Core/InstanceGuard.cs`). The app holds the legacy Amber mutex for
its whole lifetime *and* scans running processes for the app family's
executable names (own launcher parent exempted) before touching hardware,
refusing to start beside another copy with a message instead of fighting over
the ramps. Guarded by `InstanceNamesTests` and `InstanceGuardTests`.

**Rule going forward.** Never rename the mutex without acquiring the old name
too, and never rely on a single exclusion mechanism for hardware-owning
singletons. A duplicate-launch report must show the message; a second tray
moon must be impossible.

## Checklist for the next "slider feels wrong" report

1. One copy running? (Task Manager → Details: a launcher plus its payload is
   one copy; two `Moonglow.exe` rows, or a `Moonglow*` plus an `Amber*`, is two.)
2. Drag the full range with the log running; every settled value must show an
   `apply … via=` line on one thread id.
3. The gradient stops in `App.xaml` must still be the four values in §1.
