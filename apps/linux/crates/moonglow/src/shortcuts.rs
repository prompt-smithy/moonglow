//! Global keyboard shortcuts for brightness and colour temperature.
//!
//! Three backends cover the desktops the app supports:
//!
//! - **GNOME custom keybindings** (`gsettings`): the desktop itself runs the
//!   bound command, so this works on both X11 and Wayland and the shortcuts
//!   stay visible and editable in GNOME Settings.
//! - **X11 key grabs**: a dedicated X connection grabs the combinations on the
//!   root window. Used on plain X11 sessions that are not GNOME.
//! - **XDG GlobalShortcuts portal**: the standards-based Wayland path, used on
//!   KDE and wlroots compositors.
//!
//! Registration is re-run whenever the bindings change; the returned status is
//! surfaced in Settings so an unsupported desktop explains itself instead of
//! silently dropping the feature.

use crate::commands::SharedState;
use crate::session::SessionKind;
use moonglow_core::settings::AppSettings;
use moonglow_core::shortcut::{Shortcut, ShortcutAction};
use moonglow_core::traits::SettingsPersistence;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How the running session registered the shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutBackend {
    Gnome,
    X11,
    Portal,
    Unavailable,
}

impl ShortcutBackend {
    pub const fn label(self) -> &'static str {
        match self {
            ShortcutBackend::Gnome => "GNOME custom shortcuts",
            ShortcutBackend::X11 => "X11 key grabs",
            ShortcutBackend::Portal => "Desktop portal",
            ShortcutBackend::Unavailable => "Unavailable",
        }
    }
}

/// Backend state shown in the Shortcuts card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutStatus {
    pub backend: ShortcutBackend,
    pub detail: String,
}

impl ShortcutStatus {
    fn new(backend: ShortcutBackend, detail: impl Into<String>) -> Self {
        Self {
            backend,
            detail: detail.into(),
        }
    }

    // The native window surfaces `detail` instead of a separate availability
    // pill; kept for backends and tests that read the flag directly.
    #[allow(dead_code)]
    pub fn is_available(&self) -> bool {
        self.backend != ShortcutBackend::Unavailable
    }
}

impl Default for ShortcutStatus {
    fn default() -> Self {
        Self::new(ShortcutBackend::Unavailable, "Shortcuts are starting.")
    }
}

pub type SharedShortcutStatus = Arc<Mutex<ShortcutStatus>>;

/// Handle to the shortcut worker thread.
pub struct ShortcutHandle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ShortcutHandle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Current bindings, in [`ShortcutAction::ALL`] order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bindings {
    shortcuts: [Option<Shortcut>; 4],
}

impl Bindings {
    fn from_settings(settings: &AppSettings) -> Self {
        Self {
            shortcuts: [
                settings.increase_brightness_shortcut,
                settings.decrease_brightness_shortcut,
                settings.increase_temperature_shortcut,
                settings.decrease_temperature_shortcut,
            ],
        }
    }

    fn from_state<S: SettingsPersistence>(state: &SharedState<S>) -> Self {
        state
            .lock()
            .map(|s| Self::from_settings(s.settings()))
            .unwrap_or(Bindings {
                shortcuts: [None; 4],
            })
    }

    fn get(&self, action: ShortcutAction) -> Option<Shortcut> {
        let index = ShortcutAction::ALL
            .iter()
            .position(|candidate| *candidate == action)
            .unwrap_or(0);
        self.shortcuts[index]
    }

    fn is_empty(&self) -> bool {
        self.shortcuts.iter().all(Option::is_none)
    }
}

/// Spawns the worker that keeps the global shortcuts in sync with settings.
pub fn spawn<S: SettingsPersistence + 'static>(
    state: SharedState<S>,
    session: SessionKind,
) -> (ShortcutHandle, SharedShortcutStatus) {
    let stop = Arc::new(AtomicBool::new(false));
    let status = Arc::new(Mutex::new(ShortcutStatus::default()));
    let worker_stop = stop.clone();
    let worker_status = status.clone();
    let thread = std::thread::Builder::new()
        .name("moonglow-shortcuts".to_string())
        .spawn(move || run(state, session, worker_stop, worker_status))
        .ok();
    (ShortcutHandle { stop, thread }, status)
}

fn set_status(shared: &SharedShortcutStatus, backend: ShortcutBackend, detail: impl Into<String>) {
    if let Ok(mut status) = shared.lock() {
        let next = ShortcutStatus::new(backend, detail);
        if *status != next {
            eprintln!("shortcuts: {} - {}", next.backend.label(), next.detail);
            *status = next;
        }
    }
}

