//! Settings window (egui).
//!
//! Mirrors the macOS settings sheet: brightness and temperature sliders,
//! twelve-hour schedule pickers, the day/night range as one two-handle bar,
//! startup toggle, and the display list. The window starts hidden (tray-first,
//! like the menu-bar-only Mac app) and hides instead of closing; visibility
//! state is owned by [`VisibilityController`] so OS-side restores (dock,
//! taskbar, Alt-Tab) can never strand the window contentless.

use crate::tray::{SharedState, TrayCommand};
use crate::visibility::{VisCommand, VisibilityController};
use nmc_core::settings::AppSettings;
use nmc_core::state::ScheduleTime;
use nmc_core::temperature_range::{TemperatureRange, TemperatureScale};
use nmc_core::time_of_day::TimeOfDay;
use nmc_core::traits::SettingsPersistence;
use nmc_core::twelve_hour::{Meridiem, TwelveHourTime};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Idle time after which the window hides itself, matching macOS.
const AUTO_HIDE_AFTER: Duration = Duration::from_secs(10);

const SCHEDULE_FIELDS: [ScheduleTime; 4] = [
    ScheduleTime::MorningStart,
    ScheduleTime::MorningEnd,
    ScheduleTime::EveningStart,
    ScheduleTime::EveningEnd,
];

pub struct SettingsApp<S: SettingsPersistence> {
    state: SharedState<S>,
    cmd_rx: mpsc::Receiver<TrayCommand>,
    /// Set once the StatusNotifier tray registers (live flag: a late panel
    /// start flips settings into tray mode without a restart).
    tray_popup: Arc<AtomicBool>,
    shutdown: crate::SharedShutdown,
    vis: VisibilityController,
    theme_reported: bool,
    last_system_theme: Option<egui::Theme>,
    gsettings_dark: bool,
    gsettings_checked: Instant,
    last_interaction: Instant,
    // Local control state, re-synced whenever settings change elsewhere.
    last_seen: AppSettings,
    brightness: i32,
    temperature: i32,
    auto: bool,
    launch_at_login: bool,
    times: [TwelveHourTime; 4],
    range: TemperatureRange,
    cli_copied_at: Option<Instant>,
}

impl<S: SettingsPersistence> SettingsApp<S> {
    pub fn new(
        state: SharedState<S>,
        cmd_rx: mpsc::Receiver<TrayCommand>,
        tray_popup: Arc<AtomicBool>,
        visible: bool,
        shutdown: crate::SharedShutdown,
    ) -> Self {
        let settings = state
            .lock()
            .map(|s| s.settings().clone())
            .unwrap_or_default();
        let mut app = Self {
            state,
            cmd_rx,
            tray_popup,
            shutdown,
            vis: VisibilityController::new(visible),
            theme_reported: false,
            last_system_theme: None,
            gsettings_dark: gsettings_dark().unwrap_or(true),
            gsettings_checked: Instant::now(),
            last_interaction: Instant::now(),
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
            cli_copied_at: None,
        };
        // In automatic mode the slider shows the live scheduled value.
        app.resync_displayed_temp();
        app
    }

    fn sync_from(&mut self, settings: &AppSettings) {
        self.brightness = settings.manual_brightness;
        self.auto = settings.auto_color_temperature;
        self.launch_at_login = settings.launch_at_login;
        self.times = [
            settings.morning_start.twelve_hour(),
            settings.morning_end.twelve_hour(),
            settings.evening_start.twelve_hour(),
            settings.evening_end.twelve_hour(),
        ];
        self.range = TemperatureRange::from_settings(settings);
        if !settings.auto_color_temperature {
            self.temperature = settings.manual_color_temp_k;
        }
    }

    fn resync_displayed_temp(&mut self) {
        if let Ok(state) = self.state.lock() {
            self.temperature = state.color_temperature();
        }
    }

    fn mark_written(&mut self) {
        if let Ok(state) = self.state.lock() {
            self.last_seen = state.settings().clone();
        }
    }

