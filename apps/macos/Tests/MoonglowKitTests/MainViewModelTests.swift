import Foundation
import Testing
@testable import MoonglowKit

/// Covers the behaviour the .NET test suite did not: clamping, persistence triggers,
/// capability gating, and the observation side effects that replace `SetProperty`.
@Suite("Main view model")
@MainActor
struct MainViewModelTests {

    @Test("Hardware brightness takes priority over software dimming")
    func hardwareBrightnessTakesPriority() {
        let hardware = FakeBrightnessService(isSupported: true)
        let software = FakeBrightnessService(isSupported: true)
        let service = BrightnessFallbackService(hardware: hardware, software: software)

        service.setBrightness(percent: 40)

        #expect(service.isSupported)
        #expect(hardware.applied == [40])
        // The software reset, not a dim: exactly one mechanism owns brightness.
        #expect(software.applied == [100])
    }

    @Test("Routing to hardware resets software dimming so the two never compound")
    func hardwareRoutingResetsSoftwareDimming() {
        let hardware = FakeBrightnessService(isSupported: true)
        let software = FakeBrightnessService(isSupported: true)
        let service = BrightnessFallbackService(hardware: hardware, software: software)

        service.setBrightness(percent: 40)
        service.setBrightness(percent: 70)

        #expect(hardware.applied == [40, 70])
        #expect(software.applied == [100, 100])
    }

    @Test("Software brightness is used when hardware DDC is unavailable")
    func softwareBrightnessIsFallback() {
        let hardware = FakeBrightnessService(isSupported: false)
        let software = FakeBrightnessService(isSupported: true)
        let service = BrightnessFallbackService(hardware: hardware, software: software)

        service.setBrightness(percent: 40)

        #expect(service.isSupported)
        #expect(hardware.applied.isEmpty)
        #expect(software.applied == [40])
    }

    @Test("Brightness is unavailable when neither strategy is supported")
    func unsupportedBrightnessHasNoFallback() {
        let hardware = FakeBrightnessService(isSupported: false)
        let software = FakeBrightnessService(isSupported: false)
        let service = BrightnessFallbackService(hardware: hardware, software: software)

        service.setBrightness(percent: 40)

        #expect(!service.isSupported)
        #expect(hardware.applied.isEmpty)
        #expect(software.applied.isEmpty)
    }

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

    @Test("Turning automatic off preserves the held temperature across a restart")
    func heldTemperatureSurvivesRestart() {
        var settings = AppSettings.default
        settings.dayColorTempK = 3700
        settings.nightColorTempK = 3700
        let harness = makeHarness(settings: settings)
        harness.viewModel.autoColorTemperature = false
        let restarted = makeHarness(settings: harness.store.load())
        #expect(restarted.viewModel.colorTemperature == 3700)
        #expect(!restarted.viewModel.autoColorTemperature)
    }

