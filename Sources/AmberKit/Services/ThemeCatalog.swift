import Foundation

/// The light and dark palettes, in their required order.
///
/// Ported from `Services/ThemeCatalog.cs`, minus the two amber palettes. Order is significant:
/// it drives the picker, is asserted by tests, and decides the fallback for an unknown name.
public enum ThemeCatalog {

    public static let light: [AppTheme] = [
        AppTheme(
            name: "Light Orange", isDark: false,
            windowBackground: "#F7E4CE", cardBackground: "#FBEEDD", controlBackground: "#FDF5EC",
            primaryText: "#4A2F14", secondaryText: "#9A6E3C", border: "#EBCFA9",
            accent: "#E8802A", onAccentText: "#FDF5EC", buttonHover: "#F3DBBE", strongText: "#2E1C0A"
        ),
        AppTheme(
            name: "Light Green", isDark: false,
            windowBackground: "#DDEEDD", cardBackground: "#E9F5E9", controlBackground: "#F1F9F1",
            primaryText: "#163A22", secondaryText: "#4C7A54", border: "#C3E0C6",
            accent: "#35A254", onAccentText: "#F1F9F1", buttonHover: "#D3EBD5", strongText: "#0D2716"
        ),
        AppTheme(
            name: "Light Violet", isDark: false,
            windowBackground: "#E9E1F6", cardBackground: "#F1ECFA", controlBackground: "#F7F3FC",
            primaryText: "#2E1F4E", secondaryText: "#6A5794", border: "#DDD0F0",
            accent: "#7A4FD0", onAccentText: "#F7F3FC", buttonHover: "#E4D9F5", strongText: "#1F1339"
        ),
        AppTheme(
            name: "Light Grey", isDark: false,
            windowBackground: "#EBECEE", cardBackground: "#F5F6F7", controlBackground: "#FAFBFC",
            primaryText: "#24272B", secondaryText: "#6B6F74", border: "#D4D7DB",
            accent: "#596471", onAccentText: "#F5F6F7", buttonHover: "#E3E5E8", strongText: "#24272B"
        ),
        AppTheme(
            name: "White", isDark: false,
            windowBackground: "#F7F8FA", cardBackground: "#FBFBFC", controlBackground: "#F6F7F9",
            primaryText: "#1A1A1A", secondaryText: "#666666", border: "#E2E4E8",
            accent: "#2D7FF9", onAccentText: "#F7F8FA", buttonHover: "#FDF0B5", strongText: "#1A1A1A"
        ),
    ]

    public static let dark: [AppTheme] = [
        AppTheme(
            name: "Dark Orange", isDark: true,
            windowBackground: "#1A1206", cardBackground: "#241A0B", controlBackground: "#2F2312",
            primaryText: "#E7D2B0", secondaryText: "#B1926B", border: "#493822",
            accent: "#E8802A", onAccentText: "#1A1206", buttonHover: "#3C2C15", strongText: "#F2E1C6"
        ),
        AppTheme(
            name: "Dark Green", isDark: true,
            windowBackground: "#0D1A11", cardBackground: "#142317", controlBackground: "#1C2E20",
            primaryText: "#C7E1CB", secondaryText: "#87AC8C", border: "#284A31",
            accent: "#3AA655", onAccentText: "#0D1A11", buttonHover: "#1D3925", strongText: "#DBEEDF"
        ),
        AppTheme(
            name: "Dark Violet", isDark: true,
            windowBackground: "#14101E", cardBackground: "#1D172D", controlBackground: "#271E3B",
            primaryText: "#D6CAEB", secondaryText: "#9C8DBC", border: "#3A2D57",
            accent: "#7A4FD0", onAccentText: "#14101E", buttonHover: "#2D2247", strongText: "#E6DDF6"
        ),
        AppTheme(
            name: "Dark Grey", isDark: true,
            windowBackground: "#2B2E33", cardBackground: "#34383F", controlBackground: "#3E434B",
            primaryText: "#D8DDE4", secondaryText: "#B0B4BA", border: "#4A4F57",
            accent: "#6D8AC0", onAccentText: "#E2E8EE", buttonHover: "#565D69", strongText: "#E2E8EE"
        ),
        AppTheme(
            name: "Black", isDark: true,
            windowBackground: "#101112", cardBackground: "#18191C", controlBackground: "#222428",
            primaryText: "#E4E7EB", secondaryText: "#A4AAB2", border: "#34373D",
            accent: "#3D7EFF", onAccentText: "#F2F4F7", buttonHover: "#30343A", strongText: "#F2F4F7"
        ),
    ]

    public static let lightNames: [String] = light.map(\.name)
    public static let darkNames: [String] = dark.map(\.name)

    /// Looks up a light palette, falling back to the first entry (Light Orange).
    public static func light(named name: String?) -> AppTheme {
        light.first { $0.name == name } ?? light[0]
    }

    /// Looks up a dark palette, falling back to the first entry (Dark Orange).
    public static func dark(named name: String?) -> AppTheme {
        dark.first { $0.name == name } ?? dark[0]
    }

    /// The accent used for control tinting. White, Dark Grey, and Black get a neutral
    /// accent instead of their (blue) palette accent, matching the .NET behaviour.
    public static func controlAccent(for theme: AppTheme) -> String {
        switch theme.name {
        case "White": return "#1A1A1A"
        case "Dark Grey": return "#B0B4BA"
        case "Black": return "#A4AAB2"
        default: return theme.accent
        }
    }
}
