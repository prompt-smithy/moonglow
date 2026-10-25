//! Settings window (egui).
//!
//! The application's main surface: brightness, colour temperature with its
//! automatic switch and day/night range, global shortcuts, schedule pickers,
//! startup, displays, and the CLI reference. The window is a normal desktop
//! window — it closes to the background so the schedule and shortcuts keep
//! running, and a second launch (or `--show-settings`) brings it back.

use crate::commands::{AppCommand, SharedState};
use crate::shortcuts::SharedShortcutStatus;
use crate::visibility::{VisCommand, VisibilityController};
use moonglow_core::settings::AppSettings;
use moonglow_core::shortcut::{Shortcut, ShortcutAction, ShortcutKey};
use moonglow_core::state::ScheduleTime;
use moonglow_core::temperature_range::{TemperatureRange, TemperatureScale};
use moonglow_core::theme::{parse_hex, ThemeCatalog};
use moonglow_core::time_of_day::TimeOfDay;
use moonglow_core::traits::SettingsPersistence;
use moonglow_core::twelve_hour::{Meridiem, TwelveHourTime};
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

/// The app icon — the crescent mark shared with the macOS and Windows builds.
/// Embedded so the window icon and the header mark work when running from the
/// build directory, where no desktop entry or hicolor theme is installed.
const APP_ICON_PNG: &[u8] =
    include_bytes!("../../../../../resources/linux/hicolor/256x256/apps/moonglow.png");

/// Decodes the embedded icon for the winit viewport and the egui header.
fn app_icon() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(APP_ICON_PNG).ok()
}

pub struct SettingsApp<S: SettingsPersistence> {
    state: SharedState<S>,
    cmd_rx: mpsc::Receiver<AppCommand>,
    shortcut_status: SharedShortcutStatus,
    shutdown: crate::SharedShutdown,
    vis: VisibilityController,
    theme_reported: bool,
    last_system_theme: Option<egui::Theme>,
    gsettings_dark: bool,
    gsettings_checked: Instant,
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
    icon: Option<egui::TextureHandle>,
}

impl<S: SettingsPersistence> SettingsApp<S> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: SharedState<S>,
        cmd_rx: mpsc::Receiver<AppCommand>,
        shortcut_status: SharedShortcutStatus,
        visible: bool,
        shutdown: crate::SharedShutdown,
        icon: Option<egui::TextureHandle>,
    ) -> Self {
        let settings = state
            .lock()
            .map(|s| s.settings().clone())
            .unwrap_or_default();
        let mut app = Self {
            state,
            cmd_rx,
            shortcut_status,
            shutdown,
            vis: VisibilityController::new(visible),
            theme_reported: false,
            last_system_theme: None,
            gsettings_dark: gsettings_dark().unwrap_or(true),
            gsettings_checked: Instant::now(),
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
            icon,
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
        self.shortcuts = shortcuts_of(settings);
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
                AppCommand::ShowWindow => {
                    let commands = self.vis.show();
                    Self::apply_cmds(ctx, commands);
                }
                AppCommand::Quit => {
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

    /// Captures the next key press while a shortcut row is recording.
    fn capture_shortcut(&mut self, ctx: &egui::Context) {
        let Some(index) = self.recording else {
            return;
        };
        let events = ctx.input(|input| input.events.clone());
        for event in events {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            if key == egui::Key::Escape {
                self.recording = None;
                return;
            }
            let Some(shortcut_key) = shortcut_key_from_egui(key) else {
                self.shortcut_message = Some((
                    "That key cannot be used. Try an arrow, letter, digit, or function key."
                        .to_string(),
                    Instant::now(),
                ));
                continue;
            };
            if !(modifiers.ctrl || modifiers.alt || modifiers.shift) {
                self.shortcut_message = Some((
                    "Add Ctrl, Alt, or Shift so the shortcut does not fire while typing."
                        .to_string(),
                    Instant::now(),
                ));
                continue;
            }
            let shortcut = Shortcut::new(
                shortcut_key,
                modifiers.ctrl,
                modifiers.alt,
                modifiers.shift,
                false,
            );
            self.assign_shortcut(index, Some(shortcut));
            self.recording = None;
            return;
        }
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

impl<S: SettingsPersistence + 'static> eframe::App for SettingsApp<S> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_commands(ctx);
        // Follow the desktop appearance. The winit report wins where the
        // backend provides it; otherwise the GNOME fallback below, which is
        // re-polled so live theme flips still apply within seconds.
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
            .map(|theme| theme == egui::Theme::Dark)
            .unwrap_or(self.gsettings_dark);
        configure_style(ctx, dark);
        // Stay alive while hidden so IPC commands still land.
        ctx.request_repaint_after(Duration::from_millis(500));

        let (os_focused, focus_event) = ctx.input(|input| {
            let focus_event = input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::WindowFocused(true)));
            (input.viewport().focused == Some(true), focus_event)
        });
        if (os_focused || focus_event) && !self.vis.visible() {
            // The OS restored or focused the window (dock, taskbar, Alt-Tab):
            // adopt the truth and re-assert a visible, unminimized window.
            let commands = self.vis.on_os_restore_signal();
            Self::apply_cmds(ctx, commands);
        }
        if ctx.input(|input| input.viewport().close_requested()) {
            let commands = self.vis.on_close_requested();
            Self::apply_cmds(ctx, commands);
        }
        if !self.vis.visible() {
            return;
        }

        // Re-sync local controls when settings changed elsewhere (CLI, IPC).
        let settings = self.state.lock().ok().map(|state| state.settings().clone());
        if let Some(settings) = settings {
            if settings != self.last_seen {
                self.sync_from(&settings);
                self.last_seen = settings;
            }
        }
        if self.auto {
            self.resync_displayed_temp();
        }
        self.capture_shortcut(ctx);

        // The footer stays pinned: Quit and Hide must be reachable without
        // scrolling past every card.
        egui::TopBottomPanel::bottom("moonglow-footer")
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin::symmetric(18, 10)),
            )
            .show(ctx, |ui| self.footer(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(ctx.style().visuals.panel_fill))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        let margin = egui::Margin::symmetric(18, 16);
                        egui::Frame::new().inner_margin(margin).show(ui, |ui| {
                            ui.set_max_width(ui.available_width());
                            self.header(ui);
                            self.brightness_card(ui);
                            self.temperature_card(ui);
                            self.shortcuts_card(ui);
                            self.schedule_card(ui);
                            self.startup_card(ui);
                            self.displays_card(ui);
                            self.cli_card(ui);
                        });
                    });
            });
    }
}

