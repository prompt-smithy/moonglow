//! UI state for the settings window, global shortcuts, and CLI.
//!
//! Ported from `ViewModels/MainViewModel.swift`. This is the single shared
//! instance (wrapped in `Arc<Mutex<..>>` by the app) that keeps every surface
//! in sync — do not create a second one.
//!
//! ## Range rules (preserved from the original)
//! - Brightness: `1...100` percent.
//! - Colour temperature: `900...6600` K, in 100 K steps.

use crate::schedule::ScheduleCalculator;
use crate::settings::{AppSettings, KELVIN_STEP};
use crate::shortcut::{Shortcut, ShortcutAction};
use crate::temperature_range::TemperatureRange;
use crate::time_of_day::{TimeOfDay, MINUTES_PER_DAY};
use crate::traits::{
    BrightnessService, ColorTemperatureService, DisplayService, LoginItemService,
    SettingsPersistence,
};
use crate::DisplayInfo;
use std::sync::{Arc, Mutex};

/// Which schedule time a setter targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleTime {
    MorningStart,
    MorningEnd,
    EveningStart,
    EveningEnd,
}

/// Which transition a pair setter targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleWindow {
    Morning,
    Evening,
}

/// Single shared UI state. See the module docs.
pub struct StateManager<S: SettingsPersistence> {
    store: S,
    display: Box<dyn DisplayService>,
    brightness: Arc<Mutex<dyn BrightnessService>>,
    color: Arc<Mutex<dyn ColorTemperatureService>>,
    login: Box<dyn LoginItemService>,
    settings: AppSettings,
    displays: Vec<DisplayInfo>,
    /// The temperature currently on screen. In automatic mode this tracks the
    /// schedule rather than the saved manual value, hence separate storage.
    displayed_temperature: i32,
    /// True only after the active desktop accepted the restore-to-100 shortcut.
    brightness_recovery_shortcut_registered: bool,
    shortcut_backend_unavailable: bool,
    hardware_owned: bool,
    recovery_registration_pending: bool,
    recovery_floor_holds_preference: bool,
    last_brightness_route: Option<crate::brightness_plan::RoutingOutput>,
    last_brightness_availability: Option<(bool, bool, bool)>,
    login_registration_failed: bool,
    login_requires_approval: bool,
    on_show_settings: Option<Box<dyn FnMut() + Send>>,
    on_quit: Option<Box<dyn FnMut() + Send>>,
}

impl<S: SettingsPersistence> StateManager<S> {
    pub fn new(
        store: S,
        mut display: Box<dyn DisplayService>,
        brightness: Arc<Mutex<dyn BrightnessService>>,
        color: Arc<Mutex<dyn ColorTemperatureService>>,
        login: Box<dyn LoginItemService>,
    ) -> Self {
        let loaded = store.load();
        let displayed_temperature = if loaded.auto_color_temperature {
            ScheduleCalculator::temperature_now(&loaded)
        } else {
            loaded.manual_color_temp_k
        };
        let displays = display.refresh();
        let recovery_registration_pending = loaded.restore_brightness_shortcut.is_some()
            || loaded.restore_brightness_bound_externally;
        Self {
            store,
            display,
            brightness,
            color,
            login,
            settings: loaded,
            displays,
            displayed_temperature,
            brightness_recovery_shortcut_registered: false,
            shortcut_backend_unavailable: false,
            hardware_owned: false,
            recovery_registration_pending,
            recovery_floor_holds_preference: false,
            last_brightness_route: None,
            last_brightness_availability: None,
            login_registration_failed: false,
            login_requires_approval: false,
            on_show_settings: None,
            on_quit: None,
        }
    }

    /// Raised when the user asks for the settings window.
    pub fn on_show_settings(&mut self, f: impl FnMut() + Send + 'static) {
        self.on_show_settings = Some(Box::new(f));
    }

    /// Raised when the user asks to quit.
    pub fn on_quit(&mut self, f: impl FnMut() + Send + 'static) {
        self.on_quit = Some(Box::new(f));
    }

    /// Normalized settings. Mutate through the setters below so writes persist.
    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    /// Active displays, refreshed on demand.
    pub fn displays(&self) -> &[DisplayInfo] {
        &self.displays
    }

    pub fn has_no_displays(&self) -> bool {
        self.displays.is_empty()
    }

    pub fn all_displays_support_hardware_brightness(&self) -> bool {
        !self.displays.is_empty()
            && self
                .brightness
                .lock()
                .map(|b| b.is_hardware_supported())
                .unwrap_or(false)
            && self
                .displays
                .iter()
                .all(|display| display.supports_hardware_brightness)
    }

    pub fn has_hardware_brightness_support(&self) -> bool {
        self.settings.hardware_brightness_enabled
            && self
                .brightness
                .lock()
                .map(|b| b.is_hardware_supported())
                .unwrap_or(false)
            && self.displays.iter().any(|d| d.supports_hardware_brightness)
    }

    pub fn has_software_brightness_support(&self) -> bool {
        self.brightness
            .lock()
            .map(|b| b.is_software_supported())
            .unwrap_or(false)
            && !self.displays.is_empty()
            && self
                .displays
                .iter()
                .all(|display| display.supports_software_brightness)
    }

    pub fn has_brightness_support(&self) -> bool {
        self.has_hardware_brightness_support() || self.has_software_brightness_support()
    }

    pub fn has_brightness_recovery_shortcut(&self) -> bool {
        (self.settings.restore_brightness_shortcut.is_some()
            && self.brightness_recovery_shortcut_registered)
            || (self.shortcut_backend_unavailable
                && self.settings.restore_brightness_bound_externally)
    }

    pub fn set_shortcut_backend_unavailable(&mut self, unavailable: bool) {
        self.shortcut_backend_unavailable = unavailable;
        self.apply_brightness_safely();
    }

    pub fn acknowledge_external_brightness_recovery(&mut self) {
        if self.shortcut_backend_unavailable && !self.settings.restore_brightness_bound_externally {
            self.mutate_settings(|s| s.restore_brightness_bound_externally = true);
        }
    }

    pub fn software_brightness_minimum(&self) -> i32 {
        if self.has_brightness_recovery_shortcut() {
            1
        } else {
            crate::settings::SOFTWARE_BRIGHTNESS_FLOOR
        }
    }

    pub fn brightness_minimum(&self) -> i32 {
        if !self.hardware_brightness_control_enabled() && self.software_brightness_control_enabled()
        {
            self.software_brightness_minimum()
        } else {
            1
        }
    }

    pub fn brightness_control_enabled(&self) -> bool {
        self.hardware_brightness_control_enabled() || self.software_brightness_control_enabled()
    }

    pub fn hardware_brightness_control_enabled(&self) -> bool {
        self.settings.hardware_brightness_enabled
            && self
                .brightness
                .lock()
                .map(|b| b.is_hardware_supported())
                .unwrap_or(false)
            && self.displays.iter().any(|d| d.supports_hardware_brightness)
    }

    pub fn software_brightness_control_enabled(&self) -> bool {
        self.settings.software_brightness_enabled && self.has_software_brightness_support()
    }

    pub fn has_color_support(&self) -> bool {
        self.color.lock().map(|c| c.is_supported()).unwrap_or(false)
            && self.displays.iter().any(|d| d.supports_color_temperature)
    }

    pub fn brightness_status(&self) -> String {
        let status = self.brightness_status_summary();
        if self.settings.hardware_brightness_enabled {
            if let Some(detail) = self.brightness.lock().ok().and_then(|b| b.status_detail()) {
                return format!("{status} {detail}");
            }
        }
        status
    }

    fn brightness_status_summary(&self) -> String {
        if !self.settings.hardware_brightness_enabled && !self.settings.software_brightness_enabled
        {
            return "Hardware and software brightness are disabled in Settings.".to_string();
        }
        if self.hardware_brightness_control_enabled() {
            if !self.all_displays_support_hardware_brightness() {
                return "Hardware brightness controls capable displays. Use Digital Shade in split controls to dim all displays.".to_string();
            }
            if self.software_brightness_control_enabled() {
                return if self.has_brightness_recovery_shortcut() {
                    "Hardware brightness is available. Software dimming provides extra-low levels."
                        .to_string()
                } else {
                    "Hardware brightness is available. Software dimming is limited to 50% until a restore shortcut is registered.".to_string()
                };
            }
            return "Hardware brightness is available. Software Digital Shade is disabled in Settings.".to_string();
        }
        if self.settings.hardware_brightness_enabled
            && self
                .brightness
                .lock()
                .map(|b| b.is_hardware_supported())
                .unwrap_or(false)
            && !self.all_displays_support_hardware_brightness()
        {
            if self.software_brightness_control_enabled() {
                return if self.has_brightness_recovery_shortcut() {
                    "At least one display lacks hardware brightness. Software dimming is applied across all displays.".to_string()
                } else {
                    "At least one display lacks hardware brightness. Software dimming is limited to 50% until a restore shortcut is registered.".to_string()
                };
            }
            return "All connected displays must support hardware brightness, or software brightness must be enabled.".to_string();
        }
        if self.has_software_brightness_support() {
            if !self.settings.software_brightness_enabled {
                return "Software Digital Shade is disabled in Settings.".to_string();
            }
            return if self.has_brightness_recovery_shortcut() {
                "Software brightness is available. The recovery shortcut restores it to 100%."
                    .to_string()
            } else {
                "Software dimming is limited to 50% until a restore shortcut is registered."
                    .to_string()
            };
        }
        "Brightness control is unavailable for the detected display connection.".to_string()
    }

    pub fn color_status(&self) -> String {
        if self.has_color_support() {
            let name = self
                .color
                .lock()
                .map(|c| c.backend_name())
                .unwrap_or_else(|_| "Gamma control".to_string());
            format!("{name} is available.")
        } else {
            "Color-temperature control is unavailable on this system.".to_string()
        }
    }

