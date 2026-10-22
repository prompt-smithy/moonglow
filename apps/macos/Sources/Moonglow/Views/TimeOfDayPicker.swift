import SwiftUI
import MoonglowKit

/// Picks a ``TimeOfDay`` with three palette-drawn drop-downs: hour, minute, and AM/PM.
///
/// Each drop-down shows one value at rest and opens a scrolling list. They are ``ThemedDropDown``s
/// rather than native menus or AppKit's `NSDatePicker` because every system control in this slot
/// paints itself with the *system* accent, which reads as system blue inside the app palette:
///
/// - A native menu paints its bezel and its selected row with the system accent, and `NSMenu`
///   highlighting follows the macOS accent setting, so `.tint` cannot repaint it.
/// - `NSDatePicker` — the control System Settings uses for the Night Shift schedule — paints the
///   clicked segment through `NSDatePickerCell`. That cell never attaches the window's field
///   editor, so `selectedTextAttributes` cannot reach the highlight, the focus ring and control
///   colours do not affect it, and there is no editable switch to suppress it.
///
/// Moonglow is twelve-hour only, so AM/PM is always available. The bound value stays the 24-hour
/// ``TimeOfDay``, so the displayed format can never move a scheduled time.
struct TimeOfDayPicker: View {

    @Binding var time: TimeOfDay
    let theme: AppTheme

    var body: some View {
        HStack(spacing: 6) {
            ThemedDropDown(
                options: TwelveHourTime.hourValues,
                selection: hourBinding,
                theme: theme,
                accessibilityName: "Hour",
                title: { String($0) },
                fieldWidth: 58,
                listWidth: 76
            )

            ThemedDropDown(
                options: TimeOfDay.minuteValues,
                selection: minuteBinding,
                theme: theme,
                accessibilityName: "Minute",
                title: { String(format: "%02d", $0) },
                fieldWidth: 58,
                listWidth: 76
            )

            ThemedDropDown(
                options: Meridiem.allCases,
                selection: meridiemBinding,
                theme: theme,
                accessibilityName: "AM/PM",
                title: { $0.displayName },
                fieldWidth: 68,
                listWidth: 90
            )
        }
    }

    // MARK: - Bindings

    private var hourBinding: Binding<Int> {
        Binding(
            get: { time.twelveHour.hour },
            set: { newHour in
                let current = time.twelveHour
                guard newHour != current.hour else { return }

                rebuild(
                    TwelveHourTime(
                        hour: newHour,
                        minute: current.minute,
                        meridiem: current.meridiem
                    )
                )
            }
        )
    }

    private var minuteBinding: Binding<Int> {
        Binding(
            get: { time.minute },
            set: { newMinute in
                guard newMinute != time.minute else { return }

                let current = time.twelveHour
                rebuild(
                    TwelveHourTime(
                        hour: current.hour,
                        minute: newMinute,
                        meridiem: current.meridiem
                    )
                )
            }
        )
    }

    private var meridiemBinding: Binding<Meridiem> {
        Binding(
            get: { time.twelveHour.meridiem },
            set: { newMeridiem in
                let current = time.twelveHour
                guard newMeridiem != current.meridiem else { return }
                rebuild(current.with(meridiem: newMeridiem))
            }
        )
    }

    /// Rebuilds the 24-hour value from twelve-hour components, ignoring an out-of-range
    /// reading rather than writing a wrapped time.
    private func rebuild(_ twelveHour: TwelveHourTime) {
        if let rebuilt = TimeOfDay(twelveHour) {
            time = rebuilt
        }
    }
}
