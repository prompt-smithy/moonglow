//! Global keyboard shortcuts.
//!
//! The persisted form is a readable string such as `"Ctrl+Alt+Up"`. Keeping a
//! platform-neutral value in the core lets the settings window, the CLI, and
//! each shortcut backend (X11, GNOME, desktop portal) agree on one
//! representation without importing any UI or OS API here.
//!
//! A shortcut must name at least one modifier: a bare key would hijack typing
//! system-wide.

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// One physical key in the shortcut table.
///
/// A table index is used instead of one enum variant per key: every backend
/// needs the same spellings (settings, portal, GNOME, label), and a row
/// per key keeps them from drifting apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShortcutKey(u8);

struct KeyDef {
    /// Canonical settings spelling and UI label.
    name: &'static str,
    /// X11 keysym, used to derive a keycode at registration time.
    keysym: u32,
    /// XDG GlobalShortcuts portal / Qt trigger spelling.
    portal: &'static str,
    /// GTK accelerator spelling for GNOME custom keybindings.
    gnome: &'static str,
}

macro_rules! key_rows {
    ($([$name:expr, $keysym:expr, $portal:expr, $gnome:expr]),* $(,)?) => {
        &[$(KeyDef { name: $name, keysym: $keysym, portal: $portal, gnome: $gnome }),*]
    };
}

const KEY_DEFS: &[KeyDef] = key_rows![
    ["Up", 0xFF52, "Up", "Up"],
    ["Down", 0xFF54, "Down", "Down"],
    ["Left", 0xFF51, "Left", "Left"],
    ["Right", 0xFF53, "Right", "Right"],
    ["PageUp", 0xFF55, "PgUp", "Page_Up"],
    ["PageDown", 0xFF56, "PgDown", "Page_Down"],
    ["Home", 0xFF50, "Home", "Home"],
    ["End", 0xFF57, "End", "End"],
    ["Insert", 0xFF63, "Ins", "Insert"],
    ["Delete", 0xFFFF, "Del", "Delete"],
    ["Space", 0x0020, "Space", "space"],
    ["Tab", 0xFF09, "Tab", "Tab"],
    ["Enter", 0xFF0D, "Return", "Return"],
    ["Escape", 0xFF1B, "Esc", "Escape"],
    ["Backspace", 0xFF08, "Backspace", "BackSpace"],
    ["Minus", 0x002D, "-", "minus"],
    ["Equal", 0x003D, "=", "equal"],
    ["BracketLeft", 0x005B, "[", "bracketleft"],
    ["BracketRight", 0x005D, "]", "bracketright"],
    ["Semicolon", 0x003B, ";", "semicolon"],
    ["Quote", 0x0027, "'", "apostrophe"],
    ["Backslash", 0x005C, "\\", "backslash"],
    ["Comma", 0x002C, ",", "comma"],
    ["Period", 0x002E, ".", "period"],
    ["Slash", 0x002F, "/", "slash"],
    ["Backquote", 0x0060, "`", "grave"],
    ["F1", 0xFFBE, "F1", "F1"],
    ["F2", 0xFFBF, "F2", "F2"],
    ["F3", 0xFFC0, "F3", "F3"],
    ["F4", 0xFFC1, "F4", "F4"],
    ["F5", 0xFFC2, "F5", "F5"],
    ["F6", 0xFFC3, "F6", "F6"],
    ["F7", 0xFFC4, "F7", "F7"],
    ["F8", 0xFFC5, "F8", "F8"],
    ["F9", 0xFFC6, "F9", "F9"],
    ["F10", 0xFFC7, "F10", "F10"],
    ["F11", 0xFFC8, "F11", "F11"],
    ["F12", 0xFFC9, "F12", "F12"],
    ["A", 0x0061, "A", "a"],
    ["B", 0x0062, "B", "b"],
    ["C", 0x0063, "C", "c"],
    ["D", 0x0064, "D", "d"],
    ["E", 0x0065, "E", "e"],
    ["F", 0x0066, "F", "f"],
    ["G", 0x0067, "G", "g"],
    ["H", 0x0068, "H", "h"],
    ["I", 0x0069, "I", "i"],
    ["J", 0x006A, "J", "j"],
    ["K", 0x006B, "K", "k"],
    ["L", 0x006C, "L", "l"],
    ["M", 0x006D, "M", "m"],
    ["N", 0x006E, "N", "n"],
    ["O", 0x006F, "O", "o"],
    ["P", 0x0070, "P", "p"],
    ["Q", 0x0071, "Q", "q"],
    ["R", 0x0072, "R", "r"],
    ["S", 0x0073, "S", "s"],
    ["T", 0x0074, "T", "t"],
    ["U", 0x0075, "U", "u"],
    ["V", 0x0076, "V", "v"],
    ["W", 0x0077, "W", "w"],
    ["X", 0x0078, "X", "x"],
    ["Y", 0x0079, "Y", "y"],
    ["Z", 0x007A, "Z", "z"],
    ["0", 0x0030, "0", "0"],
    ["1", 0x0031, "1", "1"],
    ["2", 0x0032, "2", "2"],
    ["3", 0x0033, "3", "3"],
    ["4", 0x0034, "4", "4"],
    ["5", 0x0035, "5", "5"],
    ["6", 0x0036, "6", "6"],
    ["7", 0x0037, "7", "7"],
    ["8", 0x0038, "8", "8"],
    ["9", 0x0039, "9", "9"],
];