    fn poll_commands(&mut self, ctx: &egui::Context) {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            match cmd {
                TrayCommand::ShowSettings => {
                    let commands = self.vis.show();
                    Self::apply_cmds(ctx, commands);
                    self.last_interaction = Instant::now();
                }
                TrayCommand::Quit => {
                    // Diverges: runs shutdown and exits (the event loop has
                    // no programmatic exit for the root viewport).
                    crate::quit_now(&self.shutdown);
                }
            }
        }
    }

    fn apply_cmds(ctx: &egui::Context, commands: Vec<VisCommand>) {
        for command in commands {
            match command {
                VisCommand::SetVisible(visible) => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(visible));
                }
                VisCommand::SetMinimized(minimized) => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(minimized));
                }
                VisCommand::Focus => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                VisCommand::CancelClose => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
            }
        }
    }
}

impl<S: SettingsPersistence + 'static> eframe::App for SettingsApp<S> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_commands(ctx);
        // No manual theme: always follow the OS. The winit report wins where
        // the backend provides it; otherwise the GNOME fallback below, which
        // is re-polled so live theme flips still apply within seconds.
        let system = ctx.system_theme();
        if self.gsettings_checked.elapsed() > Duration::from_secs(5) {
            self.gsettings_checked = Instant::now();
            if let Some(dark) = gsettings_dark() {
                self.gsettings_dark = dark;
            }
        }
        if !self.theme_reported || system != self.last_system_theme {
            self.theme_reported = true;
            self.last_system_theme = system;
            eprintln!(
                "theme: system reports {system:?}, fallback dark={}",
                self.gsettings_dark
            );
        }
        let dark = system
            .map(|t| t == egui::Theme::Dark)
            .unwrap_or(self.gsettings_dark);
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        // Stay alive while hidden so tray/IPC commands still land.
        ctx.request_repaint_after(Duration::from_millis(500));

        let (os_minimized, os_focused, focus_event, interacted) = ctx.input(|i| {
            let focus_event = i
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::WindowFocused(true)));
            let interacted = i.pointer.any_pressed()
                || i.pointer.button_down(egui::PointerButton::Primary)
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Key { pressed: true, .. }));
            (
                i.viewport().minimized == Some(true),
                i.viewport().focused == Some(true),
                focus_event,
                interacted,
            )
        });
        if interacted || os_focused || focus_event {
            // Input or focus while hidden means the OS restored the window
            // out from under us (dock, taskbar, Alt-Tab): adopt the truth.
            if !self.vis.visible() {
                let commands = self.vis.on_os_restore_signal();
                Self::apply_cmds(ctx, commands);
            }
            self.last_interaction = Instant::now();
        }

        if ctx.input(|i| i.viewport().close_requested()) {
            let commands = self.vis.on_close_requested();
            Self::apply_cmds(ctx, commands);
        }
        if self.last_interaction.elapsed() > AUTO_HIDE_AFTER {
            let commands = self.vis.on_idle_timeout(os_minimized);
            Self::apply_cmds(ctx, commands);
        }
        if !self.vis.visible() {
            return;
        }

        // Re-sync local controls when settings changed elsewhere (tray, IPC).
        let settings = self.state.lock().ok().map(|s| s.settings().clone());
        if let Some(settings) = settings {
            if settings != self.last_seen {
                self.sync_from(&settings);
                self.last_seen = settings;
            }
        }
        if self.auto {
            self.resync_displayed_temp();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // NOTE: no global slider_width here. egui sizes every Slider to
            // exactly spacing.slider_width with no shrink-to-fit, so a width
            // measured at the outer panel overruns nested content (frame +
            // card + scrollbar gutter) and clips on the right. Each card sets
            // it from its own local width instead (see card()).
            ui.spacing_mut().scroll = settings_scroll_style();
            egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Cap content at the viewport: an over-wide row must wrap
                    // or shrink, never push the right margin off-screen.
                    ui.set_max_width(ui.available_width());
                    self.header(ui);
                    self.brightness_card(ui);
                    self.temperature_card(ui);
                    self.schedule_card(ui);
                    self.startup_card(ui);
                    self.displays_card(ui);
                    self.cli_card(ui);
                    self.footer(ui);
                });
            });
        });
    }
}

