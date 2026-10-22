import Foundation

/// The two built-in appearances used by the settings window.
public enum ThemeCatalog {

    public static let light = AppTheme(
        name: "Light", isDark: false,
        windowBackground: "#F5F5F5", cardBackground: "#FFFFFF", controlBackground: "#E8E8E8",
        primaryText: "#1D1D1F", secondaryText: "#6E6E73", border: "#D2D2D7",
        accent: "#007AFF", onAccentText: "#FFFFFF", buttonHover: "#E5E5EA", strongText: "#1D1D1F"
    )

    public static let dark = AppTheme(
        name: "Dark", isDark: true,
        windowBackground: "#1C1C1E", cardBackground: "#2C2C2E", controlBackground: "#3A3A3C",
        primaryText: "#F5F5F7", secondaryText: "#AEAEB2", border: "#48484A",
        accent: "#0A84FF", onAccentText: "#FFFFFF", buttonHover: "#3A3A3C", strongText: "#F5F5F7"
    )
}
