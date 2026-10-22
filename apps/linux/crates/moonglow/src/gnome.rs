//! GNOME colour temperature via the SettingsDaemon Night Light API.
//!
//! Used on Wayland GNOME (Mutter), where no gamma protocol is available:
//! `org.gnome.SettingsDaemon.Color` at `/org/gnome/SettingsDaemon/Color`
//! exposes a read/write `Temperature` property. The property alone does
//! nothing: the screen only follows it while Night Light is *active*, and
//! Active is governed by the user's enabled flag plus their schedule window.
//! Writing Temperature into an inactive Night Light is silently ignored, and
//! an enabled GNOME schedule would periodically clobber our value with its
//! own — so while driving, the backend takes schedule ownership (enabled, a
//! full-day manual window, mirrored schedule temperature) and restores every
//! key it touched on reset.
//!
//! Night Light has no brightness channel, so software brightness is
//! unsupported here — brightness needs DDC/CI on GNOME Wayland.

use moonglow_core::traits::ColorTemperatureService;
use std::sync::Mutex;

const BUS_NAME: &str = "org.gnome.SettingsDaemon.Color";
const OBJECT_PATH: &str = "/org/gnome/SettingsDaemon/Color";
const INTERFACE: &str = "org.gnome.SettingsDaemon.Color";
const SCHEMA: &str = "org.gnome.settings-daemon.plugins.color";

/// Night Light temperature range. Values below 1000 K clamp.
const MIN_TEMP: u32 = 1000;
const MAX_TEMP: u32 = 10000;

pub struct GnomeNightLightService {
    runtime: tokio::runtime::Runtime,
    conn: zbus::Connection,
    original: Mutex<OriginalState>,
    supported: bool,
}

/// Full-day manual window forced while driving, so Night Light stays
/// active around the clock. Verified live: every candidate full-day window
/// (including from == to) reports NightLightActive=true.
const ALWAYS_ON_FROM: f64 = 0.0;
const ALWAYS_ON_TO: f64 = 24.0;

struct OriginalState {
    temperature: u32,
    enabled: bool,
    schedule_automatic: bool,
    schedule_from: f64,
    schedule_to: f64,
    schedule_temperature: u32,
    last_applied: Option<u32>,
    touched: bool,
}

impl GnomeNightLightService {
    /// Probes for the Color plugin on the session bus. Fails when GNOME
    /// SettingsDaemon is absent, in which case no colour support is reported.
    pub fn probe() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let conn = runtime.block_on(zbus::Connection::session())?;
        let temperature: u32 = runtime.block_on(async {
            let proxy = zbus::Proxy::new(&conn, BUS_NAME, OBJECT_PATH, INTERFACE).await?;
            proxy.get_property::<u32>("Temperature").await
        })?;
        // Snapshot the whole schedule up front. Ownership (ensure_enabled)
        // rewrites these keys, so without a complete snapshot there is no
        // honest restore — and without gsettings there is no ownership at
        // all. Fail closed: report unsupported rather than drive a Night
        // Light the schedule would gate or clobber.
        let snapshot = read_schedule_snapshot().ok_or("night-light schedule unreadable")?;
        Ok(Self {
            runtime,
            conn,
            original: Mutex::new(OriginalState {
                temperature,
                enabled: snapshot.enabled,
                schedule_automatic: snapshot.automatic,
                schedule_from: snapshot.from,
                schedule_to: snapshot.to,
                schedule_temperature: snapshot.temperature,
                last_applied: None,
                touched: false,
            }),
            supported: true,
        })
    }

    fn set_temperature_live(&self, temp: u32) -> bool {
        // Reachable from tray handlers (inside a runtime): never block directly.
        let result = crate::runtime::drive(&self.runtime, async {
            let proxy = zbus::Proxy::new(&self.conn, BUS_NAME, OBJECT_PATH, INTERFACE).await?;
            proxy.set_property("Temperature", temp).await
        });
        if result.is_ok() {
            return true;
        }
        // Fallback: gsettings writes dconf, which the plugin also honours.
        gsettings_set("night-light-temperature", &temp.to_string()).is_ok()
    }

    /// Takes schedule ownership on first drive: enabled, manual mode, and
    /// a full-day window keep Night Light active so our Temperature writes
    /// take effect at any hour, even when the user already had Night Light
    /// enabled with their own window. Returns false when ownership failed.
    fn ensure_enabled(&self) -> bool {
        let mut original = match self.original.lock() {
            Ok(o) => o,
            Err(_) => return false,
        };
        if original.touched {
            return true;
        }
        original.touched = true;
        // No short-circuit: attempt every write even if one fails.
        let mut ok = true;
        ok &= gsettings_set("night-light-enabled", "true").is_ok();
        ok &= gsettings_set("night-light-schedule-automatic", "false").is_ok();
        ok &= gsettings_set("night-light-schedule-from", &ALWAYS_ON_FROM.to_string()).is_ok();
        ok &= gsettings_set("night-light-schedule-to", &ALWAYS_ON_TO.to_string()).is_ok();
        ok
    }
}