impl<S: SettingsPersistence> SettingsApp<S> {
    fn header(&mut self, ui: &mut egui::Ui) {
        let (automatic, temperature, brightness) = match self.state.lock() {
            Ok(state) => (
                state.auto_color_temperature(),
                state.color_temperature(),
                state.brightness(),
            ),
            Err(_) => (false, self.temperature, self.brightness),
        };
        let weak = ui.visuals().weak_text_color();
        ui.horizontal(|ui| {
            if let Some(icon) = &self.icon {
                ui.add(egui::Image::new((icon.id(), egui::vec2(46.0, 46.0))));
            } else {
                paint_mark(ui, 46.0);
            }
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(egui::RichText::new("Moonglow").size(26.0).strong());
                ui.label(
                    egui::RichText::new(format!(
                        "{} · {} K · Brightness {}%",
                        if automatic { "Automatic" } else { "Manual" },
                        temperature,
                        brightness
                    ))
                    .size(12.0)
                    .color(weak),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                pill(
                    ui,
                    if automatic { "Automatic" } else { "Manual" },
                    automatic,
                );
            });
        });
        ui.add_space(14.0);
    }

    fn brightness_card(&mut self, ui: &mut egui::Ui) {
        let (supported, status) = match self.state.lock() {
            Ok(state) => (state.has_brightness_support(), state.brightness_status()),
            Err(_) => return,
        };
        card(ui, |ui| {
            card_title(
                ui,
                "Brightness",
                &format!("{}%", self.brightness),
                supported,
            );
            if value_slider(
                ui,
                &mut self.brightness,
                1..=100,
                1,
                supported,
                SliderLook::Brightness,
            ) {
                if let Ok(mut state) = self.state.lock() {
                    state.set_brightness(self.brightness);
                }
                self.mark_written();
            }
            hint(ui, &status);
        });
    }

    fn temperature_card(&mut self, ui: &mut egui::Ui) {
        let (supported, status) = match self.state.lock() {
            Ok(state) => (state.has_color_support(), state.color_status()),
            Err(_) => return,
        };
        card(ui, |ui| {
            card_title(
                ui,
                "Colour temperature",
                &format!("{} K", self.temperature),
                supported,
            );
            let toggle = ui.checkbox(&mut self.auto, "Automatic (follow schedule)");
            if toggle.changed() {
                if let Ok(mut state) = self.state.lock() {
                    state.set_auto_color_temperature(self.auto);
                }
                self.mark_written();
                self.resync_displayed_temp();
            }
            let slider_enabled = supported && !self.auto;
            if value_slider(
                ui,
                &mut self.temperature,
                900..=6600,
                100,
                slider_enabled,
                SliderLook::Temperature,
            ) {
                // Snap to the 100 K step like the other platform sliders.
                self.temperature = TemperatureRange::snap(self.temperature);
                if let Ok(mut state) = self.state.lock() {
                    state.set_color_temperature(self.temperature);
                }
                self.mark_written();
            }
            if !supported {
                hint(ui, &status);
            } else if self.auto {
                hint(
                    ui,
                    "The schedule is driving the temperature. Switch Automatic off to adjust it by hand.",
                );
            } else {
                hint(ui, &status);
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Day / night range").strong().size(13.0));
            ui.label(
                egui::RichText::new(
                    "Warmest night target at the left, coolest day target at the right.",
                )
                .small()
                .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(2.0);
            if temperature_range_bar(ui, &mut self.range) {
                if let Ok(mut state) = self.state.lock() {
                    state.set_temperature_range(self.range);
                }
                self.mark_written();
            }
            ui.horizontal(|ui| {
                let weak = ui.visuals().weak_text_color();
                ui.label(egui::RichText::new("900 K").small().color(weak));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("6600 K").small().color(weak));
                });
            });
            hint(ui, &self.range.display_text());
        });
    }

    fn shortcuts_card(&mut self, ui: &mut egui::Ui) {
        let status = self
            .shortcut_status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default();
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Shortcuts").strong().size(15.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let color = if status.is_available() {
                        ui.visuals().hyperlink_color
                    } else {
                        ui.visuals().weak_text_color()
                    };
                    ui.label(
                        egui::RichText::new(status.backend.label())
                            .small()
                            .color(color),
                    );
                });
            });
            ui.label(
                egui::RichText::new(
                    "Global keys that work from any application. Click a shortcut, then press the new combination.",
                )
                .small()
                .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(6.0);

            for (index, action) in ShortcutAction::ALL.into_iter().enumerate() {
                self.shortcut_row(ui, index, action);
                if index + 1 < ShortcutAction::ALL.len() {
                    ui.add_space(2.0);
                }
            }

            let message = self.shortcut_message.as_ref().and_then(|(text, at)| {
                (at.elapsed() < Duration::from_secs(MESSAGE_SECONDS)).then_some(text)
            });
            if let Some(message) = message {
                ui.add_space(4.0);
                hint(ui, message);
            }
            hint(ui, &status.detail);
        });
    }

    fn shortcut_row(&mut self, ui: &mut egui::Ui, index: usize, action: ShortcutAction) {
        let recording = self.recording == Some(index);
        let shortcut = self.shortcuts[index];
        let weak = ui.visuals().weak_text_color();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(egui::RichText::new(action.label()).size(13.0));
                ui.label(
                    egui::RichText::new(shortcut_hint(action))
                        .size(11.0)
                        .color(weak),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if shortcut.is_some()
                    && !recording
                    && ui
                        .add(egui::Button::new("Clear").small())
                        .on_hover_text("Remove this shortcut")
                        .clicked()
                {
                    self.assign_shortcut(index, None);
                }
                let label = if recording {
                    "Press keys…".to_string()
                } else {
                    shortcut
                        .map(Shortcut::display_text)
                        .unwrap_or_else(|| "Record".to_string())
                };
                let button = egui::Button::new(egui::RichText::new(label).monospace().size(12.0))
                    .min_size(egui::vec2(104.0, 26.0));
                let button = if recording {
                    button.fill(ui.visuals().hyperlink_color)
                } else {
                    button
                };
                if ui.add(button).clicked() {
                    self.recording = if recording { None } else { Some(index) };
                    self.shortcut_message = None;
                }
            });
        });
    }

    fn schedule_card(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.label(egui::RichText::new("Schedule").strong().size(15.0));
            ui.label(
                egui::RichText::new("When the temperature fades between day and night.")
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(6.0);
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
            ui.label(
                egui::RichText::new(status)
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
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
                egui::RichText::new(
                    "Terminal equivalents. With no app running, the setters drive hardware directly.",
                )
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
                        .map(|command| format!("moonglow {}  # {}", command.usage, command.desc))
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
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Closing this window keeps Moonglow running so the schedule and shortcuts stay active.")
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
                ui.label(
                    egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .small()
                        .weak(),
                );
            });
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui
                        .add(egui::Button::new("Quit Moonglow"))
                        .on_hover_text("Stop the schedule and shortcuts")
                        .clicked()
                    {
                        crate::quit_now(&self.shutdown);
                    }
                    if ui.button("Hide").clicked() {
                        let commands = self.vis.on_close_requested();
                        Self::apply_cmds(ui.ctx(), commands);
                    }
                },
            );
        });
    }
}

