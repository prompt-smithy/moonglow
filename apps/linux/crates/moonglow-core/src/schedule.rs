//! Pure schedule evaluation and the fixed-interval runner.
//!
//! Ported from `Services/ScheduleService.swift`.

use crate::settings::{AppSettings, KELVIN_STEP};
use crate::time_of_day::TimeOfDay;
use crate::traits::ColorTemperatureService;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The original evaluates every 10 seconds; kept identical because the cadence
/// is user-visible during transitions.
pub const TICK_INTERVAL_SECS: u64 = 10;

/// Pure schedule evaluation.
///
/// The five states are, in order:
///
/// 1. Night target before the morning transition.
/// 2. Linear warming-to-cooling interpolation during the morning transition.
/// 3. Day target between the transitions.
/// 4. Linear cooling-to-warming interpolation during the evening transition.
/// 5. Night target after the evening transition.
///
/// Results are rounded to [`KELVIN_STEP`].
pub struct ScheduleCalculator;

impl ScheduleCalculator {
    /// Computes the target temperature for a wall-clock time of day.
    pub fn temperature_for(settings: &AppSettings, time: TimeOfDay) -> i32 {
        if time < settings.morning_start {
            return settings.night_color_temp_k;
        }
        if time < settings.morning_end {
            return Self::interpolate(
                time,
                settings.morning_start,
                settings.morning_end,
                settings.night_color_temp_k,
                settings.day_color_temp_k,
            );
        }
        if time < settings.evening_start {
            return settings.day_color_temp_k;
        }
        if time < settings.evening_end {
            return Self::interpolate(
                time,
                settings.evening_start,
                settings.evening_end,
                settings.day_color_temp_k,
                settings.night_color_temp_k,
            );
        }
        settings.night_color_temp_k
    }

    /// Convenience overload for the live path, using the local clock.
    pub fn temperature_now(settings: &AppSettings) -> i32 {
        Self::temperature_for(settings, now_local())
    }

    fn interpolate(
        time: TimeOfDay,
        start: TimeOfDay,
        end: TimeOfDay,
        from_kelvin: i32,
        to_kelvin: i32,
    ) -> i32 {
        let span = (end.minutes_since_midnight() - start.minutes_since_midnight()) as f64;
        if span <= 0.0 {
            return Self::snap(to_kelvin as f64);
        }
        let elapsed = (time.minutes_since_midnight() - start.minutes_since_midnight()) as f64;
        let progress = (elapsed / span).clamp(0.0, 1.0);
        Self::snap(from_kelvin as f64 + (to_kelvin - from_kelvin) as f64 * progress)
    }

    fn snap(kelvin: f64) -> i32 {
        ((kelvin / KELVIN_STEP as f64).round() as i32) * KELVIN_STEP
    }
}

/// Current local wall-clock time, minute precision.
pub fn now_local() -> TimeOfDay {
    let now = chrono::Local::now();
    use chrono::Timelike;
    TimeOfDay::new(now.hour() as i32, now.minute() as i32)
}

/// Drives the schedule on a fixed interval and applies the result.
///
/// The runner owns a background thread. [`ScheduleService::tick`] holds the
/// single-evaluation logic and is also called directly by the state manager
/// after settings changes.
pub struct ScheduleService {
    color: Arc<Mutex<dyn ColorTemperatureService>>,
    settings_provider: Box<dyn Fn() -> AppSettings + Send>,
    on_temperature_applied: Option<Box<dyn FnMut(i32) + Send>>,
}

impl ScheduleService {
    pub fn new(
        color: Arc<Mutex<dyn ColorTemperatureService>>,
        settings_provider: impl Fn() -> AppSettings + Send + 'static,
    ) -> Self {
        Self {
            color,
            settings_provider: Box::new(settings_provider),
            on_temperature_applied: None,
        }
    }

    /// Invoked after a temperature is successfully applied.
    pub fn on_temperature_applied(&mut self, f: impl FnMut(i32) + Send + 'static) {
        self.on_temperature_applied = Some(Box::new(f));
    }

