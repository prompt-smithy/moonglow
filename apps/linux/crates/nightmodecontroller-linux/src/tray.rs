//! System tray via StatusNotifierItem and D-Bus menus.
//!
//! Implemented directly on `zbus` (no system libraries): one item per channel
//! (brightness, colour) registers with `org.kde.StatusNotifierWatcher`, each
//! serving its own `com.canonical.dbusmenu`. Works wherever a StatusNotifier
//! host runs — KDE Plasma, GNOME with the AppIndicator extension, wlroots
//! bars with SNI support. Two bus names on two connections (one item per
//! connection) keep registration to the plain bus-name form every host accepts.
//!
//! Each menu refreshes synchronously on `AboutToShow`, so no change signals
//! are needed for correctness. Slider-style controls are not expressible in
//! dbusmenu, so the tray offers ± step actions plus scroll-on-icon; fine
//! control lives in the settings window.
//!
//! The split is SNI-only: the XEmbed fallback (`xembed`) stays a single icon
//! that opens settings, since XEmbed offers no menu or scroll channel.

use nmc_core::settings::AppSettings;
use nmc_core::state::StateManager;
use nmc_core::traits::SettingsPersistence;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use zbus::zvariant::{Dict, OwnedObjectPath, OwnedValue, Signature, StructureBuilder, Value};

/// Commands the tray asks the main loop to perform. State mutations happen
/// directly on the shared state; only main-thread effects travel by channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    ShowSettings,
    Quit,
}

pub type SharedState<S> = Arc<Mutex<StateManager<S>>>;

/// Which control a tray item owns. Each channel gets its own icon, popup
/// menu, and scroll-wheel target, so both channels stay mouse-driven even
/// though dbusmenu popups close on every click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayChannel {
    Brightness,
    Color,
}

impl TrayChannel {
    /// Bus-name suffix, so the two items register as distinct services.
    fn bus_suffix(self) -> &'static str {
        match self {
            TrayChannel::Brightness => "brightness",
            TrayChannel::Color => "color",
        }
    }
}

// Menu item ids. The root node is 0; separators share the shape but never act.
const ID_STATUS: i32 = 1;
const ID_SEPARATOR_1: i32 = 2;
const ID_BRIGHTER: i32 = 10;
const ID_DIMMER: i32 = 11;
const ID_WARMER: i32 = 12;
const ID_COOLER: i32 = 13;
const ID_AUTO: i32 = 14;
const ID_SEPARATOR_2: i32 = 3;
const ID_SETTINGS: i32 = 20;
const ID_QUIT: i32 = 21;

const BRIGHTNESS_STEP: i32 = 5;
const TEMP_STEP: i32 = 250;
/// Scroll-wheel steps: finer than menu steps for smooth wheel adjustment.
const SCROLL_BRIGHTNESS_STEP: i32 = 3;
const SCROLL_TEMP_STEP: i32 = 250;

struct MenuItem {
    id: i32,
    label: String,
    enabled: bool,
    separator: bool,
    /// Checkmark state for the automatic toggle; `None` for plain items.
    toggle_state: Option<i32>,
}

/// Status line for one channel's menu and tooltip.
fn channel_status<S: SettingsPersistence>(state: &StateManager<S>, channel: TrayChannel) -> String {
    match channel {
        TrayChannel::Brightness => format!("Brightness {}%", state.brightness()),
        TrayChannel::Color => {
            let temp = state.color_temperature();
            if state.auto_color_temperature() {
                format!("Automatic · {temp} K")
            } else if state.has_color_support() {
                format!("Manual · {temp} K")
            } else {
                "Manual".to_string()
            }
        }
    }
}

fn status_item(label: String) -> MenuItem {
    MenuItem {
        id: ID_STATUS,
        label,
        enabled: false,
        separator: false,
        toggle_state: None,
    }
}

fn separator(id: i32) -> MenuItem {
    MenuItem {
        id,
        label: String::new(),
        enabled: false,
        separator: true,
        toggle_state: None,
    }
}

/// Builds one channel's menu from live state. Item ids are shared across
/// channels (each menu is a separate dbusmenu tree) and [`handle_action`]
/// serves both.
fn build_menu<S: SettingsPersistence>(
    state: &StateManager<S>,
    channel: TrayChannel,
) -> Vec<MenuItem> {
    let mut items = vec![
        status_item(channel_status(state, channel)),
        separator(ID_SEPARATOR_1),
    ];
    match channel {
        TrayChannel::Brightness => {
            let enabled = state.has_brightness_support();
            items.push(MenuItem {
                id: ID_BRIGHTER,
                label: format!("Brightness +{}%", BRIGHTNESS_STEP),
                enabled,
                separator: false,
                toggle_state: None,
            });
            items.push(MenuItem {
                id: ID_DIMMER,
                label: format!("Brightness −{}%", BRIGHTNESS_STEP),
                enabled,
                separator: false,
                toggle_state: None,
            });
        }
        TrayChannel::Color => {
            let temp_enabled = state.manual_color_enabled();
            items.push(MenuItem {
                id: ID_WARMER,
                label: format!("Warmer −{TEMP_STEP} K"),
                enabled: temp_enabled,
                separator: false,
                toggle_state: None,
            });
            items.push(MenuItem {
                id: ID_COOLER,
                label: format!("Cooler +{TEMP_STEP} K"),
                enabled: temp_enabled,
                separator: false,
                toggle_state: None,
            });
            items.push(MenuItem {
                id: ID_AUTO,
                label: "Automatic temperature".to_string(),
                // A mode switch, not a hardware action: always clickable.
                // It only arms the scheduler, which stays idle until a
                // colour-capable display appears.
                enabled: true,
                separator: false,
                toggle_state: Some(if state.auto_color_temperature() { 1 } else { 0 }),
            });
        }
    }
    items.push(separator(ID_SEPARATOR_2));
    items.push(MenuItem {
        id: ID_SETTINGS,
        label: "Settings…".to_string(),
        enabled: true,
        separator: false,
        toggle_state: None,
    });
    items.push(MenuItem {
        id: ID_QUIT,
        label: "Quit".to_string(),
        enabled: true,
        separator: false,
        toggle_state: None,
    });
    items
}