// MARK: - Shared chrome

/// Applies the moonlit palette to egui's widgets.
fn configure_style(ctx: &egui::Context, dark: bool) {
    let theme = if dark {
        &ThemeCatalog::DARK
    } else {
        &ThemeCatalog::LIGHT
    };
    let color = |hex: &str| {
        parse_hex(hex)
            .map(|(r, g, b)| egui::Color32::from_rgb(r, g, b))
            .unwrap_or(egui::Color32::GRAY)
    };
    let background = color(theme.window_background);
    let card = color(theme.card_background);
    let control = color(theme.control_background);
    let text = color(theme.primary_text);
    let weak = color(theme.secondary_text);
    let border = color(theme.border);
    let accent = color(theme.accent);
    let hover = color(theme.button_hover);

    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.dark_mode = dark;
    visuals.panel_fill = background;
    visuals.window_fill = card;
    visuals.window_stroke = egui::Stroke::new(1.0_f32, border);
    visuals.window_corner_radius = egui::CornerRadius::same(16);
    visuals.menu_corner_radius = egui::CornerRadius::same(10);
    visuals.extreme_bg_color = control;
    visuals.faint_bg_color = control;
    visuals.code_bg_color = control;
    visuals.override_text_color = Some(text);
    visuals.weak_text_color = Some(weak);
    visuals.hyperlink_color = accent;
    visuals.selection.bg_fill = accent.gamma_multiply(0.32);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, accent);
    visuals.widgets.noninteractive.bg_fill = card;
    visuals.widgets.noninteractive.weak_bg_fill = card;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, border);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, text);
    visuals.widgets.inactive.bg_fill = control;
    visuals.widgets.inactive.weak_bg_fill = control;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, border);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, text);
    visuals.widgets.hovered.bg_fill = hover;
    visuals.widgets.hovered.weak_bg_fill = hover;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, accent);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, text);
    visuals.widgets.active.bg_fill = accent;
    visuals.widgets.active.weak_bg_fill = accent;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, accent);
    // `strong_text_color()` is the active foreground, so it must stay the
    // primary text colour; accent-filled controls set their own label colour.
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, text);
    visuals.widgets.open.bg_fill = hover;
    visuals.widgets.open.weak_bg_fill = hover;
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        style.animation_time = 0.12;
    });
}

