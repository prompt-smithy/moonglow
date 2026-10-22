//! UI state for the tray controls, settings window, and CLI.
//!
//! Ported from `ViewModels/MainViewModel.swift`. This is the single shared
//! instance (wrapped in `Arc<Mutex<..>>` by the app) that keeps every surface
//! in sync — do not create a second one.
//!
//! ## Range rules (preserved from the original)
//! - Brightness: `1...100` percent.
//! - Colour temperature: `900...6600` K, in 100 K steps.

use crate::schedule::ScheduleCalculator;
use crate::settings::AppSettings;
use crate::temperature_range::TemperatureRange;
use crate::time_of_day::TimeOfDay;
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
        Self {
            store,
            display,
            brightness,
            color,
            login,
            settings: loaded,
            displays,
            displayed_temperature,
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

    pub fn has_brightness_support(&self) -> bool {
        self.brightness
            .lock()
            .map(|b| b.is_supported())
            .unwrap_or(false)
            && self.displays.iter().any(|d| d.supports_brightness)
    }

    pub fn has_color_support(&self) -> bool {
        self.color.lock().map(|c| c.is_supported()).unwrap_or(false)
            && self.displays.iter().any(|d| d.supports_color_temperature)
    }

    pub fn brightness_status(&self) -> String {
        if self.has_brightness_support() {
            return "Brightness control is available.".to_string();
        }
        if let Some(detail) = self.brightness.lock().ok().and_then(|b| b.status_detail()) {
            return format!("Brightness control is unavailable. {detail}");
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
        if fresh == self.displays {
            return false;
        }
        self.displays = fresh;
        if let Ok(mut brightness) = self.brightness.lock() {
            brightness.reprobe();
            if brightness.is_supported() {
                brightness.set_brightness(self.settings.manual_brightness);
            }
        }
        true
    }

    // MARK: - Brightness

    pub fn brightness(&self) -> i32 {
        self.settings.manual_brightness
    }

    pub fn set_brightness(&mut self, value: i32) {
        let clamped = AppSettings::clamp_brightness(value);
        if clamped == self.settings.manual_brightness {
            return;
        }
        self.mutate_settings(|s| s.manual_brightness = clamped);
        // The setting persists regardless; hardware only moves with a
        // brightness-capable display attached.
        if self.has_brightness_support() {
            if let Ok(b) = self.brightness.lock() {
                b.set_brightness(clamped);
            }
        }
    }

    // MARK: - Colour temperature

    pub fn color_temperature(&self) -> i32 {
        self.displayed_temperature
    }

    pub fn set_color_temperature(&mut self, value: i32) {
        let clamped = AppSettings::clamp_kelvin(value);
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
        self.mutate_settings(|s| s.auto_color_temperature = value);
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

    pub fn schedule_time(&self, which: ScheduleTime) -> TimeOfDay {
        match which {
            ScheduleTime::MorningStart => self.settings.morning_start,
            ScheduleTime::MorningEnd => self.settings.morning_end,
            ScheduleTime::EveningStart => self.settings.evening_start,
            ScheduleTime::EveningEnd => self.settings.evening_end,
        }
    }

    /// Sets a schedule time. Invalid values are ignored, never persisted.
    pub fn set_schedule_time(&mut self, which: ScheduleTime, value: TimeOfDay) {
        if !value.valid() || self.schedule_time(which) == value {
            return;
        }
        self.mutate_settings(|s| match which {
            ScheduleTime::MorningStart => s.morning_start = value,
            ScheduleTime::MorningEnd => s.morning_end = value,
            ScheduleTime::EveningStart => s.evening_start = value,
            ScheduleTime::EveningEnd => s.evening_end = value,
        });
        if self.settings.auto_color_temperature {
            self.tick_schedule();
        }
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
            return "Could not register Moonglow as a login item on this system."
                .to_string();
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
                self.login_requires_approval = self.login.requires_approval();
                return;
            }
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
        if self.has_brightness_support() {
            let level = self.settings.manual_brightness;
            if let Ok(b) = self.brightness.lock() {
                b.set_brightness(level);
            }
        }
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
        supported: bool,
        applied: Arc<StdMutex<Vec<i32>>>,
        detail: Option<String>,
        reprobes: Arc<StdMutex<usize>>,
    }

    impl BrightnessService for FakeBrightness {
        fn is_supported(&self) -> bool {
            self.supported
        }
        fn set_brightness(&self, percent: i32) {
            self.applied.lock().unwrap().push(percent);
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
                supported: brightness_supported,
                applied: brightness_applied.clone(),
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
        let mut h = default_harness();
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
            "Brightness control is available."
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
                supported: false,
                applied: Arc::new(StdMutex::new(Vec::new())),
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
        assert!(state.brightness_status().contains("permission denied"));
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
        let mut h = default_harness();
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
        assert!(h
            .state
            .launch_at_login_status()
            .contains("Could not register"));
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
            supported: true,
            applied: hw_applied.clone(),
            detail: None,
            reprobes: Arc::new(StdMutex::new(0usize)),
        }));
        let sw: Arc<StdMutex<dyn BrightnessService>> = Arc::new(StdMutex::new(FakeBrightness {
            supported: true,
            applied: sw_applied.clone(),
            detail: None,
            reprobes: Arc::new(StdMutex::new(0usize)),
        }));
        let service = BrightnessFallbackService::new(hw, sw);
        service.set_brightness(40);
        assert!(service.is_supported());
        assert_eq!(*hw_applied.lock().unwrap(), [40]);
        assert!(sw_applied.lock().unwrap().is_empty());
    }

    #[test]
    fn fallback_uses_software_when_hardware_unsupported() {
        use crate::traits::BrightnessFallbackService;
        let hw_applied = Arc::new(StdMutex::new(Vec::new()));
        let sw_applied = Arc::new(StdMutex::new(Vec::new()));
        let hw: Arc<StdMutex<dyn BrightnessService>> = Arc::new(StdMutex::new(FakeBrightness {
            supported: false,
            applied: hw_applied.clone(),
            detail: Some("no reply".to_string()),
            reprobes: Arc::new(StdMutex::new(0usize)),
        }));
        let sw: Arc<StdMutex<dyn BrightnessService>> = Arc::new(StdMutex::new(FakeBrightness {
            supported: true,
            applied: sw_applied.clone(),
            detail: None,
            reprobes: Arc::new(StdMutex::new(0usize)),
        }));
        let service = BrightnessFallbackService::new(hw, sw);
        service.set_brightness(40);
        assert!(service.is_supported());
        assert!(hw_applied.lock().unwrap().is_empty());
        assert_eq!(*sw_applied.lock().unwrap(), [40]);
    }
}