/// Performs a menu action. Returns true when the menu should refresh.
/// Value steppers also post an OSD bubble: the popup closes on click, so
/// without it the new value would have no visible confirmation.
fn handle_action<S: SettingsPersistence>(
    state: &SharedState<S>,
    cmd_tx: &mpsc::Sender<TrayCommand>,
    osd_tx: &mpsc::Sender<crate::osd::OsdRequest>,
    id: i32,
) -> bool {
    let mut guard = match state.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    match id {
        ID_BRIGHTER => {
            let next = guard.brightness() + BRIGHTNESS_STEP;
            guard.set_brightness(next);
            let _ = osd_tx.send(crate::osd::brightness_request(guard.brightness()));
            true
        }
        ID_DIMMER => {
            let next = guard.brightness() - BRIGHTNESS_STEP;
            guard.set_brightness(next);
            let _ = osd_tx.send(crate::osd::brightness_request(guard.brightness()));
            true
        }
        ID_WARMER => {
            let next = AppSettings::clamp_kelvin(guard.color_temperature() - TEMP_STEP);
            guard.set_color_temperature(next);
            let _ = osd_tx.send(crate::osd::temperature_request(guard.color_temperature()));
            true
        }
        ID_COOLER => {
            let next = AppSettings::clamp_kelvin(guard.color_temperature() + TEMP_STEP);
            guard.set_color_temperature(next);
            let _ = osd_tx.send(crate::osd::temperature_request(guard.color_temperature()));
            true
        }
        ID_AUTO => {
            let next = !guard.auto_color_temperature();
            guard.set_auto_color_temperature(next);
            true
        }
        ID_SETTINGS => {
            let _ = cmd_tx.send(TrayCommand::ShowSettings);
            crate::settings_ui::wake_ui();
            false
        }
        ID_QUIT => {
            // Direct first (never waits for the event loop), channel as backup.
            crate::request_quit();
            let _ = cmd_tx.send(TrayCommand::Quit);
            crate::settings_ui::wake_ui();
            false
        }
        _ => false,
    }
}

const PROPERTY_NAMES: [&str; 6] = [
    "label",
    "enabled",
    "visible",
    "type",
    "toggle-type",
    "toggle-state",
];

/// One dbusmenu property value, or `None` when the item has no such property.
fn property_value(item: &MenuItem, name: &str) -> Option<Value<'static>> {
    match name {
        "label" if !item.separator => Some(Value::new(item.label.clone())),
        "enabled" if !item.separator => Some(Value::new(item.enabled)),
        "visible" => Some(Value::new(true)),
        "type" if item.separator => Some(Value::new("separator")),
        "toggle-type" if item.toggle_state.is_some() => Some(Value::new("checkmark")),
        "toggle-state" => item.toggle_state.map(Value::new),
        _ => None,
    }
}

fn item_properties(item: &MenuItem, filter: &[String]) -> Dict<'static, 'static> {
    let mut dict = Dict::new(
        Signature::from_static_str("s").unwrap(),
        Signature::from_static_str("v").unwrap(),
    );
    for name in PROPERTY_NAMES {
        if !filter.is_empty() && !filter.iter().any(|f| f == name) {
            continue;
        }
        if let Some(value) = property_value(item, name) {
            // Dict values carry the "v" signature: every value is
            // variant-wrapped or `add` rejects it.
            let _ = dict.add(name, Value::Value(Box::new(value)));
        }
    }
    dict
}

/// One dbusmenu layout node with its wire shape `(ia{sv}av)` declared up
/// front. Returning a bare `OwnedValue` (a `v`) instead makes strict hosts —
/// GDBus proxies built from the interface XML — reject `GetLayout` outright,
/// so the menu never populates and clicks do nothing.
#[derive(Debug, serde::Serialize, serde::Deserialize, zbus::zvariant::Type)]
struct LayoutNode {
    id: i32,
    properties: std::collections::HashMap<String, OwnedValue>,
    children: Vec<OwnedValue>,
}

/// Item properties as an owned map for [`LayoutNode`].
fn item_properties_map(
    item: &MenuItem,
    filter: &[String],
) -> std::collections::HashMap<String, OwnedValue> {
    let mut map = std::collections::HashMap::new();
    for name in PROPERTY_NAMES {
        if !filter.is_empty() && !filter.iter().any(|f| f == name) {
            continue;
        }
        if let Some(value) = property_value(item, name) {
            if let Ok(owned) = OwnedValue::try_from(value) {
                map.insert(name.to_string(), owned);
            }
        }
    }
    map
}

/// Serializes one item as a dbusmenu layout node: `(id, props, children)`,
/// where children is an array of variants each holding a node.
fn layout_node(item: &MenuItem, filter: &[String], children: Vec<OwnedValue>) -> OwnedValue {
    let props = item_properties(item, filter);
    let mut array = zbus::zvariant::Array::new(Signature::from_static_str("v").unwrap());
    for child in children {
        let _ = array.append(Value::Value(Box::new(child.into())));
    }
    let structure = StructureBuilder::new()
        .append_field(Value::I32(item.id))
        .append_field(Value::Dict(props))
        .append_field(Value::Array(array))
        .build();
    OwnedValue::try_from(Value::Structure(structure)).unwrap()
}