impl ShortcutKey {
    /// Looks a key up by its canonical (case-insensitive) name or its
    /// symbolic label, so `"BracketRight"` and `"]"` are the same key.
    pub fn from_name(name: &str) -> Option<Self> {
        let needle = name.trim();
        if let Some(index) = KEY_DEFS
            .iter()
            .position(|def| def.name.eq_ignore_ascii_case(needle))
        {
            return Some(Self(index as u8));
        }
        let canonical = match needle {
            "[" => "BracketLeft",
            "]" => "BracketRight",
            "-" => "Minus",
            "=" => "Equal",
            "," => "Comma",
            "." => "Period",
            "/" => "Slash",
            ";" => "Semicolon",
            "'" => "Quote",
            "\\" => "Backslash",
            "`" => "Backquote",
            _ => return None,
        };
        Self::from_name(canonical)
    }

    fn def(self) -> &'static KeyDef {
        &KEY_DEFS[(self.0 as usize).min(KEY_DEFS.len() - 1)]
    }

    pub fn name(self) -> &'static str {
        self.def().name
    }

    /// Compact spelling for the settings window, e.g. `]` instead of
    /// `BracketRight`. Parsing accepts both.
    pub fn label(self) -> &'static str {
        match self.def().name {
            "BracketLeft" => "[",
            "BracketRight" => "]",
            "Minus" => "-",
            "Equal" => "=",
            "Comma" => ",",
            "Period" => ".",
            "Slash" => "/",
            "Semicolon" => ";",
            "Quote" => "'",
            "Backslash" => "\\",
            "Backquote" => "`",
            other => other,
        }
    }

    /// X11 keysym. The app turns this into a keycode on the active layout.
    pub fn keysym(self) -> u32 {
        self.def().keysym
    }

    /// Trigger spelling understood by the GlobalShortcuts portal.
    pub fn portal_name(self) -> &'static str {
        self.def().portal
    }

    /// Key spelling for a GNOME custom-keybinding accelerator.
    pub fn gnome_name(self) -> &'static str {
        self.def().gnome
    }
}

/// A global shortcut: modifiers plus one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub key: ShortcutKey,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

impl Shortcut {
    pub const fn new(
        key: ShortcutKey,
        ctrl: bool,
        alt: bool,
        shift: bool,
        super_key: bool,
    ) -> Self {
        Self {
            key,
            ctrl,
            alt,
            shift,
            super_key,
        }
    }

    /// At least one modifier is required; a bare key would fire while typing.
    pub fn is_valid(self) -> bool {
        self.ctrl || self.alt || self.shift || self.super_key
    }

    /// Whether two assignments would fight over the same key press.
    pub fn conflicts_with(self, other: Self) -> bool {
        self.key == other.key
            && self.ctrl == other.ctrl
            && self.alt == other.alt
            && self.shift == other.shift
            && self.super_key == other.super_key
    }

    /// `Ctrl+Alt+Up`: canonical order, suitable for settings and display.
    pub fn display_text(self) -> String {
        let mut parts = Vec::with_capacity(5);
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.super_key {
            parts.push("Super");
        }
        parts.push(self.key.label());
        parts.join("+")
    }

    /// `Ctrl+Alt+Up`, for a GNOME accelerator.
    pub fn gnome_accelerator(self) -> String {
        let mut text = String::new();
        if self.ctrl {
            text.push_str("<Control>");
        }
        if self.alt {
            text.push_str("<Alt>");
        }
        if self.shift {
            text.push_str("<Shift>");
        }
        if self.super_key {
            text.push_str("<Super>");
        }
        text.push_str(self.key.gnome_name());
        text
    }