    @Test("Display refresh restores manual temperature and brightness")
    func refreshRestoresManualControls() {
        var settings = AppSettings.default
        settings.autoColorTemperature = false
        settings.manualColorTempK = 3400
        settings.manualBrightness = 47
        let harness = makeHarness(settings: settings)
        harness.viewModel.applyInitialState()
        let previousWrites = harness.color.applied.count
        let previousBrightnessWrites = harness.brightness.applied.count
        harness.viewModel.refreshDisplays()
        #expect(harness.color.applied.count == previousWrites + 1)
        #expect(harness.color.applied.last == 3400)
        #expect(harness.brightness.applied.count == previousBrightnessWrites + 1)
        #expect(harness.brightness.applied.last == 47)
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
        #expect(supported.viewModel.brightnessStatus == "Brightness control is available.")
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

    @Test("Hardware without a controllable display explains itself")
    func hardwareWithoutControllableDisplayExplainsItself() {
        let inert = DisplayInfo(
            nativeId: 9,
            name: "Inert",
            isPrimary: true,
            supportsBrightness: false,
            supportsColorTemperature: false
        )
        let harness = makeHarness(brightnessSupported: true, displays: [inert])

        #expect(!harness.viewModel.hasBrightnessSupport)
        #expect(harness.viewModel.brightnessLimitation?.contains("main display") == true)
    }

    @Test("No limitation is reported when brightness works or is genuinely absent")
    func limitationOnlyForUnusableHardware() {
        let working = makeHarness()
        #expect(working.viewModel.brightnessLimitation == nil)

        let unsupported = makeHarness(brightnessSupported: false)
        #expect(unsupported.viewModel.brightnessLimitation == nil)

        let empty = makeHarness(displays: [])
        #expect(empty.viewModel.brightnessLimitation == nil)
    }

    @Test("Refreshing re-enumerates displays")
    func refreshingReenumerates() {
        let harness = makeHarness()

        harness.viewModel.refreshDisplays()

        // Once from init, once from the explicit refresh.
        #expect(harness.displays.refreshCount == 2)
    }

    @Test("Refreshing re-probes brightness hardware")
    func refreshingReprobesBrightness() {
        let harness = makeHarness()

        harness.viewModel.refreshDisplays()

        // Once from init, once from the explicit refresh.
        #expect(harness.brightness.refreshCount == 2)
    }

    @Test("Refreshing re-probes hardware, not software dimming")
    func refreshReprobesHardwareOnly() {
        let hardware = FakeBrightnessService(isSupported: false)
        let software = FakeBrightnessService(isSupported: true)
        let service = BrightnessFallbackService(hardware: hardware, software: software)

        service.refreshSupport()

        #expect(hardware.refreshCount == 1)
        #expect(software.refreshCount == 0)
    }

    @Test("The default async probe reports whether support was newly established")
    func asyncProbeDefaultReportsNewSupport() {
        let late = LateBrightnessService()
        let first = FlagBox()
        late.refreshSupportAsync { first.value = $0 }
        #expect(first.value == true)

        let second = FlagBox()
        late.refreshSupportAsync { second.value = $0 }
        #expect(second.value == false)

        let stable = FakeBrightnessService(isSupported: true)
        let third = FlagBox()
        stable.refreshSupportAsync { third.value = $0 }
        #expect(third.value == false)
    }

    @Test("Late hardware triggers a second enumeration and applies the persisted brightness")
    func lateHardwareReenumeratesAndApplies() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("moonglow-vm-\(UUID().uuidString)", isDirectory: true)
        let store = SettingsStore(fileURL: directory.appendingPathComponent("settings.json"))
        store.save(AppSettings.default)

        let displayService = FakeDisplayService(displays: [
            DisplayInfo(
                nativeId: 1,
                name: "Test display",
                isPrimary: true,
                supportsBrightness: true,
                supportsColorTemperature: true
            )
        ])
        let brightnessService = LateBrightnessService()
        let colorService = FakeColorService(isSupported: true)
        let viewModel = MainViewModel(
            store: store,
            displayService: displayService,
            brightnessService: brightnessService,
            colorService: colorService,
            schedule: ScheduleService(colorService: colorService),
            loginItemService: FakeLoginItemService(),
            hotKeyService: FakeHotKeyService()
        )

        // The settle hop runs on the main actor behind the test; poll briefly rather
        // than asserting on unsynchronised state.
        for _ in 0..<100 {
            if displayService.refreshCount >= 2 { break }
            try await Task.sleep(for: .milliseconds(10))
        }

        #expect(displayService.refreshCount == 2)
        #expect(brightnessService.applied == [AppSettings.default.manualBrightness])
        #expect(viewModel.hasBrightnessSupport)
    }

    @Test("Refreshing drops the cached gamma ramp")
    func refreshingInvalidatesGammaCache() {
        let harness = makeHarness()

        harness.viewModel.refreshDisplays()

        // Once from init, once from the explicit refresh: a display may have moved under
        // the cached ramp, so the next tick has to write it again.
        #expect(harness.color.invalidateCount == 2)
    }