    /// Re-enumerates active displays.
    /// Re-reads the display list. Returns true when the set changed, in
    /// which case brightness hardware is re-probed (hotplug, sleep/wake or
    /// a KVM switch can resurrect or remove panels) and the configured
    /// level is re-applied to whatever answered.
    pub fn refresh_displays(&mut self) -> bool {
        let fresh = self.display.refresh();
        let retry_hardware = self
            .brightness
            .lock()
            .map(|b| b.needs_reprobe())
            .unwrap_or(false);
        if fresh == self.displays && !retry_hardware {
            return false;
        }
        self.displays = fresh;
        // The display set changed under us, so anything the colour backend thinks
        // is still on screen is no longer known to be — a panel unplugged and
        // replugged can reuse an id. The next tick writes it again.
        if let Ok(color) = self.color.lock() {
            color.invalidate();
        }
        if let Ok(mut brightness) = self.brightness.lock() {
            brightness.reprobe();
        }
        // Capabilities are read again after probing, so newly discovered hardware
        // is immediately available to controls as well as to the backend.
        self.displays = self.display.refresh();
        self.apply_brightness_safely();
        if !self.settings.auto_color_temperature && self.has_color_support() {
            if let Ok(color) = self.color.lock() {
                color.apply(self.displayed_temperature);
            }
        }
        true
    }

    // MARK: - Brightness

    pub fn release_hardware_if_owned(&mut self) {
        if !self.hardware_owned {
            return;
        }
        if let Ok(brightness) = self.brightness.lock() {
            if brightness.is_hardware_supported() {
                brightness.set_hardware_brightness(100);
            }
        }
        self.hardware_owned = false;
    }

    pub fn brightness(&self) -> i32 {
        if !self.hardware_brightness_control_enabled()
            && self.software_brightness_control_enabled()
            && !self.has_brightness_recovery_shortcut()
        {
            self.settings
                .manual_brightness
                .max(crate::settings::SOFTWARE_BRIGHTNESS_FLOOR)
        } else {
            self.settings.manual_brightness
        }
    }

    pub fn set_brightness(&mut self, value: i32) {
        if self.settings.split_brightness_controls {
            return;
        }
        let clamped = AppSettings::clamp_brightness(value).max(self.brightness_minimum());
        if clamped == self.settings.manual_brightness {
            return;
        }
        self.recovery_registration_pending = false;
        self.recovery_floor_holds_preference = false;
        self.mutate_settings(|s| s.manual_brightness = clamped);
        self.last_brightness_availability = None;
        self.apply_brightness_safely();
    }

    pub fn hardware_brightness(&self) -> i32 {
        AppSettings::clamp_brightness(self.settings.manual_hardware_brightness)
    }

    pub fn set_hardware_brightness(&mut self, value: i32) {
        if !self.hardware_brightness_control_enabled() {
            return;
        }
        let value = AppSettings::clamp_brightness(value);
        if value == self.settings.manual_hardware_brightness {
            return;
        }
        self.mutate_settings(|settings| settings.manual_hardware_brightness = value);
        self.last_brightness_availability = None;
        self.apply_brightness_safely();
    }

    pub fn software_brightness(&self) -> i32 {
        if !self.has_brightness_recovery_shortcut() {
            AppSettings::clamp_brightness(self.settings.manual_software_brightness)
                .max(crate::settings::SOFTWARE_BRIGHTNESS_FLOOR)
        } else {
            AppSettings::clamp_brightness(self.settings.manual_software_brightness)
        }
    }

    pub fn set_software_brightness(&mut self, value: i32) {
        if !self.software_brightness_control_enabled() {
            return;
        }
        let value = AppSettings::clamp_brightness(value).max(self.software_brightness_minimum());
        if value == self.settings.manual_software_brightness {
            return;
        }
        self.mutate_settings(|settings| settings.manual_software_brightness = value);
        self.last_brightness_availability = None;
        self.apply_brightness_safely();
    }

    pub fn set_hardware_brightness_enabled(&mut self, enabled: bool) {
        if enabled == self.settings.hardware_brightness_enabled {
            return;
        }
        if !enabled {
            self.release_hardware_if_owned();
        }
        self.mutate_settings(|settings| settings.hardware_brightness_enabled = enabled);
        self.apply_brightness_safely();
    }

    pub fn set_software_brightness_enabled(&mut self, enabled: bool) {
        if enabled == self.settings.software_brightness_enabled {
            return;
        }
        if !enabled {
            if let Ok(brightness) = self.brightness.lock() {
                brightness.set_software_brightness(100);
            }
        }
        self.mutate_settings(|settings| settings.software_brightness_enabled = enabled);
        self.apply_brightness_safely();
    }

    pub fn set_split_brightness_controls(&mut self, enabled: bool) {
        if enabled == self.settings.split_brightness_controls {
            return;
        }
        if enabled {
            let split = crate::brightness_plan::unified_to_split(
                self.settings.manual_brightness,
                self.hardware_brightness_control_enabled(),
                self.software_brightness_control_enabled()
                    && (!self.hardware_brightness_control_enabled()
                        || self.all_displays_support_hardware_brightness()),
                self.has_brightness_recovery_shortcut(),
            );
            self.mutate_settings(|settings| {
                settings.split_brightness_controls = true;
                settings.manual_hardware_brightness = split.hardware;
                settings.manual_software_brightness = split.software;
            });
        } else {
            let hw = self.hardware_brightness_control_enabled();
            let sw = self.software_brightness_control_enabled();
            let output = crate::brightness_plan::compute_routing(
                self.settings.manual_brightness,
                true,
                self.settings.manual_hardware_brightness,
                self.settings.manual_software_brightness,
                hw,
                sw,
                self.has_brightness_recovery_shortcut(),
            );
            let master = crate::brightness_plan::master_preserving_output(
                output,
                hw,
                sw && (!hw || self.all_displays_support_hardware_brightness()),
                self.has_brightness_recovery_shortcut(),
            );
            self.mutate_settings(|settings| {
                settings.split_brightness_controls = false;
                settings.manual_brightness = master;
            });
        }
        self.apply_brightness_safely();
    }

    /// Brightness points per shortcut press.
    pub const HOTKEY_BRIGHTNESS_STEP: i32 = 5;

    /// One shortcut step up. Does nothing without a brightness-capable display,
    /// matching the macOS hot key.
    pub fn increase_brightness(&mut self) {
        if self.brightness_control_enabled() {
            if self.settings.split_brightness_controls {
                if self.software_brightness_control_enabled() && self.software_brightness() < 100 {
                    self.set_software_brightness(
                        self.software_brightness() + Self::HOTKEY_BRIGHTNESS_STEP,
                    );
                } else if self.hardware_brightness_control_enabled() {
                    self.set_hardware_brightness(
                        self.hardware_brightness() + Self::HOTKEY_BRIGHTNESS_STEP,
                    );
                }
            } else {
                self.set_brightness(self.brightness() + Self::HOTKEY_BRIGHTNESS_STEP);
            }
        }
    }

    /// One shortcut step down.
    pub fn decrease_brightness(&mut self) {
        if self.brightness_control_enabled() {
            if self.settings.split_brightness_controls {
                if self.hardware_brightness_control_enabled() && self.hardware_brightness() > 1 {
                    self.set_hardware_brightness(
                        self.hardware_brightness() - Self::HOTKEY_BRIGHTNESS_STEP,
                    );
                } else if self.software_brightness_control_enabled() {
                    self.set_software_brightness(
                        self.software_brightness() - Self::HOTKEY_BRIGHTNESS_STEP,
                    );
                }
            } else {
                self.set_brightness(self.brightness() - Self::HOTKEY_BRIGHTNESS_STEP);
            }
        }
    }

    /// Restores brightness immediately, even when the normal software control is locked.
    pub fn restore_brightness_to_full(&mut self) {
        self.last_brightness_availability = None;
        self.recovery_registration_pending = false;
        self.recovery_floor_holds_preference = false;
        if let Ok(color) = self.color.lock() {
            color.invalidate();
        }
        self.mutate_settings(|settings| {
            settings.manual_brightness = 100;
            settings.manual_hardware_brightness = 100;
            settings.manual_software_brightness = 100;
        });
        self.apply_brightness_safely();
    }

    /// Called by the platform shortcut worker after a binding is accepted or rejected.
    pub fn set_brightness_recovery_shortcut_registered(&mut self, registered: bool) {
        self.brightness_recovery_shortcut_registered =
            registered && self.settings.restore_brightness_shortcut.is_some();
        self.apply_brightness_safely();
    }

    fn apply_brightness_safely(&mut self) {
        let hardware_usable = self.hardware_brightness_control_enabled();
        let software_usable = self.software_brightness_control_enabled()
            && (self.settings.split_brightness_controls
                || !hardware_usable
                || self.all_displays_support_hardware_brightness());
        let availability = (
            hardware_usable,
            software_usable,
            self.has_brightness_recovery_shortcut(),
        );
        if !self.settings.split_brightness_controls {
            if let (Some(previous), Some(output)) = (
                self.last_brightness_availability,
                self.last_brightness_route,
            ) {
                let recovery_changed = previous.2 != availability.2;
                let keep_preference = recovery_changed
                    && (self.recovery_registration_pending
                        || self.recovery_floor_holds_preference
                        || (previous.2
                            && output.software < crate::settings::SOFTWARE_BRIGHTNESS_FLOOR));
                if recovery_changed
                    && previous.2
                    && output.software < crate::settings::SOFTWARE_BRIGHTNESS_FLOOR
                {
                    self.recovery_floor_holds_preference = true;
                }
                let changed = previous.0 != availability.0
                    || (previous.0
                        && (previous.1 != availability.1
                            || (recovery_changed && !keep_preference)));
                if (previous.0 || previous.1) && (availability.0 || availability.1) && changed {
                    let master = crate::brightness_plan::master_preserving_output(
                        output,
                        availability.0,
                        availability.1,
                        availability.2,
                    );
                    self.mutate_settings(|s| s.manual_brightness = master);
                }
            }
        }
        if availability.2 {
            self.recovery_registration_pending = false;
            self.recovery_floor_holds_preference = false;
        }
        let Ok(brightness) = self.brightness.lock() else {
            return;
        };

        let route = crate::brightness_plan::compute_routing(
            self.settings.manual_brightness,
            self.settings.split_brightness_controls,
            self.settings.manual_hardware_brightness,
            self.settings.manual_software_brightness,
            hardware_usable,
            software_usable,
            self.has_brightness_recovery_shortcut(),
        );

        if let Some(hw) = route.hardware {
            brightness.set_hardware_brightness(hw);
            self.hardware_owned = true;
        } else {
            if self.hardware_owned {
                if brightness.is_hardware_supported() {
                    brightness.set_hardware_brightness(100);
                }
                self.hardware_owned = false;
            }
        }

        brightness.set_software_brightness(route.software);
        self.last_brightness_route = Some(route);
        self.last_brightness_availability = Some(availability);
    }