fn run<S: SettingsPersistence>(
    state: SharedState<S>,
    session: SessionKind,
    stop: Arc<AtomicBool>,
    status: SharedShortcutStatus,
) {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();

    if desktop.contains("gnome") && gnome::available() {
        gnome::run(&state, &stop, &status);
        return;
    }

    let wayland = !matches!(session, SessionKind::X11 | SessionKind::Headless);
    if wayland && portal::run(&state, &stop, &status) {
        return;
    }

    if x11::run(&state, &stop, &status) {
        if wayland {
            set_status(
                &status,
                ShortcutBackend::X11,
                "X11 key grabs are active. On Wayland they only fire while an X11 window has focus; use your desktop's keyboard settings for global coverage.",
            );
        }
        return;
    }

    if !wayland && portal::run(&state, &stop, &status) {
        return;
    }

    set_status(
        &status,
        ShortcutBackend::Unavailable,
        "This desktop does not expose global shortcuts to applications. Set them in your keyboard settings to run the CLI commands below.",
    );
}

/// Applies one shortcut action to the shared state.
fn apply_action<S: SettingsPersistence>(state: &SharedState<S>, action: ShortcutAction) {
    let Ok(mut state) = state.lock() else {
        return;
    };
    match action {
        ShortcutAction::IncreaseBrightness => state.increase_brightness(),
        ShortcutAction::DecreaseBrightness => state.decrease_brightness(),
        ShortcutAction::IncreaseTemperature => state.increase_color_temperature(),
        ShortcutAction::DecreaseTemperature => state.decrease_color_temperature(),
    }
}

/// Sleeps in short slices so teardown never waits out a full poll interval.
fn sleep_while(stop: &AtomicBool, duration: Duration) {
    let slice = Duration::from_millis(50);
    let mut waited = Duration::ZERO;
    while waited < duration {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        std::thread::sleep(slice);
        waited += slice;
    }
}

// MARK: - GNOME custom keybindings

mod gnome {
    use super::*;

    const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
    const CUSTOM_PREFIX: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/";

