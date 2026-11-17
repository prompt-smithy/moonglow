//! Display enumeration from DRM sysfs (`/sys/class/drm`).
//!
//! Used on Wayland, where there is no RandR. Connectors reporting `connected`
//! become [`DisplayInfo`] entries; capability flags come from the backends that
//! act on them, threaded in rather than probed here.

use moonglow_core::traits::DisplayService;
use moonglow_core::DisplayInfo;
use std::collections::HashSet;
use std::path::PathBuf;

type CapabilityProvider = Box<dyn Fn() -> (HashSet<String>, bool, bool) + Send>;

/// Enumerates connected DRM connectors.
pub struct SysfsDisplayService {
    drm_dir: PathBuf,
    ddc_brightness_supported: bool,
    ddc_connectors: Option<HashSet<String>>,
    software_brightness_supported: bool,
    gamma_supported: bool,
    displays: Vec<DisplayInfo>,
    live_capabilities: Option<CapabilityProvider>,
}

impl SysfsDisplayService {
    pub fn with_live_capabilities(
        provider: impl Fn() -> (HashSet<String>, bool, bool) + Send + 'static,
    ) -> Self {
        let mut service = Self::new(false, false, false);
        service.live_capabilities = Some(Box::new(provider));
        service
    }
    /// `software_brightness` is separate from `gamma`: GNOME/KDE night lights
    /// drive temperature with no brightness channel, so gamma support must
    /// not imply software dimming.
    pub fn new(
        ddc_brightness_supported: bool,
        software_brightness_supported: bool,
        gamma_supported: bool,
    ) -> Self {
        Self::with_drm_dir(
            PathBuf::from("/sys/class/drm"),
            ddc_brightness_supported,
            software_brightness_supported,
            gamma_supported,
        )
    }

    pub fn with_drm_dir(
        drm_dir: PathBuf,
        ddc: bool,
        software_brightness: bool,
        gamma: bool,
    ) -> Self {
        Self {
            drm_dir,
            ddc_brightness_supported: ddc,
            ddc_connectors: None,
            software_brightness_supported: software_brightness,
            gamma_supported: gamma,
            displays: Vec::new(),
            live_capabilities: None,
        }
    }
}

impl DisplayService for SysfsDisplayService {
    fn displays(&self) -> Vec<DisplayInfo> {
        self.displays.clone()
    }

    fn refresh(&mut self) -> Vec<DisplayInfo> {
        if let Some(provider) = &self.live_capabilities {
            let (ddc_connectors, software, gamma) = provider();
            self.ddc_brightness_supported = !ddc_connectors.is_empty();
            self.ddc_connectors = Some(ddc_connectors);
            self.software_brightness_supported = software;
            self.gamma_supported = gamma;
        }
        let mut connectors = connected_connectors(&self.drm_dir);
        connectors.sort_by(|left, right| left.0.cmp(&right.0));
        self.displays = connectors
            .iter()
            .enumerate()
            .map(|(index, (drm_name, name))| {
                // First connected connector is treated as primary; sysfs has no
                // primary concept. eDP/LVDS panels are internal: DDC never
                // reaches them, but gamma and (laptop) software dimming do.
                let internal = name.contains("eDP") || name.contains("LVDS");
                let hardware_supported = !internal
                    && self
                        .ddc_connectors
                        .as_ref()
                        .map_or(self.ddc_brightness_supported, |connectors| {
                            connectors.contains(drm_name)
                        });
                DisplayInfo::new(
                    name.clone(),
                    name.clone(),
                    format!("drm-{drm_name}"),
                    index == 0,
                    hardware_supported || self.software_brightness_supported,
                    self.gamma_supported,
                )
                .with_brightness_modes(hardware_supported, self.software_brightness_supported)
            })
            .collect();
        self.displays.clone()
    }
}

/// Returns `(card-qualified DRM connector, display name)` pairs. Connector
/// suffixes such as `DP-1` can repeat on different graphics cards.
fn connected_connectors(drm_dir: &PathBuf) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(drm_dir) {
        Ok(entries) => entries,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("card") || !name.contains('-') {
            continue;
        }
        let status = entry.path().join("status");
        if let Ok(text) = std::fs::read_to_string(&status) {
            if text.trim() == "connected"
                && !std::fs::read_to_string(entry.path().join("enabled"))
                    .is_ok_and(|v| v.trim() == "disabled")
            {
                if let Some((_, connector)) = name.split_once('-') {
                    let display_name = connector.to_string();
                    out.push((name, display_name));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_drm(name: &str, connectors: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "moonglow-drm-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for (connector, status) in connectors {
            let card = dir.join(format!("card1-{connector}"));
            std::fs::create_dir_all(&card).unwrap();
            std::fs::write(card.join("status"), format!("{status}\n")).unwrap();
        }
        dir
    }

    #[test]
    fn lists_connected_connectors_only() {
        let dir = fake_drm(
            "basic",
            &[("DP-1", "connected"), ("HDMI-A-1", "disconnected")],
        );
        let mut service = SysfsDisplayService::with_drm_dir(dir.clone(), true, true, true);
        let displays = service.refresh();
        assert_eq!(displays.len(), 1);
        assert_eq!(displays[0].name, "DP-1");
        assert!(displays[0].is_primary);
        assert!(displays[0].supports_brightness);
        assert!(displays[0].supports_color_temperature);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn internal_panels_never_get_ddc() {
        let dir = fake_drm("edp", &[("eDP-1", "connected")]);
        let mut service = SysfsDisplayService::with_drm_dir(dir.clone(), true, false, false);
        let displays = service.refresh();
        assert_eq!(displays.len(), 1);
        assert!(!displays[0].supports_brightness);
        assert!(!displays[0].supports_color_temperature);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gamma_without_software_brightness_is_temperature_only() {
        // GNOME/KDE night lights: temperature yes, brightness no.
        let dir = fake_drm("nightlight", &[("DP-1", "connected")]);
        let mut service = SysfsDisplayService::with_drm_dir(dir.clone(), false, false, true);
        let displays = service.refresh();
        assert_eq!(displays.len(), 1);
        assert!(!displays[0].supports_brightness);
        assert!(displays[0].supports_color_temperature);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_sysfs_yields_no_displays() {
        let mut service =
            SysfsDisplayService::with_drm_dir(PathBuf::from("/nonexistent-drm"), true, true, true);
        assert!(service.refresh().is_empty());
    }

    #[test]
    fn capability_changes_are_visible_without_reconnecting_the_display() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let dir = fake_drm("late-hardware", &[("DP-1", "connected")]);
        let supported = Arc::new(AtomicBool::new(false));
        let live = supported.clone();
        let mut service = SysfsDisplayService::with_drm_dir(dir.clone(), false, false, true);
        service.live_capabilities = Some(Box::new(move || {
            (
                if live.load(Ordering::Relaxed) {
                    HashSet::from(["card1-DP-1".to_owned()])
                } else {
                    HashSet::new()
                },
                false,
                true,
            )
        }));
        assert!(!service.refresh()[0].supports_brightness);
        supported.store(true, Ordering::Relaxed);
        assert!(service.refresh()[0].supports_brightness);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn lid_closed_panel_is_not_an_active_display() {
        let dir = fake_drm(
            "lid-closed",
            &[("eDP-1", "connected"), ("DP-1", "connected")],
        );
        std::fs::write(dir.join("card1-eDP-1/enabled"), "disabled").unwrap();
        let mut service = SysfsDisplayService::with_drm_dir(dir.clone(), true, true, true);
        let displays = service.refresh();
        assert_eq!(displays.len(), 1);
        assert_eq!(displays[0].name, "DP-1");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