    @Test("Hot keys register the defaults at launch")
    func hotKeysRegisterDefaultsAtLaunch() {
        let harness = makeHarness()

        #expect(harness.hotKeys.updateCount == 1)
        #expect(harness.hotKeys.shortcut(for: .increaseBrightness) == AppSettings.defaultIncreaseBrightnessShortcut)
        #expect(harness.hotKeys.shortcut(for: .decreaseBrightness) == AppSettings.defaultDecreaseBrightnessShortcut)
        #expect(harness.hotKeys.shortcut(for: .increaseTemperature) == AppSettings.defaultIncreaseTemperatureShortcut)
        #expect(harness.hotKeys.shortcut(for: .decreaseTemperature) == AppSettings.defaultDecreaseTemperatureShortcut)
    }

    @Test("Recording a shortcut persists it and re-registers")
    func recordingShortcutPersistsAndReregisters() {
        let harness = makeHarness()
        let recorded = HotKeyShortcut(keyCode: 0, modifiers: HotKeyShortcut.commandModifier)

        harness.viewModel.increaseBrightnessShortcut = recorded

        #expect(harness.viewModel.increaseBrightnessShortcut == recorded)
        #expect(harness.store.load().increaseBrightnessShortcut == recorded)
        #expect(harness.hotKeys.updateCount == 2)
        #expect(harness.hotKeys.shortcut(for: .increaseBrightness) == recorded)
    }

    @Test("A duplicate shortcut is stolen, not shared")
    func duplicateShortcutIsStolen() {
        let harness = makeHarness()
        let shared = HotKeyShortcut(keyCode: 12, modifiers: HotKeyShortcut.controlModifier)

        harness.viewModel.increaseBrightnessShortcut = shared
        harness.viewModel.decreaseBrightnessShortcut = shared

        #expect(harness.viewModel.increaseBrightnessShortcut == nil)
        #expect(harness.viewModel.decreaseBrightnessShortcut == shared)
        #expect(harness.hotKeys.shortcut(for: .increaseBrightness) == nil)
        #expect(harness.hotKeys.shortcut(for: .decreaseBrightness) == shared)
    }

    @Test("Clearing a shortcut disables its hot key")
    func clearingShortcutDisablesHotKey() {
        let harness = makeHarness()

        harness.viewModel.increaseBrightnessShortcut = nil

        #expect(harness.store.load().increaseBrightnessShortcut == nil)
        #expect(harness.hotKeys.shortcut(for: .increaseBrightness) == nil)
    }

    @Test("Modifier-less shortcuts are refused")
    func modifierLessShortcutsAreRefused() {
        let harness = makeHarness()
        let original = harness.viewModel.increaseBrightnessShortcut

        harness.viewModel.increaseBrightnessShortcut = HotKeyShortcut(keyCode: 0, modifiers: 0)

        #expect(harness.viewModel.increaseBrightnessShortcut == original)
        #expect(harness.hotKeys.updateCount == 1)
    }

    @Test("Brightness hot keys step within 1...100")
    func brightnessHotKeysStep() {
        var settings = AppSettings.default
        settings.manualBrightness = 99
        let harness = makeHarness(settings: settings)

        harness.viewModel.increaseBrightness()
        #expect(harness.viewModel.brightness == 100)
        #expect(harness.brightness.applied == [100])

        harness.viewModel.decreaseBrightness()
        harness.viewModel.decreaseBrightness()
        #expect(harness.viewModel.brightness == 90)
        #expect(harness.brightness.applied == [100, 95, 90])
    }

    @Test("Brightness hot keys are inert without support")
    func brightnessHotKeysInertWithoutSupport() {
        let harness = makeHarness(brightnessSupported: false)

        harness.viewModel.increaseBrightness()
        harness.viewModel.decreaseBrightness()

        #expect(harness.brightness.applied.isEmpty)
        #expect(harness.viewModel.brightness == AppSettings.defaultManualBrightness)
    }