/// Handle overhang allowance for sliders. The slider handle is centred on
/// the value position, so at an extreme it extends ~half a handle past the
/// widget edge; without this the handle clips on the right at maximum.
const SLIDER_HANDLE_ALLOWANCE: f32 = 8.0;

/// Slider width for card content: the local width minus handle room.
/// Measured HERE (not at the outer panel) because egui renders sliders at
/// exactly `slider_width` with no shrink-to-fit — an outer measurement
/// overruns the nested card and clips on the right.
fn card_slider_width(content_width: f32) -> f32 {
    (content_width - SLIDER_HANDLE_ALLOWANCE).max(0.0)
}

/// Collapsible settings section, collapsed at launch. Expansion lasts the
/// session (egui memory); relaunching collapses again.
fn collapsed_card(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().slider_width = card_slider_width(ui.available_width());
            egui::CollapsingHeader::new(egui::RichText::new(title).strong().size(14.0))
                .default_open(false)
                .show(ui, content);
        });
    ui.add_space(8.0);
}

/// Fallback system-theme detection for backends where winit reports None
/// (XWayland): GNOME's interface color-scheme. None when unreadable.
fn gsettings_dark() -> Option<bool> {
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_color_scheme(&String::from_utf8_lossy(&output.stdout))
}

fn parse_color_scheme(raw: &str) -> Option<bool> {
    match raw.trim().trim_matches('\'') {
        "prefer-dark" => Some(true),
        // 'default' is the light style on stock GNOME and Ubuntu.
        "prefer-light" | "default" => Some(false),
        _ => None,
    }
}

/// One CLI flag for the Command Line card. The catalog test pins these to
/// the clap definition so new flags cannot land without UI coverage.
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
        desc: "Show the settings window.",
    },
    CliCommand {
        name: "quit",
        usage: "--quit",
        desc: "Quit the running app.",
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
        name: "set-temp",
        usage: "--set-temp KELVIN",
        desc: "Set temperature (manual mode).",
    },
    CliCommand {
        name: "warmer",
        usage: "--warmer",
        desc: "Warm by 250 K (manual mode).",
    },
    CliCommand {
        name: "cooler",
        usage: "--cooler",
        desc: "Cool by 250 K (manual mode).",
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
        name: "no-tray",
        usage: "--no-tray",
        desc: "Launch without a tray icon.",
    },
    CliCommand {
        name: "headless",
        usage: "--headless",
        desc: "Schedule plus tray, no window.",
    },
];

/// One settings section: titled card with separated content.
fn card(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().slider_width = card_slider_width(ui.available_width());
            ui.label(egui::RichText::new(title).strong().size(14.0));
            ui.separator();
            content(ui);
        });
    ui.add_space(8.0);
}

/// Whether settings shows the manual temperature slider. The tray popup
/// already offers warmer/cooler steppers, so the slider only appears on
/// tray-less systems, which would otherwise lose manual control entirely.
fn show_manual_slider(manual_color_enabled: bool, tray_popup_available: bool) -> bool {
    manual_color_enabled && !tray_popup_available
}

/// Time-picker combo widths. The side-by-side threshold below is derived
/// from these so the layout can never outgrow its own budget again.
const HOUR_COMBO_WIDTH: f32 = 42.0;
const MINUTE_COMBO_WIDTH: f32 = 46.0;
const MERIDIEM_COMBO_WIDTH: f32 = 52.0;
const PICKER_ITEM_SPACING: f32 = 4.0;
const PICKER_GAP: f32 = 12.0;
const TIME_PICKER_WIDTH: f32 =
    HOUR_COMBO_WIDTH + MINUTE_COMBO_WIDTH + MERIDIEM_COMBO_WIDTH + 2.0 * PICKER_ITEM_SPACING;
/// Headroom above the nominal pair width: rendered text and icon metrics can
/// exceed the combo settings by a few points each.
const PICKER_PAIR_FIT_MARGIN: f32 = 32.0;
/// Minimum card-content width for the from/to pickers to share one row.
const PICKER_PAIR_MIN_WIDTH: f32 = 2.0 * TIME_PICKER_WIDTH + PICKER_GAP + PICKER_PAIR_FIT_MARGIN;

