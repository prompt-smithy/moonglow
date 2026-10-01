import Testing
@testable import AmberKit

@Suite("System appearance")
struct ThemeCatalogTests {

    @Test("Only the automatic light and dark appearances are available")
    func onlyLightAndDarkAppearancesExist() {
        #expect(ThemeCatalog.light.name == "Light")
        #expect(!ThemeCatalog.light.isDark)
        #expect(ThemeCatalog.dark.name == "Dark")
        #expect(ThemeCatalog.dark.isDark)
    }

    @Test("Built-in appearance colors are parseable")
    func appearanceColorsAreParseable() {
        for theme in [ThemeCatalog.light, ThemeCatalog.dark] {
            for hex in theme.allHexColors {
                #expect(hex.hasPrefix("#"))
                #expect(hex.count == 7)
                #expect(UInt64(hex.dropFirst(), radix: 16) != nil)
            }
        }
    }
}
