//! Service protocols behind which all platform behaviour sits.
//!
//! Ported from `Services/ServiceProtocols.swift`. The app crate supplies the
//! Linux backends; `moonglow-core` never touches hardware.

use crate::{AppSettings, DisplayInfo};
use std::sync::{Arc, Mutex};

/// Loads and persists [`AppSettings`]. The app crate implements this against
/// the XDG config directory; tests use an in-memory store.
pub trait SettingsPersistence: Send {
    /// Reads settings, falling back to defaults for a missing or corrupt file.
    fn load(&self) -> AppSettings;
    /// Persists settings, normalizing them first. Best effort: a filesystem
    /// failure must never crash the app.
    fn save(&self, settings: &AppSettings);
}

/// Enumerates displays and reports what the app can control on each one.
pub trait DisplayService: Send {
    fn displays(&self) -> Vec<DisplayInfo>;
    fn refresh(&mut self) -> Vec<DisplayInfo>;
}

/// Hardware (DDC/CI) or software (gamma) brightness.
pub trait BrightnessService: Send {
    fn is_supported(&self) -> bool;
    fn set_brightness(&self, percent: i32);
    /// Releases hardware handles. Called on the way out.
    fn dispose(&mut self) {}
    /// Human-readable reason support is missing, shown when a capability is
    /// reported unavailable so the status never just says "no".
    fn status_detail(&self) -> Option<String> {
        None
    }
    /// Re-runs hardware detection, e.g. after the display set changed
    /// (hotplug, sleep/wake, KVM switch). Backends without hardware to
    /// rediscover keep the default no-op.
    fn reprobe(&mut self) {}
}

/// Uses hardware brightness when its DDC/CI probe succeeds, otherwise software
/// gamma dimming.
pub struct BrightnessFallbackService {
    hardware: Arc<Mutex<dyn BrightnessService>>,
    software: Arc<Mutex<dyn BrightnessService>>,
}

impl BrightnessFallbackService {
    pub fn new(
        hardware: Arc<Mutex<dyn BrightnessService>>,
        software: Arc<Mutex<dyn BrightnessService>>,
    ) -> Self {
        Self { hardware, software }
    }
}

impl BrightnessService for BrightnessFallbackService {
    fn is_supported(&self) -> bool {
        self.hardware
            .lock()
            .map(|h| h.is_supported())
            .unwrap_or(false)
            || self
                .software
                .lock()
                .map(|s| s.is_supported())
                .unwrap_or(false)
    }

    fn set_brightness(&self, percent: i32) {
        let hardware_supported = self
            .hardware
            .lock()
            .map(|h| h.is_supported())
            .unwrap_or(false);
        if hardware_supported {
            // Exactly one mechanism owns brightness: a fallback episode may have
            // left the gamma ramps dimmed, so reset them before the hardware
            // write or the two would compound. Nearly free on the steady path —
            // gamma backends skip the rewrite when already at 100%.
            if let Ok(software) = self.software.lock() {
                software.set_brightness(100);
            }
            if let Ok(h) = self.hardware.lock() {
                h.set_brightness(percent);
            }
        } else if self
            .software
            .lock()
            .map(|s| s.is_supported())
            .unwrap_or(false)
        {
            if let Ok(s) = self.software.lock() {
                s.set_brightness(percent);
            }
        }
    }

    fn dispose(&mut self) {
        if let Ok(mut h) = self.hardware.lock() {
            h.dispose();
        }
        if let Ok(mut s) = self.software.lock() {
            s.dispose();
        }
    }

    fn reprobe(&mut self) {
        // Software support is derived live from the displays; only hardware can be
        // late, so only that backend is re-probed.
        if let Ok(mut h) = self.hardware.lock() {
            h.reprobe();
        }
    }

    fn status_detail(&self) -> Option<String> {
        // Only the hardware path diagnoses itself; software dimming is either
        // there (gamma backend) or not, with nothing actionable to report.
        let hardware_supported = self
            .hardware
            .lock()
            .map(|h| h.is_supported())
            .unwrap_or(false);
        if hardware_supported {
            return None;
        }
        self.hardware.lock().ok().and_then(|h| h.status_detail())
    }
}

/// Gamma-table colour temperature.
pub trait ColorTemperatureService: Send {
    /// Human name of the active backend, e.g. "X11 gamma control".
    fn backend_name(&self) -> String {
        "Gamma control".to_string()
    }
    fn is_supported(&self) -> bool;
    /// Applies `kelvin`. Returns whether at least one display took it, so a
    /// caller can tell "applied" from "no capable display attached".
    fn apply(&self, kelvin: i32) -> bool;
    /// Restores the pre-launch state, undoing every ramp this service applied.
    fn reset(&self);
    /// Drops any "already applied" cache. Gamma tables are global state that
    /// the desktop, a daemon, or a display change can replace or clear, so the
    /// last write is never assumed to still be on screen. Callers invalidate on
    /// every display change; the next apply writes again.
    fn invalidate(&self) {}
}

/// Registers the app to start at login.
///
/// The system owns this state, not the app: the user can remove the autostart
/// entry outside the app. The trait therefore reports status instead of storing
/// a flag, and [`LoginItemService::set_enabled`] says whether the system
/// accepted the change so a caller never records intent that did not take effect.
pub trait LoginItemService: Send {
    /// Whether the app is currently registered to open at login, as the system sees it.
    fn is_enabled(&self) -> bool;

    /// Whether the registration is parked until the user approves it. Linux has
    /// no approval flow; backends report false.
    fn requires_approval(&self) -> bool;

    /// Registers or unregisters the app. Returns false when the change was refused.
    fn set_enabled(&mut self, enabled: bool) -> bool;

    /// Opens the autostart settings so the registration can be inspected.
    fn open_settings(&self) {}
}
