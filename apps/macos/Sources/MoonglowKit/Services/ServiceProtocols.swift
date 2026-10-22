import Foundation

/// Enumerates displays and reports what Moonglow can control on each one.
///
/// Ported from `IDisplayService` in `Services/DisplayServices.cs`.
public protocol DisplayService: AnyObject {
    var displays: [DisplayInfo] { get }
    func refresh() -> [DisplayInfo]
}

/// Hardware (DDC/CI) brightness.
///
/// Ported from `IBrightnessService`.
public protocol BrightnessService: AnyObject {
    var isSupported: Bool { get }
    func setBrightness(percent: Int)
    func dispose()
    /// Re-probes the backend when support is currently unavailable.
    ///
    /// A DDC-capable panel may answer late — powered on after launch, or not yet ready at
    /// login — and quitting to re-probe must never be the recovery path. Backends without
    /// anything to re-probe keep the default no-op.
    ///
    /// Blocking: a DDC probe sleeps between transactions. Main-thread callers must use
    /// ``refreshSupportAsync(completion:)`` instead.
    func refreshSupport()

    /// Re-probes the backend off the calling thread.
    ///
    /// The probe runs on the backend's serial queue; `completion` fires on an unspecified
    /// queue with `true` when this probe newly established support (absent before, present
    /// after). Backends without anything to re-probe keep the default, which runs the sync
    /// probe inline and therefore always completes `false` unless support flipped.
    func refreshSupportAsync(completion: (@Sendable (Bool) -> Void)?)
}

public extension BrightnessService {
    func dispose() {}
    func refreshSupport() {}
    func refreshSupportAsync(completion: (@Sendable (Bool) -> Void)?) {
        let wasSupported = isSupported
        refreshSupport()
        completion?(isSupported && !wasSupported)
    }
}

/// Uses hardware brightness when its DDC/CI probe succeeds, otherwise software gamma dimming.
public final class BrightnessFallbackService: BrightnessService {

    private let hardware: BrightnessService
    private let software: BrightnessService

    public init(hardware: BrightnessService, software: BrightnessService) {
        self.hardware = hardware
        self.software = software
    }

    public var isSupported: Bool {
        hardware.isSupported || software.isSupported
    }

    public func setBrightness(percent: Int) {
        if hardware.isSupported {
            // Exactly one mechanism owns brightness: a fallback episode may have left
            // the gamma dimmed, so reset it before the hardware write or the two would
            // compound. Nearly free on the steady path — the gamma cache skips the
            // rewrite when already at 100%.
            software.setBrightness(percent: 100)
            AppLog.shared.log(.brightness, "brightness \(percent)% → hardware DDC")
            hardware.setBrightness(percent: percent)
        } else if software.isSupported {
            AppLog.shared.log(.brightness, "brightness \(percent)% → software gamma")
            software.setBrightness(percent: percent)
        } else {
            AppLog.shared.log(.brightness, "brightness \(percent)% → unsupported, dropped")
        }
    }

    public func refreshSupport() {
        // Software support is derived live from the displays; only hardware can be late.
        hardware.refreshSupport()
    }

    public func refreshSupportAsync(completion: (@Sendable (Bool) -> Void)?) {
        hardware.refreshSupportAsync(completion: completion)
    }

    public func dispose() {
        hardware.dispose()
        software.dispose()
    }
}

/// Gamma-table colour temperature.
///
/// Ported from `IColorTemperatureService`.
public protocol ColorTemperatureService: AnyObject {
    var isSupported: Bool { get }
    @discardableResult
    func apply(kelvin: Int) -> Bool
    func reset()

    /// Drops any cached ramp, so the next ``apply(kelvin:)`` writes again even when the
    /// target has not changed.
    ///
    /// The gamma tables are global state that anything can replace: a display change, a
    /// wake from sleep, a colour-profile change, or another gamma app. Without this, a
    /// backend that skips unchanged writes can only trust its own last write, and a
    /// display whose ramp was cleared stays untinted until the target moves. Backends
    /// that keep no cache keep the default no-op.
    func invalidate()
}

public extension ColorTemperatureService {
    func invalidate() {}
}

/// The four global hot-key actions. Raw values double as Carbon hot-key IDs, so they
/// are part of the contract: never renumber.
public enum HotKeyAction: UInt32, Sendable, CaseIterable {
    case increaseBrightness = 1
    case decreaseBrightness = 2
    case increaseTemperature = 3
    case decreaseTemperature = 4
}

/// Registers the global hot keys with the system.
///
/// The view model owns the assignments (they are settings); this service only carries
/// them to the platform API. A nil shortcut disables its action.
public protocol HotKeyService: AnyObject {
    func updateHotKeys(_ assignments: [(action: HotKeyAction, shortcut: HotKeyShortcut?)])
}

/// Registers Moonglow as a login item so it starts with the Mac.
///
/// The system owns this state, not the app: the user can remove the login item in System Settings,
/// and macOS can accept a registration but hold it pending approval. The protocol therefore
/// reports status instead of storing a flag, and ``setEnabled(_:)`` says whether the system
/// accepted the change so a caller never records intent that did not take effect.
public protocol LoginItemService: AnyObject {
    /// Whether the app is currently registered to open at login, as the system sees it.
    var isEnabled: Bool { get }

    /// Whether macOS has the registration parked until the user approves it.
    var requiresApproval: Bool { get }

    /// Registers or unregisters the app. Returns false when the system refused the change.
    @discardableResult
    func setEnabled(_ enabled: Bool) -> Bool

    /// Opens the Login Items pane so a pending registration can be approved.
    func openLoginItemsSettings()
}
