import Foundation

/// A wall-clock time of day, independent of any calendar date.
///
/// Ported from the `ParseTime` handling in `Models/AppSettings.cs`. The persisted
/// representation is a zero-padded `HH:mm` string so settings files stay readable.
public struct TimeOfDay: Equatable, Hashable, Comparable, Sendable {
    public var hour: Int
    public var minute: Int

    public init(hour: Int, minute: Int) {
        self.hour = hour
        self.minute = minute
    }

    /// Creates a time only when both components are in range.
    public init?(validatingHour hour: Int, minute: Int) {
        guard TimeOfDay.isValid(hour: hour, minute: minute) else { return nil }
        self.init(hour: hour, minute: minute)
    }

    /// Creates a time from a zero-padded or bare `H:mm` string.
    public init?(storageDescription: String) {
        let parts = storageDescription.split(separator: ":", omittingEmptySubsequences: false)
        guard parts.count == 2,
              let hour = Int(parts[0]),
              let minute = Int(parts[1]),
              TimeOfDay.isValid(hour: hour, minute: minute)
        else { return nil }

        self.init(hour: hour, minute: minute)
    }

    public static func isValid(hour: Int, minute: Int) -> Bool {
        (0...23).contains(hour) && (0...59).contains(minute)
    }

    /// Midnight, the start of the scheduling day.
    public static let midnight = TimeOfDay(hour: 0, minute: 0)

    /// Minutes elapsed since midnight. Used by the schedule comparison and interpolation.
    public var minutesSinceMidnight: Int { (hour * 60) + minute }

    public var isValid: Bool { TimeOfDay.isValid(hour: hour, minute: minute) }

    /// `HH:mm` representation used for persistence.
    public var storageDescription: String {
        String(format: "%02d:%02d", hour, minute)
    }

    /// A time built from minutes since midnight, wrapped into a single day.
    public static func from(minutesSinceMidnight minutes: Int) -> TimeOfDay {
        let wrapped = ((minutes % 1440) + 1440) % 1440
        return TimeOfDay(hour: wrapped / 60, minute: wrapped % 60)
    }

    public static func < (lhs: TimeOfDay, rhs: TimeOfDay) -> Bool {
        lhs.minutesSinceMidnight < rhs.minutesSinceMidnight
    }
}

extension TimeOfDay: Codable {
    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let raw = try container.decode(String.self)
        guard let parsed = TimeOfDay(storageDescription: raw) else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "Expected a \"HH:mm\" time, received \"\(raw)\"."
            )
        }
        self = parsed
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(storageDescription)
    }
}
