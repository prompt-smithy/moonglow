import Foundation
import Testing
@testable import AmberKit

/// Covers the behaviour the .NET test suite did not: clamping, persistence triggers,
/// capability gating, and the observation side effects that replace `SetProperty`.
@Suite("Main view model")
@MainActor
struct MainViewModelTests {

    @Test("Starts from the persisted values")
    func startsFromPersistedValues() {
        var settings = AppSettings.default
        settings.manualBrightness = 55
        let harness = makeHarness(settings: settings)

        #expect(harness.viewModel.brightness == 55)
    }

    @Test("Brightness is clamped to 1...100 and written through")
    func brightnessClampsToRange() {
        let harness = makeHarness()

        harness.viewModel.brightness = 500
        #expect(harness.viewModel.brightness == 100)
        #expect(harness.brightness.applied == [100])

        harness.viewModel.brightness = -20
        #expect(harness.viewModel.brightness == 1)
        #expect(harness.brightness.applied == [100, 1])
    }

    @Test("Assigning the same brightness does not re-apply or rewrite")
    func unchangedBrightnessIsIgnored() {
        var settings = AppSettings.default
        settings.manualBrightness = 60
        let harness = makeHarness(settings: settings)

        harness.viewModel.brightness = 60

        #expect(harness.brightness.applied.isEmpty)
    }

    @Test("Manual colour temperature is clamped, persisted, and applied")
    func manualColourTemperatureApplies() {
        var settings = AppSettings.default
        settings.autoColorTemperature = false
        settings.manualColorTempK = 4000
        let harness = makeHarness(settings: settings)

        harness.viewModel.colorTemperature = 9999

        #expect(harness.viewModel.colorTemperature == 6600)
        #expect(harness.store.load().manualColorTempK == 6600)
        #expect(harness.color.applied == [6600])
    }

    @Test("In automatic mode the manual value is displayed but not persisted or applied")
    func automaticModeDoesNotPersistManualValue() {
        var settings = AppSettings.default
        settings.autoColorTemperature = true
        settings.manualColorTempK = 6500
        let harness = makeHarness(settings: settings)

        harness.viewModel.colorTemperature = 3000

        #expect(harness.viewModel.colorTemperature == 3000)
        #expect(harness.store.load().manualColorTempK == 6500)
        #expect(harness.color.applied.isEmpty)
    }

    @Test("Turning automatic mode off applies the displayed temperature")
    func turningAutomaticOffAppliesTemperature() {
        var settings = AppSettings.default
        settings.autoColorTemperature = true
        let harness = makeHarness(settings: settings)

        let displayed = harness.viewModel.colorTemperature
        harness.viewModel.autoColorTemperature = false

        #expect(harness.store.load().autoColorTemperature == false)
        #expect(harness.color.applied == [displayed])
    }

    @Test("The manual slider is disabled without colour support")
    func manualSliderNeedsSupport() {
        var settings = AppSettings.default
        settings.autoColorTemperature = false

        let withoutSupport = makeHarness(settings: settings, colorSupported: false)
        #expect(!withoutSupport.viewModel.manualColorEnabled)

        let withSupport = makeHarness(settings: settings)
        #expect(withSupport.viewModel.manualColorEnabled)

        let automatic = makeHarness(settings: .default)
        #expect(!automatic.viewModel.manualColorEnabled)
    }

    @Test("Capability flags and status text follow the services")
    func capabilityFlagsFollowServices() {
        let supported = makeHarness()
        #expect(supported.viewModel.hasBrightnessSupport)
        #expect(supported.viewModel.hasColorSupport)
        #expect(supported.viewModel.brightnessStatus == "Hardware brightness is available through DDC/CI.")
        #expect(supported.viewModel.colorStatus == "CoreGraphics gamma control is available.")

        let unsupported = makeHarness(brightnessSupported: false, colorSupported: false)
        #expect(!unsupported.viewModel.hasBrightnessSupport)
        #expect(!unsupported.viewModel.hasColorSupport)
        #expect(unsupported.viewModel.brightnessStatus == "Brightness control is unavailable for the detected display connection.")
        #expect(unsupported.viewModel.colorStatus == "Color-temperature control is unavailable on this system.")
    }