/// Horizontal chrome around card content, in points: the outer margin (2×12),
/// the card frame (2×(14 margin + 1 stroke)), and the solid scrollbar gutter.
/// The gutter comes from egui's own [`ScrollStyle`](egui::style::ScrollStyle)
/// metrics (not a magic number) so an egui upgrade cannot silently desync the
/// budget — the pin test below fails loudly instead.
fn layout_chrome_width() -> f32 {
    24.0 + 30.0 + egui::style::ScrollStyle::solid().allocated_width()
}

/// Card-content width available at this viewport width.
fn schedule_content_budget(viewport_width: f32) -> f32 {
    viewport_width - layout_chrome_width()
}

/// Whether both schedule pickers fit in one row at this content width.
/// Below the threshold they stack, so a narrow window clips nothing.
/// Measure the VIEWPORT (not the scroll content: an over-wide row inflates
/// the content width, which would make this check self-fulfilling).
fn schedule_side_by_side(content_width: f32) -> bool {
    content_width >= PICKER_PAIR_MIN_WIDTH
}

/// Scroll bars that reserve their own space instead of overlaying content.
/// egui's default floating bars paint over the right edge, hiding the margin
/// of full-width rows (range-bar end labels, picker pairs) in a narrow window.
fn settings_scroll_style() -> egui::style::ScrollStyle {
    egui::style::ScrollStyle::solid()
}

/// Label row with a right-aligned value, e.g. `Brightness` … `80%`.
fn labeled_value(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).strong());
        });
    });
}

impl<S: SettingsPersistence> SettingsApp<S> {
    fn header(&mut self, ui: &mut egui::Ui) {
        ui.heading("Night Mode Controller");
        if let Ok(state) = self.state.lock() {
            let mode = if state.auto_color_temperature() {
                "Automatic"
            } else {
                "Manual"
            };
            ui.label(
                egui::RichText::new(format!(
                    "{mode} · {} K · Brightness {}%",
                    state.color_temperature(),
                    state.brightness()
                ))
                .small()
                .weak(),
            );
        }
        ui.add_space(6.0);
    }

    fn brightness_card(&mut self, ui: &mut egui::Ui) {
        let (supported, status) = match self.state.lock() {
            Ok(state) => (state.has_brightness_support(), state.brightness_status()),
            Err(_) => return,
        };
        card(ui, "Brightness", |ui| {
            if supported {
                labeled_value(ui, "Display brightness", &format!("{}%", self.brightness));
                if ui
                    .add(egui::Slider::new(&mut self.brightness, 1..=100).show_value(false))
                    .changed()
                {
                    if let Ok(mut state) = self.state.lock() {
                        state.set_brightness(self.brightness);
                    }
                    self.mark_written();
                }
            }
            ui.label(egui::RichText::new(status).small().weak());
        });
    }

    fn temperature_card(&mut self, ui: &mut egui::Ui) {
        let snapshot = match self.state.lock() {
            Ok(state) => (
                state.has_color_support(),
                state.color_status(),
                state.manual_color_enabled(),
            ),
            Err(_) => return,
        };
        // Read the live tray flag: the slider hides as soon as a popup exists.
        let tray_popup = self.tray_popup.load(Ordering::Relaxed);
        card(ui, "Colour temperature", |ui| {
            if ui
                .checkbox(&mut self.auto, "Automatic (follow schedule)")
                .changed()
            {
                if let Ok(mut state) = self.state.lock() {
                    state.set_auto_color_temperature(self.auto);
                }
                self.mark_written();
                self.resync_displayed_temp();
            }
            if !show_manual_slider(snapshot.2, tray_popup) {
                if self.auto {
                    ui.label(
                        egui::RichText::new(format!("Scheduled now: {} K", self.temperature))
                            .small()
                            .weak(),
                    );
                } else if tray_popup {
                    ui.label(
                        egui::RichText::new("Adjust from the tray icon menu.")
                            .small()
                            .weak(),
                    );
                }
                ui.label(egui::RichText::new(snapshot.1).small().weak());
                return;
            }
            labeled_value(ui, "Manual temperature", &format!("{} K", self.temperature));
            let slider = egui::Slider::new(&mut self.temperature, 900..=6600)
                .step_by(100.0)
                .show_value(false);
            if ui.add(slider).changed() {
                // Snap to the 100 K step like the macOS slider.
                self.temperature = TemperatureRange::snap(self.temperature);
                if let Ok(mut state) = self.state.lock() {
                    state.set_color_temperature(self.temperature);
                }
                self.mark_written();
            }
            ui.label(egui::RichText::new(snapshot.1).small().weak());
        });
    }

