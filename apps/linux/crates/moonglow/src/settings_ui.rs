//! Settings window (GTK4 + libadwaita).
//!
//! The application's main surface: brightness, colour temperature with its
//! automatic switch and day/night range, global shortcuts, schedule pickers,
//! startup, displays, and the CLI reference. The window is a native Adwaita
//! preferences window — a normal desktop window that closes to the background
//! so the schedule and shortcuts keep running, and a second launch (or
//! `--show-settings`) brings it back. The desktop's own style manager drives
//! light/dark, so there is no theme polling.
//!
//! Behavioural contract (same as the previous UI shell):
//! - Local control state re-syncs whenever settings change elsewhere, and
//!   every mutation records `last_seen` so the re-sync never stomps a drag.
//! - The brightness and temperature controls are sensitive exactly when the
//!   matching `has_*_support()` holds; a dead slider is never offered (see
//!   `brightness_needs_service_and_capable_display` in moonglow-core).
//! - GTK version floor: only APIs available in GTK 4.8 / libadwaita 1.2, so
//!   the same build runs on Debian 12 through current releases. The Cargo
//!   features (`gnome_43`, `v1_2`) enforce this at compile time.

use crate::commands::{AppCommand, SharedState};
use crate::shortcuts::SharedShortcutStatus;
use crate::visibility::{VisCommand, VisibilityController};
use adw::prelude::*;
use moonglow_core::settings::AppSettings;
use moonglow_core::shortcut::{Shortcut, ShortcutAction, ShortcutKey};
use moonglow_core::state::ScheduleTime;
use moonglow_core::temperature_range::{TemperatureRange, TemperatureScale};
use moonglow_core::time_of_day::TimeOfDay;
use moonglow_core::traits::SettingsPersistence;
use moonglow_core::twelve_hour::{Meridiem, TwelveHourTime};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const SCHEDULE_FIELDS: [ScheduleTime; 4] = [
    ScheduleTime::MorningStart,
    ScheduleTime::MorningEnd,
    ScheduleTime::EveningStart,
    ScheduleTime::EveningEnd,
];

/// A message shown for a few seconds under the shortcut rows.
const MESSAGE_SECONDS: u64 = 4;

/// Command-poll and live-label refresh cadence.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Range-bar geometry, ported from the previous shell.
const RANGE_HANDLE_RADIUS: f32 = 10.0;
const HANDLE_HALF_WIDTH: f32 = 8.0;
const HANDLE_APEX_RISE: f32 = 10.5;
const HANDLE_BASE_DROP: f32 = 4.5;
const TRACK_HEIGHT: f32 = 10.0;
const RANGE_BAR_HEIGHT: i32 = 36;

/// Temperature slider bounds (kelvin) and step.
const TEMP_MIN: f64 = 900.0;
const TEMP_MAX: f64 = 6600.0;
const TEMP_STEP: f64 = 100.0;

pub struct CliCommand {
    // Read only by the catalog drift test; the card renders usage + desc.
    #[allow(dead_code)]
    pub name: &'static str,
    pub usage: &'static str,
    pub desc: &'static str,
}

pub const CLI_COMMANDS: &[CliCommand] = &[
    CliCommand {
        name: "show-settings",
        usage: "--show-settings",
        desc: "Show the window.",
    },
    CliCommand {
        name: "quit",
        usage: "--quit",
        desc: "Quit the running app.",
    },
    CliCommand {
        name: "hidden",
        usage: "--hidden",
        desc: "Start in the background.",
    },
    CliCommand {
        name: "set-brightness",
        usage: "--set-brightness PERCENT",
        desc: "Set brightness 1-100.",
    },
    CliCommand {
        name: "brighter",
        usage: "--brighter",
        desc: "Raise brightness by 5%.",
    },
    CliCommand {
        name: "dimmer",
        usage: "--dimmer",
        desc: "Lower brightness by 5%.",
    },
    CliCommand {
        name: "brightness-up",
        usage: "--brightness-up",
        desc: "Shortcut action: brightness +5%.",
    },
    CliCommand {
        name: "brightness-down",
        usage: "--brightness-down",
        desc: "Shortcut action: brightness -5%.",
    },
    CliCommand {
        name: "set-temp",
        usage: "--set-temp KELVIN",
        desc: "Set temperature (manual mode).",
    },
    CliCommand {
        name: "warmer",
        usage: "--warmer",
        desc: "Warm by 100 K (manual mode).",
    },
    CliCommand {
        name: "cooler",
        usage: "--cooler",
        desc: "Cool by 100 K (manual mode).",
    },
    CliCommand {
        name: "temperature-up",
        usage: "--temperature-up",
        desc: "Shortcut action: +100 K, leaves automatic.",
    },
    CliCommand {
        name: "temperature-down",
        usage: "--temperature-down",
        desc: "Shortcut action: -100 K, leaves automatic.",
    },
    CliCommand {
        name: "auto",
        usage: "--auto on|off",
        desc: "Automatic temperature.",
    },
    CliCommand {
        name: "day-temp",
        usage: "--day-temp KELVIN",
        desc: "Daytime schedule target.",
    },
    CliCommand {
        name: "night-temp",
        usage: "--night-temp KELVIN",
        desc: "Nighttime schedule target.",
    },
    CliCommand {
        name: "morning",
        usage: "--morning FROM TO",
        desc: "Morning window, 24h HH:MM.",
    },
    CliCommand {
        name: "evening",
        usage: "--evening FROM TO",
        desc: "Evening window, 24h HH:MM.",
    },
    CliCommand {
        name: "login",
        usage: "--login on|off",
        desc: "Start at login.",
    },
    CliCommand {
        name: "status",
        usage: "--status",
        desc: "Print status.",
    },
    CliCommand {
        name: "json",
        usage: "--status --json",
        desc: "Machine-readable status.",
    },
    CliCommand {
        name: "headless",
        usage: "--headless",
        desc: "Schedule and shortcuts, no window.",
    },
];

fn shortcuts_of(settings: &AppSettings) -> [Option<Shortcut>; 4] {
    [
        settings.increase_brightness_shortcut,
        settings.decrease_brightness_shortcut,
        settings.increase_temperature_shortcut,
        settings.decrease_temperature_shortcut,
    ]
}