    @Test("No supported displays means no capability, even when the service works")
    func noSupportedDisplaysMeansNoCapability() {
        let inert = DisplayInfo(
            nativeId: 9,
            name: "Inert",
            identifier: "9",
            isPrimary: true,
            supportsBrightness: false,
            supportsColorTemperature: false
        )
        let harness = makeHarness(displays: [inert])

        #expect(!harness.viewModel.hasBrightnessSupport)
        #expect(!harness.viewModel.hasColorSupport)
        #expect(!harness.viewModel.hasNoDisplays)
    }

    @Test("An empty display list reports the empty state")
    func emptyDisplayListReportsEmptyState() {
        let harness = makeHarness(displays: [])

        #expect(harness.viewModel.hasNoDisplays)
    }

    @Test("Refreshing re-enumerates displays")
    func refreshingReenumerates() {
        let harness = makeHarness()

        harness.viewModel.refreshDisplays()

        // Once from init, once from the explicit refresh.
        #expect(harness.displays.refreshCount == 2)
    }

    @Test("Schedule targets are clamped to 900...6600")
    func scheduleTargetsClamp() {
        let harness = makeHarness()

        harness.viewModel.dayColorTemp = 10_000
        harness.viewModel.nightColorTemp = 0

        #expect(harness.viewModel.dayColorTemp == 6600)
        #expect(harness.viewModel.nightColorTemp == 900)
    }

    @Test("Both schedule targets are applied in a single write")
    func temperatureRangeAppliesBothTargets() {
        let harness = makeHarness()

        harness.viewModel.setTemperatureRange(TemperatureRange(night: 1000, day: 5000))

        #expect(harness.viewModel.nightColorTemp == 1000)
        #expect(harness.viewModel.dayColorTemp == 5000)

        let reloaded = harness.store.load()
        #expect(reloaded.nightColorTempK == 1000)
        #expect(reloaded.dayColorTempK == 5000)
    }

    @Test("An unchanged range neither rewrites nor re-applies")
    func unchangedTemperatureRangeIsIgnored() {
        let harness = makeHarness()

        harness.viewModel.setTemperatureRange(TemperatureRange(settings: harness.viewModel.settings))

        #expect(harness.color.applied.isEmpty)
    }

    @Test("Invalid schedule times are ignored")
    func invalidScheduleTimesAreIgnored() {
        let harness = makeHarness()
        let original = harness.viewModel.morningStart

        harness.viewModel.morningStart = TimeOfDay(hour: 99, minute: 0)
        harness.viewModel.morningEnd = nil

        #expect(harness.viewModel.morningStart == original)
        #expect(harness.viewModel.morningEnd == AppSettings.defaultMorningEnd)
    }

    @Test("Valid schedule times are persisted")
    func validScheduleTimesArePersisted() {
        let harness = makeHarness()

        harness.viewModel.morningStart = TimeOfDay(hour: 5, minute: 15)

        #expect(harness.viewModel.morningStart == TimeOfDay(hour: 5, minute: 15))
        #expect(harness.store.load().morningStart == TimeOfDay(hour: 5, minute: 15))
    }

    @Test("An unknown theme name is rejected")
    func unknownThemeNameIsRejected() {
        let harness = makeHarness()

        harness.viewModel.lightModeTheme = "Chartreuse"

        #expect(harness.viewModel.lightModeTheme == "Light Green")
        #expect(harness.theme.applyCount == 0)
    }

    @Test("A valid theme change is persisted and applied")
    func validThemeChangeIsApplied() {
        let harness = makeHarness()

        harness.viewModel.darkModeTheme = "Dark Violet"

        #expect(harness.store.load().darkModeTheme == "Dark Violet")
        #expect(harness.theme.applyCount == 1)
    }

    @Test("Callbacks fire for the settings window and quit")
    func callbacksFire() {
        let harness = makeHarness()
        var settings = 0
        var quit = 0
        harness.viewModel.onShowSettings = { settings += 1 }
        harness.viewModel.onQuit = { quit += 1 }

        harness.viewModel.showSettings()
        harness.viewModel.quit()

        #expect(settings == 1)
        #expect(quit == 1)
    }

