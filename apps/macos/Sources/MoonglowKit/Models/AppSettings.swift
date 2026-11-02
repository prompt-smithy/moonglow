import Foundation

/// Persisted application settings.
///
/// Ported from `Models/AppSettings.cs`. Range constants and defaults are preserved exactly.
/// The .NET `Use24HourClock` setting is not carried over: Moonglow is twelve-hour only, so the
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

    /// Ctrl+Option+arrows: vertical for brightness, horizontal along the warm-to-cool
    /// track for colour temperature. Any of them can be re-recorded or cleared in
    /// Settings; nil disables that hot key.
    public static let defaultIncreaseBrightnessShortcut = HotKeyShortcut(keyCode: 126, modifiers: 0x1800)
    public static let defaultDecreaseBrightnessShortcut = HotKeyShortcut(keyCode: 125, modifiers: 0x1800)
    public static let defaultIncreaseTemperatureShortcut = HotKeyShortcut(keyCode: 124, modifiers: 0x1800)
    public static let defaultDecreaseTemperatureShortcut = HotKeyShortcut(keyCode: 123, modifiers: 0x1800)

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
    /// Whether the user wants Moonglow to start with the Mac.
    ///
    /// This records intent only. The actual registration lives in the system's login items, which
    /// the user can change outside the app, so it is re-asserted at launch rather than trusted.
    public var launchAtLogin: Bool = false

    /// Global hot keys. Higher temperature means a higher kelvin value (cooler light).
    public var increaseBrightnessShortcut: HotKeyShortcut? = defaultIncreaseBrightnessShortcut
    public var decreaseBrightnessShortcut: HotKeyShortcut? = defaultDecreaseBrightnessShortcut
    public var increaseTemperatureShortcut: HotKeyShortcut? = defaultIncreaseTemperatureShortcut
    public var decreaseTemperatureShortcut: HotKeyShortcut? = defaultDecreaseTemperatureShortcut

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
        copy.manualColorTempK = TemperatureRange.snap(manualColorTempK)
        let targets = TemperatureRange(night: nightColorTempK, day: dayColorTempK)
        copy.dayColorTempK = targets.day
        copy.nightColorTempK = targets.night

        if !morningStart.isValid { copy.morningStart = AppSettings.defaultMorningStart }
        if !morningEnd.isValid { copy.morningEnd = AppSettings.defaultMorningEnd }
        if !eveningStart.isValid { copy.eveningStart = AppSettings.defaultEveningStart }
        if !eveningEnd.isValid { copy.eveningEnd = AppSettings.defaultEveningEnd }
        if !(copy.morningStart < copy.morningEnd && copy.morningEnd <= copy.eveningStart && copy.eveningStart < copy.eveningEnd) {
            copy.morningStart = Self.defaultMorningStart
            copy.morningEnd = Self.defaultMorningEnd
            copy.eveningStart = Self.defaultEveningStart
            copy.eveningEnd = Self.defaultEveningEnd
        }

        // A modifier-less shortcut (a hand edit; the recorder refuses these) would hijack
        // typing if registered, so it is dropped rather than kept.
        if copy.increaseBrightnessShortcut?.isValid == false { copy.increaseBrightnessShortcut = nil }
        if copy.decreaseBrightnessShortcut?.isValid == false { copy.decreaseBrightnessShortcut = nil }
        if copy.increaseTemperatureShortcut?.isValid == false { copy.increaseTemperatureShortcut = nil }
        if copy.decreaseTemperatureShortcut?.isValid == false { copy.decreaseTemperatureShortcut = nil }
        let paths: [WritableKeyPath<AppSettings, HotKeyShortcut?>] = [
            \.increaseBrightnessShortcut, \.decreaseBrightnessShortcut,
            \.increaseTemperatureShortcut, \.decreaseTemperatureShortcut,
        ]
        var seen: [HotKeyShortcut] = []
        for path in paths {
            guard let shortcut = copy[keyPath: path] else { continue }
            if seen.contains(shortcut) { copy[keyPath: path] = nil }
            else { seen.append(shortcut) }
        }

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
        case increaseBrightnessShortcut
        case decreaseBrightnessShortcut
        case increaseTemperatureShortcut
        case decreaseTemperatureShortcut
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
        increaseBrightnessShortcut = Self.decode(container, .increaseBrightnessShortcut, increaseBrightnessShortcut)
        decreaseBrightnessShortcut = Self.decode(container, .decreaseBrightnessShortcut, decreaseBrightnessShortcut)
        increaseTemperatureShortcut = Self.decode(container, .increaseTemperatureShortcut, increaseTemperatureShortcut)
        decreaseTemperatureShortcut = Self.decode(container, .decreaseTemperatureShortcut, decreaseTemperatureShortcut)
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
        try container.encode(increaseBrightnessShortcut, forKey: .increaseBrightnessShortcut)
        try container.encode(decreaseBrightnessShortcut, forKey: .decreaseBrightnessShortcut)
        try container.encode(increaseTemperatureShortcut, forKey: .increaseTemperatureShortcut)
        try container.encode(decreaseTemperatureShortcut, forKey: .decreaseTemperatureShortcut)
    }

    private static func decode<T: Decodable>(
        _ container: KeyedDecodingContainer<CodingKeys>,
        _ key: CodingKeys,
        _ fallback: T
    ) -> T {
        (try? container.decode(T.self, forKey: key)) ?? fallback
    }
}