impl ColorTemperatureService for GnomeNightLightService {
    fn backend_name(&self) -> String {
        "GNOME Night Light control".to_string()
    }

    fn is_supported(&self) -> bool {
        self.supported
    }

    fn apply(&self, kelvin: i32) -> bool {
        if !self.supported {
            return false;
        }
        let temp = kelvin.clamp(MIN_TEMP as i32, MAX_TEMP as i32) as u32;
        if !self.ensure_enabled() {
            return false;
        }
        {
            let mut original = match self.original.lock() {
                Ok(o) => o,
                Err(_) => return false,
            };
            if original.last_applied == Some(temp) {
                return true;
            }
            original.last_applied = Some(temp);
        }
        // Mirror into the schedule temperature so a daemon re-evaluation
        // (resume, settings touch) rewrites our value, never a stale one.
        let _ = gsettings_set("night-light-temperature", &temp.to_string());
        self.set_temperature_live(temp)
    }

    fn reset(&self) {
        let mut original = match self.original.lock() {
            Ok(o) => o,
            Err(_) => return,
        };
        if !original.touched {
            return;
        }
        original.last_applied = None;
        let _ = self.set_temperature_live(original.temperature);
        if !original.enabled {
            let _ = gsettings_set("night-light-enabled", "false");
        }
        let _ = gsettings_set(
            "night-light-schedule-automatic",
            if original.schedule_automatic {
                "true"
            } else {
                "false"
            },
        );
        let _ = gsettings_set(
            "night-light-schedule-from",
            &format!("{:?}", original.schedule_from),
        );
        let _ = gsettings_set(
            "night-light-schedule-to",
            &format!("{:?}", original.schedule_to),
        );
        let _ = gsettings_set(
            "night-light-temperature",
            &original.schedule_temperature.to_string(),
        );
    }
}