/// Paints the two-tone crescent mark used in the header when the embedded icon texture
/// is unavailable: a black moon with a white keyline, matching the shared artwork.
fn paint_mark(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter();
    let radius = size * 0.4688;
    // The crescent's bounding box sits up- and left-of the disc centre; shift it back so the
    // mark looks centred.
    let center = rect.center() + egui::vec2(size * 0.0330, -size * 0.0330);
    let cut = center + egui::vec2(radius * 0.4596, -radius * 0.4596);
    let cut_radius = radius * 1.05;
    // White keyline from the shared artwork: 24/1024 of the width, at least ~1px.
    let outline = (size * 24.0 / 1024.0).max(1.15);
    // Outline crescent first, then the black crescent on top.
    painter.circle_filled(center, radius + outline, egui::Color32::WHITE);
    painter.circle_filled(cut, cut_radius + outline, ui.visuals().window_fill);
    painter.circle_filled(center, radius, egui::Color32::BLACK);
    painter.circle_filled(cut, cut_radius, ui.visuals().window_fill);
}

/// Titled surface with separated content.
fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(egui::Stroke::new(1.0_f32, ui.visuals().window_stroke.color))
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            content(ui);
        });
    ui.add_space(12.0);
}

/// Collapsible secondary card, collapsed at launch.
fn collapsed_card(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(egui::Stroke::new(1.0_f32, ui.visuals().window_stroke.color))
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::CollapsingHeader::new(egui::RichText::new(title).strong().size(13.0))
                .default_open(false)
                .show(ui, content);
        });
    ui.add_space(10.0);
}