    fn schedule_card(&mut self, ui: &mut egui::Ui) {
        card(ui, "Schedule", |ui| {
            // Adaptive rows: side-by-side pickers when they fit, stacked
            // when the window is narrow, so the right side never clips.
            for (label, from_field, to_field) in [("Morning", 0, 1), ("Evening", 2, 3)] {
                ui.label(egui::RichText::new(label).strong());
                let budget = schedule_content_budget(ui.ctx().content_rect().width());
                if schedule_side_by_side(budget) {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("from").weak());
                            self.time_picker(ui, from_field);
                        });
                        ui.add_space(PICKER_GAP);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("to").weak());
                            self.time_picker(ui, to_field);
                        });
                    });
                } else {
                    for (tag, field) in [("from", from_field), ("to", to_field)] {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(tag).weak());
                            self.time_picker(ui, field);
                        });
                    }
                }
                ui.add_space(2.0);
            }
            ui.add_space(6.0);
            ui.label("Day / night targets");
            if temperature_range_bar(ui, &mut self.range) {
                if let Ok(mut state) = self.state.lock() {
                    state.set_temperature_range(self.range);
                }
                self.mark_written();
            }
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("900 K").small().weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("6600 K").small().weak());
                });
            });
            ui.label(
                egui::RichText::new(self.range.display_text())
                    .small()
                    .weak(),
            );
        });
    }

    fn time_picker(&mut self, ui: &mut egui::Ui, field: usize) {
        let before = self.times[field];
        let time = &mut self.times[field];
        let id = format!("time-{field}");
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = PICKER_ITEM_SPACING;
            egui::ComboBox::from_id_salt(format!("{id}-hour"))
                .selected_text(format!("{}", time.hour))
                .width(HOUR_COMBO_WIDTH)
                .show_ui(ui, |ui| {
                    for hour in 1..=12 {
                        ui.selectable_value(&mut time.hour, hour, format!("{hour}"));
                    }
                });
            egui::ComboBox::from_id_salt(format!("{id}-minute"))
                .selected_text(format!("{:02}", time.minute))
                .width(MINUTE_COMBO_WIDTH)
                .show_ui(ui, |ui| {
                    for minute in 0..=59 {
                        ui.selectable_value(&mut time.minute, minute, format!("{minute:02}"));
                    }
                });
            egui::ComboBox::from_id_salt(format!("{id}-meridiem"))
                .selected_text(time.meridiem.display_name())
                .width(MERIDIEM_COMBO_WIDTH)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut time.meridiem, Meridiem::Am, "AM");
                    ui.selectable_value(&mut time.meridiem, Meridiem::Pm, "PM");
                });
        });
        if *time != before {
            if let Some(parsed) = TimeOfDay::from_twelve_hour(*time) {
                if let Ok(mut state) = self.state.lock() {
                    state.set_schedule_time(SCHEDULE_FIELDS[field], parsed);
                }
                self.mark_written();
            }
        }
    }

    fn startup_card(&mut self, ui: &mut egui::Ui) {
        let status = match self.state.lock() {
            Ok(state) => state.launch_at_login_status(),
            Err(_) => return,
        };
        collapsed_card(ui, "Startup", |ui| {
            if ui
                .checkbox(&mut self.launch_at_login, "Open at login")
                .changed()
            {
                if let Ok(mut state) = self.state.lock() {
                    state.set_launch_at_login(self.launch_at_login);
                    // A refused change is not recorded: snap back to the truth.
                    self.launch_at_login = state.launch_at_login();
                }
                self.mark_written();
            }
            ui.label(egui::RichText::new(status).small().weak());
            if ui.small_button("Open autostart folder").clicked() {
                if let Ok(state) = self.state.lock() {
                    state.open_login_settings();
                }
            }
        });
    }

    fn displays_card(&mut self, ui: &mut egui::Ui) {
        collapsed_card(ui, "Displays", |ui| {
            let displays = match self.state.lock() {
                Ok(state) => state.displays().to_vec(),
                Err(_) => return,
            };
            if displays.is_empty() {
                ui.label(egui::RichText::new("No displays detected.").weak());
            }
            for display in &displays {
                ui.label(egui::RichText::new(&display.name).strong());
                ui.label(
                    egui::RichText::new(display.capability_summary())
                        .small()
                        .weak(),
                );
            }
            ui.add_space(4.0);
            if ui.small_button("Refresh displays").clicked() {
                if let Ok(mut state) = self.state.lock() {
                    state.refresh_displays();
                }
            }
        });
    }

    fn cli_card(&mut self, ui: &mut egui::Ui) {
        collapsed_card(ui, "Command line", |ui| {
            ui.label(
                egui::RichText::new("Terminal equivalents. With no app running, the setters drive hardware directly.")
                    .small()
                    .weak(),
            );
            ui.add_space(4.0);
            for command in CLI_COMMANDS {
                ui.add(
                    egui::Label::new(egui::RichText::new(command.usage).monospace().strong())
                        .selectable(true),
                );
                ui.label(egui::RichText::new(command.desc).small().weak());
                ui.add_space(2.0);
            }
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if ui.small_button("Copy all to clipboard").clicked() {
                    let sheet = CLI_COMMANDS
                        .iter()
                        .map(|c| format!("nightmodecontroller-linux {}  # {}", c.usage, c.desc))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.ctx().copy_text(sheet);
                    self.cli_copied_at = Some(Instant::now());
                }
                if self
                    .cli_copied_at
                    .map(|at| at.elapsed() < Duration::from_secs(2))
                    .unwrap_or(false)
                {
                    ui.label(egui::RichText::new("Copied.").small().weak());
                }
            });
        });
    }

    fn footer(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .small()
                        .weak(),
                );
            });
        });
    }
}