struct CachedMenu {
    revision: u32,
    items: Vec<MenuItem>,
}

struct TrayMenu<S: SettingsPersistence> {
    state: SharedState<S>,
    cmd_tx: mpsc::Sender<TrayCommand>,
    osd_tx: mpsc::Sender<crate::osd::OsdRequest>,
    channel: TrayChannel,
    cached: Mutex<CachedMenu>,
}

impl<S: SettingsPersistence> TrayMenu<S> {
    fn refresh(&self) {
        let items = match self.state.lock() {
            Ok(state) => build_menu(&state, self.channel),
            Err(_) => return,
        };
        if let Ok(mut cached) = self.cached.lock() {
            cached.revision += 1;
            cached.items = items;
        }
    }

    fn find(&self, id: i32) -> Option<MenuItem> {
        // Rebuild from live state so a host that never calls AboutToShow
        // still sees current values.
        let state = self.state.lock().ok()?;
        build_menu(&state, self.channel)
            .into_iter()
            .find(|i| i.id == id)
    }
}

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl<S: SettingsPersistence + 'static> TrayMenu<S> {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }

    #[zbus(out_args("revision", "layout"))]
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: Vec<String>,
    ) -> (u32, LayoutNode) {
        let (revision, items) = match self.cached.lock() {
            Ok(cached) => (
                cached.revision,
                cached.items.iter().map(clone_item).collect::<Vec<_>>(),
            ),
            Err(_) => (0, Vec::new()),
        };
        let root = MenuItem {
            id: parent_id,
            label: String::new(),
            enabled: true,
            separator: false,
            toggle_state: None,
        };
        let children = if parent_id == 0 && recursion_depth != 0 {
            items
                .iter()
                .map(|item| layout_node(item, &property_names, Vec::new()))
                .collect()
        } else {
            Vec::new()
        };
        (
            revision,
            LayoutNode {
                id: parent_id,
                properties: item_properties_map(&root, &property_names),
                children,
            },
        )
    }

    #[zbus(out_args("properties"))]
    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        property_names: Vec<String>,
    ) -> Vec<(i32, std::collections::HashMap<String, OwnedValue>)> {
        ids.into_iter()
            .filter_map(|id| {
                let item = self.find(id)?;
                let mut map = std::collections::HashMap::new();
                for name in PROPERTY_NAMES {
                    if !property_names.is_empty() && !property_names.iter().any(|n| n == name) {
                        continue;
                    }
                    if let Some(value) = property_value(&item, name) {
                        if let Ok(owned) = OwnedValue::try_from(value) {
                            map.insert(name.to_string(), owned);
                        }
                    }
                }
                Some((id, map))
            })
            .collect()
    }

    fn get_property(&self, id: i32, name: String) -> zbus::fdo::Result<OwnedValue> {
        let item = self
            .find(id)
            .ok_or_else(|| zbus::fdo::Error::UnknownObject(format!("unknown menu id {id}")))?;
        let value = property_value(&item, &name)
            .ok_or_else(|| zbus::fdo::Error::UnknownProperty(format!("unknown property {name}")))?;
        OwnedValue::try_from(value).map_err(|e| zbus::fdo::Error::Failed(format!("{e}")))
    }

    fn event(&self, id: i32, event_id: String, _data: OwnedValue, _timestamp: u32) {
        if event_id == "clicked" && handle_action(&self.state, &self.cmd_tx, &self.osd_tx, id) {
            self.refresh();
        }
    }

    fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) {
        let mut refresh = false;
        for (id, event_id, _, _) in events {
            if event_id == "clicked" {
                refresh |= handle_action(&self.state, &self.cmd_tx, &self.osd_tx, id);
            }
        }
        if refresh {
            self.refresh();
        }
    }

    fn about_to_show(&self, _id: i32) {
        // The spec declares no return value; an extra out-arg only invites
        // strict hosts to complain. Refresh synchronously instead.
        self.refresh();
    }

    #[zbus(out_args("updates_needed", "id_errors"))]
    fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        self.refresh();
        (Vec::new(), Vec::new())
    }
}

/// Theme icon names served when the packaged icons are installed. Shared
/// with the OSD bubbles so tray and notifications use the same artwork.
pub(crate) const COLOR_ICON_NAME: &str = "nightmodecontroller-linux";
pub(crate) const BRIGHTNESS_ICON_NAME: &str = "nightmodecontroller-linux-brightness";

/// Icon theme roots searched for the packaged icon.
fn icon_search_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/usr/share/icons/hicolor"),
        PathBuf::from("/usr/share/pixmaps"),
        PathBuf::from("/usr/local/share/icons/hicolor"),
        PathBuf::from("/usr/local/share/pixmaps"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        roots.push(PathBuf::from(&home).join(".local/share/icons/hicolor"));
        roots.push(PathBuf::from(&home).join(".icons"));
    }
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        roots.push(PathBuf::from(data_home).join("icons/hicolor"));
    }
    if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
        roots.extend(
            data_dirs
                .split(':')
                .filter(|d| !d.is_empty())
                .map(|d| PathBuf::from(d).join("icons/hicolor")),
        );
    }
    roots
}

/// Whether any root tree holds `name` with an icon file extension.
fn theme_has_icon(roots: &[PathBuf], name: &str) -> bool {
    roots.iter().any(|root| find_icon_file(root, name, 6))
}

fn find_icon_file(dir: &Path, name: &str, depth: u8) -> bool {
    if depth == 0 {
        return false;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if find_icon_file(&path, name, depth - 1) {
                return true;
            }
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext, "png" | "svg" | "xpm"))
            && path.file_stem().is_some_and(|stem| stem == name)
        {
            return true;
        }
    }
    false
}

