import Foundation
import Testing

@testable import MoonglowKit

/// Verifies the platform against the shared contract in `spec/vectors`.
///
/// These vectors are the only thing the three implementations share. If one of these fails,
/// the platform has drifted from the contract — fix the implementation, or change the spec
/// and update every platform together.
@Suite("Spec conformance")
struct SpecConformanceTests {

    private static let vectorsURL: URL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()  // MoonglowKitTests
        .deletingLastPathComponent()  // Tests
        .deletingLastPathComponent()  // macos
        .deletingLastPathComponent()  // apps
        .deletingLastPathComponent()  // repo root
        .appendingPathComponent("spec/vectors")

    private static func load<T: Decodable>(_ name: String) throws -> T {
        let data = try Data(contentsOf: vectorsURL.appendingPathComponent(name))
        return try JSONDecoder().decode(T.self, from: data)
    }

    // MARK: - Kelvin

    @Test("Kelvin vectors match within 1e-6")
    func kelvinVectorsMatch() throws {
        let document: KelvinDocument = try Self.load("kelvin.json")
        #expect(!document.vectors.isEmpty)

        for row in document.vectors {
            let scale = KelvinConversion.rgbScale(kelvin: row.kelvin)
            #expect(abs(scale.red - row.red) <= 1e-6, "red at \(row.kelvin) K")
            #expect(abs(scale.green - row.green) <= 1e-6, "green at \(row.kelvin) K")
            #expect(abs(scale.blue - row.blue) <= 1e-6, "blue at \(row.kelvin) K")
        }
    }

    // MARK: - Schedule

    @Test("Schedule vectors match exactly")
    func scheduleVectorsMatch() throws {
        let document: ScheduleDocument = try Self.load("schedule.json")
        #expect(!document.cases.isEmpty)

        for entry in document.cases {
            guard let settings = document.settings[entry.settings] else {
                Issue.record("vector references unknown settings profile \(entry.settings)")
                continue
            }
            guard let time = TimeOfDay(storageDescription: entry.time) else {
                Issue.record("vector has malformed time \(entry.time)")
                continue
            }

            let result = ScheduleCalculator.temperature(for: settings.appSettings, at: time)
            #expect(result == entry.kelvin, "at \(entry.time) with \(entry.settings)")
        }
    }

    // MARK: - Temperature range

    @Test("Snapping vectors match exactly")
    func snappingVectorsMatch() throws {
        let document: RangeDocument = try Self.load("temperature-range.json")

        for row in document.snap {
            #expect(TemperatureRange.snap(row.input) == row.output, "snap(\(row.input))")
        }
    }

    @Test("Range ordering vectors match exactly")
    func rangeOrderingVectorsMatch() throws {
        let document: RangeDocument = try Self.load("temperature-range.json")

        for row in document.range {
            let range = TemperatureRange(night: row.night, day: row.day)
            #expect(range.night == row.nightOut, "night for \(row.night)/\(row.day)")
            #expect(range.day == row.dayOut, "day for \(row.night)/\(row.day)")
        }
    }
}

// MARK: - Vector shapes

private struct KelvinDocument: Decodable {
    struct Row: Decodable {
        let kelvin: Int
        let red: Double
        let green: Double
        let blue: Double
    }

    let vectors: [Row]
}

private struct ScheduleDocument: Decodable {
    struct Case: Decodable {
        let settings: String
        let time: String
        let kelvin: Int
    }

    let settings: [String: Profile]
    let cases: [Case]

    struct Profile: Decodable {
        let dayColorTempK: Int
        let nightColorTempK: Int
        let morningStart: [Int]
        let morningEnd: [Int]
        let eveningStart: [Int]
        let eveningEnd: [Int]
        let autoColorTemperature: Bool
        let manualBrightness: Int
        let manualColorTempK: Int
        let launchAtLogin: Bool

        var appSettings: AppSettings {
            var settings = AppSettings()
            settings.dayColorTempK = dayColorTempK
            settings.nightColorTempK = nightColorTempK
            settings.morningStart = TimeOfDay(morningStart)
            settings.morningEnd = TimeOfDay(morningEnd)
            settings.eveningStart = TimeOfDay(eveningStart)
            settings.eveningEnd = TimeOfDay(eveningEnd)
            settings.autoColorTemperature = autoColorTemperature
            settings.manualBrightness = manualBrightness
            settings.manualColorTempK = manualColorTempK
            settings.launchAtLogin = launchAtLogin
            return settings
        }
    }
}

private struct RangeDocument: Decodable {
    struct SnapRow: Decodable {
        let input: Int
        let output: Int
    }

    struct RangeRow: Decodable {
        let night: Int
        let day: Int
        let nightOut: Int
        let dayOut: Int
    }

    let snap: [SnapRow]
    let range: [RangeRow]
}

private extension TimeOfDay {
    /// Vector times are two-element `[hour, minute]` arrays.
    init(_ pair: [Int]) {
        self.init(hour: pair.first ?? 0, minute: pair.count > 1 ? pair[1] : 0)
    }
}