fn shortcut_hint(action: ShortcutAction) -> &'static str {
    match action {
        ShortcutAction::IncreaseBrightness => "Brightness +5%",
        ShortcutAction::DecreaseBrightness => "Brightness −5%",
        ShortcutAction::IncreaseTemperature => "+100 K, cooler",
        ShortcutAction::DecreaseTemperature => "−100 K, warmer",
    }
}

/// Maps a GDK keyval name to a key the global-shortcut backends understand.
/// `ShortcutKey::from_name` is case-insensitive, so most names land directly;
/// the aliases below cover GDK spellings that differ from the canonical ones.
/// Anything else (keypad, lone modifiers, media keys) is rejected, matching
/// the old shell's "that key cannot be used" behaviour.
fn shortcut_key_from_gdk(name: &str) -> Option<ShortcutKey> {
    if let Some(key) = ShortcutKey::from_name(name) {
        return Some(key);
    }
    let canonical = match name {
        "Page_Up" => "PageUp",
        "Page_Down" => "PageDown",
        "Return" => "Enter",
        "apostrophe" => "Quote",
        "grave" => "Backquote",
        "ISO_Left_Tab" => "Tab",
        _ => return None,
    };
    ShortcutKey::from_name(canonical)
}

/// A warm-to-cool colour for the temperature track, as linear RGB channels.
fn temperature_rgb(kelvin: i32) -> (f64, f64, f64) {
    const STOPS: [(i32, [u8; 3]); 4] = [
        (900, [255, 150, 60]),
        (2700, [255, 196, 130]),
        (4200, [255, 232, 205]),
        (6600, [210, 230, 255]),
    ];
    let kelvin = kelvin.clamp(STOPS[0].0, STOPS[3].0);
    for pair in STOPS.windows(2) {
        let (from_k, from_rgb) = pair[0];
        let (to_k, to_rgb) = pair[1];
        if kelvin <= to_k {
            let t = (kelvin - from_k) as f64 / (to_k - from_k) as f64;
            let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t) / 255.0;
            return (
                mix(from_rgb[0], to_rgb[0]),
                mix(from_rgb[1], to_rgb[1]),
                mix(from_rgb[2], to_rgb[2]),
            );
        }
    }
    let last = STOPS[3].1;
    (
        last[0] as f64 / 255.0,
        last[1] as f64 / 255.0,
        last[2] as f64 / 255.0,
    )
}

// MARK: - Model

/// Widget handles the refresh pass writes to. All signal handlers and the
/// poll timer share one [`SharedModel`]; every handler borrows it briefly, and
/// programmatic widget writes hold no borrow at all (see [`suppressed`]) —
/// otherwise GTK's synchronous notify signals would re-enter a handler while
/// borrowed and panic.
struct UiWidgets {
    window: adw::PreferencesWindow,
    status_row: adw::ActionRow,
    brightness_scale: gtk::Scale,
    brightness_value: gtk::Label,
    brightness_group: adw::PreferencesGroup,
    temp_scale: gtk::Scale,
    temp_value: gtk::Label,
    temp_hint: gtk::Label,
    auto_switch: gtk::Switch,
    range_area: gtk::DrawingArea,
    range_text: gtk::Label,
    record_buttons: [gtk::Button; 4],
    clear_buttons: [gtk::Button; 4],
    shortcut_message: gtk::Label,
    backend_detail: gtk::Label,
    time_drops: [[gtk::DropDown; 3]; 4],
    login_switch: gtk::Switch,
    login_status: gtk::Label,
    open_button: gtk::Button,
    displays_box: gtk::Box,
    refresh_button: gtk::Button,
    copy_button: gtk::Button,
    cli_copied: gtk::Label,
    hide_button: gtk::Button,
    quit_button: gtk::Button,
}

struct Model<S: SettingsPersistence> {
    state: SharedState<S>,
    cmd_rx: mpsc::Receiver<AppCommand>,
    shortcut_status: SharedShortcutStatus,
    shutdown: crate::SharedShutdown,
    vis: VisibilityController,
    /// Set while refresh writes widgets, so the resulting notify signals are
    /// ignored instead of treated as user input.
    guard: bool,
    // Local control state, re-synced whenever settings change elsewhere.
    last_seen: AppSettings,
    brightness: i32,
    temperature: i32,
    auto: bool,
    launch_at_login: bool,
    times: [TwelveHourTime; 4],
    range: TemperatureRange,
    shortcuts: [Option<Shortcut>; 4],
    recording: Option<usize>,
    shortcut_message: Option<(String, Instant)>,
    cli_copied_at: Option<Instant>,
    cached_displays: Vec<moonglow_core::DisplayInfo>,
    drag_handle_night: Option<bool>,
    drag_start_x: f64,
    w: UiWidgets,
}

type SharedModel<S> = Rc<RefCell<Model<S>>>;

/// Runs `write` with value handlers suppressed. `write` must not borrow the
/// model: GTK emits notify signals synchronously, so any borrow held here
/// would panic in the re-entered handler. Pass cloned handles and values in.
fn suppressed<S: SettingsPersistence>(model: &SharedModel<S>, write: impl FnOnce()) {
    model.borrow_mut().guard = true;
    write();
    model.borrow_mut().guard = false;
}

impl<S: SettingsPersistence> Model<S> {
    fn mark_written(&mut self) {
        if let Ok(state) = self.state.lock() {
            self.last_seen = state.settings().clone();
        }
    }

    fn sync_from(model: &SharedModel<S>, settings: &AppSettings) {
        {
            let mut m = model.borrow_mut();
            m.brightness = settings.manual_brightness;
            m.auto = settings.auto_color_temperature;
            m.launch_at_login = settings.launch_at_login;
            m.times = [
                settings.morning_start.twelve_hour(),
                settings.morning_end.twelve_hour(),
                settings.evening_start.twelve_hour(),
                settings.evening_end.twelve_hour(),
            ];
            m.range = TemperatureRange::from_settings(settings);
            m.shortcuts = shortcuts_of(settings);
            m.last_seen = settings.clone();
        }
        Self::push_to_widgets(model);
    }

