import Foundation

/// The night-to-day colour temperature window.
///
/// The schedule moves between a warmer night target and a cooler day target. The settings
/// window edits both endpoints with a single two-handle control, so they are modelled here
/// as one value: every initializer and mutation keeps `night <= day` and snaps to the kelvin
/// step, which means the UI never has to defend the invariant itself.
public struct TemperatureRange: Equatable, Sendable {

    /// Warmer end of the window — the night target.
    public private(set) var night: Int

    /// Cooler end of the window — the day target.
    public private(set) var day: Int

    /// Builds a range from two raw values, snapping both and ordering them.
    public init(night: Int, day: Int) {
        let snappedNight = TemperatureRange.snap(night)
        let snappedDay = TemperatureRange.snap(day)

        self.night = min(snappedNight, snappedDay)
        self.day = max(snappedNight, snappedDay)
    }

    /// The window currently stored in settings.
    public init(settings: AppSettings) {
        self.init(night: settings.nightColorTempK, day: settings.dayColorTempK)
    }

    /// Snaps a raw value to the nearest kelvin step and clamps it to the supported range.
    ///
    /// Integer division rounds toward zero, so the halves are handled explicitly to match
    /// round-half-away-from-zero on both sides of it.
    public static func snap(_ value: Int) -> Int {
        let value = AppSettings.clampKelvin(value)
        let step = AppSettings.kelvinStep
        let quotient = value / step
        let remainder = value % step

        let rounded: Int
        if remainder * 2 >= step {
            rounded = quotient + 1
        } else if remainder * 2 <= -step {
            rounded = quotient - 1
        } else {
            rounded = quotient
        }
        return AppSettings.clampKelvin(rounded * step)
    }

    /// Moves the night handle. It stops at the day handle rather than crossing it, so
    /// dragging past the other thumb pins the handles together instead of swapping them.
    public func movingNight(to value: Int) -> TemperatureRange {
        TemperatureRange(night: min(TemperatureRange.snap(value), day), day: day)
    }

    /// Moves the day handle. It stops at the night handle rather than crossing it.
    public func movingDay(to value: Int) -> TemperatureRange {
        TemperatureRange(night: night, day: max(TemperatureRange.snap(value), night))
    }

    /// Distance between the handles, in kelvin.
    public var span: Int { day - night }

    /// `1200 K – 6500 K`, for the caption under the control.
    public var displayText: String { "\(night) K – \(day) K" }
}

/// Converts between kelvin values and positions along a linear track.
///
/// Kept separate from the SwiftUI control so the geometry maths is testable without a
/// window; the control only supplies pixels.
public enum TemperatureScale {

    /// Position of a value along the range, as `0...1`.
    public static func fraction(
        of value: Int,
        in range: ClosedRange<Int> = AppSettings.kelvinRange
    ) -> Double {
        let span = Double(range.upperBound - range.lowerBound)
        guard span > 0 else { return 0 }

        let offset = Double(AppSettings.clampKelvin(value) - range.lowerBound)
        return min(max(offset / span, 0), 1)
    }

    /// The snapped value at a `0...1` position. Positions beyond either end clamp.
    public static func value(
        atFraction fraction: Double,
        in range: ClosedRange<Int> = AppSettings.kelvinRange
    ) -> Int {
        let span = Double(range.upperBound - range.lowerBound)
        let raw = Double(range.lowerBound) + (min(max(fraction, 0), 1) * span)
        let snapped = TemperatureRange.snap(Int(raw.rounded()))

        return min(max(snapped, range.lowerBound), range.upperBound)
    }
}