/// Theme icon name, or empty when the packaged icon is not installed.
///
/// Ubuntu's AppIndicator host builds a `ThemedIcon` from any non-empty name
/// and never reaches `IconPixmap`, so serving a name that is not in the theme
/// (portable runs from a build directory) renders a blank slot. Blanking the
/// name sends hosts straight to the embedded pixmap instead.
fn installed_icon_name(name: &'static str) -> &'static str {
    if theme_has_icon(&icon_search_roots(), name) {
        name
    } else {
        ""
    }
}

/// Embedded 24x24 crescent-moon icon as `(width, height, ARGB32 bytes)`.
///
/// The colour channel's icon whenever the packaged theme icon is not
/// installed — e.g. running from a build directory — so the tray item stays
/// visible everywhere instead of rendering blank.
fn color_icon_pixmap() -> (i32, i32, Vec<u8>) {
    const SIZE: i32 = 24;
    // Amber disc with an offset transparent bite taken out of it.
    const DISC: (f64, f64, f64) = (11.0, 12.0, 9.0);
    const BITE: (f64, f64, f64) = (15.5, 9.0, 7.0);
    const AMBER: (u8, u8, u8) = (235, 150, 60);
    let mut bytes = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dist =
                |c: (f64, f64, f64)| ((x as f64 - c.0).powi(2) + (y as f64 - c.1).powi(2)).sqrt();
            if dist(DISC) <= DISC.2 && dist(BITE) > BITE.2 {
                bytes.extend_from_slice(&[255, AMBER.0, AMBER.1, AMBER.2]);
            } else {
                bytes.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    (SIZE, SIZE, bytes)
}

/// Embedded 24x24 half-filled disc as `(width, height, ARGB32 bytes)`: the
/// brightness channel's icon. The ISO half-disc mark reads as brightness at
/// tray size without resembling the desktop's sun glyphs.
fn brightness_icon_pixmap() -> (i32, i32, Vec<u8>) {
    const SIZE: i32 = 24;
    const CENTER: f64 = 12.0;
    const OUTER_RADIUS: f64 = 9.0;
    const INNER_RADIUS: f64 = 7.0;
    const AMBER: (u8, u8, u8) = (235, 150, 60);
    let mut bytes = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let px = x as f64 + 0.5;
            let py = y as f64 + 0.5;
            let radius = ((px - CENTER).powi(2) + (py - CENTER).powi(2)).sqrt();
            // Filled left half plus a full outline ring: the ◐ mark.
            let filled = px < CENTER && radius <= OUTER_RADIUS;
            let ring = radius > INNER_RADIUS && radius <= OUTER_RADIUS;
            if filled || ring {
                bytes.extend_from_slice(&[255, AMBER.0, AMBER.1, AMBER.2]);
            } else {
                bytes.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    (SIZE, SIZE, bytes)
}

fn clone_item(item: &MenuItem) -> MenuItem {
    MenuItem {
        id: item.id,
        label: item.label.clone(),
        enabled: item.enabled,
        separator: item.separator,
        toggle_state: item.toggle_state,
    }
}

struct TrayItem<S: SettingsPersistence> {
    state: SharedState<S>,
    cmd_tx: mpsc::Sender<TrayCommand>,
    osd_tx: mpsc::Sender<crate::osd::OsdRequest>,
    channel: TrayChannel,
    icon_name: &'static str,
}

/// SNI tooltip wire shape: `(icon-name, pixmaps, title, text)`.
type ToolTip = (String, Vec<(i32, i32, Vec<u8>)>, String, String);

impl<S: SettingsPersistence> TrayItem<S> {
    fn status_text(&self) -> String {
        self.state
            .lock()
            .map(|state| channel_status(&state, self.channel))
            .unwrap_or_else(|_| "Night Mode Controller".to_string())
    }
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl<S: SettingsPersistence + 'static> TrayItem<S> {
    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        match self.channel {
            TrayChannel::Brightness => "nightmodecontroller-linux-brightness",
            TrayChannel::Color => "nightmodecontroller-linux",
        }
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        match self.channel {
            TrayChannel::Brightness => "Night Mode Controller — Brightness",
            TrayChannel::Color => "Night Mode Controller",
        }
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn window_id(&self) -> i32 {
        0
    }

    #[zbus(property)]
    fn icon_name(&self) -> &str {
        self.icon_name
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        let (width, height, bytes) = match self.channel {
            TrayChannel::Brightness => brightness_icon_pixmap(),
            TrayChannel::Color => color_icon_pixmap(),
        };
        vec![(width, height, bytes)]
    }

    #[zbus(property)]
    fn menu(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from("/Menu").unwrap()
    }

    #[zbus(property)]
    fn tool_tip(&self) -> ToolTip {
        (
            "nightmodecontroller-linux".to_string(),
            Vec::new(),
            match self.channel {
                TrayChannel::Brightness => "Night Mode Controller — Brightness".to_string(),
                TrayChannel::Color => "Night Mode Controller".to_string(),
            },
            self.status_text(),
        )
    }

    fn activate(&self, _x: i32, _y: i32) {
        let _ = self.cmd_tx.send(TrayCommand::ShowSettings);
        crate::settings_ui::wake_ui();
    }

    fn secondary_activate(&self, _x: i32, _y: i32) {
        // Colour only: flip automatic mode. Brightness has no channel
        // toggle — its mouse path is the scroll wheel — so it ignores this.
        if self.channel != TrayChannel::Color {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            let next = !state.auto_color_temperature();
            state.set_auto_color_temperature(next);
        }
    }

    fn scroll(&self, delta: i32, orientation: String) {
        if orientation != "vertical" {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            match self.channel {
                TrayChannel::Brightness => {
                    let next = state.brightness() + delta.signum() * SCROLL_BRIGHTNESS_STEP;
                    state.set_brightness(next);
                    let _ = self
                        .osd_tx
                        .send(crate::osd::brightness_request(state.brightness()));
                }
                TrayChannel::Color => {
                    // Manual mode only, like the menu steppers: in automatic
                    // mode the schedule would override this on the next tick.
                    if !state.manual_color_enabled() {
                        return;
                    }
                    let next = AppSettings::clamp_kelvin(
                        state.color_temperature() + delta.signum() * SCROLL_TEMP_STEP,
                    );
                    state.set_color_temperature(next);
                    let _ = self
                        .osd_tx
                        .send(crate::osd::temperature_request(state.color_temperature()));
                }
            }
        }
    }

    fn context_menu(&self, _x: i32, _y: i32) {}
}

/// Handle to the running tray thread.
pub struct TrayHandle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl TrayHandle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Serves the tray item on a background thread with its own runtime. Fails
/// cleanly without a session bus — the app keeps running trayless.
pub fn spawn_tray<S: SettingsPersistence + 'static>(
    state: SharedState<S>,
    cmd_tx: mpsc::Sender<TrayCommand>,
    osd_tx: mpsc::Sender<crate::osd::OsdRequest>,
    registered: Arc<AtomicBool>,
) -> Result<TrayHandle, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    let thread = std::thread::Builder::new()
        .name("nmc-tray".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("tray: no async runtime: {e}");
                    return;
                }
            };
            runtime.block_on(serve(state, cmd_tx, osd_tx, stop_flag, registered));
        })
        .map_err(|e| format!("tray thread failed: {e}"))?;
    Ok(TrayHandle {
        stop,
        thread: Some(thread),
    })
}