    @Test("applyInitialState pushes persisted values to the hardware")
    func applyInitialStatePushesValues() {
        var settings = AppSettings.default
        settings.autoColorTemperature = false
        settings.manualBrightness = 70
        settings.manualColorTempK = 4400
        let harness = makeHarness(settings: settings)

        harness.viewModel.applyInitialState()

        #expect(harness.brightness.applied == [70])
        #expect(harness.color.applied == [4400])
    }

    @Test("Turning launch at login on registers it and persists the choice")
    func launchAtLoginRegistersAndPersists() {
        let harness = makeHarness()

        harness.viewModel.launchAtLogin = true

        #expect(harness.loginItem.requests == [true])
        #expect(harness.loginItem.isEnabled)
        #expect(harness.store.load().launchAtLogin)
    }

    @Test("A refused change is not recorded as intent")
    func refusedChangeIsNotRecorded() {
        let harness = makeHarness(loginItemRegisters: false)

        harness.viewModel.launchAtLogin = true

        // The toggle has to fall back to the truth rather than showing a registration that was
        // never made, so the setting must stay off.
        #expect(harness.viewModel.launchAtLogin == false)
        #expect(harness.store.load().launchAtLogin == false)
        #expect(harness.loginItem.requests == [true])
    }

    @Test("A stored preference the system now refuses is reported")
    func storedPreferenceRefusedAtLaunchIsReported() {
        // Registered on an earlier run, but this system will not accept it now. The intent stays on
        // disk, so the status has to admit that it is not in effect.
        var settings = AppSettings.default
        settings.launchAtLogin = true
        let harness = makeHarness(settings: settings, loginItemRegisters: false)

        harness.viewModel.applyLoginItemState()

        #expect(harness.viewModel.launchAtLogin)
        #expect(harness.viewModel.launchAtLoginStatus.contains("Could not register"))
    }

    @Test("A pending approval is reported rather than treated as active")
    func pendingApprovalIsReported() {
        let harness = makeHarness(loginItemRequiresApproval: true)

        harness.viewModel.launchAtLogin = true

        #expect(harness.viewModel.launchAtLogin)
        #expect(harness.viewModel.loginItemRequiresApproval)
        #expect(harness.viewModel.launchAtLoginStatus.contains("Waiting for approval"))
    }

    @Test("An unchanged value neither touches the service nor rewrites settings")
    func unchangedLaunchAtLoginIsIgnored() {
        let harness = makeHarness()

        harness.viewModel.launchAtLogin = false

        #expect(harness.loginItem.requests.isEmpty)
    }

    @Test("The stored preference is re-asserted at launch")
    func storedPreferenceIsReassertedAtLaunch() {
        var settings = AppSettings.default
        settings.launchAtLogin = true
        let harness = makeHarness(settings: settings)

        harness.viewModel.applyLoginItemState()

        #expect(harness.loginItem.requests == [true])
    }

    @Test("A first launch after install clears a preserved login preference")
    func firstLaunchAfterInstallDisablesLoginItem() {
        var settings = AppSettings.default
        settings.launchAtLogin = true
        let harness = makeHarness(settings: settings, loginItemAlreadyEnabled: true)

        harness.viewModel.applyLoginItemState(isFirstLaunchAfterInstall: true)

        #expect(!harness.viewModel.launchAtLogin)
        #expect(!harness.store.load().launchAtLogin)
        #expect(!harness.loginItem.isEnabled)
        #expect(harness.loginItem.requests == [false])
    }

    @Test("Nothing is registered at launch when the preference is off")
    func nothingRegisteredWhenPreferenceOff() {
        let harness = makeHarness()

        harness.viewModel.applyLoginItemState()

        #expect(harness.loginItem.requests.isEmpty)
        #expect(harness.viewModel.launchAtLoginStatus.contains("only when you launch"))
    }

    @Test("A login item Night Mode Controller did not create is adopted, not switched off")
    func existingRegistrationIsAdopted() {
        // The user added Night Mode Controller at login in System Settings before this setting existed. Launching
        // must not report "off" while the system says otherwise, and must not unregister it.
        let harness = makeHarness(loginItemAlreadyEnabled: true)

        harness.viewModel.applyLoginItemState()

        #expect(harness.viewModel.launchAtLogin)
        #expect(harness.store.load().launchAtLogin)
        #expect(harness.loginItem.requests.isEmpty)
        #expect(harness.viewModel.launchAtLoginStatus.contains("opens automatically"))
    }
}