    // MARK: - Colour temperature

    pub fn color_temperature(&self) -> i32 {
        self.displayed_temperature
    }

    pub fn set_color_temperature(&mut self, value: i32) {
        let clamped = TemperatureRange::snap(value);
        if clamped == self.displayed_temperature {
            return;
        }
        self.displayed_temperature = clamped;
        if self.settings.auto_color_temperature {
            return;
        }
        self.mutate_settings(|s| s.manual_color_temp_k = clamped);
        // The setting persists regardless; hardware only moves with a
        // colour-capable display attached (driving Night Light with no
        // screen would override the user's schedule for no visible effect).
        if self.has_color_support() {
            if let Ok(c) = self.color.lock() {
                c.apply(clamped);
            }
        }
    }

    pub fn auto_color_temperature(&self) -> bool {
        self.settings.auto_color_temperature
    }

    pub fn set_auto_color_temperature(&mut self, value: bool) {
        if value == self.settings.auto_color_temperature {
            return;
        }
        let held_temperature = self.displayed_temperature;
        self.mutate_settings(|s| {
            s.auto_color_temperature = value;
            if !value {
                s.manual_color_temp_k = held_temperature;
            }
        });
        if value {
            self.tick_schedule();
        } else if self.has_color_support() {
            if let Ok(c) = self.color.lock() {
                c.apply(self.displayed_temperature);
            }
        }
    }

    /// The manual slider is only live when automatic mode is off and the hardware supports it.
    pub fn manual_color_enabled(&self) -> bool {
        !self.settings.auto_color_temperature && self.has_color_support()
    }

    /// One shortcut step cooler (higher kelvin).
    ///
    /// A fixed temperature cannot survive the next schedule tick, so a
    /// temperature shortcut leaves automatic mode first. The macOS build asks
    /// before doing that; here the key press itself is the intent, and no
    /// modal can be shown reliably while another window is focused.
    pub fn increase_color_temperature(&mut self) {
        self.step_color_temperature(KELVIN_STEP);
    }

    /// One shortcut step warmer (lower kelvin).
    pub fn decrease_color_temperature(&mut self) {
        self.step_color_temperature(-KELVIN_STEP);
    }

    fn step_color_temperature(&mut self, delta: i32) {
        if !self.has_color_support() {
            return;
        }
        if self.settings.auto_color_temperature {
            self.set_auto_color_temperature(false);
        }
        self.set_color_temperature(self.color_temperature() + delta);
    }

    // MARK: - Schedule targets

    pub fn day_color_temp(&self) -> i32 {
        self.settings.day_color_temp_k
    }

    pub fn set_day_color_temp(&mut self, value: i32) {
        self.set_temperature_target(value, true);
    }

    pub fn night_color_temp(&self) -> i32 {
        self.settings.night_color_temp_k
    }

    pub fn set_night_color_temp(&mut self, value: i32) {
        self.set_temperature_target(value, false);
    }

    /// Applies both schedule targets in one write.
    ///
    /// The two-handle control fires continuously while dragging, so routing the
    /// endpoints through two separate setters would persist settings and
    /// re-evaluate the schedule twice per update.
    pub fn set_temperature_range(&mut self, range: TemperatureRange) {
        if range.night() == self.settings.night_color_temp_k
            && range.day() == self.settings.day_color_temp_k
        {
            return;
        }
        self.mutate_settings(|s| {
            s.night_color_temp_k = range.night();
            s.day_color_temp_k = range.day();
        });
        if self.settings.auto_color_temperature {
            self.tick_schedule();
        }
    }

    // MARK: - Shortcuts

    pub fn shortcut(&self, action: ShortcutAction) -> Option<Shortcut> {
        match action {
            ShortcutAction::RestoreBrightness => self.settings.restore_brightness_shortcut,
            ShortcutAction::IncreaseBrightness => self.settings.increase_brightness_shortcut,
            ShortcutAction::DecreaseBrightness => self.settings.decrease_brightness_shortcut,
            ShortcutAction::IncreaseTemperature => self.settings.increase_temperature_shortcut,
            ShortcutAction::DecreaseTemperature => self.settings.decrease_temperature_shortcut,
            ShortcutAction::ShowSettings => self.settings.show_settings_shortcut,
        }
    }

    /// Assigns or clears one shortcut.
    ///
    /// The same key combination is stolen from other actions so one
    /// press can never fire two things. A modifier-less shortcut is refused;
    /// `None` clears the action.
    pub fn set_shortcut(&mut self, action: ShortcutAction, value: Option<Shortcut>) {
        if let Some(shortcut) = value {
            if !shortcut.is_valid() {
                return;
            }
        }
        if self.shortcut(action) == value {
            return;
        }
        let previous_recovery_shortcut = self.settings.restore_brightness_shortcut;
        self.mutate_settings(|settings| {
            if let Some(value) = value {
                for other in ShortcutAction::ALL {
                    if other != action && shortcut_of(settings, other) == Some(value) {
                        set_shortcut(settings, other, None);
                    }
                }
            }
            set_shortcut(settings, action, value);
        });
        if self.settings.restore_brightness_shortcut != previous_recovery_shortcut {
            self.brightness_recovery_shortcut_registered = false;
            self.apply_brightness_safely();
        }
    }

    pub fn schedule_time(&self, which: ScheduleTime) -> TimeOfDay {
        match which {
            ScheduleTime::MorningStart => self.settings.morning_start,
            ScheduleTime::MorningEnd => self.settings.morning_end,
            ScheduleTime::EveningStart => self.settings.evening_start,
            ScheduleTime::EveningEnd => self.settings.evening_end,
        }
    }

    /// Sets a schedule time. Invalid values are ignored, never persisted.
    ///
    /// The edited time is clamped against its neighbours so the four times always
    /// satisfy `morning_start < morning_end <= evening_start < evening_end`.
    /// Without this an inverted window collapses the day to night plus one
    /// abrupt flash at the boundary. When the neighbours already touch there is
    /// no valid value, so the edit is refused rather than inventing a time.
    pub fn set_schedule_time(&mut self, which: ScheduleTime, value: TimeOfDay) {
        if !value.valid() {
            return;
        }
        let clamped = self.clamp_schedule_time(which, value);
        if self.schedule_time(which) == clamped {
            return;
        }
        self.mutate_settings(|s| match which {
            ScheduleTime::MorningStart => s.morning_start = clamped,
            ScheduleTime::MorningEnd => s.morning_end = clamped,
            ScheduleTime::EveningStart => s.evening_start = clamped,
            ScheduleTime::EveningEnd => s.evening_end = clamped,
        });
        if self.settings.auto_color_temperature {
            self.tick_schedule();
        }
    }

    /// Sets both ends of one transition at once.
    ///
    /// The CLI edits a pair in a single command; applying the ends one at a time
    /// would clamp the first against the window it is about to replace, so a
    /// window that moves earlier could never be shortened.
    pub fn set_schedule_window(&mut self, window: ScheduleWindow, from: TimeOfDay, to: TimeOfDay) {
        if !from.valid() || !to.valid() || to <= from {
            return;
        }

        let mut start = from.minutes_since_midnight();
        let mut end = to.minutes_since_midnight();
        match window {
            // The morning window must finish by the evening start.
            ScheduleWindow::Morning => {
                end = end.min(self.settings.evening_start.minutes_since_midnight());
                start = start.min(end - 1);
            }
            // The evening window must start after the morning end.
            ScheduleWindow::Evening => {
                start = start.max(self.settings.morning_end.minutes_since_midnight());
                end = end.max(start + 1).min(MINUTES_PER_DAY - 1);
            }
        }
        if start >= end {
            return;
        }

        let start = TimeOfDay::from_minutes(start);
        let end = TimeOfDay::from_minutes(end);
        self.mutate_settings(|s| match window {
            ScheduleWindow::Morning => {
                s.morning_start = start;
                s.morning_end = end;
            }
            ScheduleWindow::Evening => {
                s.evening_start = start;
                s.evening_end = end;
            }
        });
        if self.settings.auto_color_temperature {
            self.tick_schedule();
        }
    }

    /// Keeps the schedule ordered by clamping an edit against its neighbours,
    /// leaving the other three times untouched.
    fn clamp_schedule_time(&self, which: ScheduleTime, value: TimeOfDay) -> TimeOfDay {
        let settings = &self.settings;
        let bounds = match which {
            ScheduleTime::MorningStart => (0, settings.morning_end.minutes_since_midnight() - 1),
            ScheduleTime::MorningEnd => (
                settings.morning_start.minutes_since_midnight() + 1,
                settings.evening_start.minutes_since_midnight(),
            ),
            ScheduleTime::EveningStart => (
                settings.morning_end.minutes_since_midnight(),
                settings.evening_end.minutes_since_midnight() - 1,
            ),
            ScheduleTime::EveningEnd => (
                settings.evening_start.minutes_since_midnight() + 1,
                MINUTES_PER_DAY - 1,
            ),
        };

        // No valid value exists (the neighbours already touch): refuse the edit
        // rather than invent a time.
        if bounds.0 > bounds.1 {
            return self.schedule_time(which);
        }

        TimeOfDay::from_minutes(value.minutes_since_midnight().clamp(bounds.0, bounds.1))
    }

    // MARK: - Startup

    /// Whether the app should start at login.
    ///
    /// This is the user's stated intent; the system holds the actual
    /// registration. A change the system refuses is deliberately *not*
    /// recorded, so the toggle snaps back to the real state instead of
    /// claiming a registration that does not exist.
    pub fn launch_at_login(&self) -> bool {
        self.settings.launch_at_login
    }

    pub fn set_launch_at_login(&mut self, value: bool) {
        if value == self.settings.launch_at_login {
            return;
        }
        if !self.login.set_enabled(value) {
            self.login_registration_failed = true;
            return;
        }
        self.login_registration_failed = false;
        self.mutate_settings(|s| s.launch_at_login = value);
        self.login_requires_approval = self.login.requires_approval();
    }