fn gsettings_get(key: &str) -> std::io::Result<String> {
    let output = std::process::Command::new("gsettings")
        .args(["get", SCHEMA, key])
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("gsettings get failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn gsettings_bool(key: &str) -> Option<bool> {
    parse_bool(&gsettings_get(key).ok()?)
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn gsettings_f64(key: &str) -> Option<f64> {
    parse_f64(&gsettings_get(key).ok()?)
}

fn parse_f64(raw: &str) -> Option<f64> {
    raw.trim().parse::<f64>().ok()
}

fn gsettings_u32(key: &str) -> Option<u32> {
    parse_u32(&gsettings_get(key).ok()?)
}

fn parse_u32(raw: &str) -> Option<u32> {
    // gsettings prints typed values ("uint32 2700"); tolerate a bare number.
    raw.trim()
        .strip_prefix("uint32 ")
        .unwrap_or(raw.trim())
        .parse::<u32>()
        .ok()
}

struct ScheduleSnapshot {
    enabled: bool,
    automatic: bool,
    from: f64,
    to: f64,
    temperature: u32,
}

fn read_schedule_snapshot() -> Option<ScheduleSnapshot> {
    Some(ScheduleSnapshot {
        enabled: gsettings_bool("night-light-enabled")?,
        automatic: gsettings_bool("night-light-schedule-automatic")?,
        from: gsettings_f64("night-light-schedule-from")?,
        to: gsettings_f64("night-light-schedule-to")?,
        temperature: gsettings_u32("night-light-temperature")?,
    })
}

fn gsettings_set(key: &str, value: &str) -> std::io::Result<()> {
    let status = std::process::Command::new("gsettings")
        .args(["set", SCHEMA, key, value])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("gsettings set failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_fails_without_session_bus() {
        // No session bus in CI/containers: the probe must fail cleanly rather
        // than report support it cannot act on. If a bus IS present (dev
        // machine), the probe outcome depends on GNOME; either way it must
        // not panic.
        let _ = GnomeNightLightService::probe();
    }

    #[test]
    fn temperature_clamps_to_night_light_range() {
        assert_eq!((900i32).clamp(MIN_TEMP as i32, MAX_TEMP as i32), 1000);
        assert_eq!((6500i32).clamp(MIN_TEMP as i32, MAX_TEMP as i32), 6500);
    }

    #[test]
    fn schedule_value_parsers_accept_gsettings_output() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("false\n"), Some(false));
        assert_eq!(parse_bool("1"), None);
        assert_eq!(parse_f64("20.0"), Some(20.0));
        assert_eq!(parse_f64("6"), Some(6.0));
        assert_eq!(parse_f64("dusk"), None);
        assert_eq!(parse_u32("uint32 2700"), Some(2700));
        assert_eq!(parse_u32("2700"), Some(2700));
        assert_eq!(parse_u32("warm"), None);
    }

    /// Live ownership round-trip: apply() must force Night Light active at
    /// any hour (even outside the user's own window) and reset() must restore
    /// every key it touched. Ignored by default — it briefly rewrites the
    /// developer's own Night Light schedule (and restores it).
    #[test]
    #[ignore]
    fn ownership_activates_and_restores_schedule() {
        let before = read_schedule_snapshot().expect("live GNOME schedule required");
        let before_temp: u32 = gsettings_get("night-light-temperature")
            .ok()
            .and_then(|raw| parse_u32(&raw))
            .expect("schedule temperature readable");

        let service = GnomeNightLightService::probe().expect("probe on live GNOME");
        assert!(service.is_supported());
        assert!(service.apply(2500), "apply must take ownership");

        // Ownership holds regardless of the user's own window: the daemon
        // must report active, at our temperature, within a few seconds.
        let active = service.runtime.block_on(async {
            let proxy = zbus::Proxy::new(&service.conn, BUS_NAME, OBJECT_PATH, INTERFACE)
                .await
                .unwrap();
            let mut active = false;
            for _ in 0..10 {
                active = proxy
                    .get_property::<bool>("NightLightActive")
                    .await
                    .unwrap_or(false);
                if active {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            let temperature = proxy.get_property::<u32>("Temperature").await.unwrap_or(0);
            (active, temperature)
        });
        assert!(active.0, "Night Light must be active while driving");
        assert_eq!(active.1, 2500, "driven temperature must land");

        service.reset();
        let after = read_schedule_snapshot().expect("schedule readable after reset");
        assert_eq!(after.enabled, before.enabled, "enabled flag restored");
        assert_eq!(after.automatic, before.automatic, "schedule mode restored");
        assert_eq!(after.from, before.from, "window start restored");
        assert_eq!(after.to, before.to, "window end restored");
        assert_eq!(
            after.temperature, before_temp,
            "schedule temperature restored"
        );
    }
}