    /// Writes local control state into the widgets. Handles and values are
    /// cloned out first so no borrow is held while GTK synchronously
    /// re-enters the value handlers (which then see `guard` and return).
    fn push_to_widgets(model: &SharedModel<S>) {
        struct Snap {
            brightness: i32,
            temperature: i32,
            auto: bool,
            login: bool,
            times: [TwelveHourTime; 4],
        }
        let (snap, bscale, bval, tscale, tval, asw, lsw, drops, area) = {
            let m = model.borrow();
            (
                Snap {
                    brightness: m.brightness,
                    temperature: m.temperature,
                    auto: m.auto,
                    login: m.launch_at_login,
                    times: m.times,
                },
                m.w.brightness_scale.clone(),
                m.w.brightness_value.clone(),
                m.w.temp_scale.clone(),
                m.w.temp_value.clone(),
                m.w.auto_switch.clone(),
                m.w.login_switch.clone(),
                m.w.time_drops.clone(),
                m.w.range_area.clone(),
            )
        };
        suppressed(model, move || {
            bscale.set_value(snap.brightness as f64);
            bval.set_label(&format!("{}%", snap.brightness));
            tscale.set_value(snap.temperature as f64);
            tval.set_label(&format!("{} K", snap.temperature));
            asw.set_active(snap.auto);
            lsw.set_active(snap.login);
            for (time, triple) in snap.times.iter().zip(drops.iter()) {
                triple[0].set_selected((time.hour as u32).saturating_sub(1));
                triple[1].set_selected(time.minute as u32);
                triple[2].set_selected(match time.meridiem {
                    Meridiem::Am => 0,
                    Meridiem::Pm => 1,
                });
            }
            area.queue_draw();
        });
    }

    fn resync_displayed_temp(model: &SharedModel<S>) {
        let temperature = model
            .borrow()
            .state
            .clone()
            .lock()
            .ok()
            .map(|state| state.color_temperature());
        let Some(temperature) = temperature else {
            return;
        };
        model.borrow_mut().temperature = temperature;
        let (scale, label) = {
            let m = model.borrow();
            (m.w.temp_scale.clone(), m.w.temp_value.clone())
        };
        suppressed(model, move || {
            scale.set_value(temperature as f64);
            label.set_label(&format!("{temperature} K"));
        });
    }

    fn apply_time(&mut self, field: usize, which: ScheduleTime, time: TwelveHourTime) {
        if time == self.times[field] {
            return;
        }
        self.times[field] = time;
        if let Some(parsed) = TimeOfDay::from_twelve_hour(time) {
            if let Ok(mut state) = self.state.lock() {
                state.set_schedule_time(which, parsed);
            }
        }
        self.mark_written();
    }

    fn assign_shortcut(&mut self, index: usize, value: Option<Shortcut>) {
        let action = ShortcutAction::ALL[index];
        if let Ok(mut state) = self.state.lock() {
            state.set_shortcut(action, value);
        }
        if let Some(value) = value {
            for (other_index, other) in self.shortcuts.iter_mut().enumerate() {
                if other_index != index && *other == Some(value) {
                    *other = None;
                }
            }
        }
        self.shortcuts[index] = value;
        self.shortcut_message = Some((
            match value {
                Some(shortcut) => format!("{} set to {}.", action.label(), shortcut),
                None => format!("{} shortcut cleared.", action.label()),
            },
            Instant::now(),
        ));
        self.mark_written();
    }
}

// MARK: - Widget helpers

/// Small explanatory label.
fn caption(text: &str) -> gtk::Label {
    let label = gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .build();
    label.add_css_class("caption");
    label.add_css_class("dim-label");
    label
}

fn value_label(text: &str) -> gtk::Label {
    gtk::Label::builder().label(text).xalign(1.0).build()
}

fn make_scale(value: f64, lower: f64, upper: f64, step: f64) -> gtk::Scale {
    let adjustment = gtk::Adjustment::new(value, lower, upper, step, step * 5.0, 0.0);
    gtk::Scale::builder()
        .orientation(gtk::Orientation::Horizontal)
        .adjustment(&adjustment)
        .draw_value(false)
        .hexpand(true)
        .build()
}

fn string_drops(items: &[String]) -> gtk::DropDown {
    let refs: Vec<&str> = items.iter().map(String::as_str).collect();
    gtk::DropDown::from_strings(&refs)
}

/// Value at an X inside a bar of `width`, snapped and clamped to the scale.
fn range_bar_value_at(x: f32, width: f32) -> i32 {
    let span = (width - 2.0 * RANGE_HANDLE_RADIUS).max(0.0);
    let fraction = if span > 0.0 {
        ((x - RANGE_HANDLE_RADIUS) / span).clamp(0.0, 1.0)
    } else {
        0.0
    } as f64;
    TemperatureScale::value_at_fraction(fraction, TemperatureScale::bounds())
}

fn range_bar_x(value: i32, width: f32) -> f32 {
    let span = (width - 2.0 * RANGE_HANDLE_RADIUS).max(0.0);
    RANGE_HANDLE_RADIUS
        + TemperatureScale::fraction(value, TemperatureScale::bounds()) as f32 * span
}