    pub fn login_registration_failed(&self) -> bool {
        self.login_registration_failed
    }

    pub fn login_requires_approval(&self) -> bool {
        self.login_requires_approval
    }

    /// Describes the autostart state for the settings window.
    ///
    /// Read live from the service rather than derived from the setting, because
    /// the two can legitimately disagree — the user can remove the entry
    /// outside the app without it ever finding out.
    pub fn launch_at_login_status(&self) -> String {
        if self.login_registration_failed {
            return "The desktop did not accept this autostart entry. Check the autostart directory permissions, then try again.".to_string();
        }
        // Approval is tested first: a request the user has not approved yet is
        // not running, and saying "opens automatically" would be a lie the user
        // cannot act on. (Linux has no approval flow; the branch stays for parity.)
        if self.login.requires_approval() {
            return "Waiting for approval.".to_string();
        }
        if self.login.is_enabled() {
            return "Moonglow opens automatically when you log in.".to_string();
        }
        if self.settings.launch_at_login {
            return "Could not register Moonglow as a login item on this system.".to_string();
        }
        "Moonglow opens only when you launch it.".to_string()
    }

    /// Re-asserts the stored preference on the system. Called once at launch.
    ///
    /// The registration is not the app's to assume: it lives outside the
    /// settings file, so a preference written on one run may no longer hold on
    /// the next.
    pub fn apply_login_item_state(&mut self, is_first_launch_after_install: bool) {
        if is_first_launch_after_install {
            if self.login.is_enabled() || self.login.requires_approval() {
                let _ = self.login.set_enabled(false);
            }
            if self.settings.launch_at_login {
                self.mutate_settings(|s| s.launch_at_login = false);
            }
            self.login_requires_approval = self.login.requires_approval();
            return;
        }

        if self.settings.launch_at_login {
            if !self.login.set_enabled(true) {
                // Keep the stored intent so the status can say the registration
                // did not take; the toggle would otherwise silently forget what
                // the user asked for.
                self.login_registration_failed = true;
                self.login_requires_approval = self.login.requires_approval();
                return;
            }
            self.login_registration_failed = false;
        } else if self.login.is_enabled() {
            // Registered, but the app never recorded it — almost certainly added
            // by hand before this setting existed. Adopt it rather than
            // reporting a state that contradicts the toggle, and never switch
            // it off behind the user's back.
            self.mutate_settings(|s| s.launch_at_login = true);
        }

        self.login_requires_approval = self.login.requires_approval();
    }

    pub fn open_login_settings(&self) {
        self.login.open_settings();
    }

    // MARK: - Lifecycle

    /// Pushes the persisted values to the hardware. Called once at launch.
    pub fn apply_initial_state(&mut self) {
        self.apply_brightness_safely();
        if !self.settings.auto_color_temperature && self.has_color_support() {
            let kelvin = self.displayed_temperature;
            if let Ok(c) = self.color.lock() {
                c.apply(kelvin);
            }
        }
    }

    /// Evaluates and applies the schedule once. Called by the background thread
    /// and after settings changes. No-op when automatic mode is off or the
    /// colour service is unavailable.
    pub fn tick_schedule(&mut self) -> Option<i32> {
        if !self.settings.auto_color_temperature {
            return None;
        }
        // Gate on displays like the manual setters: with no colour-capable
        // display there is nothing to drive, and applying anyway would take
        // Night Light ownership (GNOME) for no visible effect.
        if !self.has_color_support() {
            return None;
        }
        let kelvin = ScheduleCalculator::temperature_now(&self.settings);
        let applied = self.color.lock().map(|c| c.apply(kelvin)).unwrap_or(false);
        if applied {
            // Applied by the scheduler, so record the displayed value without
            // persisting it as a manual override.
            self.displayed_temperature = kelvin;
            Some(kelvin)
        } else {
            None
        }
    }

    pub fn show_settings(&mut self) {
        if let Some(cb) = self.on_show_settings.as_mut() {
            cb();
        }
    }

    pub fn quit(&mut self) {
        if let Some(cb) = self.on_quit.as_mut() {
            cb();
        }
    }

    pub fn flush_settings(&self) {
        self.store.flush();
    }

    // MARK: - Private

    fn mutate_settings(&mut self, change: impl FnOnce(&mut AppSettings)) {
        let mut copy = self.settings.clone();
        change(&mut copy);
        self.settings = copy.normalized();
        self.store.save(&self.settings);
    }

    fn set_temperature_target(&mut self, value: i32, is_day: bool) {
        let clamped = AppSettings::clamp_kelvin(value);
        let current = if is_day {
            self.settings.day_color_temp_k
        } else {
            self.settings.night_color_temp_k
        };
        if clamped == current {
            return;
        }
        self.mutate_settings(|s| {
            if is_day {
                s.day_color_temp_k = clamped;
            } else {
                s.night_color_temp_k = clamped;
            }
        });
        if self.settings.auto_color_temperature {
            self.tick_schedule();
        }
    }
}

fn shortcut_of(settings: &AppSettings, action: ShortcutAction) -> Option<Shortcut> {
    match action {
        ShortcutAction::RestoreBrightness => settings.restore_brightness_shortcut,
        ShortcutAction::IncreaseBrightness => settings.increase_brightness_shortcut,
        ShortcutAction::DecreaseBrightness => settings.decrease_brightness_shortcut,
        ShortcutAction::IncreaseTemperature => settings.increase_temperature_shortcut,
        ShortcutAction::DecreaseTemperature => settings.decrease_temperature_shortcut,
        ShortcutAction::ShowSettings => settings.show_settings_shortcut,
    }
}

