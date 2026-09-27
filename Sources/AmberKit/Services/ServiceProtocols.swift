import Foundation

/// Enumerates displays and reports what Amber can control on each one.
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