/// Range-bar handle radius. Handles are centred on their value position, so
/// the value mapping must inset both ends by this radius — otherwise the
/// night handle bleeds into the left padding and the day handle clips on the
/// right at maximum.
const RANGE_HANDLE_RADIUS: f32 = 10.0;

/// Handle travel width within a bar of `rect_width`: the bar minus one
/// handle radius at each end.
fn range_bar_span(rect_width: f32) -> f32 {
    (rect_width - 2.0 * RANGE_HANDLE_RADIUS).max(0.0)
}

/// X of a kelvin value inside a bar starting at `rect_left`.
fn range_bar_x(value: i32, rect_left: f32, rect_width: f32) -> f32 {
    let bounds = TemperatureScale::bounds();
    rect_left
        + RANGE_HANDLE_RADIUS
        + TemperatureScale::fraction(value, bounds) as f32 * range_bar_span(rect_width)
}

/// Kelvin value at an X inside a bar starting at `rect_left`.
fn range_bar_value_at(x: f32, rect_left: f32, rect_width: f32) -> i32 {
    let span = range_bar_span(rect_width);
    let fraction = if span > 0.0 {
        ((x - rect_left - RANGE_HANDLE_RADIUS) / span).clamp(0.0, 1.0)
    } else {
        0.0
    } as f64;
    TemperatureScale::value_at_fraction(fraction, TemperatureScale::bounds())
}