/// Paints the two-handle day/night range bar: warm-to-cool gradient, dimmed
/// outside the selected span, triangle handles on the night and day targets.
fn paint_range(range: &TemperatureRange, cr: &cairo::Context, width: i32, height: i32) {
    let width = width as f64;
    let height = height as f64;
    let track_y = height / 2.0 - TRACK_HEIGHT as f64 / 2.0;
    let left = RANGE_HANDLE_RADIUS as f64;
    let track_w = (width - 2.0 * left).max(0.0);
    let segments = 72;
    for index in 0..segments {
        let t0 = index as f64 / segments as f64;
        let t1 = (index + 1) as f64 / segments as f64;
        let kelvin = (TEMP_MIN + (t0 + t1) / 2.0 * (TEMP_MAX - TEMP_MIN)) as i32;
        let (r, g, b) = temperature_rgb(kelvin);
        cr.set_source_rgb(r, g, b);
        cr.rectangle(
            left + t0 * track_w,
            track_y,
            (t1 - t0) * track_w + 1.0,
            TRACK_HEIGHT as f64,
        );
        let _ = cr.fill();
    }
    // Dim the track outside the selected span.
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.45);
    let night_x = range_bar_x(range.night(), width as f32) as f64;
    let day_x = range_bar_x(range.day(), width as f32) as f64;
    cr.rectangle(
        left,
        track_y,
        (night_x - left).max(0.0),
        TRACK_HEIGHT as f64,
    );
    let _ = cr.fill();
    cr.rectangle(
        day_x,
        track_y,
        (left + track_w - day_x).max(0.0),
        TRACK_HEIGHT as f64,
    );
    let _ = cr.fill();
    // Triangle handles.
    for x in [night_x, day_x] {
        cr.move_to(x, height / 2.0 - HANDLE_APEX_RISE as f64);
        cr.line_to(
            x + HANDLE_HALF_WIDTH as f64,
            height / 2.0 + HANDLE_BASE_DROP as f64,
        );
        cr.line_to(
            x - HANDLE_HALF_WIDTH as f64,
            height / 2.0 + HANDLE_BASE_DROP as f64,
        );
        cr.close_path();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        let _ = cr.fill_preserve();
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.55);
        cr.set_line_width(2.0);
        let _ = cr.stroke();
    }
}

// MARK: - Construction

/// Builds the window and every control, without connecting signals yet.
/// Returns the widget handles plus the page the tail groups attach to.
#[allow(clippy::too_many_lines)]
fn build_widgets(settings: &AppSettings) -> (UiWidgets, adw::PreferencesPage) {
    let window = adw::PreferencesWindow::builder()
        .title("Moonglow")
        .default_width(470)
        .default_height(680)
        .build();
    let page = adw::PreferencesPage::new();
    window.add(&page);

    // Status: the live "Automatic · 6500 K · Brightness 80%" line.
    let status_group = adw::PreferencesGroup::builder().title("Status").build();
    let status_row = adw::ActionRow::builder().title("Starting…").build();
    status_group.add(&status_row);
    page.add(&status_group);

    // Brightness.
    let brightness_group = adw::PreferencesGroup::builder().title("Brightness").build();
    let brightness_scale = make_scale(settings.manual_brightness as f64, 1.0, 100.0, 1.0);
    let brightness_value = value_label(&format!("{}%", settings.manual_brightness));
    let brightness_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    brightness_box.append(&brightness_scale);
    brightness_box.append(&brightness_value);
    let brightness_row = adw::ActionRow::builder().title("Level").build();
    brightness_row.add_suffix(&brightness_box);
    brightness_group.add(&brightness_row);
    page.add(&brightness_group);

    // Colour temperature.
    let temp_group = adw::PreferencesGroup::builder()
        .title("Colour temperature")
        .build();
    let auto_switch = gtk::Switch::builder()
        .active(settings.auto_color_temperature)
        .valign(gtk::Align::Center)
        .build();
    let auto_row = adw::ActionRow::builder()
        .title("Automatic (follow schedule)")
        .build();
    auto_row.add_suffix(&auto_switch);
    temp_group.add(&auto_row);
    let temp_scale = make_scale(
        settings.manual_color_temp_k as f64,
        TEMP_MIN,
        TEMP_MAX,
        TEMP_STEP,
    );
    let temp_value = value_label(&format!("{} K", settings.manual_color_temp_k));
    let temp_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    temp_box.append(&temp_scale);
    temp_box.append(&temp_value);
    let temp_row = adw::ActionRow::builder().title("Temperature").build();
    temp_row.add_suffix(&temp_box);
    temp_group.add(&temp_row);
    let temp_hint = caption("");
    temp_group.add(&temp_hint);
    page.add(&temp_group);

    // Day / night range.
    let range_group = adw::PreferencesGroup::builder()
        .title("Day / night range")
        .description("Warmest night target at the left, coolest day target at the right.")
        .build();
    let range_area = gtk::DrawingArea::builder()
        .content_height(RANGE_BAR_HEIGHT)
        .hexpand(true)
        .build();
    range_group.add(&range_area);
    let ends = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .build();
    let min_label = caption("900 K");
    let max_label = caption("6600 K");
    max_label.set_xalign(1.0);
    ends.append(&min_label);
    ends.set_homogeneous(true);
    ends.append(&max_label);
    range_group.add(&ends);
    let range_text = caption("");
    range_group.add(&range_text);
    page.add(&range_group);

    let widgets = UiWidgets {
        window,
        status_row,
        brightness_scale,
        brightness_value,
        brightness_group,
        temp_scale,
        temp_value,
        temp_hint,
        auto_switch,
        range_area,
        range_text,
        record_buttons: std::array::from_fn(|_| gtk::Button::builder().label("Record").build()),
        clear_buttons: std::array::from_fn(|_| gtk::Button::builder().label("Clear").build()),
        shortcut_message: caption(""),
        backend_detail: caption(""),
        time_drops: std::array::from_fn(|_| {
            let hours: Vec<String> = (1..=12).map(|h| h.to_string()).collect();
            let minutes: Vec<String> = (0..=59).map(|m| format!("{m:02}")).collect();
            let meridiems = vec!["AM".to_string(), "PM".to_string()];
            [
                string_drops(&hours),
                string_drops(&minutes),
                string_drops(&meridiems),
            ]
        }),
        login_switch: gtk::Switch::builder()
            .active(settings.launch_at_login)
            .valign(gtk::Align::Center)
            .build(),
        login_status: caption(""),
        open_button: gtk::Button::builder().label("Open").build(),
        displays_box: gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .build(),
        refresh_button: gtk::Button::builder().label("Refresh displays").build(),
        copy_button: gtk::Button::builder()
            .label("Copy all to clipboard")
            .build(),
        cli_copied: caption(""),
        hide_button: gtk::Button::builder().label("Hide").build(),
        quit_button: {
            let button = gtk::Button::builder().label("Quit Moonglow").build();
            button.add_css_class("destructive-action");
            button
        },
    };
    (widgets, page)
}

