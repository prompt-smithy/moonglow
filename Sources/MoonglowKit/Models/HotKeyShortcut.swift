import Foundation

/// A global keyboard shortcut: a key code plus Carbon modifier flags.
///
/// The modifier values match Carbon's (`cmdKey`, `optionKey`, `controlKey`, `shiftKey`)
/// so the app shell can pass them straight to `RegisterEventHotKey`, without this module
/// importing Carbon. At least one modifier is required — a bare key would hijack typing.
public struct HotKeyShortcut: Equatable, Hashable, Sendable, Codable {

    /// Carbon modifier bits. Only these four are kept; caps lock, fn, and friends are
    /// stripped at capture time because hot-key registration ignores them.
    public static let commandModifier: UInt32 = 0x100
    public static let shiftModifier: UInt32 = 0x200
    public static let optionModifier: UInt32 = 0x800
    public static let controlModifier: UInt32 = 0x1000

    public var keyCode: UInt32
    public var modifiers: UInt32

    public init(keyCode: UInt32, modifiers: UInt32) {
        self.keyCode = keyCode
        self.modifiers = modifiers
    }

    /// False for a bare key, which must never be registered.
    public var isValid: Bool { modifiers != 0 }

    /// `⌃⌥↑`, with modifiers in Apple's canonical order.
    public var displayText: String {
        var text = ""
        if modifiers & Self.controlModifier != 0 { text += "⌃" }
        if modifiers & Self.optionModifier != 0 { text += "⌥" }
        if modifiers & Self.shiftModifier != 0 { text += "⇧" }
        if modifiers & Self.commandModifier != 0 { text += "⌘" }
        return text + Self.keyLabel(for: keyCode)
    }

    /// US-layout label for a key code. Layout-dependent keys outside this table fall back
    /// to their numeric code rather than guessing wrong.
    public static func keyLabel(for keyCode: UInt32) -> String {
        switch keyCode {
        case 0: return "A"
        case 1: return "S"
        case 2: return "D"
        case 3: return "F"
        case 4: return "H"
        case 5: return "G"
        case 6: return "Z"
        case 7: return "X"
        case 8: return "C"
        case 9: return "V"
        case 11: return "B"
        case 12: return "Q"
        case 13: return "W"
        case 14: return "E"
        case 15: return "R"
        case 16: return "Y"
        case 17: return "T"
        case 18: return "1"
        case 19: return "2"
        case 20: return "3"
        case 21: return "4"
        case 22: return "6"
        case 23: return "5"
        case 24: return "="
        case 25: return "9"
        case 26: return "7"
        case 27: return "-"
        case 28: return "8"
        case 29: return "0"
        case 30: return "]"
        case 31: return "O"
        case 32: return "U"
        case 33: return "["
        case 34: return "I"
        case 35: return "P"
        case 36: return "↩"
        case 37: return "L"
        case 38: return "J"
        case 39: return "'"
        case 40: return "K"
        case 41: return ";"
        case 42: return "\\"
        case 43: return ","
        case 44: return "/"
        case 45: return "N"
        case 46: return "M"
        case 47: return "."
        case 48: return "⇥"
        case 49: return "Space"
        case 50: return "`"
        case 51: return "⌫"
        case 53: return "⎋"
        case 64: return "F17"
        case 65: return "."
        case 67: return "*"
        case 69: return "+"
        case 71: return "Clear"
        case 75: return "/"
        case 76: return "Enter"
        case 78: return "-"
        case 79: return "F17"
        case 80: return "F18"
        case 81: return "="
        case 82: return "0"
        case 83: return "1"
        case 84: return "2"
        case 85: return "3"
        case 86: return "4"
        case 87: return "5"
        case 88: return "6"
        case 89: return "7"
        case 90: return "F20"
        case 91: return "8"
        case 92: return "9"
        case 96: return "F5"
        case 97: return "F6"
        case 98: return "F7"
        case 99: return "F3"
        case 100: return "F8"
        case 101: return "F9"
        case 103: return "F11"
        case 105: return "F13"
        case 106: return "F16"
        case 107: return "F14"
        case 109: return "F10"
        case 111: return "F12"
        case 113: return "F15"
        case 114: return "Help"
        case 115: return "Home"
        case 116: return "PageUp"
        case 117: return "⌦"
        case 119: return "End"
        case 121: return "PageDown"
        case 118: return "F4"
        case 120: return "F2"
        case 122: return "F1"
        case 123: return "←"
        case 124: return "→"
        case 125: return "↓"
        case 126: return "↑"
        default: return "Key \(keyCode)"
        }
    }
}
