import Foundation
import Combine

/// UI state for both the quick-controls popup and the settings window.
///
/// Ported from `ViewModels/MainViewModel.cs`. This is the single shared instance that
/// keeps the popup and the settings window in sync — do not create a second one.
///
/// ## Range rules (preserved from the original)
/// - Brightness: `1...100` percent.
/// - Colour temperature: `900...6600` K, in 100 K steps.
///
/// ## Observation
/// Published properties keep the popup and settings window in sync on macOS 13 and later.
@MainActor
public final class MainViewModel: ObservableObject {

    // MARK: - Dependencies

    private let store: SettingsStore
    private let displayService: DisplayService
    private let brightnessService: BrightnessService
    private let colorService: ColorTemperatureService
    private let schedule: ScheduleService
    private let loginItemService: LoginItemService

    // MARK: - State

    /// Normalized settings. Mutate through the properties below so writes are persisted.
    @Published public private(set) var settings: AppSettings

    /// Active displays, refreshed on demand.
    @Published public private(set) var displays: [DisplayInfo] = []

    /// The temperature currently on screen. In automatic mode this tracks the schedule
    /// rather than the saved manual value, which is why it is stored separately.
    @Published private var displayedTemperature: Int

    // MARK: - Callbacks

    /// Raised when the user asks for the settings window.
    public var onShowSettings: (() -> Void)?
    /// Raised when the user asks to quit.
    public var onQuit: (() -> Void)?

    // MARK: - Init

    public init(
        store: SettingsStore,
        displayService: DisplayService,
        brightnessService: BrightnessService,
        colorService: ColorTemperatureService,
        schedule: ScheduleService,
        loginItemService: LoginItemService
    ) {
        self.store = store
        self.displayService = displayService
        self.brightnessService = brightnessService
        self.colorService = colorService
        self.schedule = schedule
        self.loginItemService = loginItemService

        let loaded = store.load()
        self.settings = loaded
        self.displayedTemperature = loaded.autoColorTemperature
            ? ScheduleCalculator.temperature(for: loaded, at: Date())
            : loaded.manualColorTempK

        schedule.settingsProvider = { [weak self] in self?.settings ?? .default }
        schedule.onTemperatureApplied = { [weak self] kelvin in
            // Applied by the scheduler, so record the displayed value without persisting
            // it as a manual override.
            self?.displayedTemperature = kelvin
        }

        refreshDisplays()
    }

    // MARK: - Displays

    public var hasNoDisplays: Bool { displays.isEmpty }

    public var hasBrightnessSupport: Bool {
        brightnessService.isSupported && displays.contains { $0.supportsBrightness }
    }

    public var hasColorSupport: Bool {
        colorService.isSupported && displays.contains { $0.supportsColorTemperature }
    }

    public var brightnessStatus: String {
        hasBrightnessSupport
            ? "Brightness control is available."
            : "Brightness control is unavailable for the detected display connection."
    }

    public var colorStatus: String {
        hasColorSupport
            ? "CoreGraphics gamma control is available."
            : "Color-temperature control is unavailable on this system."
    }

    /// Re-enumerates active displays.
    public func refreshDisplays() {
        displays = displayService.refresh()
    }

    // MARK: - Brightness

    public var brightness: Int {
        get { settings.manualBrightness }
        set {
            let clamped = AppSettings.clampBrightness(newValue)
            guard clamped != settings.manualBrightness else { return }

            mutateSettings { $0.manualBrightness = clamped }
            brightnessService.setBrightness(percent: clamped)
        }
    }

    // MARK: - Colour temperature

    public var colorTemperature: Int {
        get { displayedTemperature }
        set {
            let clamped = AppSettings.clampKelvin(newValue)
            guard clamped != displayedTemperature else { return }

            displayedTemperature = clamped
            guard !autoColorTemperature else { return }

            mutateSettings { $0.manualColorTempK = clamped }
            colorService.apply(kelvin: clamped)
        }
    }

    public var autoColorTemperature: Bool {
        get { settings.autoColorTemperature }
        set {
            guard newValue != settings.autoColorTemperature else { return }

            mutateSettings { $0.autoColorTemperature = newValue }
            if newValue {
                schedule.tick()
            } else {
                colorService.apply(kelvin: displayedTemperature)
            }
        }
    }

    /// The manual slider is only live when automatic mode is off and the hardware supports it.
    public var manualColorEnabled: Bool { !autoColorTemperature && hasColorSupport }

    // MARK: - Schedule targets

    public var dayColorTemp: Int {
        get { settings.dayColorTempK }
        set { setTemperatureTarget(newValue, isDay: true) }
    }

    public var nightColorTemp: Int {
        get { settings.nightColorTempK }
        set { setTemperatureTarget(newValue, isDay: false) }
    }

    /// Applies both schedule targets in one write.
    ///
    /// The two-handle control fires continuously while dragging, so routing the endpoints
    /// through two separate setters would persist settings and re-evaluate the schedule
    /// twice per update.
    public func setTemperatureRange(_ range: TemperatureRange) {
        guard range.night != settings.nightColorTempK || range.day != settings.dayColorTempK else {
            return
        }

        mutateSettings {
            $0.nightColorTempK = range.night
            $0.dayColorTempK = range.day
        }

        if autoColorTemperature { schedule.tick() }
    }

