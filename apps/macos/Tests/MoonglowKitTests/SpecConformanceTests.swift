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

    @Test("Settings normalization vectors match on load")
    func settingsNormalizationVectorsMatch() throws {
        let document: SettingsDocument = try Self.load("settings.json")
        for row in document.cases {
            let actual = (row.macosInput ?? row.input).normalized()
            let expected = row.macosExpected ?? row.expected
            #expect(actual.restoreBrightnessShortcut == expected.restoreBrightnessShortcut, "\(row.name)")
            #expect(actual.increaseBrightnessShortcut == expected.increaseBrightnessShortcut, "\(row.name)")
            #expect(actual.manualBrightness == expected.manualBrightness, "\(row.name)")
            #expect(actual.hardwareBrightnessEnabled == expected.hardwareBrightnessEnabled, "\(row.name)")
            #expect(actual.softwareBrightnessEnabled == expected.softwareBrightnessEnabled, "\(row.name)")
            #expect(actual.splitBrightnessControls == expected.splitBrightnessControls, "\(row.name)")
            #expect(actual.manualHardwareBrightness == expected.manualHardwareBrightness, "\(row.name)")
            #expect(actual.manualSoftwareBrightness == expected.manualSoftwareBrightness, "\(row.name)")
            #expect(actual.manualColorTempK == expected.manualColorTempK, "\(row.name)")
            #expect(actual.dayColorTempK == expected.dayColorTempK, "\(row.name)")
            #expect(actual.nightColorTempK == expected.nightColorTempK, "\(row.name)")
            #expect(actual.morningStart == expected.morningStart, "\(row.name)")
            #expect(actual.morningEnd == expected.morningEnd, "\(row.name)")
            #expect(actual.eveningStart == expected.eveningStart, "\(row.name)")
            #expect(actual.eveningEnd == expected.eveningEnd, "\(row.name)")
        }
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

    // MARK: - Brightness routing

    @Test("Brightness routing vectors match exactly")
    func brightnessRoutingVectorsMatch() throws {
        let document: BrightnessRoutingDocument = try Self.load("brightness-routing.json")
        #expect(document.softwareBrightnessFloor == AppSettings.softwareBrightnessFloor)

        for row in document.vectors {
            let output = BrightnessPlan.computeRouting(
                master: row.master,
                split: row.split,
                manualHardware: row.hardwareInput,
                manualSoftware: row.softwareInput,
                hardwareAvailable: row.hardwareAvailable,
                softwareAvailable: row.softwareAvailable,
                recoveryRegistered: row.recoveryRegistered
            )
            #expect(output.hardware == row.expectedHardware, "\(row.name) hardware")
            #expect(output.software == row.expectedSoftware, "\(row.name) software")
        }

        for row in document.transitions {
            if let expectedMaster = row.expectedMaster {
                let master = row.preserveOutput == true
                    ? BrightnessPlan.masterPreservingOutput(.init(hardware: row.hardware, software: row.software ?? 100), hardwareAvailable: row.hardwareAvailable, softwareAvailable: row.softwareAvailable, recoveryRegistered: row.recoveryRegistered)
                    : BrightnessPlan.splitToUnified(
                    hardware: row.hardware ?? 80,
                    software: row.software ?? 100,
                    hardwareAvailable: row.hardwareAvailable,
                    softwareAvailable: row.softwareAvailable,
                    recoveryRegistered: row.recoveryRegistered
                )
                #expect(master == expectedMaster, "\(row.name) master")
            } else if let expectedHw = row.expectedHardware, let expectedSw = row.expectedSoftware {
                let split = BrightnessPlan.unifiedToSplit(
                    master: row.master ?? 80,
                    hardwareAvailable: row.hardwareAvailable,
                    softwareAvailable: row.softwareAvailable,
                    recoveryRegistered: row.recoveryRegistered
                )
                #expect(split.hardware == expectedHw, "\(row.name) hardware")
                #expect(split.software == expectedSw, "\(row.name) software")
            }
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

private struct SettingsDocument: Decodable {
    struct Row: Decodable {
        let name: String
        let input: AppSettings
        let expected: AppSettings
        let macosInput: AppSettings?
        let macosExpected: AppSettings?
    }
    let cases: [Row]
}

private struct BrightnessRoutingDocument: Decodable {
    struct VectorRow: Decodable {
        let name: String
        let master: Int
        let hardwareInput: Int
        let softwareInput: Int
        let split: Bool
        let hardwareAvailable: Bool
        let softwareAvailable: Bool
        let recoveryRegistered: Bool
        let expectedHardware: Int?
        let expectedSoftware: Int
    }
    struct TransitionRow: Decodable {
        let name: String
        let master: Int?
        let hardware: Int?
        let software: Int?
        let hardwareAvailable: Bool
        let softwareAvailable: Bool
        let recoveryRegistered: Bool
        let expectedHardware: Int?
        let expectedSoftware: Int?
        let expectedMaster: Int?
        let preserveOutput: Bool?
    }
    let softwareBrightnessFloor: Int
    let vectors: [VectorRow]
    let transitions: [TransitionRow]
}
