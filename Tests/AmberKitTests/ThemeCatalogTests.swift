import Testing
@testable import AmberKit

/// Ported from `ThemeCatalogTests` in the .NET test project.
@Suite("Theme catalog")
struct ThemeCatalogTests {

    /// Looks up a palette by name across both appearance modes.
    private static func theme(named name: String) -> AppTheme {
        if let light = ThemeCatalog.light.first(where: { $0.name == name }) { return light }
        return ThemeCatalog.dark.first { $0.name == name } ?? ThemeCatalog.dark[0]
    }

    @Test("Light palettes keep their required order")
    func lightThemesHaveRequiredOrder() {
        #expect(ThemeCatalog.lightNames == [
            "Light Orange", "Light Green", "Light Violet", "Light Grey", "White",
        ])
    }

    @Test("Dark palettes keep their required order")
    func darkThemesHaveRequiredOrder() {
        #expect(ThemeCatalog.darkNames == [
            "Dark Orange", "Dark Green", "Dark Violet", "Dark Grey", "Black",
        ])
    }

    @Test("The default palette names exist in the catalog")
    func defaultPaletteNamesExist() {
        #expect(ThemeCatalog.lightNames.contains(AppSettings.defaultLightThemeName))
        #expect(ThemeCatalog.darkNames.contains(AppSettings.defaultDarkThemeName))
    }

    @Test(
        "Blue palettes use neutral control accents",
        arguments: [
            ("White", "#1A1A1A"),
            ("Dark Grey", "#B0B4BA"),
            ("Black", "#A4AAB2"),
        ]
    )
    func bluePalettesUseNeutralControlAccents(themeName: String, expected: String) {
        let palette = Self.theme(named: themeName)

        #expect(ThemeCatalog.controlAccent(for: palette) == expected)
    }

    @Test(
        "Coloured palettes tint controls with their own accent",
        arguments: [
            "Light Orange", "Light Green", "Light Violet", "Light Grey",
            "Dark Orange", "Dark Green", "Dark Violet",
        ]
    )
    func colouredPalettesUseThemeAccent(themeName: String) {
        let palette = Self.theme(named: themeName)

        #expect(ThemeCatalog.controlAccent(for: palette) == palette.accent)
    }

    @Test("No palette uses pure white as a surface")
    func themesDoNotUsePureWhite() {
        for theme in ThemeCatalog.light + ThemeCatalog.dark {
            for hex in theme.allHexColors {
                #expect(hex.uppercased() != "#FFFFFF")
                #expect(hex.uppercased() != "#FFF")
            }
        }
    }

    @Test("Every palette declares the correct appearance flag")
    func appearanceFlagsAreCorrect() {
        for theme in ThemeCatalog.light {
            #expect(!theme.isDark)
        }
        for theme in ThemeCatalog.dark {
            #expect(theme.isDark)
        }
    }

    @Test("Unknown names fall back to the first palette")
    func unknownNamesFallBack() {
        #expect(ThemeCatalog.light(named: "Chartreuse") == ThemeCatalog.light[0])
        #expect(ThemeCatalog.dark(named: nil) == ThemeCatalog.dark[0])
    }

    @Test("Every colour is a parseable 6-digit hex string")
    func everyColourIsAParseableHexString() {
        for theme in ThemeCatalog.light + ThemeCatalog.dark {
            for hex in theme.allHexColors {
                #expect(hex.hasPrefix("#"))
                #expect(hex.count == 7)
                #expect(UInt64(hex.dropFirst(), radix: 16) != nil)
            }
        }
    }
}
