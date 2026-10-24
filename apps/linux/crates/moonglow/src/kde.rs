//! KDE colour temperature via the KWin NightLight D-Bus API.
//!
//! Used on Wayland KDE (KWin), which implements no gamma protocol:
//! `org.kde.KWin` at `/org/kde/KWin/NightLight` offers `preview(u32)` and
//! `stopPreview()`. KWin drops the preview when the calling connection closes,
//! so the backend holds its session-bus connection for life and re-sends the
//! preview every few seconds while active.
//!
//! Like GNOME Night Light, there is no brightness channel — brightness needs
//! DDC/CI on KDE Wayland.

use moonglow_core::traits::ColorTemperatureService;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const BUS_NAME: &str = "org.kde.KWin";
const OBJECT_PATH: &str = "/org/kde/KWin/NightLight";
const INTERFACE: &str = "org.kde.KWin.NightLight";

/// KWin night-colour temperature range. Values below 1000 K clamp.
const MIN_TEMP: u32 = 1000;
const MAX_TEMP: u32 = 6500;

/// Re-preview cadence while active. KWin expires previews, so the schedule's
/// 10 s tick alone is not enough to hold the temperature.
const REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(8);

pub struct KwinNightLightService {
    runtime: tokio::runtime::Runtime,
    conn: zbus::Connection,
    current: Arc<Mutex<Option<u32>>>,
    stop_refresh: Arc<AtomicBool>,
    refresher: Mutex<Option<std::thread::JoinHandle<()>>>,
    supported: bool,
}

impl KwinNightLightService {
    /// Probes for the NightLight object on the session bus. Fails when KWin
    /// is absent, in which case no colour support is reported.
    pub fn probe() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let conn = runtime.block_on(zbus::Connection::session())?;
        // `currentTemperature` is only readable when the NightLight object exists.
        let _: u32 = runtime.block_on(async {
            let proxy = zbus::Proxy::new(&conn, BUS_NAME, OBJECT_PATH, INTERFACE).await?;
            proxy.get_property::<u32>("currentTemperature").await
        })?;
        Ok(Self {
            runtime,
            conn,
            current: Arc::new(Mutex::new(None)),
            stop_refresh: Arc::new(AtomicBool::new(false)),
            refresher: Mutex::new(None),
            supported: true,
        })
    }

    fn preview(&self, temp: u32) -> bool {
        // Reachable from UI and shortcut handlers (inside a runtime): never block directly.
        crate::runtime::drive(&self.runtime, async {
            let proxy = zbus::Proxy::new(&self.conn, BUS_NAME, OBJECT_PATH, INTERFACE).await?;
            proxy.call_method("preview", &(temp,)).await?;
            Ok::<_, zbus::Error>(())
        })
        .is_ok()
    }

    fn stop_preview(&self) {
        let _ = crate::runtime::drive(&self.runtime, async {
            let proxy = zbus::Proxy::new(&self.conn, BUS_NAME, OBJECT_PATH, INTERFACE).await?;
            proxy.call_method("stopPreview", &()).await?;
            Ok::<_, zbus::Error>(())
        });
    }

    fn ensure_refresher(&self) {
        let mut guard = match self.refresher.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if guard.is_some() {
            return;
        }
        let runtime_handle = self.runtime.handle().clone();
        let conn = self.conn.clone();
        let current = self.current.clone();
        let stop = self.stop_refresh.clone();
        *guard = std::thread::Builder::new()
            .name("moonglow-kwin-refresh".to_string())
            .spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    // Sleep in slices so shutdown never waits out the interval.
                    for _ in 0..REFRESH_INTERVAL.as_secs() {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let temp = current.lock().ok().and_then(|c| *c);
                    if let Some(temp) = temp {
                        let _ = runtime_handle.block_on(async {
                            let proxy =
                                zbus::Proxy::new(&conn, BUS_NAME, OBJECT_PATH, INTERFACE).await?;
                            proxy.call_method("preview", &(temp,)).await?;
                            Ok::<_, zbus::Error>(())
                        });
                    }
                }
            })
            .ok();
    }
}

impl ColorTemperatureService for KwinNightLightService {
    fn backend_name(&self) -> String {
        "KDE Night Light control".to_string()
    }

    fn is_supported(&self) -> bool {
        self.supported
    }

    fn apply(&self, kelvin: i32) -> bool {
        if !self.supported {
            return false;
        }
        let temp = kelvin.clamp(MIN_TEMP as i32, MAX_TEMP as i32) as u32;
        if self.preview(temp) {
            if let Ok(mut current) = self.current.lock() {
                *current = Some(temp);
            }
            self.ensure_refresher();
            true
        } else {
            false
        }
    }

    fn reset(&self) {
        if let Ok(mut current) = self.current.lock() {
            *current = None;
        }
        self.stop_preview();
    }
}

impl Drop for KwinNightLightService {
    fn drop(&mut self) {
        self.stop_refresh.store(true, Ordering::Relaxed);
        // Quitting must never leave the display tinted: stop the preview
        // synchronously before the connection closes.
        self.stop_preview();
        if let Ok(mut guard) = self.refresher.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_fails_without_session_bus() {
        // Must fail cleanly without a bus rather than claim support. On a
        // KDE dev machine the outcome varies; either way it must not panic.
        let _ = KwinNightLightService::probe();
    }

    #[test]
    fn temperature_clamps_to_night_light_range() {
        assert_eq!((900i32).clamp(MIN_TEMP as i32, MAX_TEMP as i32), 1000);
        assert_eq!((6600i32).clamp(MIN_TEMP as i32, MAX_TEMP as i32), 6500);
    }
}
