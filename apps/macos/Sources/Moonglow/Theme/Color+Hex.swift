import SwiftUI
import MoonglowKit

extension Color {
    /// Creates a colour from the catalog's `#RRGGBB` or `#AARRGGBB` hex strings.
    ///
    /// Malformed input resolves to clear rather than crashing.
    init(hex: String) {
        var value = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if value.hasPrefix("#") { value.removeFirst() }

        // Six digits (`RRGGBB`) or eight (`AARRGGBB`); anything else is malformed.
        // The scanner alone cannot tell: it happily parses a prefix of a longer
        // string and any short string parses as a small number.
        guard value.count == 6 || value.count == 8 else {
            self = .clear
            return
        }

        var raw: UInt64 = 0
        let scanner = Scanner(string: value)
        guard scanner.scanHexInt64(&raw), scanner.isAtEnd else {
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