/// Card heading with a large right-aligned value.
fn card_title(ui: &mut egui::Ui, title: &str, value: &str, enabled: bool) {
    let color = if enabled {
        ui.visuals().hyperlink_color
    } else {
        ui.visuals().weak_text_color()
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).strong().size(15.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).size(26.0).strong().color(color));
        });
    });
}

/// A short status line under a control.
fn hint(ui: &mut egui::Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(text)
            .small()
            .color(ui.visuals().weak_text_color()),
    );
}

/// Rounded accent-tinted status pill.
fn pill(ui: &mut egui::Ui, text: &str, active: bool) {
    let accent = ui.visuals().hyperlink_color;
    let fill = if active {
        accent.gamma_multiply(0.18)
    } else {
        ui.visuals().widgets.inactive.bg_fill
    };
    let text_color = if active {
        accent
    } else {
        ui.visuals().weak_text_color()
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(99))
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(11.0)
                    .strong()
                    .color(text_color),
            );
        });
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
        desc: "Warm by 250 K (manual mode).",
    },
    CliCommand {
        name: "cooler",
        usage: "--cooler",
        desc: "Cool by 250 K (manual mode).",
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

/// Maps an egui key to a key the global-shortcut backends understand.
fn shortcut_key_from_egui(key: egui::Key) -> Option<ShortcutKey> {
    use egui::Key;
    let name = match key {
        Key::ArrowUp => "Up",
        Key::ArrowDown => "Down",
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::Home => "Home",
        Key::End => "End",
        Key::Insert => "Insert",
        Key::Delete => "Delete",
        Key::Space => "Space",
        Key::Tab => "Tab",
        Key::Enter => "Enter",
        Key::Escape => "Escape",
        Key::Backspace => "Backspace",
        Key::Minus => "Minus",
        Key::Equals => "Equal",
        Key::OpenBracket => "BracketLeft",
        Key::CloseBracket => "BracketRight",
        Key::Semicolon => "Semicolon",
        Key::Quote => "Quote",
        Key::Backslash => "Backslash",
        Key::Comma => "Comma",
        Key::Period => "Period",
        Key::Slash => "Slash",
        Key::Backtick => "Backquote",
        Key::F1 => "F1",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        Key::A => "A",
        Key::B => "B",
        Key::C => "C",
        Key::D => "D",
        Key::E => "E",
        Key::F => "F",
        Key::G => "G",
        Key::H => "H",
        Key::I => "I",
        Key::J => "J",
        Key::K => "K",
        Key::L => "L",
        Key::M => "M",
        Key::N => "N",
        Key::O => "O",
        Key::P => "P",
        Key::Q => "Q",
        Key::R => "R",
        Key::S => "S",
        Key::T => "T",
        Key::U => "U",
        Key::V => "V",
        Key::W => "W",
        Key::X => "X",
        Key::Y => "Y",
        Key::Z => "Z",
        _ => return None,
    };
    ShortcutKey::from_name(name)
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

// MARK: - Sliders

const RANGE_HANDLE_RADIUS: f32 = 10.0;
const HANDLE_HALF_WIDTH: f32 = 8.0;
const HANDLE_APEX_RISE: f32 = 10.5;
const HANDLE_BASE_DROP: f32 = 4.5;
const TRACK_HEIGHT: f32 = 10.0;

/// How a single-handle slider paints its track.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SliderLook {
    Brightness,
    Temperature,
}

/// A warm-to-cool colour for the temperature track.
fn temperature_color(kelvin: i32) -> egui::Color32 {
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
            let t = (kelvin - from_k) as f32 / (to_k - from_k) as f32;
            let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
            return egui::Color32::from_rgb(
                mix(from_rgb[0], to_rgb[0]),
                mix(from_rgb[1], to_rgb[1]),
                mix(from_rgb[2], to_rgb[2]),
            );
        }
    }
    let last = STOPS[3].1;
    egui::Color32::from_rgb(last[0], last[1], last[2])
}

