import AppKit
import AmberKit
import Observation

/// Applies the palette that matches the current system appearance.
///
/// Replaces the Avalonia resource-dictionary writes in `Services/ThemeService.cs`. Views
/// observe ``current``, so a palette change re-renders them without recreating windows.
///
/// `effectiveAppearance` is KVO-observable, which is the AppKit equivalent of Avalonia's
/// `ActualThemeVariantChanged`.
@MainActor
@Observable
final class ThemeService: ThemeApplying {

    /// The palette in effect right now.
    private(set) var current: AppTheme = ThemeCatalog.light[0]

    private var latestSettings: AppSettings = .default
    private var appearanceObservation: NSKeyValueObservation?

    init() {
        appearanceObservation = NSApplication.shared.observe(\.effectiveAppearance, options: [.new]) {
            [weak self] _, _ in
            MainActor.assumeIsolated {
                self?.reapply()
            }
        }
    }

    /// Applies the palette for the current appearance using the supplied settings.
    func applyForCurrentMode(settings: AppSettings) {
        latestSettings = settings
        reapply()
    }

    private func reapply() {
        let isDark = NSApplication.shared.effectiveAppearance
            .bestMatch(from: [.aqua, .darkAqua]) == .darkAqua

        current = isDark
            ? ThemeCatalog.dark(named: latestSettings.darkModeTheme)
            : ThemeCatalog.light(named: latestSettings.lightModeTheme)
    }
}