    pub(super) fn available() -> bool {
        Command::new("gsettings")
            .args(["get", MEDIA_KEYS, "custom-keybindings"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn path(action: ShortcutAction) -> String {
        format!("{CUSTOM_PREFIX}moonglow-{}/", action.id())
    }

    fn schema_for(path: &str) -> String {
        format!("{MEDIA_KEYS}.custom-keybinding:{path}")
    }

    /// CLI flag the desktop should run for an action.
    fn cli_flag(action: ShortcutAction) -> &'static str {
        match action {
            ShortcutAction::IncreaseBrightness => "--brightness-up",
            ShortcutAction::DecreaseBrightness => "--brightness-down",
            ShortcutAction::IncreaseTemperature => "--temperature-up",
            ShortcutAction::DecreaseTemperature => "--temperature-down",
        }
    }

    fn quote(value: &str) -> String {
        format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
    }

    fn list_literal(paths: &[String]) -> String {
        if paths.is_empty() {
            "[]".to_string()
        } else {
            format!(
                "[{}]",
                paths
                    .iter()
                    .map(|path| quote(path))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }

    /// Extracts the quoted strings from `gsettings get` output.
    fn parse_strv(raw: &str) -> Vec<String> {
        let mut values = Vec::new();
        let mut current = String::new();
        let mut in_quote = false;
        let mut escaped = false;
        for ch in raw.chars() {
            if !in_quote {
                if ch == '\'' {
                    in_quote = true;
                    current.clear();
                }
                continue;
            }
            if escaped {
                current.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '\'' {
                in_quote = false;
                values.push(std::mem::take(&mut current));
            } else {
                current.push(ch);
            }
        }
        values
    }

    fn get_list() -> Result<Vec<String>, String> {
        let output = Command::new("gsettings")
            .args(["get", MEDIA_KEYS, "custom-keybindings"])
            .output()
            .map_err(|error| format!("cannot run gsettings: {error}"))?;
        if !output.status.success() {
            return Err("gsettings refused to read the shortcut list.".to_string());
        }
        Ok(parse_strv(&String::from_utf8_lossy(&output.stdout)))
    }

    fn set(schema: &str, key: &str, value: &str) -> Result<(), String> {
        let output = Command::new("gsettings")
            .args(["set", schema, key, value])
            .output()
            .map_err(|error| format!("cannot run gsettings: {error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "gsettings set {schema} {key} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    fn register(bindings: &Bindings) -> Result<(), String> {
        let exe = std::env::current_exe()
            .map_err(|error| format!("cannot locate the executable: {error}"))?
            .to_string_lossy()
            .into_owned();
        let exe = if exe.contains(' ') {
            format!("'{}'", exe.replace('\'', ""))
        } else {
            exe
        };

        // Keep third-party entries, replace every entry this app owns.
        let mut paths: Vec<String> = get_list()?
            .into_iter()
            .filter(|path| !path.starts_with(&format!("{CUSTOM_PREFIX}moonglow-")))
            .collect();

        for action in ShortcutAction::ALL {
            let Some(shortcut) = bindings.get(action) else {
                continue;
            };
            let path = path(action);
            let schema = schema_for(&path);
            set(
                &schema,
                "name",
                &quote(&format!("Moonglow: {}", action.label())),
            )?;
            set(
                &schema,
                "command",
                &quote(&format!("{exe} {}", cli_flag(action))),
            )?;
            set(&schema, "binding", &quote(&shortcut.gnome_accelerator()))?;
            paths.push(path);
        }

        set(MEDIA_KEYS, "custom-keybindings", &list_literal(&paths))
    }

    pub(super) fn run<S: SettingsPersistence>(
        state: &SharedState<S>,
        stop: &Arc<AtomicBool>,
        status: &SharedShortcutStatus,
    ) {
        let mut last: Option<Bindings> = None;
        while !stop.load(Ordering::Relaxed) {
            let bindings = Bindings::from_state(state);
            if last != Some(bindings) {
                match register(&bindings) {
                    Ok(()) => set_status(
                        status,
                        ShortcutBackend::Gnome,
                        "Registered with GNOME. They also appear under Settings › Keyboard › Custom Shortcuts.",
                    ),
                    Err(error) => set_status(status, ShortcutBackend::Unavailable, error),
                }
                last = Some(bindings);
            }
            sleep_while(stop, Duration::from_millis(500));
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_present_and_empty_lists() {
            assert!(parse_strv("@as []").is_empty());
            assert_eq!(
                parse_strv("['/org/a/', '/org/b/']"),
                vec!["/org/a/".to_string(), "/org/b/".to_string()]
            );
        }

        #[test]
        fn parses_escaped_quotes_and_backslashes() {
            assert_eq!(
                parse_strv("['/org/it\\'s/', '/org/back\\\\slash/']"),
                vec!["/org/it's/".to_string(), "/org/back\\slash/".to_string()]
            );
        }

        #[test]
        fn formats_a_gvariant_string_array() {
            assert_eq!(list_literal(&[]), "[]");
            assert_eq!(
                list_literal(&["/org/a/".to_string(), "/org/b/".to_string()]),
                "['/org/a/', '/org/b/']"
            );
        }

        #[test]
        fn binding_paths_are_stable() {
            assert_eq!(
                path(ShortcutAction::IncreaseBrightness),
                "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/moonglow-increase-brightness/"
            );
            assert_eq!(
                cli_flag(ShortcutAction::DecreaseTemperature),
                "--temperature-down"
            );
        }
    }
}

// MARK: - X11 key grabs

mod x11 {
    use super::*;
    use std::collections::HashMap;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt as _, GrabMode, ModMask};
    use x11rb::protocol::Event;
    use x11rb::rust_connection::RustConnection;

    /// Locks whose state must be ignored for a grab to fire.
    const LOCK_MODIFIERS: [u16; 4] = [0, 1 << 1, 1 << 4, (1 << 1) | (1 << 4)];

    struct Grab {
        keycode: u8,
        modifiers: u16,
    }

    struct X11Backend {
        conn: RustConnection,
        root: u32,
        grabs: Vec<Grab>,
        actions: HashMap<(u8, u16), ShortcutAction>,
    }

    impl X11Backend {
        fn connect() -> Result<Self, String> {
            let (conn, screen) = x11rb::connect(None)
                .map_err(|error| format!("cannot open the X11 display: {error}"))?;
            let root = conn.setup().roots[screen].root;
            Ok(Self {
                conn,
                root,
                grabs: Vec::new(),
                actions: HashMap::new(),
            })
        }

        fn keyboard_keysyms(&self) -> Result<HashMap<u32, u8>, String> {
            let setup = self.conn.setup();
            let count = setup.max_keycode - setup.min_keycode + 1;
            let reply = self
                .conn
                .get_keyboard_mapping(setup.min_keycode, count)
                .map_err(|error| error.to_string())?
                .reply()
                .map_err(|error| error.to_string())?;
            let per_keycode = reply.keysyms_per_keycode.max(1) as usize;
            let mut map = HashMap::new();
            for (index, keysyms) in reply.keysyms.chunks(per_keycode).enumerate() {
                for keysym in keysyms {
                    if *keysym != 0 {
                        map.entry(*keysym)
                            .or_insert(setup.min_keycode + index as u8);
                    }
                }
            }
            Ok(map)
        }

        fn ungrab(&mut self) {
            for grab in self.grabs.drain(..) {
                let _ = self
                    .conn
                    .ungrab_key(grab.keycode, self.root, grab.modifiers.into());
            }
            let _ = self.conn.flush();
        }

        fn register(&mut self, bindings: &Bindings) -> Result<String, String> {
            self.ungrab();
            self.actions.clear();
            let keycodes = self.keyboard_keysyms()?;
            for action in ShortcutAction::ALL {
                let Some(shortcut) = bindings.get(action) else {
                    continue;
                };
                let keycode = *keycodes.get(&shortcut.key.keysym()).ok_or_else(|| {
                    format!(
                        "{} is not present on this keyboard layout.",
                        shortcut.key.name()
                    )
                })?;
                let modifiers = modifier_mask(shortcut);
                for lock in LOCK_MODIFIERS {
                    self.conn
                        .grab_key(
                            false,
                            self.root,
                            (modifiers | lock).into(),
                            keycode,
                            GrabMode::ASYNC,
                            GrabMode::ASYNC,
                        )
                        .map_err(|error| error.to_string())?
                        .check()
                        .map_err(|error| format!("cannot grab {shortcut}: {error}"))?;
                    self.grabs.push(Grab {
                        keycode,
                        modifiers: modifiers | lock,
                    });
                }
                self.actions.insert((keycode, modifiers), action);
            }
            let _ = self.conn.flush();
            if bindings.is_empty() {
                Ok("No shortcuts assigned.".to_string())
            } else {
                Ok("Captured globally on X11.".to_string())
            }
        }

        fn drain_actions(&mut self) -> Vec<ShortcutAction> {
            let mut fired = Vec::new();
            loop {
                match self.conn.poll_for_event() {
                    Ok(Some(Event::KeyPress(event))) => {
                        let modifiers = u16::from(event.state) & all_modifiers();
                        if let Some(action) = self.actions.get(&(event.detail, modifiers)) {
                            fired.push(*action);
                        }
                    }
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
            fired
        }
    }

    /// Every modifier bit a shortcut can use, stripped of lock bits for
    /// matching a key press to its grab.
    fn all_modifiers() -> u16 {
        u16::from(ModMask::SHIFT)
            | u16::from(ModMask::CONTROL)
            | u16::from(ModMask::M1)
            | u16::from(ModMask::M4)
    }

    fn modifier_mask(shortcut: Shortcut) -> u16 {
        let mut mask = 0;
        if shortcut.shift {
            mask |= u16::from(ModMask::SHIFT);
        }
        if shortcut.ctrl {
            mask |= u16::from(ModMask::CONTROL);
        }
        if shortcut.alt {
            mask |= u16::from(ModMask::M1);
        }
        if shortcut.super_key {
            mask |= u16::from(ModMask::M4);
        }
        mask
    }

    /// Runs the X11 backend. Returns false when there is no X display, so the
    /// caller can try the portal instead.
    pub(super) fn run<S: SettingsPersistence>(
        state: &SharedState<S>,
        stop: &Arc<AtomicBool>,
        status: &SharedShortcutStatus,
    ) -> bool {
        let Ok(mut backend) = X11Backend::connect() else {
            return false;
        };
        let mut last: Option<Bindings> = None;
        while !stop.load(Ordering::Relaxed) {
            let bindings = Bindings::from_state(state);
            if last != Some(bindings) {
                match backend.register(&bindings) {
                    Ok(detail) => set_status(status, ShortcutBackend::X11, detail),
                    Err(error) => set_status(status, ShortcutBackend::Unavailable, error),
                }
                last = Some(bindings);
            }
            for action in backend.drain_actions() {
                apply_action(state, action);
            }
            sleep_while(stop, Duration::from_millis(20));
        }
        backend.ungrab();
        true
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn x11_modifiers_match_xproto_bits() {
            let shortcut: Shortcut = "Ctrl+Alt+Shift+Super+Up".parse().unwrap();
            assert_eq!(
                modifier_mask(shortcut),
                u16::from(ModMask::CONTROL)
                    | u16::from(ModMask::M1)
                    | u16::from(ModMask::SHIFT)
                    | u16::from(ModMask::M4)
            );
        }

        #[test]
        fn arrow_keysyms_are_stable() {
            let up: Shortcut = "Ctrl+Alt+Up".parse().unwrap();
            assert_eq!(up.key.keysym(), 0xFF52);
        }
    }
}

// MARK: - Portal backend

mod portal;