    /// Evaluates and applies the schedule once. No-op when automatic mode is
    /// off or the colour service is unavailable.
    pub fn tick(&mut self) {
        let settings = (self.settings_provider)();
        if !settings.auto_color_temperature {
            return;
        }
        let kelvin = ScheduleCalculator::temperature_now(&settings);
        let applied = self
            .color
            .lock()
            .map(|c| c.is_supported() && c.apply(kelvin))
            .unwrap_or(false);
        if applied {
            if let Some(cb) = self.on_temperature_applied.as_mut() {
                cb(kelvin);
            }
        }
    }

    /// Pure tick used by the background thread: computes and applies without
    /// touching the callback (the callback is only safe on the owner thread).
    fn tick_headless(
        color: &Arc<Mutex<dyn ColorTemperatureService>>,
        settings_provider: &(dyn Fn() -> AppSettings + Send + Sync),
    ) -> Option<i32> {
        let settings = settings_provider();
        if !settings.auto_color_temperature {
            return None;
        }
        let kelvin = ScheduleCalculator::temperature_now(&settings);
        color.lock().ok().and_then(|c| {
            if c.is_supported() && c.apply(kelvin) {
                Some(kelvin)
            } else {
                None
            }
        })
    }
}

impl ScheduleService {
    /// Starts the schedule. Ticks immediately, then every 10 seconds.
    ///
    /// The background thread shares the colour service and settings provider;
    /// applied temperatures are reported through `report`. The provider must be
    /// `Send + Sync` for the thread; wrap shared state in a mutex.
    pub fn start_threaded(
        color: Arc<Mutex<dyn ColorTemperatureService>>,
        settings_provider: Arc<dyn Fn() -> AppSettings + Send + Sync>,
        report: Arc<Mutex<dyn FnMut(i32) + Send>>,
    ) -> ScheduleHandle {
        if let Some(kelvin) = Self::tick_headless(&color, &*settings_provider) {
            if let Ok(mut r) = report.lock() {
                r(kelvin);
            }
        }
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        let handle = std::thread::Builder::new()
            .name("moonglow-schedule".to_string())
            .spawn(move || loop {
                if stop_rx
                    .recv_timeout(Duration::from_secs(TICK_INTERVAL_SECS))
                    .is_ok()
                {
                    break;
                }
                if let Some(kelvin) = Self::tick_headless(&color, &*settings_provider) {
                    if let Ok(mut r) = report.lock() {
                        r(kelvin);
                    }
                }
            })
            .expect("schedule thread spawns");
        ScheduleHandle {
            stop_tx: Some(stop_tx),
            handle: Some(handle),
        }
    }
}

/// Handle to a running schedule thread. Stops the thread on drop.
pub struct ScheduleHandle {
    stop_tx: Option<std::sync::mpsc::Sender<()>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl ScheduleHandle {
    pub fn stop(mut self) {
        let _ = self.stop_tx.take().map(|tx| tx.send(()));
        let _ = self.handle.take().map(|h| h.join());
    }
}

impl Drop for ScheduleHandle {
    fn drop(&mut self) {
        let _ = self.stop_tx.take().map(|tx| tx.send(()));
        // Best effort: never block a drop on the join.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    fn at(hour: i32, minute: i32) -> TimeOfDay {
        TimeOfDay::new(hour, minute)
    }

    #[test]
    fn follows_five_daily_states() {
        // Defaults: morning 06:00-07:00, evening 17:30-18:30, day 6500 K, night 1200 K.
        for (h, m, expected) in [
            (5, 59, 1200),
            (6, 0, 1200),
            (6, 30, 3900),
            (7, 0, 6500),
            (12, 0, 6500),
            (17, 29, 6500),
            (17, 30, 6500),
            (18, 0, 3900),
            (18, 30, 1200),
            (23, 59, 1200),
            (0, 30, 1200),
        ] {
            assert_eq!(
                ScheduleCalculator::temperature_for(&AppSettings::default(), at(h, m)),
                expected,
                "at {h:02}:{m:02}"
            );
        }
    }

