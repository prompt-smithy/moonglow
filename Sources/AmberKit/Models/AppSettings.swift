import Foundation

/// Persisted application settings.
///
/// Ported from `Models/AppSettings.cs`. Range constants and defaults are preserved exactly.
/// The .NET `Use24HourClock` setting is not carried over: Night Mode Controller is twelve-hour only, so the
/// schedule pickers always show AM/PM. The on-disk representation is camelCase, and the legacy
/// .NET settings file is deliberately not read (this is a fresh start).
public struct AppSettings: Equatable, Sendable {

    // MARK: - Ranges

    /// Hardware brightness, in percent.
    public static let brightnessRange = 1...100
    /// Colour temperature in kelvin. Lower is warmer.
    public static let kelvinRange = 900...6600
    /// Slider step and schedule rounding step.
    public static let kelvinStep = 100

    // MARK: - Defaults

    public static let defaultMorningStart = TimeOfDay(hour: 6, minute: 0)
    public static let defaultMorningEnd = TimeOfDay(hour: 7, minute: 0)
    public static let defaultEveningStart = TimeOfDay(hour: 17, minute: 30)
    public static let defaultEveningEnd = TimeOfDay(hour: 18, minute: 30)

    public static let defaultDayColorTempK = 6500
    public static let defaultNightColorTempK = 1200
    public static let defaultManualColorTempK = 6500
    public static let defaultManualBrightness = 80

    // MARK: - Values

    public var dayColorTempK: Int = defaultDayColorTempK
    public var nightColorTempK: Int = defaultNightColorTempK
    public var morningStart: TimeOfDay = defaultMorningStart
    public var morningEnd: TimeOfDay = defaultMorningEnd
    public var eveningStart: TimeOfDay = defaultEveningStart
    public var eveningEnd: TimeOfDay = defaultEveningEnd
    public var autoColorTemperature: Bool = true
    public var manualBrightness: Int = defaultManualBrightness
    public var manualColorTempK: Int = defaultManualColorTempK
    /// Whether the user wants Night Mode Controller to start with the Mac.
    ///
    /// This records intent only. The actual registration lives in the system's login items, which
    /// the user can change outside the app, so it is re-asserted at launch rather than trusted.
    public var launchAtLogin: Bool = false

    public init() {}

    public static let `default` = AppSettings()

    // MARK: - Clamping

    public static func clampBrightness(_ value: Int) -> Int {
        min(max(value, brightnessRange.lowerBound), brightnessRange.upperBound)
    }

    public static func clampKelvin(_ value: Int) -> Int {
        min(max(value, kelvinRange.lowerBound), kelvinRange.upperBound)
    }

    /// Returns a copy with every value inside its supported range and every schedule time well formed.
    public func normalized() -> AppSettings {
        var copy = self

        copy.manualBrightness = AppSettings.clampBrightness(manualBrightness)
        copy.manualColorTempK = AppSettings.clampKelvin(manualColorTempK)
        copy.dayColorTempK = AppSettings.clampKelvin(dayColorTempK)
        copy.nightColorTempK = AppSettings.clampKelvin(nightColorTempK)

        if !morningStart.isValid { copy.morningStart = AppSettings.defaultMorningStart }
        if !morningEnd.isValid { copy.morningEnd = AppSettings.defaultMorningEnd }
        if !eveningStart.isValid { copy.eveningStart = AppSettings.defaultEveningStart }
        if !eveningEnd.isValid { copy.eveningEnd = AppSettings.defaultEveningEnd }

        return copy
    }
}

// MARK: - Persistence

extension AppSettings: Codable {
    private enum CodingKeys: String, CodingKey {
        case dayColorTempK
        case nightColorTempK
        case morningStart
        case morningEnd
        case eveningStart
        case eveningEnd
        case autoColorTemperature
        case manualBrightness
        case manualColorTempK
        case launchAtLogin
    }

    /// Decodes leniently: any missing or malformed value falls back to its default
    /// rather than failing the whole load. Mirrors the defensive `ParseTime` behaviour
    /// in the .NET original.
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init()

        dayColorTempK = Self.decode(container, .dayColorTempK, dayColorTempK)
        nightColorTempK = Self.decode(container, .nightColorTempK, nightColorTempK)
        morningStart = Self.decode(container, .morningStart, morningStart)
        morningEnd = Self.decode(container, .morningEnd, morningEnd)
        eveningStart = Self.decode(container, .eveningStart, eveningStart)
        eveningEnd = Self.decode(container, .eveningEnd, eveningEnd)
        autoColorTemperature = Self.decode(container, .autoColorTemperature, autoColorTemperature)
        manualBrightness = Self.decode(container, .manualBrightness, manualBrightness)
        manualColorTempK = Self.decode(container, .manualColorTempK, manualColorTempK)
        launchAtLogin = Self.decode(container, .launchAtLogin, launchAtLogin)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)

        try container.encode(dayColorTempK, forKey: .dayColorTempK)
        try container.encode(nightColorTempK, forKey: .nightColorTempK)
        try container.encode(morningStart, forKey: .morningStart)
        try container.encode(morningEnd, forKey: .morningEnd)
        try container.encode(eveningStart, forKey: .eveningStart)
        try container.encode(eveningEnd, forKey: .eveningEnd)
        try container.encode(autoColorTemperature, forKey: .autoColorTemperature)
        try container.encode(manualBrightness, forKey: .manualBrightness)
        try container.encode(manualColorTempK, forKey: .manualColorTempK)
        try container.encode(launchAtLogin, forKey: .launchAtLogin)
    }

    private static func decode<T: Decodable>(
        _ container: KeyedDecodingContainer<CodingKeys>,
        _ key: CodingKeys,
        _ fallback: T
    ) -> T {
        (try? container.decode(T.self, forKey: key)) ?? fallback
    }
}
