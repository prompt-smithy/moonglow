import AppKit
import Combine
import AmberKit

/// Selects one of the two built-in appearances from the current system appearance.
@MainActor
final class ThemeService: ObservableObject {

    @Published private(set) var current: AppTheme = ThemeCatalog.light

    private var appearanceObservation: NSKeyValueObservation?

    init() {
        appearanceObservation = NSApplication.shared.observe(\.effectiveAppearance, options: [.new]) {
            [weak self] _, _ in
            MainActor.assumeIsolated {
                self?.reapply()
            }
        }
        reapply()
    }

    private func reapply() {
        let isDark = NSApplication.shared.effectiveAppearance
            .bestMatch(from: [.aqua, .darkAqua]) == .darkAqua

        current = isDark ? ThemeCatalog.dark : ThemeCatalog.light
    }
}