/// Two-handle day/night range bar over a warm-to-cool gradient. Returns true
/// when the range changed.
fn temperature_range_bar(ui: &mut egui::Ui, range: &mut TemperatureRange) -> bool {
    let height = 34.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter();
    let track_height = 10.0;
    let track = egui::Rect::from_min_max(
        egui::pos2(
            rect.left() + RANGE_HANDLE_RADIUS,
            rect.center().y - track_height / 2.0,
        ),
        egui::pos2(
            rect.right() - RANGE_HANDLE_RADIUS,
            rect.center().y + track_height / 2.0,
        ),
    );

    // Warm-to-cool gradient track.
    let segments = 64;
    let warm = egui::Color32::from_rgb(235, 150, 60);
    let cool = egui::Color32::from_rgb(130, 180, 235);
    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        let color = warm.lerp_to_gamma(cool, t0);
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(track.left() + t0 * track.width(), track.top()),
                egui::pos2(track.left() + t1 * track.width(), track.bottom()),
            ),
            0.0,
            color,
        );
    }

    let x_of = |value: i32| range_bar_x(value, rect.left(), rect.width());
    let value_at = |x: f32| range_bar_value_at(x, rect.left(), rect.width());
    // Dim the track outside the selected span.
    let dim = ui.visuals().window_fill().gamma_multiply(0.55);
    painter.rect_filled(
        egui::Rect::from_min_max(
            track.left_top(),
            egui::pos2(x_of(range.night()), track.bottom()),
        ),
        0.0,
        dim,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(x_of(range.day()), track.top()),
            track.right_bottom(),
        ),
        0.0,
        dim,
    );

    for value in [range.night(), range.day()] {
        let center = egui::pos2(x_of(value), rect.center().y);
        painter.circle_filled(center, RANGE_HANDLE_RADIUS, egui::Color32::WHITE);
        painter.circle_stroke(
            center,
            RANGE_HANDLE_RADIUS,
            (2.0, ui.visuals().window_stroke.color),
        );
    }
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // Which handle is being dragged, remembered across the gesture so the
    // handles pin instead of swapping mid-drag.
    let drag_id = response.id.with("handle");
    if response.drag_started() {
        if let Some(pos) = response.interact_pointer_pos() {
            let night_x = x_of(range.night());
            let day_x = x_of(range.day());
            let night_handle = (pos.x - night_x).abs() <= (pos.x - day_x).abs();
            ui.data_mut(|data| data.insert_temp(drag_id, night_handle));
        }
    }
    if response.drag_stopped() {
        ui.data_mut(|data| data.remove_temp::<bool>(drag_id));
    }
    if response.dragged() {
        if let (Some(pos), Some(night_handle)) = (
            response.interact_pointer_pos(),
            ui.data(|data| data.get_temp::<bool>(drag_id)),
        ) {
            let value = value_at(pos.x);
            let next = if night_handle {
                range.moving_night(value)
            } else {
                range.moving_day(value)
            };
            if next != *range {
                *range = next;
                return true;
            }
        }
    }
    false
}

/// Wakes the settings event loop from any thread.
///
/// A hidden window schedules no repaints, so tray/IPC/CLI commands would sit
/// in the channel unwitnessed (quit included) until some OS event happened to
/// wake the loop. Every command sender calls [`wake_ui`] after sending.
static UI_CONTEXT: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();

pub fn wake_ui() {
    if let Some(ctx) = UI_CONTEXT.get() {
        ctx.request_repaint();
    }
}

