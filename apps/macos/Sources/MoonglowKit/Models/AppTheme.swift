import Foundation

/// One named palette.
///
/// Ported from `Models/AppTheme.cs`. Colours stay as hex strings so the catalog is
/// testable without a UI framework; the app target converts them to `Color`.
public struct AppTheme: Equatable, Identifiable, Sendable {
    public let name: String
    public let isDark: Bool
    public let windowBackground: String
    public let cardBackground: String
    public let controlBackground: String
    public let primaryText: String
    public let secondaryText: String
    public let border: String
    public let accent: String
    public let onAccentText: String
    public let buttonHover: String
    public let strongText: String

    public var id: String { name }

    public init(
        name: String,
        isDark: Bool,
        windowBackground: String,
        cardBackground: String,
        controlBackground: String,
        primaryText: String,
        secondaryText: String,
        border: String,
        accent: String,
        onAccentText: String,
        buttonHover: String,
        strongText: String
    ) {
        self.name = name
        self.isDark = isDark
        self.windowBackground = windowBackground
        self.cardBackground = cardBackground
        self.controlBackground = controlBackground
        self.primaryText = primaryText
        self.secondaryText = secondaryText
        self.border = border
        self.accent = accent
        self.onAccentText = onAccentText
        self.buttonHover = buttonHover
        self.strongText = strongText
    }

    /// Every surface colour in the palette, for audit-style tests.
    public var allHexColors: [String] {
        [
            windowBackground,
            cardBackground,
            controlBackground,
            primaryText,
            secondaryText,
            border,
            accent,
            onAccentText,
            buttonHover,
            strongText,
        ]
    }
}
