import Testing
@testable import MoonglowKit

@Suite("Hot-key shortcuts")
struct HotKeyShortcutTests {

    @Test("Modifiers render in Apple's canonical order")
    func modifiersRenderInCanonicalOrder() {
        let shortcut = HotKeyShortcut(
            keyCode: 126,
            modifiers: HotKeyShortcut.commandModifier
                | HotKeyShortcut.shiftModifier
                | HotKeyShortcut.optionModifier
                | HotKeyShortcut.controlModifier
        )

        #expect(shortcut.displayText == "⌃⌥⇧⌘↑")
    }

    @Test("Letters, function keys, and words render by name")
    func keysRenderByName() {
        #expect(HotKeyShortcut(keyCode: 0, modifiers: HotKeyShortcut.commandModifier).displayText == "⌘A")
        #expect(HotKeyShortcut(keyCode: 122, modifiers: HotKeyShortcut.controlModifier).displayText == "⌃F1")
        #expect(HotKeyShortcut(keyCode: 49, modifiers: HotKeyShortcut.optionModifier).displayText == "⌥Space")
        #expect(HotKeyShortcut(keyCode: 123, modifiers: HotKeyShortcut.controlModifier).displayText == "⌃←")
    }

    @Test("Unknown codes fall back to their number")
    func unknownCodesFallBackToNumber() {
        #expect(HotKeyShortcut(keyCode: 200, modifiers: HotKeyShortcut.optionModifier).displayText == "⌥Key 200")
    }

    @Test("A bare key is invalid")
    func bareKeyIsInvalid() {
        #expect(HotKeyShortcut(keyCode: 0, modifiers: 0).isValid == false)
        #expect(HotKeyShortcut(keyCode: 0, modifiers: HotKeyShortcut.commandModifier).isValid == true)
    }

    @Test("The defaults are Ctrl+Option+arrows")
    func defaultsAreControlOptionArrows() {
        let modifiers = HotKeyShortcut.controlModifier | HotKeyShortcut.optionModifier

        #expect(AppSettings.defaultIncreaseBrightnessShortcut == HotKeyShortcut(keyCode: 126, modifiers: modifiers))
        #expect(AppSettings.defaultDecreaseBrightnessShortcut == HotKeyShortcut(keyCode: 125, modifiers: modifiers))
        #expect(AppSettings.defaultIncreaseTemperatureShortcut == HotKeyShortcut(keyCode: 124, modifiers: modifiers))
        #expect(AppSettings.defaultDecreaseTemperatureShortcut == HotKeyShortcut(keyCode: 123, modifiers: modifiers))
    }
}
