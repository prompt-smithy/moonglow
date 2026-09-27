import Foundation

/// The half of the day an hour falls in, as shown by a twelve-hour clock.
///
/// AppKit has no `ClockIdentifier` equivalent — 24-hour display is a locale property in
/// Foundation — so the two clock formats are modelled explicitly and the view layer
/// renders whichever the user has selected.
public enum Meridiem: String, CaseIterable, Identifiable, Sendable {
    case am
    case pm

    public var id: String { rawValue }

    /// `AM` / `PM`, as displayed next to the hour.
    public var displayName: String { rawValue.uppercased() }

    /// The other half of the day.
    public var toggled: Meridiem { self == .am ? .pm : .am }

    /// First hour on the 24-hour clock for this half of the day.
    public var startHour: Int { self == .am ? 0 : 12 }
}

/// A wall-clock time as read on a twelve-hour clock: `1...12`, minutes, and a meridiem.
///
/// This is a *presentation* type. Persistence and all schedule maths stay on the
/// 24-hour ``TimeOfDay``, so a locale-driven format change can never alter a saved time.
public struct TwelveHourTime: Equatable, Hashable, Sendable {

    /// Hours shown by the picker, in display order.
    public static let hourValues = Array(1...12)

    /// Hours in a valid twelve-hour reading: `1...12`, never `0`.
    public static let hourRange = 1...12

    /// Minutes in a valid reading.
    public static let minuteRange = 0...59

    public var hour: Int
    public var minute: Int
    public var meridiem: Meridiem

    public init(hour: Int, minute: Int, meridiem: Meridiem) {
        self.hour = hour
        self.minute = minute
        self.meridiem = meridiem
    }

    /// Creates a reading only when every component is in range.
    public init?(validatingHour hour: Int, minute: Int, meridiem: Meridiem) {
        guard TwelveHourTime.isValid(hour: hour, minute: minute) else { return nil }
        self.init(hour: hour, minute: minute, meridiem: meridiem)
    }

    public static func isValid(hour: Int, minute: Int) -> Bool {
        hourRange.contains(hour) && minuteRange.contains(minute)
    }

    public var isValid: Bool { TwelveHourTime.isValid(hour: hour, minute: minute) }

    /// `6:00 AM` — no leading zero on the hour, minutes always two digits. This is the
    /// conventional reading on macOS and matches the Avalonia `TimePicker`.
    public var displayText: String {
        "\(hour):\(String(format: "%02d", minute)) \(meridiem.displayName)"
    }

    /// Returns the same reading with a different meridiem, keeping the 12-hour value
    /// (so `11 AM` becomes `11 PM`, which is what a meridiem toggle should do).
    public func with(meridiem: Meridiem) -> TwelveHourTime {
        TwelveHourTime(hour: hour, minute: minute, meridiem: meridiem)
    }
}

// MARK: - Bridging to the persisted 24-hour representation

extension TimeOfDay {

    /// Minute values shown by the picker.
    public static let minuteValues = Array(0...59)

    /// This time as a twelve-hour reading.
    ///
    /// `00:30` reads as `12:30 AM` and `12:30` as `12:30 PM`, which is the part a naive
    /// `hour % 12` conversion gets wrong.
    public var twelveHour: TwelveHourTime {
        let meridiem: Meridiem = hour < 12 ? .am : .pm
        let hour12 = hour % 12 == 0 ? 12 : hour % 12
        return TwelveHourTime(hour: hour12, minute: minute, meridiem: meridiem)
    }

    /// Builds a time from a twelve-hour reading. Returns `nil` for an out-of-range
    /// reading rather than silently wrapping, so a bad picker value cannot shift the
    /// schedule by twelve hours.
    public init?(_ twelveHour: TwelveHourTime) {
        guard twelveHour.isValid else { return nil }

        let hour24 = twelveHour.meridiem == .am
            ? twelveHour.hour % 12
            : (twelveHour.hour % 12) + 12

        self.init(hour: hour24, minute: twelveHour.minute)
    }

    /// The reading to show for this time, always twelve-hour: `6:00 AM`. Persistence stays
    /// `HH:mm` through ``storageDescription``, so the displayed format can never change what
    /// is stored.
    public var displayText: String { twelveHour.displayText }
}