// MARK: - Test doubles

@MainActor
private struct Harness {
    let store: SettingsStore
    let displays: FakeDisplayService
    let brightness: FakeBrightnessService
    let color: FakeColorService
    let theme: FakeTheme
    let loginItem: FakeLoginItemService
    let viewModel: MainViewModel
}

private final class FakeDisplayService: DisplayService {
    var displays: [DisplayInfo]
    private(set) var refreshCount = 0

    init(displays: [DisplayInfo]) {
        self.displays = displays
    }

    func refresh() -> [DisplayInfo] {
        refreshCount += 1
        return displays
    }
}

private final class FakeBrightnessService: BrightnessService {
    let isSupported: Bool
    private(set) var applied: [Int] = []

    init(isSupported: Bool) {
        self.isSupported = isSupported
    }

    func setBrightness(percent: Int) {
        applied.append(percent)
    }
}

private final class FakeColorService: ColorTemperatureService {
    let isSupported: Bool
    private(set) var applied: [Int] = []
    private(set) var resetCount = 0

    init(isSupported: Bool) {
        self.isSupported = isSupported
    }

    func apply(kelvin: Int) -> Bool {
        applied.append(kelvin)
        return true
    }

    func reset() {
        resetCount += 1
    }
}

@MainActor
private final class FakeTheme: ThemeApplying {
    private(set) var applyCount = 0

    func applyForCurrentMode(settings: AppSettings) {
        applyCount += 1
    }
}

/// Stands in for `SMAppService`, which reports real system state and cannot be driven from a test.
/// `acceptsChanges` is the equivalent of the system refusing a registration.
private final class FakeLoginItemService: LoginItemService {
    private(set) var isEnabled = false
    private(set) var requiresApproval = false
    private(set) var requests: [Bool] = []

    private let acceptsChanges: Bool

    init(acceptsChanges: Bool = true, requiresApproval: Bool = false, isEnabled: Bool = false) {
        self.acceptsChanges = acceptsChanges
        self.requiresApproval = requiresApproval
        self.isEnabled = isEnabled
    }

    @discardableResult
    func setEnabled(_ enabled: Bool) -> Bool {
        requests.append(enabled)
        guard acceptsChanges else { return false }

        // "Enabled" and "awaiting approval" are mutually exclusive in `SMAppService`: a
        // registration the user has not approved yet is not running. Modelling them as
        // independent flags would let the tests assert a state macOS cannot produce.
        isEnabled = enabled && !requiresApproval
        if !enabled { requiresApproval = false }
        return true
    }

    func openLoginItemsSettings() {}
}

@MainActor
private func makeHarness(
    settings: AppSettings = .default,
    brightnessSupported: Bool = true,
    colorSupported: Bool = true,
    displays: [DisplayInfo]? = nil,
    loginItemRegisters: Bool = true,
    loginItemRequiresApproval: Bool = false,
    loginItemAlreadyEnabled: Bool = false
) -> Harness {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("amber-vm-\(UUID().uuidString)", isDirectory: true)
    let store = SettingsStore(fileURL: directory.appendingPathComponent("settings.json"))
    store.save(settings)

    let displayList = displays ?? [
        DisplayInfo(
            nativeId: 1,
            name: "Test display",
            identifier: "1",
            isPrimary: true,
            supportsBrightness: true,
            supportsColorTemperature: true
        )
    ]

    let displayService = FakeDisplayService(displays: displayList)
    let brightnessService = FakeBrightnessService(isSupported: brightnessSupported)
    let colorService = FakeColorService(isSupported: colorSupported)
    let themeService = FakeTheme()
    let schedule = ScheduleService(colorService: colorService)
    let loginItemService = FakeLoginItemService(
        acceptsChanges: loginItemRegisters,
        requiresApproval: loginItemRequiresApproval,
        isEnabled: loginItemAlreadyEnabled
    )

    let viewModel = MainViewModel(
        store: store,
        displayService: displayService,
        brightnessService: brightnessService,
        colorService: colorService,
        schedule: schedule,
        theme: themeService,
        loginItemService: loginItemService
    )

    return Harness(
        store: store,
        displays: displayService,
        brightness: brightnessService,
        color: colorService,
        theme: themeService,
        loginItem: loginItemService,
        viewModel: viewModel
    )
}