fn set_shortcut(settings: &mut AppSettings, action: ShortcutAction, value: Option<Shortcut>) {
    match action {
        ShortcutAction::RestoreBrightness => settings.restore_brightness_shortcut = value,
        ShortcutAction::IncreaseBrightness => settings.increase_brightness_shortcut = value,
        ShortcutAction::DecreaseBrightness => settings.decrease_brightness_shortcut = value,
        ShortcutAction::IncreaseTemperature => settings.increase_temperature_shortcut = value,
        ShortcutAction::DecreaseTemperature => settings.decrease_temperature_shortcut = value,
        ShortcutAction::ShowSettings => settings.show_settings_shortcut = value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::SettingsPersistence;
    use std::sync::Mutex as StdMutex;

    #[derive(Default)]
    struct MemStore {
        saved: StdMutex<AppSettings>,
    }

    impl SettingsPersistence for MemStore {
        fn load(&self) -> AppSettings {
            self.saved.lock().unwrap().clone()
        }
        fn save(&self, settings: &AppSettings) {
            *self.saved.lock().unwrap() = settings.clone();
        }
    }

    struct FakeDisplay {
        inner: Arc<StdMutex<FakeDisplayInner>>,
    }

    struct FakeDisplayInner {
        displays: Vec<DisplayInfo>,
        refresh_count: usize,
    }

    impl DisplayService for FakeDisplay {
        fn displays(&self) -> Vec<DisplayInfo> {
            self.inner.lock().unwrap().displays.clone()
        }
        fn refresh(&mut self) -> Vec<DisplayInfo> {
            let mut inner = self.inner.lock().unwrap();
            inner.refresh_count += 1;
            inner.displays.clone()
        }
    }

    struct FakeBrightness {
        is_hardware: bool,
        is_software: bool,
        applied_hardware: Arc<StdMutex<Vec<i32>>>,
        applied_software: Arc<StdMutex<Vec<i32>>>,
        detail: Option<String>,
        reprobes: Arc<StdMutex<usize>>,
    }

    impl BrightnessService for FakeBrightness {
        fn is_supported(&self) -> bool {
            self.is_hardware || self.is_software
        }
        fn is_hardware_supported(&self) -> bool {
            self.is_hardware
        }
        fn is_software_supported(&self) -> bool {
            self.is_software
        }
        fn set_brightness(&self, percent: i32) {
            if self.is_hardware {
                self.applied_hardware.lock().unwrap().push(percent);
            }
            if self.is_software {
                self.applied_software.lock().unwrap().push(percent);
            }
        }
        fn set_hardware_brightness(&self, percent: i32) {
            self.applied_hardware.lock().unwrap().push(percent);
        }
        fn set_software_brightness(&self, percent: i32) {
            self.applied_software.lock().unwrap().push(percent);
        }
        fn status_detail(&self) -> Option<String> {
            self.detail.clone()
        }
        fn reprobe(&mut self) {
            *self.reprobes.lock().unwrap() += 1;
        }
    }

    struct FakeColor {
        supported: bool,
        applied: Arc<StdMutex<Vec<i32>>>,
        name: String,
    }

    impl ColorTemperatureService for FakeColor {
        fn backend_name(&self) -> String {
            self.name.clone()
        }
        fn is_supported(&self) -> bool {
            self.supported
        }
        fn apply(&self, kelvin: i32) -> bool {
            self.applied.lock().unwrap().push(kelvin);
            true
        }
        fn reset(&self) {}
    }

    /// Stands in for the desktop autostart entry. `accepts_changes` is the
    /// equivalent of the system refusing a registration.
    struct FakeLogin {
        inner: Arc<StdMutex<FakeLoginInner>>,
        accepts_changes: bool,
    }

    struct FakeLoginInner {
        enabled: bool,
        requires_approval: bool,
        requests: Vec<bool>,
    }

    impl LoginItemService for FakeLogin {
        fn is_enabled(&self) -> bool {
            self.inner.lock().unwrap().enabled
        }
        fn requires_approval(&self) -> bool {
            self.inner.lock().unwrap().requires_approval
        }
        fn set_enabled(&mut self, enabled: bool) -> bool {
            let mut inner = self.inner.lock().unwrap();
            inner.requests.push(enabled);
            if !self.accepts_changes {
                return false;
            }
            inner.enabled = enabled && !inner.requires_approval;
            if !enabled {
                inner.requires_approval = false;
            }
            true
        }
    }

    struct Harness {
        store: Arc<MemStore>,
        display_inner: Arc<StdMutex<FakeDisplayInner>>,
        brightness_applied: Arc<StdMutex<Vec<i32>>>,
        #[allow(dead_code)]
        brightness_software_applied: Arc<StdMutex<Vec<i32>>>,
        brightness_reprobes: Arc<StdMutex<usize>>,
        color_applied: Arc<StdMutex<Vec<i32>>>,
        login_inner: Arc<StdMutex<FakeLoginInner>>,
        state: StateManager<Arc<MemStore>>,
    }

    impl SettingsPersistence for Arc<MemStore> {
        fn load(&self) -> AppSettings {
            (**self).load()
        }
        fn save(&self, settings: &AppSettings) {
            (**self).save(settings)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_harness(
        settings: AppSettings,
        brightness_supported: bool,
        color_supported: bool,
        displays: Option<Vec<DisplayInfo>>,
        login_registers: bool,
        login_requires_approval: bool,
        login_already_enabled: bool,
    ) -> Harness {
        let store = Arc::new(MemStore::default());
        store.save(&settings);
        let display_inner = Arc::new(StdMutex::new(FakeDisplayInner {
            displays: displays.unwrap_or_else(|| {
                vec![DisplayInfo::new("1", "Test display", "1", true, true, true)]
            }),
            refresh_count: 0,
        }));
        let brightness_applied = Arc::new(StdMutex::new(Vec::new()));
        let brightness_software_applied = Arc::new(StdMutex::new(Vec::new()));
        let brightness_reprobes = Arc::new(StdMutex::new(0usize));
        let color_applied = Arc::new(StdMutex::new(Vec::new()));
        let login_inner = Arc::new(StdMutex::new(FakeLoginInner {
            enabled: login_already_enabled,
            requires_approval: login_requires_approval,
            requests: Vec::new(),
        }));
        let state = StateManager::new(
            store.clone(),
            Box::new(FakeDisplay {
                inner: display_inner.clone(),
            }),
            Arc::new(StdMutex::new(FakeBrightness {
                is_hardware: brightness_supported,
                is_software: brightness_supported && color_supported,
                applied_hardware: brightness_applied.clone(),
                applied_software: brightness_software_applied.clone(),
                detail: None,
                reprobes: brightness_reprobes.clone(),
            })),
            Arc::new(StdMutex::new(FakeColor {
                supported: color_supported,
                applied: color_applied.clone(),
                name: "Test gamma control".to_string(),
            })),
            Box::new(FakeLogin {
                inner: login_inner.clone(),
                accepts_changes: login_registers,
            }),
        );
        Harness {
            store,
            display_inner,
            brightness_applied,
            brightness_software_applied,
            brightness_reprobes,
            color_applied,
            login_inner,
            state,
        }
    }

    fn default_harness() -> Harness {
        make_harness(AppSettings::default(), true, true, None, true, false, false)
    }

    #[test]
    fn starts_from_persisted_values() {
        let settings = AppSettings {
            manual_brightness: 55,
            ..Default::default()
        };
        let h = make_harness(settings, true, true, None, true, false, false);
        assert_eq!(h.state.brightness(), 55);
    }

    #[test]
    fn brightness_clamps_to_range() {
        let settings = AppSettings {
            software_brightness_enabled: false,
            ..Default::default()
        };
        let mut h = make_harness(settings, true, true, None, true, false, false);
        h.state.set_brightness(500);
        assert_eq!(h.state.brightness(), 100);
        assert_eq!(*h.brightness_applied.lock().unwrap(), [100]);
        h.state.set_brightness(-20);
        assert_eq!(h.state.brightness(), 1);
        assert_eq!(*h.brightness_applied.lock().unwrap(), [100, 1]);
    }

    #[test]
    fn unchanged_brightness_is_ignored() {
        let settings = AppSettings {
            manual_brightness: 60,
            ..Default::default()
        };
        let mut h = make_harness(settings, true, true, None, true, false, false);
        h.state.set_brightness(60);
        assert!(h.brightness_applied.lock().unwrap().is_empty());
    }

    #[test]
    fn manual_color_temperature_applies() {
        let settings = AppSettings {
            auto_color_temperature: false,
            manual_color_temp_k: 4000,
            ..Default::default()
        };
        let mut h = make_harness(settings, true, true, None, true, false, false);
        h.state.set_color_temperature(9999);
        assert_eq!(h.state.color_temperature(), 6600);
        assert_eq!(h.store.load().manual_color_temp_k, 6600);
        assert_eq!(*h.color_applied.lock().unwrap(), [6600]);
    }

    #[test]
    fn automatic_mode_does_not_persist_manual_value() {
        let settings = AppSettings {
            auto_color_temperature: true,
            ..Default::default()
        };
        let mut h = make_harness(settings, true, true, None, true, false, false);
        h.state.set_color_temperature(3000);
        assert_eq!(h.state.color_temperature(), 3000);
        assert_eq!(h.store.load().manual_color_temp_k, 6500);
        assert!(h.color_applied.lock().unwrap().is_empty());
    }

    #[test]
    fn turning_automatic_off_applies_temperature() {
        let mut h = default_harness();
        let displayed = h.state.color_temperature();
        h.state.set_auto_color_temperature(false);
        assert!(!h.store.load().auto_color_temperature);
        assert_eq!(*h.color_applied.lock().unwrap(), [displayed]);
    }

    #[test]
    fn held_temperature_survives_restart() {
        let settings = AppSettings {
            day_color_temp_k: 3700,
            night_color_temp_k: 3700,
            ..AppSettings::default()
        };
        let mut h = make_harness(settings, true, true, None, true, false, false);
        h.state.set_auto_color_temperature(false);
        let restarted = make_harness(h.store.load(), true, true, None, true, false, false);
        assert_eq!(restarted.state.color_temperature(), 3700);
        assert!(!restarted.state.auto_color_temperature());
    }

    #[test]
    fn hotplug_reapplies_manual_temperature() {
        let settings = AppSettings {
            auto_color_temperature: false,
            manual_color_temp_k: 3400,
            ..AppSettings::default()
        };
        let mut h = make_harness(settings, true, true, None, true, false, false);
        h.display_inner
            .lock()
            .unwrap()
            .displays
            .push(DisplayInfo::new("2", "New display", "2", false, true, true));
        assert!(h.state.refresh_displays());
        assert_eq!(*h.color_applied.lock().unwrap(), [3400]);
    }

    #[test]
    fn unavailable_hardware_is_retried_with_an_unchanged_display_list() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct LateBrightness(Arc<AtomicBool>);
        impl BrightnessService for LateBrightness {
            fn is_supported(&self) -> bool {
                self.0.load(Ordering::Relaxed)
            }
            fn set_brightness(&self, _percent: i32) {}
            fn needs_reprobe(&self) -> bool {
                !self.is_supported()
            }
            fn reprobe(&mut self) {
                self.0.store(true, Ordering::Relaxed);
            }
        }
        let mut h = default_harness();
        h.state.brightness = Arc::new(StdMutex::new(LateBrightness(Arc::new(AtomicBool::new(
            false,
        )))));
        assert!(!h.state.has_brightness_support());
        h.state.refresh_displays();
        assert!(h.state.has_brightness_support());
    }

    #[test]
    fn manual_slider_needs_support() {
        let settings = AppSettings {
            auto_color_temperature: false,
            ..Default::default()
        };
        let without = make_harness(settings.clone(), true, false, None, true, false, false);
        assert!(!without.state.manual_color_enabled());
        let with = make_harness(settings, true, true, None, true, false, false);
        assert!(with.state.manual_color_enabled());
        let automatic = default_harness();
        assert!(!automatic.state.manual_color_enabled());
    }

    #[test]
    fn capability_flags_follow_services() {
        let supported = default_harness();
        assert!(supported.state.has_brightness_support());
        assert!(supported.state.has_color_support());
        assert_eq!(
            supported.state.brightness_status(),
            "Hardware brightness is available. Software dimming is limited to 50% until a restore shortcut is registered."
        );
        assert_eq!(
            supported.state.color_status(),
            "Test gamma control is available."
        );

        let unsupported = make_harness(
            AppSettings::default(),
            false,
            false,
            None,
            true,
            false,
            false,
        );
        assert!(!unsupported.state.has_brightness_support());
        assert!(!unsupported.state.has_color_support());
        assert_eq!(
            unsupported.state.brightness_status(),
            "Brightness control is unavailable for the detected display connection."
        );
        assert_eq!(
            unsupported.state.color_status(),
            "Color-temperature control is unavailable on this system."
        );
    }

    #[test]
    fn brightness_status_carries_hardware_detail() {
        let store = Arc::new(MemStore::default());
        let state = StateManager::new(
            store,
            Box::new(FakeDisplay {
                inner: Arc::new(StdMutex::new(FakeDisplayInner {
                    displays: vec![DisplayInfo::new("1", "Test", "1", true, true, true)],
                    refresh_count: 0,
                })),
            }),
            Arc::new(StdMutex::new(FakeBrightness {
                is_hardware: false,
                is_software: true,
                applied_hardware: Arc::new(StdMutex::new(Vec::new())),
                applied_software: Arc::new(StdMutex::new(Vec::new())),
                detail: Some("permission denied on /dev/i2c-*".to_string()),
                reprobes: Arc::new(StdMutex::new(0usize)),
            })),
            Arc::new(StdMutex::new(FakeColor {
                supported: true,
                applied: Arc::new(StdMutex::new(Vec::new())),
                name: "Test".to_string(),
            })),
            Box::new(FakeLogin {
                inner: Arc::new(StdMutex::new(FakeLoginInner {
                    enabled: false,
                    requires_approval: false,
                    requests: Vec::new(),
                })),
                accepts_changes: true,
            }),
        );
        assert!(state.software_brightness_control_enabled());
        assert!(state.brightness_status().contains("permission denied"));
    }

    #[test]
    fn brightness_needs_service_and_capable_display() {
        // The reported setup: DDC blocked, no software dimming, and the only
        // display is temperature-only. The slider stays insensitive here.
        let night_light_only = vec![DisplayInfo::new(
            "DP-1", "DP-1", "drm-DP-1", true, false, true,
        )];
        let blocked = make_harness(
            AppSettings::default(),
            false,
            true,
            Some(night_light_only),
            true,
            false,
            false,
        );
        assert!(!blocked.state.has_brightness_support());
        assert!(blocked.state.brightness_status().contains("unavailable"));
        // A working service alone is not enough: an internal panel with no
        // brightness channel still reports unsupported, so the UI never
        // offers a dead slider.
        let internal_only = vec![DisplayInfo::new(
            "eDP-1",
            "eDP-1",
            "drm-eDP-1",
            true,
            false,
            false,
        )];
        let internal = make_harness(
            AppSettings::default(),
            true,
            true,
            Some(internal_only),
            true,
            false,
            false,
        );
        assert!(!internal.state.has_brightness_support());
        // DDC answering on an external panel re-enables it.
        let capable = vec![DisplayInfo::new(
            "DP-1", "DP-1", "drm-DP-1", true, true, true,
        )];
        let working = make_harness(
            AppSettings::default(),
            true,
            true,
            Some(capable),
            true,
            false,
            false,
        );
        assert!(working.state.has_brightness_support());
    }

    #[test]
    fn no_supported_displays_means_no_capability() {
        let inert = DisplayInfo::new("9", "Inert", "9", true, false, false);
        let h = make_harness(
            AppSettings::default(),
            true,
            true,
            Some(vec![inert]),
            true,
            false,
            false,
        );
        assert!(!h.state.has_brightness_support());
        assert!(!h.state.has_color_support());
        assert!(!h.state.has_no_displays());
    }

    #[test]
    fn empty_display_list_reports_empty_state() {
        let h = make_harness(
            AppSettings::default(),
            true,
            true,
            Some(vec![]),
            true,
            false,
            false,
        );
        assert!(h.state.has_no_displays());
    }

    #[test]
    fn schedule_tick_needs_a_color_display() {
        let auto = AppSettings {
            auto_color_temperature: true,
            ..Default::default()
        };
        // Backend supported but no displays: the tick must not drive anything.
        // Applying anyway would take Night Light ownership (GNOME) and
        // override the user's schedule for no visible effect.
        let mut bare = make_harness(auto.clone(), true, true, Some(vec![]), true, false, false);
        assert_eq!(bare.state.tick_schedule(), None);
        assert!(bare.color_applied.lock().unwrap().is_empty());
        // Display present but colour-incapable: still nothing.
        let inert = DisplayInfo::new("9", "Inert", "9", true, false, false);
        let mut inert_h = make_harness(
            auto.clone(),
            true,
            true,
            Some(vec![inert]),
            true,
            false,
            false,
        );
        assert_eq!(inert_h.state.tick_schedule(), None);
        assert!(inert_h.color_applied.lock().unwrap().is_empty());
        // Colour-capable display: the tick applies the scheduled value.
        let mut live = make_harness(auto, true, true, None, true, false, false);
        assert!(live.state.tick_schedule().is_some());
        assert_eq!(live.color_applied.lock().unwrap().len(), 1);
    }

    #[test]
    fn setters_persist_but_do_not_drive_without_displays() {
        let manual = AppSettings {
            auto_color_temperature: false,
            ..Default::default()
        };
        let mut h = make_harness(manual, true, true, Some(vec![]), true, false, false);
        // Settings update and persist; no backend sees the values.
        h.state.set_brightness(60);
        assert_eq!(h.state.brightness(), 60);
        assert_eq!(h.store.load().manual_brightness, 60);
        assert!(h.brightness_applied.lock().unwrap().is_empty());
        h.state.set_color_temperature(4000);
        assert_eq!(h.state.color_temperature(), 4000);
        assert_eq!(h.store.load().manual_color_temp_k, 4000);
        assert!(h.color_applied.lock().unwrap().is_empty());
        // Mode switches persist too, without driving anything either way.
        h.state.set_auto_color_temperature(true);
        assert!(h.store.load().auto_color_temperature);
        h.state.set_auto_color_temperature(false);
        assert!(!h.store.load().auto_color_temperature);
        assert!(h.color_applied.lock().unwrap().is_empty());
    }

    #[test]
    fn refreshing_reenumerates() {
        let mut h = default_harness();
        h.state.refresh_displays();
        // Once from init, once from the explicit refresh.
        assert_eq!(h.display_inner.lock().unwrap().refresh_count, 2);
    }

    #[test]
    fn display_change_reprobes_and_reapplies_brightness() {
        let settings = AppSettings {
            software_brightness_enabled: false,
            ..Default::default()
        };
        let mut h = make_harness(settings, true, true, None, true, false, false);
        // Unchanged refresh: no reprobe, no re-apply.
        assert!(!h.state.refresh_displays());
        assert_eq!(*h.brightness_reprobes.lock().unwrap(), 0);
        assert!(h.brightness_applied.lock().unwrap().is_empty());
        // New display set: reprobe once and re-apply the configured level.
        h.display_inner.lock().unwrap().displays =
            vec![DisplayInfo::new("2", "Other", "2", true, true, true)];
        assert!(h.state.refresh_displays());
        assert_eq!(*h.brightness_reprobes.lock().unwrap(), 1);
        assert_eq!(
            *h.brightness_applied.lock().unwrap(),
            [h.state.brightness()]
        );
    }

    #[test]
    fn schedule_targets_clamp() {
        let mut h = default_harness();
        h.state.set_day_color_temp(10_000);
        h.state.set_night_color_temp(0);
        assert_eq!(h.state.day_color_temp(), 6600);
        assert_eq!(h.state.night_color_temp(), 900);
    }

    #[test]
    fn temperature_range_applies_both_targets() {
        let mut h = default_harness();
        h.state
            .set_temperature_range(TemperatureRange::new(1000, 5000));
        assert_eq!(h.state.night_color_temp(), 1000);
        assert_eq!(h.state.day_color_temp(), 5000);
        let reloaded = h.store.load();
        assert_eq!(reloaded.night_color_temp_k, 1000);
        assert_eq!(reloaded.day_color_temp_k, 5000);
    }

    #[test]
    fn unchanged_temperature_range_is_ignored() {
        let mut h = default_harness();
        let range = TemperatureRange::from_settings(h.state.settings());
        h.state.set_temperature_range(range);
        assert!(h.color_applied.lock().unwrap().is_empty());
    }

    #[test]
    fn invalid_schedule_times_are_ignored() {
        let mut h = default_harness();
        let original = h.state.schedule_time(ScheduleTime::MorningStart);
        h.state
            .set_schedule_time(ScheduleTime::MorningStart, TimeOfDay::new(99, 0));
        assert_eq!(h.state.schedule_time(ScheduleTime::MorningStart), original);
    }

    #[test]
    fn valid_schedule_times_are_persisted() {
        let mut h = default_harness();
        h.state
            .set_schedule_time(ScheduleTime::MorningStart, TimeOfDay::new(5, 15));
        assert_eq!(
            h.state.schedule_time(ScheduleTime::MorningStart),
            TimeOfDay::new(5, 15)
        );
        assert_eq!(h.store.load().morning_start, TimeOfDay::new(5, 15));
    }

    #[test]
    fn callbacks_fire() {
        let mut h = default_harness();
        let settings_count = Arc::new(StdMutex::new(0));
        let quit_count = Arc::new(StdMutex::new(0));
        let s = settings_count.clone();
        let q = quit_count.clone();
        h.state.on_show_settings(move || *s.lock().unwrap() += 1);
        h.state.on_quit(move || *q.lock().unwrap() += 1);
        h.state.show_settings();
        h.state.quit();
        assert_eq!(*settings_count.lock().unwrap(), 1);
        assert_eq!(*quit_count.lock().unwrap(), 1);
    }

    #[test]
    fn apply_initial_state_pushes_values() {
        let settings = AppSettings {
            auto_color_temperature: false,
            manual_brightness: 70,
            manual_color_temp_k: 4400,
            software_brightness_enabled: false,
            ..Default::default()
        };
        let mut h = make_harness(settings, true, true, None, true, false, false);
        h.state.apply_initial_state();
        assert_eq!(*h.brightness_applied.lock().unwrap(), [70]);
        assert_eq!(*h.color_applied.lock().unwrap(), [4400]);
    }

    #[test]
    fn launch_at_login_registers_and_persists() {
        let mut h = default_harness();
        h.state.set_launch_at_login(true);
        assert_eq!(h.login_inner.lock().unwrap().requests, [true]);
        assert!(h.login_inner.lock().unwrap().enabled);
        assert!(h.store.load().launch_at_login);
    }

    #[test]
    fn refused_change_is_not_recorded() {
        let mut h = make_harness(
            AppSettings::default(),
            true,
            true,
            None,
            false,
            false,
            false,
        );
        h.state.set_launch_at_login(true);
        assert!(!h.state.launch_at_login());
        assert!(!h.store.load().launch_at_login);
        assert_eq!(h.login_inner.lock().unwrap().requests, [true]);
        assert!(h.state.login_registration_failed());
        assert!(h.state.launch_at_login_status().contains("did not accept"));
    }

    #[test]
    fn stored_preference_refused_at_launch_is_reported() {
        let settings = AppSettings {
            launch_at_login: true,
            ..Default::default()
        };
        let mut h = make_harness(settings, true, true, None, false, false, false);
        h.state.apply_login_item_state(false);
        assert!(h.state.launch_at_login());
        assert!(h.state.login_registration_failed());
        assert!(h.state.launch_at_login_status().contains("did not accept"));
    }

    #[test]
    fn schedule_windows_cannot_be_inverted() {
        // Morning end before its start is pinned to one minute after the start.
        let mut h = default_harness();
        h.state
            .set_schedule_time(ScheduleTime::MorningEnd, TimeOfDay::new(5, 0));
        assert_eq!(
            h.state.schedule_time(ScheduleTime::MorningEnd),
            TimeOfDay::new(6, 1)
        );

        // The morning start cannot move past the morning end.
        let mut h = default_harness();
        h.state
            .set_schedule_time(ScheduleTime::MorningStart, TimeOfDay::new(23, 0));
        assert_eq!(
            h.state.schedule_time(ScheduleTime::MorningStart),
            TimeOfDay::new(6, 59)
        );

        // Evening end before its start is pinned to one minute after the start.
        let mut h = default_harness();
        h.state
            .set_schedule_time(ScheduleTime::EveningEnd, TimeOfDay::new(17, 0));
        assert_eq!(
            h.state.schedule_time(ScheduleTime::EveningEnd),
            TimeOfDay::new(17, 31)
        );

        // The evening start cannot move past the evening end.
        let mut h = default_harness();
        h.state
            .set_schedule_time(ScheduleTime::EveningStart, TimeOfDay::new(23, 0));
        assert_eq!(
            h.state.schedule_time(ScheduleTime::EveningStart),
            TimeOfDay::new(18, 29)
        );
    }

    #[test]
    fn schedule_window_pair_applies_atomically() {
        let mut h = default_harness();
        // Moving the window earlier would clamp a one-at-a-time edit of the start
        // against the old end, so the pair is applied in a single write.
        h.state.set_schedule_window(
            ScheduleWindow::Morning,
            TimeOfDay::new(5, 0),
            TimeOfDay::new(6, 0),
        );
        assert_eq!(
            h.state.schedule_time(ScheduleTime::MorningStart),
            TimeOfDay::new(5, 0)
        );
        assert_eq!(
            h.state.schedule_time(ScheduleTime::MorningEnd),
            TimeOfDay::new(6, 0)
        );

        h.state.set_schedule_window(
            ScheduleWindow::Evening,
            TimeOfDay::new(18, 0),
            TimeOfDay::new(19, 0),
        );
        assert_eq!(
            h.state.schedule_time(ScheduleTime::EveningStart),
            TimeOfDay::new(18, 0)
        );
        assert_eq!(
            h.state.schedule_time(ScheduleTime::EveningEnd),
            TimeOfDay::new(19, 0)
        );
    }

    #[test]
    fn schedule_window_pair_rejects_inverted_pairs() {
        let mut h = default_harness();
        h.state.set_schedule_window(
            ScheduleWindow::Morning,
            TimeOfDay::new(7, 0),
            TimeOfDay::new(6, 0),
        );
        assert_eq!(
            h.state.schedule_time(ScheduleTime::MorningStart),
            TimeOfDay::new(6, 0)
        );
        assert_eq!(
            h.state.schedule_time(ScheduleTime::MorningEnd),
            TimeOfDay::new(7, 0)
        );
    }

    #[test]
    fn touching_transitions_are_allowed() {
        let mut h = default_harness();
        // The evening window may start exactly when the morning one ends.
        h.state
            .set_schedule_time(ScheduleTime::EveningStart, TimeOfDay::new(7, 0));
        assert_eq!(
            h.state.schedule_time(ScheduleTime::EveningStart),
            TimeOfDay::new(7, 0)
        );
    }

    #[test]
    fn schedule_edit_with_no_room_is_refused() {
        let settings = AppSettings {
            morning_start: TimeOfDay::new(6, 0),
            morning_end: TimeOfDay::new(10, 0),
            evening_start: TimeOfDay::new(9, 0),
            evening_end: TimeOfDay::new(8, 0),
            ..Default::default()
        };
        let mut h = make_harness(settings, true, true, None, true, false, false);
        let original = h.state.schedule_time(ScheduleTime::EveningStart);
        h.state
            .set_schedule_time(ScheduleTime::EveningStart, TimeOfDay::new(12, 0));
        assert_eq!(h.state.schedule_time(ScheduleTime::EveningStart), original);
    }

    #[test]
    fn pending_approval_is_reported() {
        let mut h = make_harness(AppSettings::default(), true, true, None, true, true, false);
        h.state.set_launch_at_login(true);
        assert!(h.state.launch_at_login());
        assert!(h.state.login_requires_approval());
        assert!(h
            .state
            .launch_at_login_status()
            .contains("Waiting for approval"));
    }

    #[test]
    fn unchanged_launch_at_login_is_ignored() {
        let mut h = default_harness();
        h.state.set_launch_at_login(false);
        assert!(h.login_inner.lock().unwrap().requests.is_empty());
    }

    #[test]
    fn stored_preference_is_reasserted_at_launch() {
        let settings = AppSettings {
            launch_at_login: true,
            ..Default::default()
        };
        let mut h = make_harness(settings, true, true, None, true, false, false);
        h.state.apply_login_item_state(false);
        assert_eq!(h.login_inner.lock().unwrap().requests, [true]);
    }

    #[test]
    fn nothing_registered_when_preference_off() {
        let mut h = default_harness();
        h.state.apply_login_item_state(false);
        assert!(h.login_inner.lock().unwrap().requests.is_empty());
        assert!(h
            .state
            .launch_at_login_status()
            .contains("only when you launch"));
    }

    #[test]
    fn existing_registration_is_adopted() {
        let mut h = make_harness(AppSettings::default(), true, true, None, true, false, true);
        h.state.apply_login_item_state(false);
        assert!(h.state.launch_at_login());
        assert!(h.store.load().launch_at_login);
        assert!(h.login_inner.lock().unwrap().requests.is_empty());
        assert!(h
            .state
            .launch_at_login_status()
            .contains("opens automatically"));
    }

    #[test]
    fn fallback_prefers_hardware_then_software() {
        use crate::traits::BrightnessFallbackService;
        let hw_applied = Arc::new(StdMutex::new(Vec::new()));
        let sw_applied = Arc::new(StdMutex::new(Vec::new()));
        let hw: Arc<StdMutex<dyn BrightnessService>> = Arc::new(StdMutex::new(FakeBrightness {
            is_hardware: true,
            is_software: false,
            applied_hardware: hw_applied.clone(),
            applied_software: Arc::new(StdMutex::new(Vec::new())),
            detail: None,
            reprobes: Arc::new(StdMutex::new(0usize)),
        }));
        let sw: Arc<StdMutex<dyn BrightnessService>> = Arc::new(StdMutex::new(FakeBrightness {
            is_hardware: false,
            is_software: true,
            applied_hardware: Arc::new(StdMutex::new(Vec::new())),
            applied_software: sw_applied.clone(),
            detail: None,
            reprobes: Arc::new(StdMutex::new(0usize)),
        }));
        let service = BrightnessFallbackService::new(hw, sw);
        service.set_brightness(40);
        assert!(service.is_supported());
        assert_eq!(*hw_applied.lock().unwrap(), [40]);
        // The software reset, not a dim: exactly one mechanism owns brightness.
        assert_eq!(*sw_applied.lock().unwrap(), [100]);
    }

    #[test]
    fn hardware_routing_resets_software_dimming() {
        use crate::traits::BrightnessFallbackService;
        let hw_applied = Arc::new(StdMutex::new(Vec::new()));
        let sw_applied = Arc::new(StdMutex::new(Vec::new()));
        let hw: Arc<StdMutex<dyn BrightnessService>> = Arc::new(StdMutex::new(FakeBrightness {
            is_hardware: true,
            is_software: false,
            applied_hardware: hw_applied.clone(),
            applied_software: Arc::new(StdMutex::new(Vec::new())),
            detail: None,
            reprobes: Arc::new(StdMutex::new(0usize)),
        }));
        let sw: Arc<StdMutex<dyn BrightnessService>> = Arc::new(StdMutex::new(FakeBrightness {
            is_hardware: false,
            is_software: true,
            applied_hardware: Arc::new(StdMutex::new(Vec::new())),
            applied_software: sw_applied.clone(),
            detail: None,
            reprobes: Arc::new(StdMutex::new(0usize)),
        }));
        let service = BrightnessFallbackService::new(hw, sw);
        service.set_brightness(40);
        service.set_brightness(70);
        assert_eq!(*hw_applied.lock().unwrap(), [40, 70]);
        assert_eq!(*sw_applied.lock().unwrap(), [100, 100]);
    }

    #[test]
    fn refreshing_reprobes_hardware_not_software_dimming() {
        use crate::traits::BrightnessFallbackService;
        let hw_reprobes = Arc::new(StdMutex::new(0usize));
        let sw_reprobes = Arc::new(StdMutex::new(0usize));
        let hw: Arc<StdMutex<dyn BrightnessService>> = Arc::new(StdMutex::new(FakeBrightness {
            is_hardware: false,
            is_software: false,
            applied_hardware: Arc::new(StdMutex::new(Vec::new())),
            applied_software: Arc::new(StdMutex::new(Vec::new())),
            detail: None,
            reprobes: hw_reprobes.clone(),
        }));
        let sw: Arc<StdMutex<dyn BrightnessService>> = Arc::new(StdMutex::new(FakeBrightness {
            is_hardware: false,
            is_software: true,
            applied_hardware: Arc::new(StdMutex::new(Vec::new())),
            applied_software: Arc::new(StdMutex::new(Vec::new())),
            detail: None,
            reprobes: sw_reprobes.clone(),
        }));
        let mut service = BrightnessFallbackService::new(hw, sw);
        service.reprobe();
        assert_eq!(*hw_reprobes.lock().unwrap(), 1);
        assert_eq!(*sw_reprobes.lock().unwrap(), 0);
    }

    #[test]
    fn fallback_uses_software_when_hardware_unsupported() {
        use crate::traits::BrightnessFallbackService;
        let hw_applied = Arc::new(StdMutex::new(Vec::new()));
        let sw_applied = Arc::new(StdMutex::new(Vec::new()));
        let hw: Arc<StdMutex<dyn BrightnessService>> = Arc::new(StdMutex::new(FakeBrightness {
            is_hardware: false,
            is_software: false,
            applied_hardware: hw_applied.clone(),
            applied_software: Arc::new(StdMutex::new(Vec::new())),
            detail: Some("no reply".to_string()),
            reprobes: Arc::new(StdMutex::new(0usize)),
        }));
        let sw: Arc<StdMutex<dyn BrightnessService>> = Arc::new(StdMutex::new(FakeBrightness {
            is_hardware: false,
            is_software: true,
            applied_hardware: Arc::new(StdMutex::new(Vec::new())),
            applied_software: sw_applied.clone(),
            detail: None,
            reprobes: Arc::new(StdMutex::new(0usize)),
        }));
        let service = BrightnessFallbackService::new(hw, sw);
        service.set_brightness(40);
        assert!(service.is_supported());
        assert!(hw_applied.lock().unwrap().is_empty());
        assert_eq!(*sw_applied.lock().unwrap(), [40]);
    }

    #[test]
    fn brightness_shortcut_steps_and_clamps() {
        let mut harness =
            make_harness(AppSettings::default(), true, true, None, true, false, false);
        harness.state.increase_brightness();
        assert_eq!(harness.state.brightness(), 85);
        harness.state.decrease_brightness();
        harness.state.decrease_brightness();
        assert_eq!(harness.state.brightness(), 75);

        harness.state.set_brightness(99);
        harness.state.increase_brightness();
        assert_eq!(harness.state.brightness(), 100);
    }

    #[test]
    fn brightness_shortcut_requires_support() {
        let mut harness = make_harness(
            AppSettings::default(),
            false,
            true,
            None,
            true,
            false,
            false,
        );
        harness.state.increase_brightness();
        assert_eq!(harness.state.brightness(), 80);
    }

    #[test]
    fn temperature_shortcut_leaves_automatic_mode() {
        let mut harness =
            make_harness(AppSettings::default(), true, true, None, true, false, false);
        assert!(harness.state.auto_color_temperature());
        harness.state.increase_color_temperature();
        assert!(!harness.state.auto_color_temperature());
        let raised = harness.state.color_temperature();
        harness.state.decrease_color_temperature();
        assert_eq!(harness.state.color_temperature(), raised - KELVIN_STEP);
        // The manual value is what persists once automatic mode is off.
        assert_eq!(
            harness.state.settings().manual_color_temp_k,
            harness.state.color_temperature()
        );
    }

    #[test]
    fn temperature_shortcut_requires_color_support() {
        let mut harness = make_harness(
            AppSettings::default(),
            true,
            false,
            None,
            true,
            false,
            false,
        );
        let before = harness.state.color_temperature();
        harness.state.decrease_color_temperature();
        assert!(harness.state.auto_color_temperature());
        assert_eq!(harness.state.color_temperature(), before);
    }
    #[test]
    fn assigning_a_shortcut_steals_it_from_its_previous_action() {
        let mut harness =
            make_harness(AppSettings::default(), true, true, None, true, false, false);
        let combo: Shortcut = "Ctrl+Shift+F9".parse().unwrap();
        harness
            .state
            .set_shortcut(ShortcutAction::IncreaseBrightness, Some(combo));
        harness
            .state
            .set_shortcut(ShortcutAction::DecreaseTemperature, Some(combo));
        assert_eq!(
            harness.state.shortcut(ShortcutAction::DecreaseTemperature),
            Some(combo)
        );
        assert_eq!(
            harness.state.shortcut(ShortcutAction::IncreaseBrightness),
            None
        );
    }

    #[test]
    fn shortcut_can_be_cleared_and_invalid_values_are_refused() {
        let mut harness =
            make_harness(AppSettings::default(), true, true, None, true, false, false);
        harness
            .state
            .set_shortcut(ShortcutAction::IncreaseBrightness, None);
        assert_eq!(
            harness.state.shortcut(ShortcutAction::IncreaseBrightness),
            None
        );
        // A bare key must never be registered.
        let bare: Shortcut = "Ctrl+Up".parse().unwrap();
        let mut naked = bare;
        naked.ctrl = false;
        harness
            .state
            .set_shortcut(ShortcutAction::DecreaseBrightness, Some(naked));
        assert_eq!(
            harness.state.shortcut(ShortcutAction::DecreaseBrightness),
            ShortcutAction::DecreaseBrightness.default_shortcut()
        );
    }

    #[test]
    fn split_mode_hotkeys_cascade_software_to_hardware_and_back() {
        let settings = AppSettings {
            split_brightness_controls: true,
            manual_hardware_brightness: 1,
            manual_software_brightness: 100,
            ..Default::default()
        };
        let mut harness = make_harness(settings, true, true, None, true, false, false);

        // Dimmer: software steps down from 100 to 95 because hardware is already at 1.
        harness.state.decrease_brightness();
        assert_eq!(harness.state.software_brightness(), 95);
        assert_eq!(harness.state.hardware_brightness(), 1);

        // Brighter: software steps back up to 100.
        harness.state.increase_brightness();
        assert_eq!(harness.state.software_brightness(), 100);
        assert_eq!(harness.state.hardware_brightness(), 1);

        // Brighter again: software is at 100, so hardware cascades up from 1 to 6.
        harness.state.increase_brightness();
        assert_eq!(harness.state.software_brightness(), 100);
        assert_eq!(harness.state.hardware_brightness(), 6);
    }

    #[test]
    fn saved_brightness_survives_late_hardware_probe_without_overwrite() {
        let settings = AppSettings {
            manual_brightness: 40,
            restore_brightness_shortcut: None,
            ..Default::default()
        };
        let mut harness = make_harness(settings, false, true, None, true, false, false);
        harness.state.apply_initial_state();
        assert_eq!(harness.state.settings().manual_brightness, 40);
        assert_eq!(harness.store.load().manual_brightness, 40);
    }

    #[test]
    fn mixed_displays_retain_hardware_and_disclosure_does_not_jump() {
        let displays = vec![
            DisplayInfo::new("1", "DDC", "1", true, true, true).with_brightness_modes(true, true),
            DisplayInfo::new("2", "Internal", "2", false, true, true)
                .with_brightness_modes(false, true),
        ];
        let settings = AppSettings {
            manual_brightness: 55,
            ..Default::default()
        };
        let mut h = make_harness(settings, true, true, Some(displays), true, false, false);
        h.state.apply_initial_state();
        assert!(h.state.hardware_brightness_control_enabled());
        assert_eq!(*h.brightness_applied.lock().unwrap(), [55]);
        assert_eq!(*h.brightness_software_applied.lock().unwrap(), [100]);
        h.state.set_split_brightness_controls(true);
        assert_eq!(h.state.hardware_brightness(), 55);
        assert_eq!(h.state.software_brightness(), 100);
    }

    #[test]
    fn software_floor_retains_saved_preference_across_portal_recreation() {
        let display = DisplayInfo::new("1", "Gamma", "1", true, true, true)
            .with_brightness_modes(false, true);
        let settings = AppSettings {
            manual_brightness: 40,
            restore_brightness_shortcut: Some("Ctrl+Alt+Home".parse().unwrap()),
            ..Default::default()
        };
        let mut h = make_harness(
            settings,
            true,
            true,
            Some(vec![display]),
            true,
            false,
            false,
        );
        h.state.apply_initial_state();
        assert_eq!(h.state.brightness(), 50);
        assert_eq!(h.state.brightness_minimum(), 50);
        assert_eq!(h.store.load().manual_brightness, 40);
        assert!(h.brightness_applied.lock().unwrap().is_empty());
        for registered in [true, false, true, false, true] {
            h.state
                .set_brightness_recovery_shortcut_registered(registered);
            assert_eq!(h.store.load().manual_brightness, 40);
            assert_eq!(
                h.brightness_software_applied.lock().unwrap().last(),
                Some(&if registered { 40 } else { 50 })
            );
        }
    }

    #[test]
    fn recovery_registration_preserves_unified_output_above_the_floor() {
        let mut h = make_harness(
            AppSettings {
                manual_brightness: 25,
                ..Default::default()
            },
            true,
            true,
            None,
            true,
            false,
            false,
        );
        h.state.apply_initial_state();
        let previous = *h
            .brightness_software_applied
            .lock()
            .unwrap()
            .last()
            .unwrap();
        h.state.set_shortcut(
            ShortcutAction::RestoreBrightness,
            Some("Ctrl+Alt+Home".parse().unwrap()),
        );
        h.state.set_brightness_recovery_shortcut_registered(true);
        assert_eq!(h.brightness_applied.lock().unwrap().last(), Some(&1));
        assert!(
            (h.brightness_software_applied
                .lock()
                .unwrap()
                .last()
                .unwrap()
                - previous)
                .abs()
                <= 1
        );
    }

    #[test]
    fn hardware_is_released_once_and_never_written_when_unowned() {
        let mut h = default_harness();
        h.state.apply_initial_state();
        h.state.set_hardware_brightness_enabled(false);
        let writes = h.brightness_applied.lock().unwrap().clone();
        assert_eq!(writes.last(), Some(&100));
        h.state.apply_initial_state();
        h.state.apply_initial_state();
        assert_eq!(*h.brightness_applied.lock().unwrap(), writes);
    }

    #[test]
    fn external_recovery_acknowledgement_is_scoped_to_backendless_sessions() {
        let settings = AppSettings {
            manual_software_brightness: 20,
            split_brightness_controls: true,
            ..Default::default()
        };
        let mut h = make_harness(settings, true, true, None, true, false, false);
        h.state.acknowledge_external_brightness_recovery();
        assert!(!h.state.has_brightness_recovery_shortcut());
        h.state.set_shortcut_backend_unavailable(true);
        assert_eq!(h.state.software_brightness(), 50);
        h.state.acknowledge_external_brightness_recovery();
        assert!(h.state.has_brightness_recovery_shortcut());
        assert!(h.store.load().restore_brightness_bound_externally);
        assert_eq!(h.state.software_brightness(), 20);
        h.state.set_shortcut_backend_unavailable(false);
        assert!(!h.state.has_brightness_recovery_shortcut());
        assert_eq!(h.state.software_brightness(), 50);
    }

    #[test]
    fn deep_unified_preference_survives_delayed_portal_registration_and_recreation() {
        let settings = AppSettings {
            manual_brightness: 20,
            restore_brightness_shortcut: Some("Ctrl+Alt+Home".parse().unwrap()),
            ..Default::default()
        };
        let mut h = make_harness(settings, true, true, None, true, false, false);
        h.state.apply_initial_state();
        for registered in [true, false, true, false, true] {
            h.state
                .set_brightness_recovery_shortcut_registered(registered);
            assert_eq!(h.store.load().manual_brightness, 20);
            let shade = *h
                .brightness_software_applied
                .lock()
                .unwrap()
                .last()
                .unwrap();
            if registered {
                assert_eq!(shade, 39);
            } else {
                assert!(shade >= 50);
            }
        }
    }

    #[test]
    fn explicit_restore_wins_over_a_concurrent_capability_change() {
        let mut h = make_harness(
            AppSettings {
                manual_brightness: 25,
                ..Default::default()
            },
            true,
            true,
            None,
            true,
            false,
            false,
        );
        h.state.apply_initial_state();
        h.state.displays[0].supports_hardware_brightness = false;
        h.state.restore_brightness_to_full();
        assert_eq!(h.state.settings().manual_brightness, 100);
        assert_eq!(
            h.brightness_software_applied.lock().unwrap().last(),
            Some(&100)
        );
    }
}
