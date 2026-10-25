namespace Moonglow.Core;

/// <summary>Identifies one of the four global-shortcut actions.</summary>
public enum HotkeySlot
{
    IncreaseBrightness,
    DecreaseBrightness,
    IncreaseTemperature,
    DecreaseTemperature,
}

/// <summary>A parsed global shortcut: Win32 modifier bits plus a virtual-key code.</summary>
/// <remarks>
/// The persisted form is the readable string the other platforms share, such as
/// <c>"Ctrl+Alt+PageUp"</c>; null disables the action. Canonical order is
/// Ctrl+Alt+Shift+Super, matching <c>spec/settings.schema.json</c> (the Windows key
/// is written as Super so the file stays in the shared shape).
/// </remarks>
public sealed class Hotkey
{
    // Win32 MOD_* bits, so Modifiers passes straight to RegisterHotKey.
    public const int ModAlt = 0x1;
    public const int ModControl = 0x2;
    public const int ModShift = 0x4;
    public const int ModWin = 0x8;

    public int Modifiers { get; }
    public int VirtualKey { get; }

    public Hotkey(int modifiers, int virtualKey)
    {
        Modifiers = modifiers;
        VirtualKey = virtualKey;
    }

    /// <summary>Linux-parity defaults: arrows stay clear of workspace switching.</summary>
    public static string DefaultFor(HotkeySlot slot) => slot switch
    {
        HotkeySlot.IncreaseBrightness => "Ctrl+Alt+PageUp",
        HotkeySlot.DecreaseBrightness => "Ctrl+Alt+PageDown",
        HotkeySlot.IncreaseTemperature => "Ctrl+Alt+]",
        HotkeySlot.DecreaseTemperature => "Ctrl+Alt+[",
        _ => throw new ArgumentOutOfRangeException(nameof(slot)),
    };

    /// <summary>Parses a persisted shortcut. At least one modifier plus one known
    /// key is required; null or blank input never parses (callers treat JSON null
    /// as "disabled" before reaching this method).</summary>
    public static bool TryParse(string? text, out Hotkey? hotkey)
    {
        hotkey = null;
        if (string.IsNullOrWhiteSpace(text))
            return false;

        string[] raw = text!.Split('+');
        if (raw.Length < 2)
            return false;

        int modifiers = 0;
        for (int i = 0; i < raw.Length - 1; i++)
        {
            switch (raw[i].Trim().ToUpperInvariant())
            {
                case "CTRL":
                case "CONTROL":
                    modifiers |= ModControl;
                    break;
                case "ALT":
                    modifiers |= ModAlt;
                    break;
                case "SHIFT":
                    modifiers |= ModShift;
                    break;
                case "WIN":
                case "WINDOWS":
                case "SUPER":
                    modifiers |= ModWin;
                    break;
                default:
                    return false;
            }
        }

        if (!KeyCodes.TryGetValue(raw[raw.Length - 1].Trim(), out int virtualKey))
            return false;

        hotkey = new Hotkey(modifiers, virtualKey);
        return true;
    }

    /// <summary>Canonical gesture text for captured modifier bits plus a display key
    /// name, or null when the key has no binding name.</summary>
    public static string? FormatGesture(int modifiers, string keyName)
    {
        if (modifiers == 0 || !KeyCodes.TryGetValue(keyName, out int virtualKey))
            return null;
        return new Hotkey(modifiers, virtualKey).ToString();
    }

    /// <summary>Display name for a virtual-key code, or null when it has none.</summary>
    public static bool TryGetKeyName(int virtualKey, out string? name)
        => KeyNames.TryGetValue(virtualKey, out name);

    /// <summary>Canonical persisted form: null stays null (disabled), anything
    /// unparseable falls back, mirroring the Linux decoding.</summary>
    public static string? Normalize(string? text, string fallback)
    {
        if (text is null)
            return null;
        return TryParse(text, out Hotkey? hotkey) && hotkey is not null
            ? hotkey.ToString()
            : fallback;
    }