async fn serve<S: SettingsPersistence + 'static>(
    state: SharedState<S>,
    cmd_tx: mpsc::Sender<TrayCommand>,
    osd_tx: mpsc::Sender<crate::osd::OsdRequest>,
    stop: Arc<AtomicBool>,
    registered: Arc<AtomicBool>,
) {
    // One item per connection: both channels share the well-known object
    // paths, distinguished by bus name alone.
    tokio::join!(
        serve_channel(
            state.clone(),
            cmd_tx.clone(),
            osd_tx.clone(),
            stop.clone(),
            registered.clone(),
            TrayChannel::Brightness
        ),
        serve_channel(state, cmd_tx, osd_tx, stop, registered, TrayChannel::Color)
    );
}

async fn serve_channel<S: SettingsPersistence + 'static>(
    state: SharedState<S>,
    cmd_tx: mpsc::Sender<TrayCommand>,
    osd_tx: mpsc::Sender<crate::osd::OsdRequest>,
    stop: Arc<AtomicBool>,
    registered: Arc<AtomicBool>,
    channel: TrayChannel,
) {
    let conn = match zbus::Connection::session().await {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("tray: no session bus ({e}); running without a tray icon");
            return;
        }
    };
    let menu = TrayMenu {
        state: state.clone(),
        cmd_tx: cmd_tx.clone(),
        osd_tx: osd_tx.clone(),
        channel,
        cached: Mutex::new(CachedMenu {
            revision: 0,
            items: state
                .lock()
                .map(|s| build_menu(&s, channel))
                .unwrap_or_default(),
        }),
    };
    let theme_name = match channel {
        TrayChannel::Brightness => BRIGHTNESS_ICON_NAME,
        TrayChannel::Color => COLOR_ICON_NAME,
    };
    let item = TrayItem {
        state,
        cmd_tx,
        osd_tx,
        channel,
        icon_name: installed_icon_name(theme_name),
    };
    if conn.object_server().at("/Menu", menu).await.is_err() {
        eprintln!("tray: cannot serve /Menu");
        return;
    }
    if conn
        .object_server()
        .at("/StatusNotifierItem", item)
        .await
        .is_err()
    {
        eprintln!("tray: cannot serve /StatusNotifierItem");
        return;
    }
    let bus_name = format!(
        "org.kde.StatusNotifierItem-{}-{}",
        std::process::id(),
        channel.bus_suffix()
    );
    if conn.request_name(bus_name.as_str()).await.is_err() {
        eprintln!("tray: cannot claim {bus_name}");
        return;
    }
    // Register with the watcher, retrying: the host (panel, extension) may
    // start after us or restart underneath us. Registration is idempotent,
    // so it re-runs every cycle.
    let mut warned = false;
    while !stop.load(Ordering::Relaxed) {
        if register_with_watcher(&conn, &bus_name).await {
            registered.store(true, Ordering::Relaxed);
            warned = false;
        } else if !warned {
            warned = true;
            eprintln!("tray: no StatusNotifierWatcher yet; retrying");
        }
        for _ in 0..60 {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
}

async fn register_with_watcher(conn: &zbus::Connection, bus_name: &str) -> bool {
    let proxy = match zbus::Proxy::new(
        conn,
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
    )
    .await
    {
        Ok(proxy) => proxy,
        Err(_) => return false,
    };
    proxy
        .call_method("RegisterStatusNotifierItem", &(bus_name,))
        .await
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nmc_core::settings::AppSettings;
    use nmc_core::traits::{
        BrightnessService, ColorTemperatureService, DisplayService, LoginItemService,
        SettingsPersistence,
    };
    use nmc_core::DisplayInfo;

    fn icon_test_dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("nmc-icon-test-{}-{tag}", std::process::id()))
    }

    #[test]
    fn theme_icon_probe_finds_nested_files() {
        let root = icon_test_dir("found");
        let nested = root.join("256x256/apps");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            nested.join("nightmodecontroller-linux.png"),
            [0x89, b'P', b'N', b'G'],
        )
        .unwrap();
        assert!(theme_has_icon(
            std::slice::from_ref(&root),
            "nightmodecontroller-linux"
        ));
        // Other names and non-icon extensions do not match.
        assert!(!theme_has_icon(
            std::slice::from_ref(&root),
            "something-else"
        ));
        std::fs::write(nested.join("nightmodecontroller-linux.txt"), []).unwrap();
        std::fs::remove_file(nested.join("nightmodecontroller-linux.png")).unwrap();
        assert!(!theme_has_icon(
            std::slice::from_ref(&root),
            "nightmodecontroller-linux"
        ));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn theme_icon_probe_tolerates_missing_dirs() {
        let missing = icon_test_dir("missing");
        let _ = std::fs::remove_dir_all(&missing);
        assert!(!theme_has_icon(&[missing], "nightmodecontroller-linux"));
        assert!(!theme_has_icon(&[], "nightmodecontroller-linux"));
    }

    #[test]
    fn installed_icon_name_is_name_or_blank() {
        // Environment-dependent which one, but never anything else.
        for name in [COLOR_ICON_NAME, BRIGHTNESS_ICON_NAME] {
            let served = installed_icon_name(name);
            assert!(served.is_empty() || served == name);
        }
    }

    #[test]
    fn layout_node_declares_dbusmenu_signature() {
        use zbus::zvariant::Type;
        // Strict hosts (GDBus proxies built from the interface XML) reject
        // GetLayout unless the layout declares (ia{sv}av); a bare variant
        // (v) fails and the menu never populates.
        assert_eq!(LayoutNode::signature(), "(ia{sv}av)");
    }

    #[test]
    fn get_layout_serves_typed_root_with_children() {
        let (tx, _rx) = mpsc::channel();
        let state = test_state(true);
        // Brightness: status + separator + 2 steppers + separator + settings + quit.
        let items = build_menu(&state.lock().unwrap(), TrayChannel::Brightness);
        let (osd_tx, _osd_rx) = mpsc::channel();
        let menu = TrayMenu {
            state: state.clone(),
            cmd_tx: tx.clone(),
            osd_tx: osd_tx.clone(),
            channel: TrayChannel::Brightness,
            cached: Mutex::new(CachedMenu { revision: 7, items }),
        };
        let (revision, root) = menu.get_layout(0, -1, Vec::new());
        assert_eq!(revision, 7);
        assert_eq!(root.id, 0);
        assert_eq!(root.children.len(), 7);
        // Colour adds the automatic toggle: 8 items.
        let items = build_menu(&state.lock().unwrap(), TrayChannel::Color);
        let menu = TrayMenu {
            state: state.clone(),
            cmd_tx: tx,
            osd_tx,
            channel: TrayChannel::Color,
            cached: Mutex::new(CachedMenu { revision: 7, items }),
        };
        let (_, root) = menu.get_layout(0, -1, Vec::new());
        assert_eq!(root.children.len(), 8);
        // Depth 0 serves the bare root, like unknown parents.
        let (_, shallow) = menu.get_layout(0, 0, Vec::new());
        assert!(shallow.children.is_empty());
        let (_, elsewhere) = menu.get_layout(99, -1, Vec::new());
        assert!(elsewhere.children.is_empty());
    }

    #[derive(Default)]
    struct MemStore {
        saved: Mutex<AppSettings>,
    }

    impl SettingsPersistence for MemStore {
        fn load(&self) -> AppSettings {
            self.saved.lock().unwrap().clone()
        }
        fn save(&self, settings: &AppSettings) {
            *self.saved.lock().unwrap() = settings.clone();
        }
    }

    struct StubDisplay(Vec<DisplayInfo>);
    impl DisplayService for StubDisplay {
        fn displays(&self) -> Vec<DisplayInfo> {
            self.0.clone()
        }
        fn refresh(&mut self) -> Vec<DisplayInfo> {
            self.0.clone()
        }
    }

    struct StubBrightness(bool);
    impl BrightnessService for StubBrightness {
        fn is_supported(&self) -> bool {
            self.0
        }
        fn set_brightness(&self, _percent: i32) {}
    }

    struct StubColor(bool);
    impl ColorTemperatureService for StubColor {
        fn is_supported(&self) -> bool {
            self.0
        }
        fn apply(&self, _kelvin: i32) -> bool {
            self.0
        }
        fn reset(&self) {}
    }

    struct StubLogin;
    impl LoginItemService for StubLogin {
        fn is_enabled(&self) -> bool {
            false
        }
        fn requires_approval(&self) -> bool {
            false
        }
        fn set_enabled(&mut self, _enabled: bool) -> bool {
            true
        }
    }

    fn test_state(auto: bool) -> SharedState<MemStore> {
        test_state_with_displays(
            auto,
            vec![DisplayInfo::new("1", "Test", "1", true, true, true)],
        )
    }

    fn test_state_with_displays(auto: bool, displays: Vec<DisplayInfo>) -> SharedState<MemStore> {
        let settings = AppSettings {
            auto_color_temperature: auto,
            ..Default::default()
        };
        let store = MemStore::default();
        store.save(&settings);
        Arc::new(Mutex::new(StateManager::new(
            store,
            Box::new(StubDisplay(displays)),
            Arc::new(Mutex::new(StubBrightness(true))),
            Arc::new(Mutex::new(StubColor(true))),
            Box::new(StubLogin),
        )))
    }

    #[test]
    fn menu_reflects_automatic_mode() {
        let state = test_state(true);
        let items = build_menu(&state.lock().unwrap(), TrayChannel::Color);
        assert!(items
            .iter()
            .any(|i| i.id == ID_AUTO && i.toggle_state == Some(1)));
        // Temp steppers are manual-mode only.
        assert!(items.iter().any(|i| i.id == ID_WARMER && !i.enabled));

        let state = test_state(false);
        let items = build_menu(&state.lock().unwrap(), TrayChannel::Color);
        assert!(items
            .iter()
            .any(|i| i.id == ID_AUTO && i.toggle_state == Some(0)));
        assert!(items.iter().any(|i| i.id == ID_WARMER && i.enabled));
    }

    #[test]
    fn auto_toggle_available_without_displays() {
        let state = test_state_with_displays(false, Vec::new());
        let items = build_menu(&state.lock().unwrap(), TrayChannel::Color);
        // The mode switch stays clickable: it only arms the scheduler.
        let auto = items.iter().find(|i| i.id == ID_AUTO).unwrap();
        assert!(auto.enabled);
        // Live adjustments stay disabled: nothing to drive.
        let warmer = items.iter().find(|i| i.id == ID_WARMER).unwrap();
        assert!(!warmer.enabled);
        // Toggling flips the persisted mode without touching hardware.
        let (tx, _rx) = mpsc::channel();
        let (osd_tx, _osd_rx) = mpsc::channel();
        assert!(handle_action(&state, &tx, &osd_tx, ID_AUTO));
        assert!(state.lock().unwrap().auto_color_temperature());
    }

    #[test]
    fn channels_own_disjoint_controls() {
        let state = test_state(false);
        let guard = state.lock().unwrap();
        let ids = |channel| {
            build_menu(&guard, channel)
                .iter()
                .map(|i| i.id)
                .collect::<Vec<_>>()
        };
        let brightness = ids(TrayChannel::Brightness);
        let color = ids(TrayChannel::Color);
        // Each channel carries its own steppers, never the other's.
        assert!(brightness.contains(&ID_BRIGHTER));
        assert!(brightness.contains(&ID_DIMMER));
        assert!(!brightness.contains(&ID_WARMER));
        assert!(!brightness.contains(&ID_COOLER));
        assert!(!brightness.contains(&ID_AUTO));
        assert!(color.contains(&ID_WARMER));
        assert!(color.contains(&ID_COOLER));
        assert!(color.contains(&ID_AUTO));
        assert!(!color.contains(&ID_BRIGHTER));
        assert!(!color.contains(&ID_DIMMER));
        // Both keep status, settings, and quit.
        for ids in [&brightness, &color] {
            assert!(ids.contains(&ID_STATUS));
            assert!(ids.contains(&ID_SETTINGS));
            assert!(ids.contains(&ID_QUIT));
        }
        // Status lines are channel-appropriate.
        let status = |channel| {
            build_menu(&guard, channel)
                .into_iter()
                .find(|i| i.id == ID_STATUS)
                .unwrap()
                .label
        };
        assert!(status(TrayChannel::Brightness).starts_with("Brightness "));
        assert!(!status(TrayChannel::Brightness).contains('K'));
        assert!(status(TrayChannel::Color).contains('K'));
        assert!(!status(TrayChannel::Color).contains("Brightness"));
    }

    fn test_item(channel: TrayChannel, state: &SharedState<MemStore>) -> TrayItem<MemStore> {
        let (tx, _rx) = mpsc::channel();
        // Receiver dropped: OSD sends fail silently instead of bubbling.
        let (osd_tx, _osd_rx) = mpsc::channel();
        TrayItem {
            state: state.clone(),
            cmd_tx: tx,
            osd_tx,
            channel,
            icon_name: "",
        }
    }

    fn test_item_with_osd(
        channel: TrayChannel,
        state: &SharedState<MemStore>,
        osd_tx: mpsc::Sender<crate::osd::OsdRequest>,
    ) -> TrayItem<MemStore> {
        let (tx, _rx) = mpsc::channel();
        TrayItem {
            state: state.clone(),
            cmd_tx: tx,
            osd_tx,
            channel,
            icon_name: "",
        }
    }

    #[test]
    fn scroll_drives_own_channel() {
        let state = test_state(false);
        // Mid-range so the 250 K step never hits the clamp.
        state.lock().unwrap().set_color_temperature(5000);
        let before_brightness = state.lock().unwrap().brightness();
        let before_temp = state.lock().unwrap().color_temperature();
        test_item(TrayChannel::Brightness, &state).scroll(1, "vertical".to_string());
        assert_eq!(
            state.lock().unwrap().brightness(),
            before_brightness + SCROLL_BRIGHTNESS_STEP
        );
        assert_eq!(state.lock().unwrap().color_temperature(), before_temp);
        test_item(TrayChannel::Color, &state).scroll(1, "vertical".to_string());
        assert_eq!(
            state.lock().unwrap().color_temperature(),
            before_temp + SCROLL_TEMP_STEP
        );
        // Horizontal scroll is ignored on both channels.
        test_item(TrayChannel::Brightness, &state).scroll(1, "horizontal".to_string());
        test_item(TrayChannel::Color, &state).scroll(1, "horizontal".to_string());
        assert_eq!(
            state.lock().unwrap().brightness(),
            before_brightness + SCROLL_BRIGHTNESS_STEP
        );
        assert_eq!(
            state.lock().unwrap().color_temperature(),
            before_temp + SCROLL_TEMP_STEP
        );
    }

    #[test]
    fn color_scroll_yields_to_automatic_mode() {
        let state = test_state(true);
        let before = state.lock().unwrap().color_temperature();
        test_item(TrayChannel::Color, &state).scroll(1, "vertical".to_string());
        assert_eq!(state.lock().unwrap().color_temperature(), before);
    }

    #[test]
    fn secondary_activation_flips_auto_on_color_only() {
        let color_state = test_state(false);
        test_item(TrayChannel::Color, &color_state).secondary_activate(0, 0);
        assert!(color_state.lock().unwrap().auto_color_temperature());
        let bright_state = test_state(false);
        test_item(TrayChannel::Brightness, &bright_state).secondary_activate(0, 0);
        assert!(!bright_state.lock().unwrap().auto_color_temperature());
    }

    #[test]
    fn channel_bus_suffixes_differ() {
        assert_eq!(TrayChannel::Brightness.bus_suffix(), "brightness");
        assert_eq!(TrayChannel::Color.bus_suffix(), "color");
    }

    #[test]
    fn actions_mutate_state() {
        let state = test_state(false);
        let (tx, rx) = mpsc::channel();
        let (osd_tx, _osd_rx) = mpsc::channel();
        let before = state.lock().unwrap().brightness();
        assert!(handle_action(&state, &tx, &osd_tx, ID_BRIGHTER));
        assert_eq!(state.lock().unwrap().brightness(), before + BRIGHTNESS_STEP);
        assert!(handle_action(&state, &tx, &osd_tx, ID_AUTO));
        assert!(state.lock().unwrap().auto_color_temperature());
        assert!(!handle_action(&state, &tx, &osd_tx, ID_SETTINGS));
        assert_eq!(rx.try_recv().unwrap(), TrayCommand::ShowSettings);
        assert!(!handle_action(&state, &tx, &osd_tx, 999));
    }

    #[test]
    fn steppers_and_scroll_post_value_bubbles() {
        let state = test_state(false);
        let (tx, _rx) = mpsc::channel();
        let (osd_tx, osd_rx) = mpsc::channel();
        let before = state.lock().unwrap().brightness();
        assert!(handle_action(&state, &tx, &osd_tx, ID_BRIGHTER));
        let bubble = osd_rx.try_recv().unwrap();
        assert_eq!(bubble.body, format!("{}%", before + BRIGHTNESS_STEP));
        // The mode toggle is not a value: no bubble.
        assert!(handle_action(&state, &tx, &osd_tx, ID_AUTO));
        assert!(osd_rx.try_recv().is_err());
        // Scroll posts on its own channel and stays silent when it applies
        // nothing (colour scroll in automatic mode).
        let auto_state = test_state(true);
        test_item_with_osd(TrayChannel::Color, &auto_state, osd_tx.clone())
            .scroll(1, "vertical".to_string());
        assert!(osd_rx.try_recv().is_err());
        let manual = test_state(false);
        // Mid-range so the 250 K step never hits the clamp.
        manual.lock().unwrap().set_color_temperature(5000);
        let temp_before = manual.lock().unwrap().color_temperature();
        test_item_with_osd(TrayChannel::Color, &manual, osd_tx).scroll(1, "vertical".to_string());
        let bubble = osd_rx.try_recv().unwrap();
        assert_eq!(bubble.body, format!("{} K", temp_before + SCROLL_TEMP_STEP));
    }

    #[test]
    fn icon_pixmap_is_a_transparent_crescent() {
        let (width, height, bytes) = color_icon_pixmap();
        assert_eq!((width, height), (24, 24));
        assert_eq!(bytes.len(), 24 * 24 * 4);
        let mut pixels = bytes.as_chunks::<4>().0.iter();
        let opaque = pixels.clone().filter(|px| px[0] == 255).count();
        let clear = pixels.clone().filter(|px| px[0] == 0).count();
        assert!(opaque > 50, "crescent should cover real pixels");
        assert!(clear > 200, "background should stay transparent");
        // Amber channel present in the opaque pixels.
        assert!(pixels.any(|px| px[0] == 255 && px[1] == 235));
    }

    #[test]
    fn brightness_pixmap_is_a_transparent_half_disc() {
        let (width, height, bytes) = brightness_icon_pixmap();
        assert_eq!((width, height), (24, 24));
        assert_eq!(bytes.len(), 24 * 24 * 4);
        let pixels = bytes.as_chunks::<4>().0.iter();
        let opaque = pixels.clone().filter(|px| px[0] == 255).count();
        let clear = pixels.clone().filter(|px| px[0] == 0).count();
        assert!(opaque > 50, "mark should cover real pixels");
        assert!(clear > 200, "background should stay transparent");
        assert!(pixels.clone().any(|px| px[0] == 255 && px[1] == 235));
        // The ◐ mark: a filled left half (plus ring) outweighs the ring-only
        // right half severalfold.
        let left = pixels
            .clone()
            .enumerate()
            .filter(|(i, px)| px[0] == 255 && i % 24 < 12)
            .count();
        let right = pixels
            .clone()
            .enumerate()
            .filter(|(i, px)| px[0] == 255 && i % 24 >= 12)
            .count();
        assert!(left > right * 2, "left half filled, right ring-only");
        // Just left of centre is filled; just right of centre is hollow.
        let at = |x: usize, y: usize| pixels.clone().nth(y * 24 + x).unwrap()[0];
        assert_eq!(at(11, 12), 255);
        assert_eq!(at(13, 12), 0);
        // The two channel icons differ.
        assert_ne!(bytes, color_icon_pixmap().2);
    }

    #[test]
    fn layout_node_serializes() {
        let item = MenuItem {
            id: 7,
            label: "Test".to_string(),
            enabled: true,
            separator: false,
            toggle_state: Some(1),
        };
        let node = layout_node(&item, &[], Vec::new());
        // Encodes without error; the host parses the (ia{sv}av) shape.
        let encoded = zbus::zvariant::to_bytes(
            zbus::zvariant::serialized::Context::new_dbus(zbus::zvariant::LE, 0),
            &node,
        )
        .unwrap();
        assert!(!encoded.bytes().is_empty());
    }
}
