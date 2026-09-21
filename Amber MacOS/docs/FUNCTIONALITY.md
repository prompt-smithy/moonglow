# Functionality

Amber is a macOS menu-bar application for external-display hardware brightness and color temperature.

## Application lifecycle

- Amber starts with a menu-bar icon and an explicit-shutdown lifetime.
- Clicking the menu-bar icon toggles a compact quick-controls popup.
- The popup provides brightness and color-temperature sliders, an automatic-color toggle, and Settings and Quit actions.
- The popup color-temperature slider is disabled while automatic color mode is enabled.
- **Settings** hides the popup and opens the main settings window.
- Closing the settings window hides it; Amber continues running in the menu bar.
- **Quit** stops scheduling, restores ColorSync gamma settings, disposes native services, and exits.
- The current implementation does not yet register a login item or enforce a single process.

## Display discovery

Amber calls CoreGraphics to enumerate active display IDs and identify the main display. Each display reports independent capability flags:

- **Color temperature:** available when CoreGraphics reports a non-zero gamma-table capacity.
- **Brightness:** available on the primary external display when the DDC/CI startup probe succeeds.

Refresh re-enumerates active displays. Display names are currently generic because CoreGraphics IDs do not provide a friendly product name through the implemented API surface.

## Brightness

The UI and persisted value support `1..100%`. Values loaded from settings and values assigned through the view model are clamped to that range.

The brightness slider is enabled when Amber can exchange DDC/CI messages through the primary external display's CoreDisplay service. Slider changes are clamped to `1..100%`, coalesced briefly, and sent as MCCS luminance (`VCP 0x10`) writes.

The same brightness state and capability gate back both the quick-controls popup and the main settings window.

This changes the monitor's hardware luminance independently of CoreGraphics color-temperature gamma tables. The current backend targets one primary external display.

## Color temperature

Manual and scheduled targets support `900..6600` K in 100 K slider increments. Lower values are warmer; approximately 6500 K is neutral.

The quick-controls popup can toggle automatic mode directly. Its manual slider follows the same disabled state as the main settings slider while scheduling is active.

Amber converts Kelvin to normalized red, green, and blue scale factors, builds a gamma table for each capable display, and applies it with `CGSetDisplayTransferByTable`. On exit it calls `CGDisplayRestoreColorSyncSettings`.

Gamma behavior can vary with HDR, sleep/wake, display-mode changes, and other color-management software. macOS may reset gamma tables outside Amber's control.

## Automatic schedule

The scheduler evaluates every 10 seconds:

1. Night target before the morning transition.
2. Linear warming-to-cooling interpolation during the morning transition.
3. Day target between transitions.
4. Linear cooling-to-warming interpolation during the evening transition.
5. Night target after the evening transition.

Computed values are rounded to 100 K steps. Invalid persisted times fall back to `06:00`, `07:00`, `17:30`, and `18:30`.

Default values:

| Setting | Default |
| --- | --- |
| Manual brightness | 80% |
| Manual temperature | 6500 K |
| Day target | 6500 K |
| Night target | 1200 K |
| Automatic mode | Enabled |

## Themes

Amber follows the current Avalonia/macOS light or dark appearance and applies the saved palette for that mode.

Light theme order: Light Amber, Light Orange, Light Green, Light Violet, Light Grey, White.

Dark theme order: Dark Amber, Dark Orange, Dark Green, Dark Violet, Dark Grey, Black.

Fresh settings default to Light Amber and Dark Amber. Legacy dark names are normalized during settings load.

## Settings

Settings are JSON at:

```text
~/Library/Application Support/Amber/settings.json
```

If no Amber settings exist, the previous application settings file is imported once. Brightness, temperatures, and theme names are normalized before use. File-system failures fall back to defaults and do not crash the application.