/// Appends the schedule, shortcuts, startup, displays, command-line, and
/// footer groups. Pure widget building; signals connect later against the
/// shared model.
fn build_tail(page: &adw::PreferencesPage, w: &UiWidgets) {
    // Schedule.
    let schedule_group = adw::PreferencesGroup::builder()
        .title("Schedule")
        .description("When the temperature fades between day and night.")
        .build();
    for (field, title) in ["Morning from", "Morning to", "Evening from", "Evening to"]
        .into_iter()
        .enumerate()
    {
        let row = adw::ActionRow::builder().title(title).build();
        let pickers = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .build();
        for drop in &w.time_drops[field] {
            pickers.append(drop);
        }
        row.add_suffix(&pickers);
        schedule_group.add(&row);
    }
    page.add(&schedule_group);

    // Shortcuts, below Schedule and collapsed like the Windows window.
    let shortcuts_group = adw::PreferencesGroup::new();
    let shortcuts = adw::ExpanderRow::builder()
        .title("Shortcuts")
        .subtitle("Global keys that work from any application")
        .build();
    for (index, action) in ShortcutAction::ALL.into_iter().enumerate() {
        let row = adw::ActionRow::builder()
            .title(action.label())
            .subtitle(shortcut_hint(action))
            .build();
        let buttons = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .build();
        buttons.append(&w.record_buttons[index]);
        buttons.append(&w.clear_buttons[index]);
        row.add_suffix(&buttons);
        shortcuts.add_row(&row);
    }
    let shortcut_notes = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .margin_top(4)
        .margin_bottom(4)
        .build();
    shortcut_notes.append(&w.shortcut_message);
    shortcut_notes.append(&w.backend_detail);
    shortcuts.add_row(&shortcut_notes);
    shortcuts_group.add(&shortcuts);
    page.add(&shortcuts_group);

    // Startup (collapsed until needed, like the old shell).
    let startup_group = adw::PreferencesGroup::new();
    let startup = adw::ExpanderRow::builder().title("Startup").build();
    let login_row = adw::ActionRow::builder().title("Open at login").build();
    login_row.add_suffix(&w.login_switch);
    startup.add_row(&login_row);
    let status_row = adw::ActionRow::builder().title("Status").build();
    status_row.add_suffix(&w.login_status);
    startup.add_row(&status_row);
    let autostart_row = adw::ActionRow::builder().title("Autostart folder").build();
    autostart_row.add_suffix(&w.open_button);
    startup.add_row(&autostart_row);
    startup_group.add(&startup);
    page.add(&startup_group);

    // Displays (collapsed until needed).
    let displays_group = adw::PreferencesGroup::new();
    let displays = adw::ExpanderRow::builder().title("Displays").build();
    let displays_inner = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    displays_inner.append(&w.displays_box);
    displays_inner.append(&w.refresh_button);
    displays.add_row(&displays_inner);
    displays_group.add(&displays);
    page.add(&displays_group);

    // Command line (collapsed until needed).
    let cli_group = adw::PreferencesGroup::new();
    let cli = adw::ExpanderRow::builder().title("Command line").build();
    let cli_inner = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    cli_inner.append(&caption(
        "Terminal equivalents. With no app running, the setters drive hardware directly.",
    ));
    for command in CLI_COMMANDS {
        let usage = gtk::Label::builder()
            .label(command.usage)
            .selectable(true)
            .xalign(0.0)
            .build();
        usage.add_css_class("monospace");
        cli_inner.append(&usage);
        cli_inner.append(&caption(command.desc));
    }
    cli_inner.append(&w.copy_button);
    cli_inner.append(&w.cli_copied);
    cli.add_row(&cli_inner);
    cli_group.add(&cli);
    page.add(&cli_group);

    // Footer: background note, version, Hide, Quit.
    let footer = adw::PreferencesGroup::builder().title("Moonglow").build();
    footer.add(&caption(
        "Closing this window keeps Moonglow running so the schedule and shortcuts stay active.",
    ));
    footer.add(&caption(&format!("v{}", env!("CARGO_PKG_VERSION"))));
    // One row: Hide on the left, Quit on the right.
    let actions = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    actions.append(&w.hide_button);
    let spacer = gtk::Box::builder().hexpand(true).build();
    actions.append(&spacer);
    actions.append(&w.quit_button);
    footer.add(&actions);
    page.add(&footer);
}

// MARK: - Signals

