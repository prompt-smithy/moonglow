import Foundation
import Testing
@testable import MoonglowKit

/// Ported from `ScheduleServiceTests` in the .NET test project.
///
/// Defaults under test: morning 06:00-07:00, evening 17:30-18:30, day 6500 K, night 1200 K.
@Suite("Schedule calculation")
@MainActor
struct ScheduleCalculatorTests {

    private func at(_ hour: Int, _ minute: Int) -> TimeOfDay {
        TimeOfDay(hour: hour, minute: minute)
    }

    @Test(
        "Follows the five daily states",
        arguments: [
            (5, 59, 1200),   // before the morning transition: night
            (6, 0, 1200),    // start of the morning ramp
            (6, 30, 3900),   // midpoint of the morning ramp
            (7, 0, 6500),    // first instant of the day target
            (12, 0, 6500),   // mid-day
            (17, 29, 6500),  // last minute before the evening ramp
            (17, 30, 6500),  // start of the evening ramp
            (18, 0, 3900),   // midpoint of the evening ramp
            (18, 30, 1200),  // after the evening transition: night
            (23, 59, 1200),
            (0, 30, 1200),
        ]
    )
    func followsDailySchedule(hour: Int, minute: Int, expectedKelvin: Int) {
        let result = ScheduleCalculator.temperature(for: .default, at: at(hour, minute))

        #expect(result == expectedKelvin)
    }

    @Test("Results are snapped to the 100 K step")
    func resultsSnapToStep() {
        // 06:00-07:00 over 1200...6500 K: 06:01 is 1/60 of the way, which is not a round value.
        let result = ScheduleCalculator.temperature(for: .default, at: at(6, 1))

        #expect(result % AppSettings.kelvinStep == 0)
    }

    @Test("Custom targets are honoured")
    func customTargetsAreHonoured() {
        var settings = AppSettings.default
        settings.dayColorTempK = 5000
        settings.nightColorTempK = 2000
        settings.morningStart = at(8, 0)
        settings.morningEnd = at(9, 0)
        settings.eveningStart = at(20, 0)
        settings.eveningEnd = at(21, 0)

        #expect(ScheduleCalculator.temperature(for: settings, at: at(7, 59)) == 2000)
        #expect(ScheduleCalculator.temperature(for: settings, at: at(8, 30)) == 3500)
        #expect(ScheduleCalculator.temperature(for: settings, at: at(12, 0)) == 5000)
        #expect(ScheduleCalculator.temperature(for: settings, at: at(20, 30)) == 3500)
        #expect(ScheduleCalculator.temperature(for: settings, at: at(21, 0)) == 2000)
    }

    @Test("An inverted transition span falls back to the end target")
    func invertedSpanFallsBackToEndTarget() {
        var settings = AppSettings.default
        settings.morningStart = at(7, 0)
        settings.morningEnd = at(6, 0) // zero-length/inverted

        // 06:30 is >= morningStart? No: 06:30 < 07:00 start, so it is the night target.
        #expect(ScheduleCalculator.temperature(for: settings, at: at(6, 30)) == settings.nightColorTempK)
    }

    @Test("Decoding an unparseable schedule time falls back to the default")
    func invalidStoredTimeFallsBackToDefault() throws {
        let json = """
        {
          "morningStart": "not-a-time",
          "morningEnd": "07:00",
          "eveningStart": "17:30",
          "eveningEnd": "18:30"
        }
        """

        let decoded = try JSONDecoder().decode(AppSettings.self, from: Data(json.utf8))

        #expect(decoded.morningStart == AppSettings.defaultMorningStart)
        #expect(decoded.morningEnd == AppSettings.defaultMorningEnd)
    }

    @Test("Times round-trip through the stored HH:mm form")
    func timesRoundTripThroughStorageForm() {
        let time = TimeOfDay(hour: 6, minute: 5)

        #expect(time.storageDescription == "06:05")
        #expect(TimeOfDay(storageDescription: "06:05") == time)
        #expect(TimeOfDay(storageDescription: "6:05") == time)
        #expect(TimeOfDay(storageDescription: "24:00") == nil)
        #expect(TimeOfDay(storageDescription: "06:60") == nil)
    }

    @Test("The scheduler refuses to run without a supported colour service")
    func schedulerRequiresColourSupport() {
        let color = StubColorService(isSupported: false)
        let schedule = ScheduleService(colorService: color)
        schedule.settingsProvider = { .default }

        var applied: [Int] = []
        schedule.onTemperatureApplied = { applied.append($0) }

        schedule.tick()

        #expect(applied.isEmpty)
    }

    @Test("The scheduler is inert while automatic mode is off")
    func schedulerInertWhenAutomaticIsOff() {
        let color = StubColorService(isSupported: true)
        let schedule = ScheduleService(colorService: color)

        var settings = AppSettings.default
        settings.autoColorTemperature = false
        schedule.settingsProvider = { settings }

        schedule.tick()

        #expect(color.applied.isEmpty)
    }

    @Test("The scheduler applies and reports the computed temperature")
    func schedulerAppliesAndReports() {
        let color = StubColorService(isSupported: true)
        let schedule = ScheduleService(colorService: color)
        schedule.settingsProvider = { .default }

        var reported: [Int] = []
        schedule.onTemperatureApplied = { reported.append($0) }

        schedule.tick()

        #expect(color.applied.count == 1)
        #expect(reported == color.applied)
    }
}

/// Minimal colour service used to observe scheduler behaviour.
private final class StubColorService: ColorTemperatureService {
    let isSupported: Bool
    private(set) var applied: [Int] = []

    init(isSupported: Bool) {
        self.isSupported = isSupported
    }

    func apply(kelvin: Int) -> Bool {
        guard isSupported else { return false }
        applied.append(kelvin)
        return true
    }

    func reset() {}
}