/// Runs the settings window on the calling (main) thread. Returns when the
/// window quits.
pub fn run_settings_window<S: SettingsPersistence + 'static>(
    state: SharedState<S>,
    cmd_rx: mpsc::Receiver<TrayCommand>,
    tray_popup: Arc<AtomicBool>,
    visible: bool,
    shutdown: crate::SharedShutdown,
) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Night Mode Controller")
            .with_app_id("nightmodecontroller-linux")
            .with_inner_size([480.0, 720.0])
            .with_min_inner_size([400.0, 500.0])
            .with_visible(visible),
        ..Default::default()
    };
    eframe::run_native(
        "Night Mode Controller",
        options,
        Box::new(move |cc| {
            // `egui::Context` is a cheap handle: clones share everything and
            // `request_repaint` is thread-safe by design.
            let _ = UI_CONTEXT.set(cc.egui_ctx.clone());
            Ok(Box::new(SettingsApp::new(
                state, cmd_rx, tray_popup, visible, shutdown,
            )))
        }),
    )
    .map_err(|e| format!("settings window failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{parse_color_scheme, show_manual_slider};

    #[test]
    fn color_scheme_maps_to_dark_flag() {
        assert_eq!(parse_color_scheme("'prefer-dark'\n"), Some(true));
        assert_eq!(parse_color_scheme("'prefer-light'\n"), Some(false));
        assert_eq!(parse_color_scheme("'default'\n"), Some(false));
        assert_eq!(parse_color_scheme("'nocturne'\n"), None);
        assert_eq!(parse_color_scheme(""), None);
    }

    #[test]
    fn manual_slider_only_without_tray_popup() {
        // Tray popup offers steppers: settings keeps just the range bar.
        assert!(!show_manual_slider(true, true));
        // Tray-less systems keep the slider as their only manual control.
        assert!(show_manual_slider(true, false));
        // Automatic mode never shows it.
        assert!(!show_manual_slider(false, false));
        assert!(!show_manual_slider(false, true));
    }

    #[test]
    fn schedule_pickers_stack_when_narrow() {
        use super::{
            layout_chrome_width, schedule_content_budget, schedule_side_by_side,
            PICKER_PAIR_MIN_WIDTH, TIME_PICKER_WIDTH,
        };
        // The threshold covers two pickers, the gap, and render headroom.
        assert_eq!(TIME_PICKER_WIDTH, 148.0);
        assert_eq!(PICKER_PAIR_MIN_WIDTH, 340.0);
        assert_eq!(layout_chrome_width(), 64.0);
        // The budget derives from the viewport, never the (inflatable) content.
        assert_eq!(schedule_content_budget(400.0), 336.0);
        assert_eq!(schedule_content_budget(480.0), 416.0);
        // Minimum window: stack, so the right side never clips.
        assert!(!schedule_side_by_side(schedule_content_budget(400.0)));
        // Default window: from/to share one row.
        assert!(schedule_side_by_side(schedule_content_budget(480.0)));
        // Boundary behavior.
        assert!(schedule_side_by_side(PICKER_PAIR_MIN_WIDTH));
        assert!(!schedule_side_by_side(PICKER_PAIR_MIN_WIDTH - 1.0));
    }

    #[test]
    fn settings_scrollbar_reserves_space() {
        // A floating bar would paint over the right margin of wide rows.
        assert!(!super::settings_scroll_style().floating);
    }

    #[test]
    fn solid_scrollbar_gutter_matches_budget() {
        // The layout budget derives this from egui; pin the value so an egui
        // upgrade that changes solid-bar metrics fails here with a pointer to
        // the budget instead of silently clipping content again.
        assert_eq!(egui::style::ScrollStyle::solid().allocated_width(), 10.0);
    }

    #[test]
    fn card_slider_stays_inside_content() {
        // Sliders render at exactly this width (no shrink-to-fit), so it must
        // be the LOCAL card width minus handle room — never an outer-panel
        // measurement, which overruns the nested card and clips on the right.
        assert_eq!(super::card_slider_width(400.0), 392.0);
        assert_eq!(super::card_slider_width(8.0), 0.0);
        assert_eq!(super::card_slider_width(0.0), 0.0);
    }

    #[test]
    fn range_bar_handles_stay_inside_bar() {
        use super::{range_bar_span, range_bar_value_at, range_bar_x};
        use nmc_core::temperature_range::TemperatureScale;
        let (min, max) = TemperatureScale::bounds();
        // Travel stops one handle radius from each edge: handles never
        // overhang the bar, so the day handle cannot clip on the right.
        assert_eq!(range_bar_span(400.0), 380.0);
        assert_eq!(range_bar_x(min, 0.0, 400.0), 10.0);
        assert_eq!(range_bar_x(max, 0.0, 400.0), 390.0);
        // Mapping round-trips through the inset span.
        assert_eq!(range_bar_value_at(10.0, 0.0, 400.0), min);
        assert_eq!(range_bar_value_at(390.0, 0.0, 400.0), max);
        // Degenerate widths collapse instead of dividing by zero.
        assert_eq!(range_bar_span(10.0), 0.0);
        assert_eq!(range_bar_value_at(5.0, 0.0, 10.0), min);
    }

    #[test]
    fn wake_without_context_is_silent() {
        // No window in tests: must be a no-op, never a panic.
        super::wake_ui();
    }
}