/// Connects every control to the shared model. Each handler borrows the model
/// briefly and returns early while `guard` is set (see [`suppressed`]).
#[allow(clippy::too_many_lines)]
fn connect_signals<S: SettingsPersistence + 'static>(model: &SharedModel<S>) {
    // Brightness slider.
    {
        let scale = model.borrow().w.brightness_scale.clone();
        let model = model.clone();
        scale.connect_value_changed(move |scale| {
            let mut model = model.borrow_mut();
            // Belt and braces with `set_sensitive`: assistive tools can drive
            // an insensitive scale, and a dead control must never write.
            if model.guard || !scale.is_sensitive() {
                return;
            }
            let value = (scale.value().round() as i32).clamp(1, 100);
            if value == model.brightness {
                return;
            }
            model.brightness = value;
            model.w.brightness_value.set_label(&format!("{value}%"));
            if let Ok(mut state) = model.state.lock() {
                state.set_brightness(value);
            }
            model.mark_written();
        });
    }
    // Temperature slider.
    {
        let scale = model.borrow().w.temp_scale.clone();
        let model = model.clone();
        scale.connect_value_changed(move |scale| {
            let mut model = model.borrow_mut();
            // Same dead-control rule as brightness (see above).
            if model.guard || !scale.is_sensitive() {
                return;
            }
            let value = TemperatureRange::snap(scale.value().round() as i32);
            if value == model.temperature {
                return;
            }
            model.temperature = value;
            model.w.temp_value.set_label(&format!("{value} K"));
            if let Ok(mut state) = model.state.lock() {
                state.set_color_temperature(value);
            }
            model.mark_written();
        });
    }
    // Automatic switch.
    {
        let auto = model.borrow().w.auto_switch.clone();
        let model = model.clone();
        auto.connect_active_notify(move |row| {
            let mut m = model.borrow_mut();
            if m.guard {
                return;
            }
            let on = row.is_active();
            if on == m.auto {
                return;
            }
            m.auto = on;
            if let Ok(mut state) = m.state.lock() {
                state.set_auto_color_temperature(on);
            }
            m.mark_written();
            drop(m);
            Model::resync_displayed_temp(&model);
        });
    }
    // Day/night range bar: drag the nearest handle.
    {
        let area = model.borrow().w.range_area.clone();
        let model = model.clone();
        area.set_draw_func(move |_area, cr, width, height| {
            let range = model.borrow().range;
            paint_range(&range, cr, width, height);
        });
    }
    {
        let area = model.borrow().w.range_area.clone();
        let state = model.borrow().state.clone();
        let drag = gtk::GestureDrag::new();
        area.add_controller(drag.clone());
        let begin_model = model.clone();
        drag.connect_drag_begin(move |_, start_x, _| {
            let mut model = begin_model.borrow_mut();
            model.drag_start_x = start_x;
            let width = model.w.range_area.width() as f32;
            let night_x = range_bar_x(model.range.night(), width);
            let day_x = range_bar_x(model.range.day(), width);
            model.drag_handle_night =
                Some((start_x as f32 - night_x).abs() <= (start_x as f32 - day_x).abs());
        });
        let update_model = model.clone();
        drag.connect_drag_update(move |_, offset_x, _| {
            let mut model = update_model.borrow_mut();
            let Some(night_handle) = model.drag_handle_night else {
                return;
            };
            let width = model.w.range_area.width() as f32;
            let value = range_bar_value_at((model.drag_start_x + offset_x) as f32, width);
            let next = if night_handle {
                model.range.moving_night(value)
            } else {
                model.range.moving_day(value)
            };
            if next != model.range {
                model.range = next;
                if let Ok(mut backend) = state.lock() {
                    backend.set_temperature_range(next);
                }
                model.mark_written();
                model.w.range_area.queue_draw();
            }
        });
        let end_model = model.clone();
        drag.connect_drag_end(move |_, _, _| {
            end_model.borrow_mut().drag_handle_night = None;
        });
    }
    // Shortcut record/clear buttons.
    for index in 0..4 {
        {
            let button = model.borrow().w.record_buttons[index].clone();
            let model = model.clone();
            button.connect_clicked(move |_| {
                let mut model = model.borrow_mut();
                model.recording = if model.recording == Some(index) {
                    None
                } else {
                    Some(index)
                };
                model.shortcut_message = None;
            });
        }
        {
            let button = model.borrow().w.clear_buttons[index].clone();
            let model = model.clone();
            button.connect_clicked(move |_| {
                model.borrow_mut().assign_shortcut(index, None);
            });
        }
    }
    // Schedule pickers.
    for (field, which) in SCHEDULE_FIELDS.iter().enumerate() {
        for component in 0..3 {
            let drop = model.borrow().w.time_drops[field][component].clone();
            let model = model.clone();
            drop.connect_selected_notify(move |drop| {
                let mut model = model.borrow_mut();
                if model.guard {
                    return;
                }
                let selected = drop.selected();
                let mut time = model.times[field];
                match component {
                    0 => time.hour = selected as i32 + 1,
                    1 => time.minute = selected as i32,
                    _ => {
                        time.meridiem = if selected == 0 {
                            Meridiem::Am
                        } else {
                            Meridiem::Pm
                        }
                    }
                }
                model.apply_time(field, *which, time);
            });
        }
    }
    // Startup switch (a refused change snaps back to the truth).
    {
        let login = model.borrow().w.login_switch.clone();
        let snap_back = login.clone();
        let model = model.clone();
        login.connect_active_notify(move |row| {
            let mut m = model.borrow_mut();
            if m.guard {
                return;
            }
            let on = row.is_active();
            if on == m.launch_at_login {
                return;
            }
            let state_arc = m.state.clone();
            if let Ok(mut backend) = state_arc.lock() {
                backend.set_launch_at_login(on);
                m.launch_at_login = backend.launch_at_login();
            }
            let back = m.launch_at_login;
            m.mark_written();
            drop(m);
            if back != on {
                suppressed(&model, {
                    let s = snap_back.clone();
                    move || s.set_active(back)
                });
            }
        });
    }
    // Startup folder, display refresh, CLI copy, hide, quit.
    {
        let button = model.borrow().w.open_button.clone();
        let state = model.borrow().state.clone();
        button.connect_clicked(move |_| {
            if let Ok(backend) = state.lock() {
                backend.open_login_settings();
            }
        });
    }
    {
        let button = model.borrow().w.refresh_button.clone();
        let state = model.borrow().state.clone();
        button.connect_clicked(move |_| {
            if let Ok(mut backend) = state.lock() {
                backend.refresh_displays();
            }
        });
    }
    {
        let button = model.borrow().w.copy_button.clone();
        let model = model.clone();
        button.connect_clicked(move |_| {
            let mut model = model.borrow_mut();
            let sheet = CLI_COMMANDS
                .iter()
                .map(|command| format!("moonglow {}  # {}", command.usage, command.desc))
                .collect::<Vec<_>>()
                .join("\n");
            if let Some(display) = gdk::Display::default() {
                display.clipboard().set_text(&sheet);
            }
            model.w.cli_copied.set_label("Copied.");
            model.cli_copied_at = Some(Instant::now());
        });
    }
    {
        let button = model.borrow().w.hide_button.clone();
        let model = model.clone();
        button.connect_clicked(move |_| {
            let mut model = model.borrow_mut();
            let commands = model.vis.on_close_requested();
            apply_cmds(&model.w.window, commands);
        });
    }
    {
        let button = model.borrow().w.quit_button.clone();
        let model = model.clone();
        button.connect_clicked(move |_| {
            let shutdown = model.borrow().shutdown.clone();
            crate::quit_now(&shutdown);
        });
    }
    // Window close hides instead of quitting.
    {
        let win = model.borrow().w.window.clone();
        let model = model.clone();
        win.connect_close_request(move |window| {
            let mut model = model.borrow_mut();
            let commands = model.vis.on_close_requested();
            apply_cmds(window, commands);
            glib::Propagation::Stop
        });
    }
    // Shortcut recorder: captures the next key press while recording.
    {
        let win = model.borrow().w.window.clone();
        let model = model.clone();
        let keys = gtk::EventControllerKey::new();
        win.add_controller(keys.clone());
        keys.connect_key_pressed(move |_, keyval, _, modifiers| {
            let mut model = model.borrow_mut();
            let Some(index) = model.recording else {
                return glib::Propagation::Proceed;
            };
            if keyval == gdk::Key::Escape {
                model.recording = None;
                return glib::Propagation::Stop;
            }
            let name = keyval
                .name()
                .map(|name| name.to_string())
                .unwrap_or_default();
            let Some(key) = shortcut_key_from_gdk(&name) else {
                model.shortcut_message = Some((
                    "That key cannot be used. Try an arrow, letter, digit, or function key."
                        .to_string(),
                    Instant::now(),
                ));
                return glib::Propagation::Stop;
            };
            let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            let alt = modifiers.contains(gdk::ModifierType::ALT_MASK);
            let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
            if !(ctrl || alt || shift) {
                model.shortcut_message = Some((
                    "Add Ctrl, Alt, or Shift so the shortcut does not fire while typing."
                        .to_string(),
                    Instant::now(),
                ));
                return glib::Propagation::Stop;
            }
            model.assign_shortcut(index, Some(Shortcut::new(key, ctrl, alt, shift, false)));
            model.recording = None;
            glib::Propagation::Stop
        });
    }
}