/// Value at an X inside `track`, snapped to `step` and clamped to `lo..=hi`.
fn triangle_slider_value_at(x: f32, track: egui::Rect, lo: i32, hi: i32, step: i32) -> i32 {
    if track.width() <= 0.0 || hi <= lo {
        return lo;
    }
    let fraction = ((x - track.left()) / track.width()).clamp(0.0, 1.0) as f64;
    let raw = lo as f64 + fraction * (hi - lo) as f64;
    let step = step.max(1) as f64;
    (((raw / step).round() * step) as i32).clamp(lo, hi)
}

/// A single-handle slider with the triangle head used across the apps.
fn value_slider(
    ui: &mut egui::Ui,
    value: &mut i32,
    range: std::ops::RangeInclusive<i32>,
    step: i32,
    enabled: bool,
    look: SliderLook,
) -> bool {
    let sense = if enabled {
        egui::Sense::click_and_drag()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), sense);
    let track = egui::Rect::from_min_max(
        egui::pos2(
            rect.left() + RANGE_HANDLE_RADIUS,
            rect.center().y - TRACK_HEIGHT / 2.0,
        ),
        egui::pos2(
            rect.right() - RANGE_HANDLE_RADIUS,
            rect.center().y + TRACK_HEIGHT / 2.0,
        ),
    );
    let (lo, hi) = (*range.start(), *range.end());
    let visuals = ui.visuals().clone();
    let accent = visuals.hyperlink_color;
    let painter = ui.painter();
    let radius = egui::CornerRadius::same((TRACK_HEIGHT / 2.0) as u8);

    if !enabled {
        painter.rect_filled(track, radius, visuals.widgets.inactive.bg_fill);
        let x = value_to_x(*value, lo, hi, track);
        paint_triangle_handle(
            painter,
            egui::pos2(x, rect.center().y),
            visuals.weak_text_color(),
            egui::Stroke::new(1.0_f32, visuals.window_stroke.color),
        );
        return false;
    }

    let mut next = *value;
    if response.clicked() || response.dragged() {
        response.request_focus();
        if let Some(pos) = response.interact_pointer_pos() {
            next = triangle_slider_value_at(pos.x, track, lo, hi, step);
        }
    }
    if response.has_focus() {
        let delta = ui.input(|input| {
            let mut delta = 0;
            if input.key_pressed(egui::Key::ArrowRight) || input.key_pressed(egui::Key::ArrowUp) {
                delta += 1;
            }
            if input.key_pressed(egui::Key::ArrowLeft) || input.key_pressed(egui::Key::ArrowDown) {
                delta -= 1;
            }
            delta
        });
        if delta != 0 {
            next = (next + delta * step).clamp(lo, hi);
        }
    }

    match look {
        SliderLook::Brightness => {
            painter.rect_filled(track, radius, visuals.widgets.inactive.bg_fill);
            let x = value_to_x(next, lo, hi, track);
            let filled = egui::Rect::from_min_max(track.left_top(), egui::pos2(x, track.bottom()));
            painter.rect_filled(filled, radius, accent);
        }
        SliderLook::Temperature => {
            // Approximate the app's own temperature curve so the track reads
            // as warm-to-cool instead of a generic gradient.
            let segments = 72;
            for index in 0..segments {
                let t0 = index as f32 / segments as f32;
                let t1 = (index + 1) as f32 / segments as f32;
                let k0 = lo as f32 + t0 * (hi - lo) as f32;
                let k1 = lo as f32 + t1 * (hi - lo) as f32;
                let color = temperature_color(((k0 + k1) / 2.0) as i32);
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(track.left() + t0 * track.width(), track.top()),
                        egui::pos2(track.left() + t1 * track.width(), track.bottom()),
                    ),
                    0,
                    color,
                );
            }
            let glow = egui::Rect::from_center_size(
                egui::pos2(value_to_x(next, lo, hi, track), track.center().y),
                egui::vec2(22.0, TRACK_HEIGHT + 8.0),
            );
            painter.rect_filled(glow, radius, visuals.window_fill.gamma_multiply(0.35));
        }
    }

    let x = value_to_x(next, lo, hi, track);
    paint_triangle_handle(
        painter,
        egui::pos2(x, rect.center().y),
        egui::Color32::WHITE,
        egui::Stroke::new(2.0_f32, visuals.window_stroke.color),
    );
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if next != *value {
        *value = next;
        true
    } else {
        false
    }
}

fn value_to_x(value: i32, lo: i32, hi: i32, track: egui::Rect) -> f32 {
    let fraction = if hi <= lo {
        0.0
    } else {
        ((value - lo) as f32 / (hi - lo) as f32).clamp(0.0, 1.0)
    };
    track.left() + fraction * track.width()
}