    /// `Ctrl+Alt+Up`, for the GlobalShortcuts portal trigger.
    pub fn portal_trigger(self) -> String {
        let mut parts = Vec::with_capacity(5);
        if self.ctrl {
            parts.push("CTRL".to_string());
        }
        if self.alt {
            parts.push("ALT".to_string());
        }
        if self.shift {
            parts.push("SHIFT".to_string());
        }
        if self.super_key {
            parts.push("META".to_string());
        }
        parts.push(self.key.portal_name().to_string());
        parts.join("+")
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display_text())
    }
}

/// Why a shortcut string was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutParseError {
    Empty,
    UnknownToken,
    MissingKey,
    MissingModifier,
}

impl fmt::Display for ShortcutParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            ShortcutParseError::Empty => "shortcut is empty",
            ShortcutParseError::UnknownToken => "shortcut contains an unknown modifier or key",
            ShortcutParseError::MissingKey => "shortcut has no key",
            ShortcutParseError::MissingModifier => "shortcut needs at least one modifier",
        };
        f.write_str(text)
    }
}

impl FromStr for Shortcut {
    type Err = ShortcutParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ShortcutParseError::Empty);
        }
        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut super_key = false;
        let mut key = None;
        for token in trimmed.split('+').map(str::trim).filter(|t| !t.is_empty()) {
            match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => ctrl = true,
                "alt" | "option" | "opt" => alt = true,
                "shift" => shift = true,
                "super" | "meta" | "cmd" | "command" | "win" => super_key = true,
                _ => {
                    let parsed =
                        ShortcutKey::from_name(token).ok_or(ShortcutParseError::UnknownToken)?;
                    if key.replace(parsed).is_some() {
                        return Err(ShortcutParseError::UnknownToken);
                    }
                }
            }
        }
        let key = key.ok_or(ShortcutParseError::MissingKey)?;
        let shortcut = Self {
            key,
            ctrl,
            alt,
            shift,
            super_key,
        };
        if !shortcut.is_valid() {
            return Err(ShortcutParseError::MissingModifier);
        }
        Ok(shortcut)
    }
}

impl Serialize for Shortcut {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.display_text())
    }
}

impl<'de> Deserialize<'de> for Shortcut {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ShortcutVisitor;
        impl Visitor<'_> for ShortcutVisitor {
            type Value = Shortcut;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a shortcut string such as \"Ctrl+Alt+Up\"")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                value.parse().map_err(|error: ShortcutParseError| {
                    E::custom(format!("invalid shortcut: {error}"))
                })
            }
        }
        deserializer.deserialize_str(ShortcutVisitor)
    }
}

/// Which action a shortcut fires. The ids double as binding names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    IncreaseBrightness,
    DecreaseBrightness,
    IncreaseTemperature,
    DecreaseTemperature,
    ShowSettings,
}