// MARK: - Refresh

/// Applies visibility commands to the window. Free function (not a [`Model`]
/// method) so callers never need to name the model's persistence type.
fn apply_cmds(window: &adw::PreferencesWindow, commands: Vec<VisCommand>) {
    for command in commands {
        match command {
            VisCommand::SetVisible(true) | VisCommand::Focus => window.present(),
            VisCommand::SetVisible(false) => window.set_visible(false),
            VisCommand::SetMinimized(true) => window.minimize(),
            VisCommand::SetMinimized(false) => window.unminimize(),
            VisCommand::CancelClose => {}
        }
    }
}

impl<S: SettingsPersistence> Model<S> {
    fn poll(&mut self) {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            match cmd {
                AppCommand::ShowWindow => {
                    let commands = self.vis.show();
                    apply_cmds(&self.w.window, commands);
                }
                AppCommand::Quit => crate::quit_now(&self.shutdown),
            }
        }
    }

    fn rebuild_displays(&mut self, displays: &[moonglow_core::DisplayInfo]) {
        self.cached_displays = displays.to_vec();
        let list = &self.w.displays_box;
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        if displays.is_empty() {
            list.append(&caption("No displays detected."));
            return;
        }
        for display in displays {
            let name = gtk::Label::builder()
                .label(&display.name)
                .xalign(0.0)
                .build();
            list.append(&name);
            list.append(&caption(&display.capability_summary()));
        }
    }

    /// Re-reads state and updates every live label. Widget positions change
    /// only through [`Model::sync_from`] (settings changed elsewhere) or the
    /// automatic-temperature tick, never from these labels.
    fn refresh_labels(&mut self) {
        let (automatic, temperature, brightness) = match self.state.lock() {
            Ok(state) => (
                state.auto_color_temperature(),
                state.color_temperature(),
                state.brightness(),
            ),
            Err(_) => (self.auto, self.temperature, self.brightness),
        };
        self.w.status_row.set_title(&format!(
            "{} · {temperature} K · Brightness {brightness}%",
            if automatic { "Automatic" } else { "Manual" },
        ));
        let (supported, status) = match self.state.lock() {
            Ok(state) => (state.has_brightness_support(), state.brightness_status()),
            Err(_) => (false, String::new()),
        };
        self.w.brightness_scale.set_sensitive(supported);
        self.w.brightness_group.set_description(Some(&status));
        let (color_supported, color_status) = match self.state.lock() {
            Ok(state) => (state.has_color_support(), state.color_status()),
            Err(_) => (false, String::new()),
        };
        self.w
            .temp_scale
            .set_sensitive(color_supported && !self.auto);
        self.w.temp_hint.set_label(if !color_supported {
            &color_status
        } else if self.auto {
            "The schedule is driving the temperature. Switch Automatic off to adjust it by hand."
        } else {
            &color_status
        });
        self.w.range_text.set_label(&self.range.display_text());
        let record_labels: [String; 4] = std::array::from_fn(|index| {
            if self.recording == Some(index) {
                "Press keys…".to_string()
            } else {
                self.shortcuts[index]
                    .map(Shortcut::display_text)
                    .unwrap_or_else(|| "Record".to_string())
            }
        });
        for (index, label) in record_labels.into_iter().enumerate() {
            self.w.record_buttons[index].set_label(&label);
            self.w.record_buttons[index].set_css_classes(if self.recording == Some(index) {
                &["suggested-action"]
            } else {
                &[]
            });
            self.w.clear_buttons[index]
                .set_visible(self.shortcuts[index].is_some() && self.recording != Some(index));
        }
        let message = self.shortcut_message.as_ref().and_then(|(text, at)| {
            (at.elapsed() < Duration::from_secs(MESSAGE_SECONDS)).then_some(text.clone())
        });
        self.w
            .shortcut_message
            .set_label(message.as_deref().unwrap_or(""));
        if let Ok(status) = self.shortcut_status.lock() {
            self.w.backend_detail.set_label(&status.detail);
        }
        let displays = match self.state.lock() {
            Ok(state) => {
                self.w
                    .login_status
                    .set_label(&state.launch_at_login_status());
                state.displays().to_vec()
            }
            Err(_) => return,
        };
        if displays != self.cached_displays {
            self.rebuild_displays(&displays);
        }
        let copied = self
            .cli_copied_at
            .map(|at| at.elapsed() < Duration::from_secs(2))
            .unwrap_or(false);
        self.w
            .cli_copied
            .set_label(if copied { "Copied." } else { "" });
    }

    /// One poll pass. Never holds a model borrow across a widget write:
    /// every helper below borrows briefly or not at all, so synchronously
    /// re-entered signal handlers always find the cell free.
    fn tick(model: &SharedModel<S>) {
        model.borrow_mut().poll();
        if !model.borrow().vis.visible() {
            // Hidden: nothing to repaint. The next visible tick re-syncs, so
            // changes made while hidden are never lost.
            return;
        }
        // Re-sync local controls when settings changed elsewhere (CLI, IPC).
        let settings = {
            let arc = model.borrow().state.clone();
            arc.lock().ok().map(|state| state.settings().clone())
        };
        if let Some(settings) = settings {
            let stale = model.borrow().last_seen != settings;
            if stale {
                Self::sync_from(model, &settings);
            }
        }
        if model.borrow().auto {
            Self::resync_displayed_temp(model);
        }
        model.borrow_mut().refresh_labels();
    }
}

