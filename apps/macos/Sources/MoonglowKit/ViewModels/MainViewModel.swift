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
    private let hotKeyService: HotKeyService

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
        loginItemService: LoginItemService,
        hotKeyService: HotKeyService
    ) {
        self.store = store
        self.displayService = displayService
        self.brightnessService = brightnessService
        self.colorService = colorService
        self.schedule = schedule
        self.loginItemService = loginItemService
        self.hotKeyService = hotKeyService

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
        updateHotKeys()
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

    /// Explains a disabled brightness slider when DDC hardware is up but no display can
    /// take it: hardware brightness targets the main display, and only when it is an
    /// external panel, so a MacBook with the built-in panel as main reports no control
    /// rather than dimming the wrong screen. Nil when there is nothing to explain.
    public var brightnessLimitation: String? {
        guard !hasBrightnessSupport, !hasNoDisplays, brightnessService.isSupported else {
            return nil
        }
        return "Hardware brightness is ready, but it controls the main display only when"
            + " that display is external. Make an external display the main display to enable it."
    }

    public var colorStatus: String {
        hasColorSupport
            ? "CoreGraphics gamma control is available."
            : "Color-temperature control is unavailable on this system."
    }

    /// Re-enumerates active displays, and re-probes brightness hardware off the main thread.
    ///
    /// Display enumeration is fast CoreGraphics calls and stays synchronous. The DDC probe
    /// sleeps between transactions, so it settles asynchronously: when it newly establishes
    /// support, the displays are enumerated again (the brightness flags read the live DDC
    /// state) and the persisted values are pushed to the now-available hardware.
    public func refreshDisplays() {
        brightnessService.refreshSupportAsync { [weak self] newlyAvailable in
            guard newlyAvailable else { return }
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.reloadDisplays()
                // Hardware that answered late can now take the persisted values.
                self.applyInitialState()
            }
        }
        reloadDisplays()
    }

    /// Re-enumerates the displays and drops the cached gamma ramp.
    ///
    /// The ramp is discarded because the display set may have changed under us — a panel
    /// unplugged and plugged back in can reuse its CoreGraphics id — so a cached ramp is no
    /// longer known to be on screen. The next schedule tick writes it again.
    private func reloadDisplays() {
        let fresh = displayService.refresh()
        if fresh != displays {
            displays = fresh
        }
        colorService.invalidate()
    }

    /// Location of the diagnostic log file, for the Settings window to reveal.
    public var logFileURL: URL {
        AppLog.shared.fileURL
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

    /// Whether Moonglow should start with the Mac.
    ///
    /// This is the user's stated intent; the system holds the actual registration. A change the
    /// system refuses is deliberately *not* recorded, so the toggle snaps back to the real state
    /// instead of claiming a registration that does not exist.
    public var launchAtLogin: Bool {
        get { settings.launchAtLogin }
        set {
            guard newValue != settings.launchAtLogin else { return }
            guard loginItemService.setEnabled(newValue) else {
                AppLog.shared.log(.loginItem, "system refused the login-item change")
                loginItemRegistrationFailed = true
                return
            }

            loginItemRegistrationFailed = false
            mutateSettings { $0.launchAtLogin = newValue }
            loginItemRequiresApproval = loginItemService.requiresApproval
            AppLog.shared.log(.loginItem, newValue ? "login item registered" : "login item unregistered")
        }
    }

    @Published public private(set) var loginItemRegistrationFailed = false

    /// True when macOS has accepted the registration but is holding it until the user approves it
    /// in System Settings. The toggle reads as on, yet Moonglow will not start until then.
    @Published public private(set) var loginItemRequiresApproval = false

    /// Describes the login-item state for the settings window.
    ///
    /// Read live from the service rather than derived from the setting, because the two can
    /// legitimately disagree — the user can remove the login item in System Settings without
    /// Moonglow ever finding out.
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
            return "Moonglow opens automatically when you log in."
        }
        if settings.launchAtLogin {
            return "Could not register Moonglow as a login item on this system."
        }
        return "Moonglow opens only when you launch it."
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
                AppLog.shared.log(.loginItem, "stored launch-at-login preference refused at launch")
                loginItemRegistrationFailed = true
                loginItemRequiresApproval = loginItemService.requiresApproval
                return
            }
            loginItemRegistrationFailed = false
        } else if loginItemService.isEnabled {
            // Registered, but Moonglow never recorded it — almost certainly added by hand in System
            // Settings before this setting existed. Adopt it rather than reporting a state that
            // contradicts the toggle, and never switch it off behind the user's back.
            AppLog.shared.log(.loginItem, "adopted a login item Moonglow did not create")
            mutateSettings { $0.launchAtLogin = true }
        }

        loginItemRequiresApproval = loginItemService.requiresApproval
    }

    public func openLoginItemsSettings() {
        loginItemService.openLoginItemsSettings()
    }

    // MARK: - Hot keys

    /// Brightness points per hot-key press.
    public static let hotKeyBrightnessStep = 5

    /// Colour-temperature kelvin per hot-key press: the app-wide step.
    public static let hotKeyKelvinStep = AppSettings.kelvinStep

    /// Asks the user before a temperature hot key leaves automatic mode. Synchronous
    /// because the key press must decide now; the host shows a modal alert. Defaults to
    /// refusing, so an unwired host never silently drops out of automatic mode.
    public var confirmAutomaticModeExit: () -> Bool = { false }

    public func increaseBrightness() {
        guard hasBrightnessSupport else { return }
        brightness += Self.hotKeyBrightnessStep
    }

    public func decreaseBrightness() {
        guard hasBrightnessSupport else { return }
        brightness -= Self.hotKeyBrightnessStep
    }

    /// Raises the colour temperature by one step. In automatic mode the user is asked
    /// first, since leaving it is the only way the change survives the next tick.
    public func increaseColorTemperature() {
        adjustColorTemperature(by: Self.hotKeyKelvinStep)
    }

    /// Lowers the colour temperature by one step, confirming before leaving automatic mode.
    public func decreaseColorTemperature() {
        adjustColorTemperature(by: -Self.hotKeyKelvinStep)
    }

    private func adjustColorTemperature(by delta: Int) {
        guard hasColorSupport else { return }
        if autoColorTemperature {
            guard confirmAutomaticModeExit() else { return }
            autoColorTemperature = false
        }
        colorTemperature += delta
    }

    public var increaseBrightnessShortcut: HotKeyShortcut? {
        get { settings.increaseBrightnessShortcut }
        set { assignShortcut(newValue, to: \.increaseBrightnessShortcut) }
    }

    public var decreaseBrightnessShortcut: HotKeyShortcut? {
        get { settings.decreaseBrightnessShortcut }
        set { assignShortcut(newValue, to: \.decreaseBrightnessShortcut) }
    }

    public var increaseTemperatureShortcut: HotKeyShortcut? {
        get { settings.increaseTemperatureShortcut }
        set { assignShortcut(newValue, to: \.increaseTemperatureShortcut) }
    }

    public var decreaseTemperatureShortcut: HotKeyShortcut? {
        get { settings.decreaseTemperatureShortcut }
        set { assignShortcut(newValue, to: \.decreaseTemperatureShortcut) }
    }

    /// Assigns a shortcut, stealing it from whichever slot holds it so one combination
    /// can never fire two actions. A modifier-less shortcut is refused; nil clears.
    private func assignShortcut(
        _ value: HotKeyShortcut?,
        to keyPath: WritableKeyPath<AppSettings, HotKeyShortcut?>
    ) {
        if let value, !value.isValid { return }
        mutateSettings {
            if value != nil {
                for path in Self.shortcutKeyPaths where path != keyPath {
                    if $0[keyPath: path] == value {
                        $0[keyPath: path] = nil
                    }
                }
            }
            $0[keyPath: keyPath] = value
        }
        updateHotKeys()
    }

    private static let shortcutKeyPaths: [WritableKeyPath<AppSettings, HotKeyShortcut?>] = [
        \.increaseBrightnessShortcut,
        \.decreaseBrightnessShortcut,
        \.increaseTemperatureShortcut,
        \.decreaseTemperatureShortcut,
    ]

    /// Pushes the current assignments to the platform registration.
    public func updateHotKeys() {
        hotKeyService.updateHotKeys([
            (action: .increaseBrightness, shortcut: settings.increaseBrightnessShortcut),
            (action: .decreaseBrightness, shortcut: settings.decreaseBrightnessShortcut),
            (action: .increaseTemperature, shortcut: settings.increaseTemperatureShortcut),
            (action: .decreaseTemperature, shortcut: settings.decreaseTemperatureShortcut),
        ])
    }

    // MARK: - Lifecycle

    /// Pushes the persisted values to the hardware. Called at launch, after the
    /// windows exist, and again whenever a late DDC probe newly establishes support.
    /// Idempotent: it only applies the current settings to currently capable displays.
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
        let clamped = clampScheduleTime(value, for: keyPath)
        guard settings[keyPath: keyPath] != clamped else { return }

        mutateSettings { $0[keyPath: keyPath] = clamped }

        if autoColorTemperature { schedule.tick() }
    }

    /// Keeps `morningStart < morningEnd ≤ eveningStart < eveningEnd` by clamping the edited
    /// value against its neighbours, leaving the other three untouched. Without this an
    /// inverted window collapses the day to night plus one abrupt flash at the boundary.
    private func clampScheduleTime(
        _ value: TimeOfDay,
        for keyPath: WritableKeyPath<AppSettings, TimeOfDay>
    ) -> TimeOfDay {
        let bounds: (lower: Int, upper: Int)
        if keyPath == \AppSettings.morningStart {
            bounds = (0, settings.morningEnd.minutesSinceMidnight - 1)
        } else if keyPath == \AppSettings.morningEnd {
            bounds = (
                settings.morningStart.minutesSinceMidnight + 1,
                settings.eveningStart.minutesSinceMidnight
            )
        } else if keyPath == \AppSettings.eveningStart {
            bounds = (
                settings.morningEnd.minutesSinceMidnight,
                settings.eveningEnd.minutesSinceMidnight - 1
            )
        } else if keyPath == \AppSettings.eveningEnd {
            bounds = (settings.eveningStart.minutesSinceMidnight + 1, 24 * 60 - 1)
        } else {
            return value
        }

        // No valid value exists (the neighbours already touch): refuse the edit rather
        // than invent a time.
        guard bounds.lower <= bounds.upper else { return settings[keyPath: keyPath] }

        let clamped = min(max(value.minutesSinceMidnight, bounds.lower), bounds.upper)
        return TimeOfDay.from(minutesSinceMidnight: clamped)
    }
}