    @Test("Temperature hot keys leave automatic mode and step")
    func temperatureHotKeysLeaveAutomaticMode() {
        var settings = AppSettings.default
        settings.autoColorTemperature = true
        let harness = makeHarness(settings: settings)
        harness.viewModel.confirmAutomaticModeExit = { true }
        let before = harness.viewModel.colorTemperature

        harness.viewModel.increaseColorTemperature()

        #expect(harness.viewModel.autoColorTemperature == false)
        #expect(harness.viewModel.colorTemperature == before + MainViewModel.hotKeyKelvinStep)
        #expect(harness.color.applied.last == before + MainViewModel.hotKeyKelvinStep)
    }

    @Test("A refused confirmation keeps automatic mode and applies nothing")
    func refusedConfirmationKeepsAutomaticMode() {
        var settings = AppSettings.default
        settings.autoColorTemperature = true
        let harness = makeHarness(settings: settings)
        harness.viewModel.confirmAutomaticModeExit = { false }

        harness.viewModel.decreaseColorTemperature()

        #expect(harness.viewModel.autoColorTemperature == true)
        #expect(harness.color.applied.isEmpty)
        #expect(harness.store.load().autoColorTemperature == true)
    }

    @Test("An unwired host never silently leaves automatic mode")
    func unwiredHostKeepsAutomaticMode() {
        var settings = AppSettings.default
        settings.autoColorTemperature = true
        let harness = makeHarness(settings: settings)

        harness.viewModel.increaseColorTemperature()

        #expect(harness.viewModel.autoColorTemperature == true)
        #expect(harness.color.applied.isEmpty)
    }

    @Test("Temperature hot keys clamp at the range ends")
    func temperatureHotKeysClamp() {
        var settings = AppSettings.default
        settings.autoColorTemperature = false
        settings.manualColorTempK = 6600
        let hot = makeHarness(settings: settings)

        hot.viewModel.increaseColorTemperature()

        #expect(hot.viewModel.colorTemperature == 6600)
        #expect(hot.color.applied.isEmpty)
    }