// MARK: - Window

/// Wakes the settings loop from any thread. The poll timer fires regardless
/// of visibility, so commands never sit unwitnessed; this stays as the
/// explicit nudge every command sender calls after sending.
pub fn wake_ui() {
    // Intentionally a no-op: GTK timeouts keep firing while hidden, unlike
    // the previous shell, which scheduled no repaints and needed a kick.
}

fn assemble<S: SettingsPersistence + 'static>(
    state: SharedState<S>,
    cmd_rx: mpsc::Receiver<AppCommand>,
    shortcut_status: SharedShortcutStatus,
    visible: bool,
    shutdown: crate::SharedShutdown,
) -> (SharedModel<S>, adw::PreferencesWindow) {
    let settings = state
        .lock()
        .map(|state| state.settings().clone())
        .unwrap_or_default();
    let (w, page) = build_widgets(&settings);
    build_tail(&page, &w);
    let window = w.window.clone();
    let model: SharedModel<S> = Rc::new(RefCell::new(Model {
        state,
        cmd_rx,
        shortcut_status,
        shutdown,
        vis: VisibilityController::new(visible),
        guard: false,
        last_seen: settings.clone(),
        brightness: settings.manual_brightness,
        temperature: settings.manual_color_temp_k,
        auto: settings.auto_color_temperature,
        launch_at_login: settings.launch_at_login,
        times: [
            settings.morning_start.twelve_hour(),
            settings.morning_end.twelve_hour(),
            settings.evening_start.twelve_hour(),
            settings.evening_end.twelve_hour(),
        ],
        range: TemperatureRange::from_settings(&settings),
        shortcuts: shortcuts_of(&settings),
        recording: None,
        shortcut_message: None,
        cli_copied_at: None,
        cached_displays: Vec::new(),
        drag_handle_night: None,
        drag_start_x: 0.0,
        w,
    }));
    connect_signals(&model);
    // In automatic mode the slider shows the live scheduled value.
    if model.borrow().auto {
        Model::resync_displayed_temp(&model);
    }
    Model::push_to_widgets(&model);
    Model::tick(&model);
    (model, window)
}

/// Runs the settings window on the calling (main) thread. Returns when the
/// window quits. Unlike the previous shell this is a native Wayland window,
/// so no display-variable surgery is needed.
pub fn run_settings_window<S: SettingsPersistence + 'static>(
    state: SharedState<S>,
    cmd_rx: mpsc::Receiver<AppCommand>,
    shortcut_status: SharedShortcutStatus,
    visible: bool,
    shutdown: crate::SharedShutdown,
) -> Result<(), String> {
    // No application_id: "moonglow" is not a valid dotted D-Bus name, and a
    // dotted one would mismatch moonglow.desktop (StartupWMClass=moonglow)
    // for dock grouping. Single-instance stays with our own IPC socket.
    let app = adw::Application::builder().build();
    // Stay alive while hidden so IPC commands still land. The guard must be
    // kept: dropping it releases the hold immediately.
    let _hold = app.hold();
    // Only the first activation owns the command channel; a repeat one just
    // re-presents the existing window. (Our own IPC socket already guarantees
    // a single primary, so this is belt and braces.)
    let cmd_rx = Rc::new(RefCell::new(Some(cmd_rx)));
    let window_slot: Rc<RefCell<Option<adw::PreferencesWindow>>> = Rc::new(RefCell::new(None));
    app.connect_activate(move |app| {
        if let Some(window) = window_slot.borrow().clone() {
            window.present();
            return;
        }
        let Some(rx) = cmd_rx.borrow_mut().take() else {
            return;
        };
        let (model, window) = assemble(
            state.clone(),
            rx,
            shortcut_status.clone(),
            visible,
            shutdown.clone(),
        );
        app.add_window(&window);
        {
            let tick_model = model.clone();
            glib::timeout_add_local(POLL_INTERVAL, move || {
                Model::tick(&tick_model);
                glib::ControlFlow::Continue
            });
        }
        *window_slot.borrow_mut() = Some(window);
        if visible {
            let handle = model.borrow().w.window.clone();
            let commands = model.borrow_mut().vis.show();
            apply_cmds(&handle, commands);
        }
    });
    app.run_with_args(&[] as &[&str]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{shortcut_key_from_gdk, temperature_rgb};

    #[test]
    fn recorder_maps_supported_keys() {
        assert_eq!(
            shortcut_key_from_gdk("Up").map(|key| key.name()),
            Some("Up")
        );
        assert_eq!(
            shortcut_key_from_gdk("F9").map(|key| key.name()),
            Some("F9")
        );
        assert_eq!(shortcut_key_from_gdk("a").map(|key| key.name()), Some("A"));
        assert_eq!(
            shortcut_key_from_gdk("Page_Up").map(|key| key.name()),
            Some("PageUp")
        );
        assert_eq!(
            shortcut_key_from_gdk("Return").map(|key| key.name()),
            Some("Enter")
        );
        assert_eq!(shortcut_key_from_gdk("Shift_L"), None);
        assert_eq!(shortcut_key_from_gdk("KP_1"), None);
    }

    #[test]
    fn temperature_track_moves_from_warm_to_cool() {
        let warm = temperature_rgb(900);
        let cool = temperature_rgb(6600);
        assert!(warm.0 > warm.2);
        assert!(cool.2 > cool.0);
    }
}