    public override string ToString()
    {
        var parts = new List<string>();
        if ((Modifiers & ModControl) != 0)
            parts.Add("Ctrl");
        if ((Modifiers & ModAlt) != 0)
            parts.Add("Alt");
        if ((Modifiers & ModShift) != 0)
            parts.Add("Shift");
        if ((Modifiers & ModWin) != 0)
            parts.Add("Super");
        parts.Add(KeyNames.TryGetValue(VirtualKey, out string? name) ? name : $"VK 0x{VirtualKey:X2}");
        return string.Join("+", parts);
    }

    private static readonly IReadOnlyDictionary<string, int> KeyCodes = BuildKeyCodes();
    private static readonly IReadOnlyDictionary<int, string> KeyNames = BuildKeyNames();

    private static Dictionary<string, int> BuildKeyCodes()
    {
        var codes = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
        for (char c = 'A'; c <= 'Z'; c++)
            codes[c.ToString()] = c;
        for (char c = '0'; c <= '9'; c++)
            codes[c.ToString()] = c;
        for (int i = 1; i <= 24; i++)
            codes["F" + i] = 0x6F + i;
        codes["Up"] = 0x26;
        codes["Down"] = 0x28;
        codes["Left"] = 0x25;
        codes["Right"] = 0x27;
        codes["PageUp"] = 0x21;
        codes["PageDown"] = 0x22;
        codes["Home"] = 0x24;
        codes["End"] = 0x23;
        codes["Insert"] = 0x2D;
        codes["Delete"] = 0x2E;
        codes["Space"] = 0x20;
        codes["Tab"] = 0x09;
        codes["Enter"] = 0x0D;
        codes["Escape"] = 0x1B;
        codes["-"] = 0xBD;
        codes["="] = 0xBB;
        codes["["] = 0xDB;
        codes["]"] = 0xDD;
        codes[";"] = 0xBA;
        codes["'"] = 0xDE;
        codes[","] = 0xBC;
        codes["."] = 0xBE;
        codes["/"] = 0xBF;
        codes["\\"] = 0xDC;
        codes["`"] = 0xC0;
        // Aliases users may write by hand.
        codes["Prior"] = 0x21;
        codes["Next"] = 0x22;
        codes["Del"] = 0x2E;
        codes["Ins"] = 0x2D;
        codes["Return"] = 0x0D;
        codes["Esc"] = 0x1B;
        for (int i = 0; i <= 9; i++)
            codes["D" + i] = 0x30 + i;
        codes["OemMinus"] = 0xBD;
        codes["OemPlus"] = 0xBB;
        codes["OemOpenBrackets"] = 0xDB;
        codes["OemCloseBrackets"] = 0xDD;
        codes["OemSemicolon"] = 0xBA;
        codes["OemQuotes"] = 0xDE;
        codes["OemComma"] = 0xBC;
        codes["OemPeriod"] = 0xBE;
        codes["OemQuestion"] = 0xBF;
        codes["OemPipe"] = 0xDC;
        codes["OemBackslash"] = 0xDC;
        codes["OemTilde"] = 0xC0;
        return codes;
    }

    private static Dictionary<int, string> BuildKeyNames()
    {
        var names = new Dictionary<int, string>();
        for (char c = 'A'; c <= 'Z'; c++)
            names[c] = c.ToString();
        for (char c = '0'; c <= '9'; c++)
            names[c] = c.ToString();
        for (int i = 1; i <= 24; i++)
            names[0x6F + i] = "F" + i;
        names[0x26] = "Up";
        names[0x28] = "Down";
        names[0x25] = "Left";
        names[0x27] = "Right";
        names[0x21] = "PageUp";
        names[0x22] = "PageDown";
        names[0x24] = "Home";
        names[0x23] = "End";
        names[0x2D] = "Insert";
        names[0x2E] = "Delete";
        names[0x20] = "Space";
        names[0x09] = "Tab";
        names[0x0D] = "Enter";
        names[0x1B] = "Escape";
        names[0xBD] = "-";
        names[0xBB] = "=";
        names[0xDB] = "[";
        names[0xDD] = "]";
        names[0xBA] = ";";
        names[0xDE] = "'";
        names[0xBC] = ",";
        names[0xBE] = ".";
        names[0xBF] = "/";
        names[0xDC] = "\\";
        names[0xC0] = "`";
        return names;
    }
}