    @Test("Temperature hot keys are inert without support")
    func temperatureHotKeysInertWithoutSupport() {
        var settings = AppSettings.default
        settings.autoColorTemperature = true
        let harness = makeHarness(settings: settings, colorSupported: false)

        harness.viewModel.increaseColorTemperature()

        #expect(harness.viewModel.autoColorTemperature == true)
        #expect(harness.color.applied.isEmpty)
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

    @Test("The morning window cannot be inverted")
    func morningWindowCannotInvert() {
        let lateStart = makeHarness()
        lateStart.viewModel.morningStart = TimeOfDay(hour: 20, minute: 0)
        #expect(lateStart.viewModel.morningStart == TimeOfDay(hour: 6, minute: 59))

        let earlyEnd = makeHarness()
        earlyEnd.viewModel.morningEnd = TimeOfDay(hour: 5, minute: 0)
        #expect(earlyEnd.viewModel.morningEnd == TimeOfDay(hour: 6, minute: 1))
    }

    @Test("The evening window cannot be inverted")
    func eveningWindowCannotInvert() {
        let lateStart = makeHarness()
        lateStart.viewModel.eveningStart = TimeOfDay(hour: 19, minute: 0)
        #expect(lateStart.viewModel.eveningStart == TimeOfDay(hour: 18, minute: 29))

        let earlyEnd = makeHarness()
        earlyEnd.viewModel.eveningEnd = TimeOfDay(hour: 17, minute: 0)
        #expect(earlyEnd.viewModel.eveningEnd == TimeOfDay(hour: 17, minute: 31))
    }

    @Test("Touching transitions are allowed")
    func touchingTransitionsAreAllowed() {
        let harness = makeHarness()

        harness.viewModel.morningEnd = AppSettings.defaultEveningStart

        #expect(harness.viewModel.morningEnd == AppSettings.defaultEveningStart)
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
        #expect(harness.viewModel.loginItemRegistrationFailed)
        #expect(harness.viewModel.launchAtLoginStatus.contains("did not accept"))
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
        #expect(harness.viewModel.loginItemRegistrationFailed)
        #expect(harness.viewModel.launchAtLoginStatus.contains("did not accept"))
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

    @Test("Nothing is registered at launch when the preference is off")
    func nothingRegisteredWhenPreferenceOff() {
        let harness = makeHarness()

        harness.viewModel.applyLoginItemState()

        #expect(harness.loginItem.requests.isEmpty)
        #expect(harness.viewModel.launchAtLoginStatus.contains("only when you launch"))
    }

    @Test("A login item Moonglow did not create is adopted, not switched off")
    func existingRegistrationIsAdopted() {
        // The user added Moonglow at login in System Settings before this setting existed. Launching
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
    let loginItem: FakeLoginItemService
    let hotKeys: FakeHotKeyService
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
    private(set) var refreshCount = 0

    init(isSupported: Bool) {
        self.isSupported = isSupported
    }

    func setBrightness(percent: Int) {
        applied.append(percent)
    }

    func refreshSupport() {
        refreshCount += 1
    }
}

/// Hardware that answers late: unsupported until the first probe, like a panel whose
/// DDC bridge wakes up after launch.
private final class LateBrightnessService: BrightnessService {
    private(set) var isSupported = false
    private(set) var applied: [Int] = []

    func setBrightness(percent: Int) {
        applied.append(percent)
    }

    func refreshSupport() {
        isSupported = true
    }
}

/// Carries one completion value out of a `@Sendable` closure. The default probe
/// completes synchronously, so no locking is needed.
private final class FlagBox: @unchecked Sendable {
    var value: Bool?
}

/// Records hot-key registrations without touching the platform API.
private final class FakeHotKeyService: HotKeyService {
    private(set) var assignments: [(action: HotKeyAction, shortcut: HotKeyShortcut?)] = []
    private(set) var updateCount = 0

    func updateHotKeys(_ assignments: [(action: HotKeyAction, shortcut: HotKeyShortcut?)]) {
        self.assignments = assignments
        updateCount += 1
    }

    func shortcut(for action: HotKeyAction) -> HotKeyShortcut? {
        assignments.first { $0.action == action }?.shortcut ?? nil
    }
}

private final class FakeColorService: ColorTemperatureService {
    let isSupported: Bool
    private(set) var applied: [Int] = []
    private(set) var resetCount = 0
    private(set) var invalidateCount = 0

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

    func invalidate() {
        invalidateCount += 1
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
        .appendingPathComponent("moonglow-vm-\(UUID().uuidString)", isDirectory: true)
    let store = SettingsStore(fileURL: directory.appendingPathComponent("settings.json"))
    store.save(settings)

    let displayList = displays ?? [
        DisplayInfo(
            nativeId: 1,
            name: "Test display",
            isPrimary: true,
            supportsBrightness: true,
            supportsColorTemperature: true
        )
    ]

    let displayService = FakeDisplayService(displays: displayList)
    let brightnessService = FakeBrightnessService(isSupported: brightnessSupported)
    let colorService = FakeColorService(isSupported: colorSupported)
    let schedule = ScheduleService(colorService: colorService)
    let loginItemService = FakeLoginItemService(
        acceptsChanges: loginItemRegisters,
        requiresApproval: loginItemRequiresApproval,
        isEnabled: loginItemAlreadyEnabled
    )
    let hotKeyService = FakeHotKeyService()

    let viewModel = MainViewModel(
        store: store,
        displayService: displayService,
        brightnessService: brightnessService,
        colorService: colorService,
        schedule: schedule,
        loginItemService: loginItemService,
        hotKeyService: hotKeyService
    )

    return Harness(
        store: store,
        displays: displayService,
        brightness: brightnessService,
        color: colorService,
        loginItem: loginItemService,
        hotKeys: hotKeyService,
        viewModel: viewModel
    )
}