impl ShortcutAction {
    pub const ALL: [ShortcutAction; 5] = [
        ShortcutAction::IncreaseBrightness,
        ShortcutAction::DecreaseBrightness,
        ShortcutAction::IncreaseTemperature,
        ShortcutAction::DecreaseTemperature,
        ShortcutAction::ShowSettings,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            ShortcutAction::IncreaseBrightness => "increase-brightness",
            ShortcutAction::DecreaseBrightness => "decrease-brightness",
            ShortcutAction::IncreaseTemperature => "increase-temperature",
            ShortcutAction::DecreaseTemperature => "decrease-temperature",
            ShortcutAction::ShowSettings => "show-settings",
        }
    }

    /// Label for the settings window and desktop binding names.
    pub const fn label(self) -> &'static str {
        match self {
            ShortcutAction::IncreaseBrightness => "Increase brightness",
            ShortcutAction::DecreaseBrightness => "Decrease brightness",
            ShortcutAction::IncreaseTemperature => "Increase temperature (cooler)",
            ShortcutAction::DecreaseTemperature => "Decrease temperature (warmer)",
            ShortcutAction::ShowSettings => "Show settings",
        }
    }

    /// Deliberately avoids Ctrl+Alt+arrows: GNOME binds all four to workspace
    /// switching. PageUp/PageDown read as up/down for brightness, and the
    /// bracket keys sit left/right on the warm-to-cool track.
    pub const fn default_shortcut(self) -> Shortcut {
        // Table indices: 4 = PageUp, 5 = PageDown, 17 = BracketLeft,
        // 18 = BracketRight.
        match self {
            ShortcutAction::IncreaseBrightness => {
                Shortcut::new(ShortcutKey(4), true, true, false, false)
            }
            ShortcutAction::DecreaseBrightness => {
                Shortcut::new(ShortcutKey(5), true, true, false, false)
            }
            ShortcutAction::IncreaseTemperature => {
                Shortcut::new(ShortcutKey(18), true, true, false, false)
            }
            ShortcutAction::DecreaseTemperature => {
                Shortcut::new(ShortcutKey(17), true, true, false, false)
            }
            ShortcutAction::ShowSettings => {
                Shortcut::new(ShortcutKey(50), true, true, false, false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_canonically() {
        let shortcut: Shortcut = "Ctrl+Alt+Up".parse().unwrap();
        assert_eq!(shortcut.key, ShortcutKey::from_name("Up").unwrap());
        assert!(shortcut.ctrl && shortcut.alt);
        assert_eq!(shortcut.display_text(), "Ctrl+Alt+Up");
        assert_eq!(shortcut.gnome_accelerator(), "<Control><Alt>Up");
        assert_eq!(shortcut.portal_trigger(), "CTRL+ALT+Up");
    }

    #[test]
    fn aliases_and_ordering_are_normalised() {
        let shortcut: Shortcut = " alt + shift + super + a ".parse().unwrap();
        assert_eq!(shortcut.display_text(), "Alt+Shift+Super+A");
        assert_eq!(
            "Control+Option+F5"
                .parse::<Shortcut>()
                .unwrap()
                .display_text(),
            "Ctrl+Alt+F5"
        );
        assert_eq!(
            "Meta+1".parse::<Shortcut>().unwrap().display_text(),
            "Super+1"
        );
    }

    #[test]
    fn rejects_unusable_shortcuts() {
        assert_eq!("".parse::<Shortcut>(), Err(ShortcutParseError::Empty));
        assert_eq!(
            "Up".parse::<Shortcut>(),
            Err(ShortcutParseError::MissingModifier)
        );
        assert_eq!(
            "Ctrl+Alt".parse::<Shortcut>(),
            Err(ShortcutParseError::MissingKey)
        );
        assert_eq!(
            "Ctrl+Nonsense".parse::<Shortcut>(),
            Err(ShortcutParseError::UnknownToken)
        );
        assert_eq!(
            "Ctrl+Up+Down".parse::<Shortcut>(),
            Err(ShortcutParseError::UnknownToken)
        );
    }

    #[test]
    fn symbolic_labels_round_trip() {
        let bracket: Shortcut = "Ctrl+Alt+]".parse().unwrap();
        assert_eq!(bracket.key.name(), "BracketRight");
        assert_eq!(bracket.display_text(), "Ctrl+Alt+]");
        assert_eq!(
            "Ctrl+Alt+BracketRight".parse::<Shortcut>().unwrap(),
            bracket
        );
        for raw in ["Ctrl+-", "Alt+=", "Shift+,", "Super+.", "Ctrl+/"] {
            let shortcut: Shortcut = raw.parse().unwrap();
            assert_eq!(shortcut.display_text(), raw);
        }
    }

    #[test]
    fn detects_conflicts() {
        let up: Shortcut = "Ctrl+Alt+Up".parse().unwrap();
        assert!(up.conflicts_with("Ctrl+Alt+Up".parse().unwrap()));
        assert!(!up.conflicts_with("Ctrl+Alt+Down".parse().unwrap()));
        assert!(!up.conflicts_with("Ctrl+Shift+Up".parse().unwrap()));
    }

    #[test]
    fn every_action_has_a_distinct_valid_default() {
        let mut seen = Vec::new();
        for action in ShortcutAction::ALL {
            let shortcut = action.default_shortcut();
            assert!(shortcut.is_valid());
            assert!(!seen.iter().any(|other| shortcut.conflicts_with(*other)));
            seen.push(shortcut);
        }
    }

    #[test]
    fn serialises_as_the_shared_string_form() {
        let shortcut: Shortcut = "Ctrl+Alt+Right".parse().unwrap();
        assert_eq!(
            serde_json::to_string(&shortcut).unwrap(),
            "\"Ctrl+Alt+Right\""
        );
        let back: Shortcut = serde_json::from_str("\"Ctrl+Alt+Right\"").unwrap();
        assert_eq!(back, shortcut);
        assert!(serde_json::from_str::<Shortcut>("\"Up\"").is_err());
    }

    #[test]
    fn every_table_row_has_backend_spellings() {
        for def in KEY_DEFS {
            assert!(!def.name.is_empty());
            assert!(!def.portal.is_empty());
            assert!(!def.gnome.is_empty());
            assert!(def.keysym > 0);
            assert!(ShortcutKey::from_name(def.name).is_some(), "{}", def.name);
        }
    }
}
