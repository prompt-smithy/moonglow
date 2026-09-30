import Foundation

/// Enumerates displays and reports what Night Mode Controller can control on each one.
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
}

/// Gamma-table colour temperature.
///
/// Ported from `IColorTemperatureService`.
public protocol ColorTemperatureService: AnyObject {
    var isSupported: Bool { get }
    @discardableResult
    func apply(kelvin: Int) -> Bool
    func reset()
}

/// Applies the palette matching the current system appearance.
///
/// This lives in AmberKit as a protocol so the view model can trigger theme changes
/// without AmberKit depending on SwiftUI. The app target provides the implementation.
///
/// Settings are passed in rather than looked up so the theme layer never re-reads from
/// disk and the view model stays the single source of truth.
@MainActor
public protocol ThemeApplying: AnyObject {
    func applyForCurrentMode(settings: AppSettings)
}

/// Registers Night Mode Controller as a login item so it starts with the Mac.
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
