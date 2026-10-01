import SwiftUI
import AmberKit

extension Color {
    /// Creates a colour from the catalog's `#RRGGBB` or `#AARRGGBB` hex strings.
    ///
    /// Malformed input resolves to clear rather than crashing.
    init(hex: String) {
        var value = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if value.hasPrefix("#") { value.removeFirst() }

        guard !value.isEmpty else {
            self = .clear
            return
        }

        var raw: UInt64 = 0
        guard Scanner(string: value).scanHexInt64(&raw) else {
            self = .clear
            return
        }

        let red: Double
        let green: Double
        let blue: Double
        let alpha: Double

        if value.count == 8 {
            alpha = Double((raw >> 24) & 0xFF) / 255
            red = Double((raw >> 16) & 0xFF) / 255
            green = Double((raw >> 8) & 0xFF) / 255
            blue = Double(raw & 0xFF) / 255
        } else {
            alpha = 1
            red = Double((raw >> 16) & 0xFF) / 255
            green = Double((raw >> 8) & 0xFF) / 255
            blue = Double(raw & 0xFF) / 255
        }

        self.init(.sRGB, red: red, green: green, blue: blue, opacity: alpha)
    }
}

/// Hex-to-`Color` accessors for the palette surfaces used by the views.
extension AppTheme {
    var windowBackgroundColor: Color { Color(hex: windowBackground) }
    var cardBackgroundColor: Color { Color(hex: cardBackground) }
    var controlBackgroundColor: Color { Color(hex: controlBackground) }
    var primaryTextColor: Color { Color(hex: primaryText) }
    var secondaryTextColor: Color { Color(hex: secondaryText) }
    var borderColor: Color { Color(hex: border) }
    var accentColor: Color { Color(hex: accent) }
    var onAccentTextColor: Color { Color(hex: onAccentText) }
    var buttonHoverColor: Color { Color(hex: buttonHover) }
    var strongTextColor: Color { Color(hex: strongText) }

    var controlAccentColor: Color { accentColor }
}