    #[test]
    fn results_snap_to_step() {
        let result = ScheduleCalculator::temperature_for(&AppSettings::default(), at(6, 1));
        assert_eq!(result % KELVIN_STEP, 0);
    }

    #[test]
    fn custom_targets_are_honoured() {
        let mut settings = AppSettings::default();
        settings.day_color_temp_k = 5000;
        settings.night_color_temp_k = 2000;
        settings.morning_start = at(8, 0);
        settings.morning_end = at(9, 0);
        settings.evening_start = at(20, 0);
        settings.evening_end = at(21, 0);
        assert_eq!(
            ScheduleCalculator::temperature_for(&settings, at(7, 59)),
            2000
        );
        assert_eq!(
            ScheduleCalculator::temperature_for(&settings, at(8, 30)),
            3500
        );
        assert_eq!(
            ScheduleCalculator::temperature_for(&settings, at(12, 0)),
            5000
        );
        assert_eq!(
            ScheduleCalculator::temperature_for(&settings, at(20, 30)),
            3500
        );
        assert_eq!(
            ScheduleCalculator::temperature_for(&settings, at(21, 0)),
            2000
        );
    }

    #[test]
    fn inverted_span_falls_back_to_end_target() {
        let mut settings = AppSettings::default();
        settings.morning_start = at(7, 0);
        settings.morning_end = at(6, 0);
        // 06:30 < 07:00 start, so it is the night target.
        assert_eq!(
            ScheduleCalculator::temperature_for(&settings, at(6, 30)),
            settings.night_color_temp_k
        );
    }

    struct StubColor {
        supported: bool,
        applied: Arc<StdMutex<Vec<i32>>>,
    }

    impl ColorTemperatureService for StubColor {
        fn is_supported(&self) -> bool {
            self.supported
        }
        fn apply(&self, kelvin: i32) -> bool {
            if !self.supported {
                return false;
            }
            self.applied.lock().unwrap().push(kelvin);
            true
        }
        fn reset(&self) {}
    }

    #[test]
    fn scheduler_refuses_without_supported_colour_service() {
        let applied = Arc::new(StdMutex::new(Vec::new()));
        let color = Arc::new(Mutex::new(StubColor {
            supported: false,
            applied: applied.clone(),
        }));
        let reported = Arc::new(StdMutex::new(Vec::new()));
        let reported2 = reported.clone();
        let mut schedule = ScheduleService::new(color, AppSettings::default);
        schedule.on_temperature_applied(move |k| reported2.lock().unwrap().push(k));
        schedule.tick();
        assert!(applied.lock().unwrap().is_empty());
        assert!(reported.lock().unwrap().is_empty());
    }

    #[test]
    fn scheduler_inert_while_automatic_is_off() {
        let applied = Arc::new(StdMutex::new(Vec::new()));
        let color = Arc::new(Mutex::new(StubColor {
            supported: true,
            applied: applied.clone(),
        }));
        let mut settings = AppSettings::default();
        settings.auto_color_temperature = false;
        let mut schedule = ScheduleService::new(color, move || settings.clone());
        schedule.tick();
        assert!(applied.lock().unwrap().is_empty());
    }

    #[test]
    fn scheduler_applies_and_reports() {
        let applied = Arc::new(StdMutex::new(Vec::new()));
        let color = Arc::new(Mutex::new(StubColor {
            supported: true,
            applied: applied.clone(),
        }));
        let reported = Arc::new(StdMutex::new(Vec::new()));
        let reported2 = reported.clone();
        let mut schedule = ScheduleService::new(color, AppSettings::default);
        schedule.on_temperature_applied(move |k| reported2.lock().unwrap().push(k));
        schedule.tick();
        assert_eq!(applied.lock().unwrap().len(), 1);
        assert_eq!(*reported.lock().unwrap(), *applied.lock().unwrap());
    }
}