fn paint_triangle_handle(
    painter: &egui::Painter,
    center: egui::Pos2,
    fill: egui::Color32,
    stroke: egui::Stroke,
) {
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(center.x, center.y - HANDLE_APEX_RISE),
            egui::pos2(center.x + HANDLE_HALF_WIDTH, center.y + HANDLE_BASE_DROP),
            egui::pos2(center.x - HANDLE_HALF_WIDTH, center.y + HANDLE_BASE_DROP),
        ],
        fill,
        stroke,
    ));
}

/// Handle travel width within a bar of `rect_width`.
fn range_bar_span(rect_width: f32) -> f32 {
    (rect_width - 2.0 * RANGE_HANDLE_RADIUS).max(0.0)
}

fn range_bar_x(value: i32, rect_left: f32, rect_width: f32) -> f32 {
    let bounds = TemperatureScale::bounds();
    rect_left
        + RANGE_HANDLE_RADIUS
        + TemperatureScale::fraction(value, bounds) as f32 * range_bar_span(rect_width)
}

fn range_bar_value_at(x: f32, rect_left: f32, rect_width: f32) -> i32 {
    let span = range_bar_span(rect_width);
    let fraction = if span > 0.0 {
        ((x - rect_left - RANGE_HANDLE_RADIUS) / span).clamp(0.0, 1.0)
    } else {
        0.0
    } as f64;
    TemperatureScale::value_at_fraction(fraction, TemperatureScale::bounds())
}

