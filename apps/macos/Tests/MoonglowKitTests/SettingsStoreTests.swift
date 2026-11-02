import Foundation
import Testing
@testable import MoonglowKit

/// Ported from `SettingsServiceTests`, minus the legacy-import cases (this port starts fresh).
@Suite("Settings persistence")
struct SettingsStoreTests {

    private static func makeStore() -> SettingsStore {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("moonglow-tests-\(UUID().uuidString)", isDirectory: true)
        return SettingsStore(fileURL: directory.appendingPathComponent("settings.json"))
    }

    private static func write(_ json: String, to store: SettingsStore) throws {
        try FileManager.default.createDirectory(
            at: store.fileURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data(json.utf8).write(to: store.fileURL)
    }

    @Test("Deferred saves flush the latest slider value before restart")
    func deferredSavesFlushLatestSnapshot() {
        let store = Self.makeStore()
        var settings = AppSettings.default
        for value in 20...60 {
            settings.manualBrightness = value
            store.scheduleSave(settings)
        }
        store.flush()
        let restarted = SettingsStore(fileURL: store.fileURL)
        #expect(restarted.load().manualBrightness == 60)
    }

    @Test("Defaults match the documented values")
    func defaultsMatchDocumentation() {
        let settings = AppSettings.default

        #expect(settings.manualBrightness == 80)
        #expect(settings.manualColorTempK == 6500)
        #expect(settings.dayColorTempK == 6500)
        #expect(settings.nightColorTempK == 1200)
        #expect(settings.autoColorTemperature)
        #expect(settings.launchAtLogin == false)
        #expect(settings.morningStart == TimeOfDay(hour: 6, minute: 0))
        #expect(settings.morningEnd == TimeOfDay(hour: 7, minute: 0))
        #expect(settings.eveningStart == TimeOfDay(hour: 17, minute: 30))
        #expect(settings.eveningEnd == TimeOfDay(hour: 18, minute: 30))
        #expect(settings.increaseBrightnessShortcut == AppSettings.defaultIncreaseBrightnessShortcut)
        #expect(settings.decreaseBrightnessShortcut == AppSettings.defaultDecreaseBrightnessShortcut)
        #expect(settings.increaseTemperatureShortcut == AppSettings.defaultIncreaseTemperatureShortcut)
        #expect(settings.decreaseTemperatureShortcut == AppSettings.defaultDecreaseTemperatureShortcut)
    }

    @Test("A missing file falls back to defaults")
    func missingFileFallsBackToDefaults() {
        let store = Self.makeStore()

        #expect(store.load() == .default)
    }

    @Test("A corrupt file falls back to defaults")
    func corruptFileFallsBackToDefaults() throws {
        let store = Self.makeStore()
        try Self.write("{ not json", to: store)

        #expect(store.load() == .default)
    }

    @Test("A corrupt file is preserved alongside before the fallback")
    func corruptFileIsPreserved() throws {
        let store = Self.makeStore()
        try Self.write("{ not json", to: store)

        #expect(store.load() == .default)

        let corrupt = store.fileURL.appendingPathExtension("corrupt")
        #expect(FileManager.default.fileExists(atPath: corrupt.path))
        #expect(try String(contentsOf: corrupt, encoding: .utf8) == "{ not json")
        #expect(!FileManager.default.fileExists(atPath: store.fileURL.path))
    }

    @Test("A missing file leaves no preservation behind")
    func missingFileLeavesNoPreservation() {
        let store = Self.makeStore()

        #expect(store.load() == .default)
        #expect(!FileManager.default.fileExists(
            atPath: store.fileURL.appendingPathExtension("corrupt").path
        ))
    }

    @Test("Out-of-range values are clamped on load")
    func outOfRangeValuesAreClampedOnLoad() throws {
        let store = Self.makeStore()
        try Self.write(
            """
            {
              "manualBrightness": 500,
              "manualColorTempK": 100,
              "dayColorTempK": 99999,
              "nightColorTempK": 0
            }
            """,
            to: store
        )

        let loaded = store.load()

        #expect(loaded.manualBrightness == 100)
        #expect(loaded.manualColorTempK == 900)
        #expect(loaded.dayColorTempK == 6600)
        #expect(loaded.nightColorTempK == 900)
    }

    @Test("Modifier-less shortcuts are dropped on load")
    func modifierLessShortcutsAreDropped() throws {
        let store = Self.makeStore()
        try Self.write(#"{"increaseBrightnessShortcut":{"keyCode":0,"modifiers":0}}"#, to: store)

        #expect(store.load().increaseBrightnessShortcut == nil)
    }

    @Test("Stored shortcuts round-trip")
    func shortcutsRoundTrip() {
        let store = Self.makeStore()

        var settings = AppSettings.default
        settings.increaseBrightnessShortcut = HotKeyShortcut(keyCode: 12, modifiers: HotKeyShortcut.commandModifier)
        settings.decreaseBrightnessShortcut = nil
        store.save(settings)

        let loaded = store.load()
        #expect(loaded.increaseBrightnessShortcut == HotKeyShortcut(keyCode: 12, modifiers: HotKeyShortcut.commandModifier))
        #expect(loaded.decreaseBrightnessShortcut == nil)
    }

    @Test("Legacy theme preferences are ignored and not written again")
    func legacyThemePreferencesAreIgnored() throws {
        let store = Self.makeStore()
        try Self.write(
            #"{"lightModeTheme":"Light Violet","darkModeTheme":"Black"}"#,
            to: store
        )

        store.save(store.load())
        let saved = try String(contentsOf: store.fileURL, encoding: .utf8)

        #expect(!saved.contains("lightModeTheme"))
        #expect(!saved.contains("darkModeTheme"))
    }

    @Test("Saving normalizes before writing")
    func savingNormalizesBeforeWriting() throws {
        let store = Self.makeStore()

        var settings = AppSettings.default
        settings.manualBrightness = 900
        store.save(settings)

        let raw = try String(contentsOf: store.fileURL, encoding: .utf8)

        #expect(raw.contains("\"manualBrightness\" : 100"))
    }

    @Test("A full round trip preserves every field")
    func fullRoundTripPreservesEveryField() {
        let store = Self.makeStore()

        var settings = AppSettings.default
        settings.autoColorTemperature = false
        settings.manualBrightness = 42
        settings.manualColorTempK = 3300
        settings.dayColorTempK = 5100
        settings.nightColorTempK = 1800
        settings.morningStart = TimeOfDay(hour: 5, minute: 45)
        settings.morningEnd = TimeOfDay(hour: 6, minute: 15)
        settings.eveningStart = TimeOfDay(hour: 19, minute: 5)
        settings.eveningEnd = TimeOfDay(hour: 20, minute: 45)
        settings.launchAtLogin = true

        store.save(settings)
        let loaded = store.load()

        #expect(loaded == settings.normalized())
    }

    @Test("A settings file written before launch-at-login existed still loads")
    func settingsFileWithoutLaunchAtLoginStillLoads() throws {
        let store = Self.makeStore()
        // The shape the previous build wrote: every field except `launchAtLogin`, and without the
        // schedule keys, so this also covers the missing-value fallback.
        try Self.write(
            """
            {"manualBrightness":55,"autoColorTemperature":false,"manualColorTempK":4300,
             "dayColorTempK":6500,"nightColorTempK":1200,
             "lightModeTheme":"Light Green","darkModeTheme":"Dark Violet"}
            """,
            to: store
        )

        let loaded = store.load()

        #expect(loaded.manualBrightness == 55)
        #expect(loaded.autoColorTemperature == false)
        #expect(loaded.launchAtLogin == false)
        #expect(loaded.morningStart == AppSettings.defaultMorningStart)
    }
}