    public var morningStart: TimeOfDay? {
        get { settings.morningStart }
        set { setTime(newValue, \.morningStart) }
    }

    public var morningEnd: TimeOfDay? {
        get { settings.morningEnd }
        set { setTime(newValue, \.morningEnd) }
    }

    public var eveningStart: TimeOfDay? {
        get { settings.eveningStart }
        set { setTime(newValue, \.eveningStart) }
    }

    public var eveningEnd: TimeOfDay? {
        get { settings.eveningEnd }
        set { setTime(newValue, \.eveningEnd) }
    }

    // MARK: - Startup

    /// Whether Night Mode Controller should start with the Mac.
    ///
    /// This is the user's stated intent; the system holds the actual registration. A change the
    /// system refuses is deliberately *not* recorded, so the toggle snaps back to the real state
    /// instead of claiming a registration that does not exist.
    public var launchAtLogin: Bool {
        get { settings.launchAtLogin }
        set {
            guard newValue != settings.launchAtLogin else { return }
            guard loginItemService.setEnabled(newValue) else {
                loginItemRegistrationFailed = true
                return
            }

            loginItemRegistrationFailed = false
            mutateSettings { $0.launchAtLogin = newValue }
            loginItemRequiresApproval = loginItemService.requiresApproval
        }
    }

    @Published public private(set) var loginItemRegistrationFailed = false

    /// True when macOS has accepted the registration but is holding it until the user approves it
    /// in System Settings. The toggle reads as on, yet Night Mode Controller will not start until then.
    @Published public private(set) var loginItemRequiresApproval = false

    /// Describes the login-item state for the settings window.
    ///
    /// Read live from the service rather than derived from the setting, because the two can
    /// legitimately disagree — the user can remove the login item in System Settings without
    /// Night Mode Controller ever finding out.
    public var launchAtLoginStatus: String {
        if loginItemRegistrationFailed {
            return "macOS did not accept this login item. Run the packaged app on macOS 13 or later, then try again."
        }
        // Approval is tested first: a request the user has not approved yet is not running, and
        // saying "opens automatically" would be a lie the user cannot act on.
        if loginItemService.requiresApproval {
            return "Waiting for approval in System Settings › General › Login Items."
        }
        if loginItemService.isEnabled {
            return "Night Mode Controller opens automatically when you log in."
        }
        if settings.launchAtLogin {
            return "Could not register Night Mode Controller as a login item on this system."
        }
        return "Night Mode Controller opens only when you launch it."
    }

    /// Re-asserts the stored preference on the system. Called once at launch.
    ///
    /// The registration is not the app's to assume: it lives outside the settings file, so a
    /// preference written on one run may no longer hold on the next.
    public func applyLoginItemState(isFirstLaunchAfterInstall: Bool = false) {
        if isFirstLaunchAfterInstall {
            if loginItemService.isEnabled || loginItemService.requiresApproval {
                _ = loginItemService.setEnabled(false)
            }
            if settings.launchAtLogin {
                mutateSettings { $0.launchAtLogin = false }
            }
            loginItemRequiresApproval = loginItemService.requiresApproval
            return
        }

        if settings.launchAtLogin {
            if !loginItemService.setEnabled(true) {
                // Keep the stored intent so the status can say the registration did not take; the
                // toggle would otherwise silently forget what the user asked for.
                loginItemRequiresApproval = loginItemService.requiresApproval
                return
            }
        } else if loginItemService.isEnabled {
            // Registered, but Night Mode Controller never recorded it — almost certainly added by hand in System
            // Settings before this setting existed. Adopt it rather than reporting a state that
            // contradicts the toggle, and never switch it off behind the user's back.
            mutateSettings { $0.launchAtLogin = true }
        }

        loginItemRequiresApproval = loginItemService.requiresApproval
    }

    public func openLoginItemsSettings() {
        loginItemService.openLoginItemsSettings()
    }

    // MARK: - Lifecycle

    /// Pushes the persisted values to the hardware. Called once at launch, after the
    /// windows exist.
    public func applyInitialState() {
        if hasBrightnessSupport {
            brightnessService.setBrightness(percent: brightness)
        }
        if !autoColorTemperature && hasColorSupport {
            colorService.apply(kelvin: displayedTemperature)
        }
    }

    public func showSettings() { onShowSettings?() }

    public func quit() { onQuit?() }

    // MARK: - Private

    private func mutateSettings(_ change: (inout AppSettings) -> Void) {
        var copy = settings
        change(&copy)
        settings = copy.normalized()
        store.save(settings)
    }

    private func setTemperatureTarget(_ value: Int, isDay: Bool) {
        let clamped = AppSettings.clampKelvin(value)
        let current = isDay ? settings.dayColorTempK : settings.nightColorTempK
        guard clamped != current else { return }

        mutateSettings {
            if isDay {
                $0.dayColorTempK = clamped
            } else {
                $0.nightColorTempK = clamped
            }
        }

        if autoColorTemperature { schedule.tick() }
    }

    private func setTime(_ value: TimeOfDay?, _ keyPath: WritableKeyPath<AppSettings, TimeOfDay>) {
        guard let value, value.isValid else { return }
        guard settings[keyPath: keyPath] != value else { return }

        mutateSettings { $0[keyPath: keyPath] = value }

        if autoColorTemperature { schedule.tick() }
    }
}