/// Two-handle day/night range bar. Returns true when the range changed.
fn temperature_range_bar(ui: &mut egui::Ui, range: &mut TemperatureRange) -> bool {
    let height = 36.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter();
    let track = egui::Rect::from_min_max(
        egui::pos2(
            rect.left() + RANGE_HANDLE_RADIUS,
            rect.center().y - TRACK_HEIGHT / 2.0,
        ),
        egui::pos2(
            rect.right() - RANGE_HANDLE_RADIUS,
            rect.center().y + TRACK_HEIGHT / 2.0,
        ),
    );
    let radius = egui::CornerRadius::same((TRACK_HEIGHT / 2.0) as u8);

    let segments = 72;
    for index in 0..segments {
        let t0 = index as f32 / segments as f32;
        let t1 = (index + 1) as f32 / segments as f32;
        let k0 = 900.0 + t0 * (6600.0 - 900.0);
        let k1 = 900.0 + t1 * (6600.0 - 900.0);
        let color = temperature_color(((k0 + k1) / 2.0) as i32);
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(track.left() + t0 * track.width(), track.top()),
                egui::pos2(track.left() + t1 * track.width(), track.bottom()),
            ),
            0,
            color,
        );
    }

    let x_of = |value: i32| range_bar_x(value, rect.left(), rect.width());
    let value_at = |x: f32| range_bar_value_at(x, rect.left(), rect.width());
    // Dim the track outside the selected span.
    let dim = ui.visuals().window_fill.gamma_multiply(0.60);
    painter.rect_filled(
        egui::Rect::from_min_max(
            track.left_top(),
            egui::pos2(x_of(range.night()), track.bottom()),
        ),
        radius,
        dim,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(x_of(range.day()), track.top()),
            track.right_bottom(),
        ),
        radius,
        dim,
    );

    for value in [range.night(), range.day()] {
        paint_triangle_handle(
            painter,
            egui::pos2(x_of(value), rect.center().y),
            egui::Color32::WHITE,
            egui::Stroke::new(2.0_f32, ui.visuals().window_stroke.color),
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

// MARK: - Layout budget

const HOUR_COMBO_WIDTH: f32 = 42.0;
const MINUTE_COMBO_WIDTH: f32 = 46.0;
const MERIDIEM_COMBO_WIDTH: f32 = 52.0;
const PICKER_ITEM_SPACING: f32 = 4.0;
const PICKER_GAP: f32 = 12.0;
const TIME_PICKER_WIDTH: f32 =
    HOUR_COMBO_WIDTH + MINUTE_COMBO_WIDTH + MERIDIEM_COMBO_WIDTH + 2.0 * PICKER_ITEM_SPACING;
const PICKER_PAIR_FIT_MARGIN: f32 = 40.0;
const PICKER_PAIR_MIN_WIDTH: f32 = 2.0 * TIME_PICKER_WIDTH + PICKER_GAP + PICKER_PAIR_FIT_MARGIN;

/// Horizontal chrome around card content, in points: the outer margin (2×18),
/// the card frame (2×(16 margin + 1 stroke)), and the solid scrollbar gutter.
fn layout_chrome_width() -> f32 {
    36.0 + 34.0 + egui::style::ScrollStyle::solid().allocated_width()
}

fn schedule_content_budget(viewport_width: f32) -> f32 {
    viewport_width - layout_chrome_width()
}

fn schedule_side_by_side(content_width: f32) -> bool {
    content_width >= PICKER_PAIR_MIN_WIDTH
}

// MARK: - Window

/// Wakes the settings event loop from any thread.
///
/// A hidden window schedules no repaints, so IPC commands would sit in the
/// channel unwitnessed until some OS event happened to wake the loop. Every
/// command sender calls [`wake_ui`] after sending.
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
    cmd_rx: mpsc::Receiver<AppCommand>,
    shortcut_status: SharedShortcutStatus,
    visible: bool,
    shutdown: crate::SharedShutdown,
) -> Result<(), String> {
    let icon = app_icon();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Moonglow")
        .with_app_id("moonglow")
        .with_inner_size([560.0, 820.0])
        .with_min_inner_size([420.0, 540.0])
        .with_visible(visible);
    if let Some(icon) = icon.clone() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Moonglow",
        options,
        Box::new(move |cc| {
            // `egui::Context` is a cheap handle: clones share everything and
            // `request_repaint` is thread-safe by design.
            let _ = UI_CONTEXT.set(cc.egui_ctx.clone());
            let texture = icon.map(|icon| {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [icon.width as usize, icon.height as usize],
                    &icon.rgba,
                );
                cc.egui_ctx
                    .load_texture("moonglow-app-icon", image, egui::TextureOptions::LINEAR)
            });
            Ok(Box::new(SettingsApp::new(
                state,
                cmd_rx,
                shortcut_status,
                visible,
                shutdown,
                texture,
            )))
        }),
    )
    .map_err(|error| format!("settings window failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        layout_chrome_width, parse_color_scheme, schedule_content_budget, schedule_side_by_side,
        shortcut_key_from_egui, temperature_color, PICKER_PAIR_MIN_WIDTH, TIME_PICKER_WIDTH,
    };

    #[test]
    fn color_scheme_maps_to_dark_flag() {
        assert_eq!(parse_color_scheme("'prefer-dark'\n"), Some(true));
        assert_eq!(parse_color_scheme("'prefer-light'\n"), Some(false));
        assert_eq!(parse_color_scheme("'default'\n"), Some(false));
        assert_eq!(parse_color_scheme("'nocturne'\n"), None);
        assert_eq!(parse_color_scheme(""), None);
    }

    #[test]
    fn recorder_maps_supported_keys() {
        assert_eq!(
            shortcut_key_from_egui(egui::Key::ArrowUp).map(|key| key.name()),
            Some("Up")
        );
        assert_eq!(
            shortcut_key_from_egui(egui::Key::F9).map(|key| key.name()),
            Some("F9")
        );
        assert_eq!(
            shortcut_key_from_egui(egui::Key::A).map(|key| key.name()),
            Some("A")
        );
        assert_eq!(shortcut_key_from_egui(egui::Key::Copy), None);
    }

    #[test]
    fn schedule_pickers_stack_when_narrow() {
        // The threshold covers two pickers, the gap, and render headroom.
        assert_eq!(TIME_PICKER_WIDTH, 148.0);
        assert_eq!(PICKER_PAIR_MIN_WIDTH, 348.0);
        assert_eq!(layout_chrome_width(), 80.0);
        assert_eq!(schedule_content_budget(420.0), 340.0);
        assert_eq!(schedule_content_budget(560.0), 480.0);
        // Minimum window: stack, so the right side never clips.
        assert!(!schedule_side_by_side(schedule_content_budget(420.0)));
        // Default window: from/to share one row.
        assert!(schedule_side_by_side(schedule_content_budget(560.0)));
    }

    #[test]
    fn temperature_track_moves_from_warm_to_cool() {
        let warm = temperature_color(900);
        let cool = temperature_color(6600);
        assert!(warm.r() > warm.b());
        assert!(cool.b() > cool.r());
    }
}
