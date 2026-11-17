//! X11 backends: RandR gamma colour temperature and display enumeration.
//!
//! Used on X11 and XWayland sessions. Like the macOS CoreGraphics backend,
//! colour temperature and software brightness share one gamma ramp per CRTC so
//! adjusting one never overwrites the other. Original ramps are captured at
//! startup and restored on quit (X11 has no ColorSync-style restore call).

use moonglow_core::kelvin;
use moonglow_core::settings::AppSettings;
use moonglow_core::traits::{ColorTemperatureService, DisplayService};
use moonglow_core::DisplayInfo;
use std::collections::HashSet;
use std::sync::Mutex;
use x11rb::connection::Connection;
use x11rb::protocol::randr::{self, ConnectionExt as _, Crtc};
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

type X11Result<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn connect() -> X11Result<(RustConnection, usize)> {
    let (conn, screen) = x11rb::connect(None)?;
    let version = conn.randr_query_version(1, 5)?.reply()?;
    if version.major_version < 1 || (version.major_version == 1 && version.minor_version < 2) {
        return Err("RandR 1.2+ required for gamma control".into());
    }
    Ok((conn, screen))
}

/// One gamma-capable CRTC and its pre-launch ramp.
struct GammaCrtc {
    crtc: Crtc,
    size: usize,
    original: (Vec<u16>, Vec<u16>, Vec<u16>),
}

/// Applies colour temperature by writing per-CRTC RandR gamma tables.
pub struct X11GammaService {
    conn: RustConnection,
    crtcs: Vec<GammaCrtc>,
    shared: Mutex<Shared>,
}

struct Shared {
    kelvin: i32,
    software_brightness: i32,
}

impl X11GammaService {
    /// Connects and captures the current ramps. Fails when no gamma-capable
    /// CRTC exists, in which case the caller reports no colour support.
    pub fn connect() -> X11Result<Self> {
        let (conn, screen) = connect()?;
        let root = conn.setup().roots[screen].root;
        let resources = conn.randr_get_screen_resources_current(root)?.reply()?;
        let mut crtcs = Vec::new();
        for crtc in &resources.crtcs {
            let size = conn.randr_get_crtc_gamma_size(*crtc)?.reply()?.size as usize;
            if size == 0 {
                continue;
            }
            let reply = conn.randr_get_crtc_gamma(*crtc)?.reply()?;
            crtcs.push(GammaCrtc {
                crtc: *crtc,
                size,
                original: (reply.red, reply.green, reply.blue),
            });
        }
        if crtcs.is_empty() {
            return Err("no gamma-capable CRTC found".into());
        }
        Ok(Self {
            conn,
            crtcs,
            shared: Mutex::new(Shared {
                kelvin: 6500,
                software_brightness: 100,
            }),
        })
    }

    /// Whether at least one CRTC can use a gamma ramp for software brightness.
    pub fn supports_software_brightness(&self) -> bool {
        !self.crtcs.is_empty()
    }

    /// Applies software dimming through the same gamma ramp as colour temperature.
    pub fn set_software_brightness(&self, percent: i32) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.software_brightness = AppSettings::clamp_brightness(percent);
        }
        let _ = self.apply_current_ramp();
    }

    fn apply_current_ramp(&self) -> bool {
        let (kelvin_value, brightness) = match self.shared.lock() {
            Ok(shared) => (shared.kelvin, shared.software_brightness),
            Err(_) => return false,
        };
        let scale = kelvin::rgb_scale(kelvin_value);
        let brightness_scale = brightness as f64 / 100.0;
        let mut applied = false;
        for crtc in &self.crtcs {
            let ramp = kelvin::build_ramp(
                crtc.size,
                scale.red,
                scale.green,
                scale.blue,
                brightness_scale,
            );
            let red = to_u16(&ramp.red);
            let green = to_u16(&ramp.green);
            let blue = to_u16(&ramp.blue);
            let ok = match self
                .conn
                .randr_set_crtc_gamma(crtc.crtc, &red, &green, &blue)
            {
                Ok(cookie) => cookie.check().is_ok(),
                Err(_) => false,
            };
            if ok {
                applied = true;
            }
        }
        let _ = self.conn.flush();
        applied
    }
}

fn to_u16(ramp: &[f32]) -> Vec<u16> {
    ramp.iter()
        .map(|v| (v.clamp(0.0, 1.0) as f64 * 65535.0).round() as u16)
        .collect()
}

impl ColorTemperatureService for X11GammaService {
    fn backend_name(&self) -> String {
        "X11 gamma control".to_string()
    }

    fn is_supported(&self) -> bool {
        !self.crtcs.is_empty()
    }

    fn apply(&self, kelvin: i32) -> bool {
        if let Ok(mut shared) = self.shared.lock() {
            shared.kelvin = kelvin;
        }
        self.apply_current_ramp()
    }

    fn reset(&self) {
        // Restore the pre-launch ramps, undoing every ramp this service applied.
        for crtc in &self.crtcs {
            let _ = self
                .conn
                .randr_set_crtc_gamma(
                    crtc.crtc,
                    &crtc.original.0,
                    &crtc.original.1,
                    &crtc.original.2,
                )
                .map(|cookie| cookie.check());
        }
        let _ = self.conn.flush();
    }
}

/// Software brightness fallback backed by the shared X11 gamma ramps.
pub struct X11SoftwareBrightnessService {
    gamma: std::sync::Arc<Mutex<X11GammaService>>,
}

impl X11SoftwareBrightnessService {
    pub fn new(gamma: std::sync::Arc<Mutex<X11GammaService>>) -> Self {
        Self { gamma }
    }
}

impl moonglow_core::traits::BrightnessService for X11SoftwareBrightnessService {
    fn is_supported(&self) -> bool {
        self.gamma
            .lock()
            .map(|g| g.supports_software_brightness())
            .unwrap_or(false)
    }

    fn is_hardware_supported(&self) -> bool {
        false
    }

    fn is_software_supported(&self) -> bool {
        self.is_supported()
    }

    fn set_brightness(&self, percent: i32) {
        if let Ok(gamma) = self.gamma.lock() {
            gamma.set_software_brightness(percent);
        }
    }
}

/// Enumerates displays through RandR outputs.
pub struct X11DisplayService {
    ddc_connectors: Box<dyn Fn() -> HashSet<String> + Send>,
    displays: Vec<DisplayInfo>,
}

impl X11DisplayService {
    pub fn new(ddc_connectors: impl Fn() -> HashSet<String> + Send + 'static) -> Self {
        Self {
            ddc_connectors: Box::new(ddc_connectors),
            displays: Vec::new(),
        }
    }
}

impl DisplayService for X11DisplayService {
    fn displays(&self) -> Vec<DisplayInfo> {
        self.displays.clone()
    }

    fn refresh(&mut self) -> Vec<DisplayInfo> {
        self.displays = enumerate(&(self.ddc_connectors)()).unwrap_or_default();
        self.displays.clone()
    }
}

fn enumerate(ddc_connectors: &HashSet<String>) -> X11Result<Vec<DisplayInfo>> {
    let (conn, screen) = connect()?;
    let root = conn.setup().roots[screen].root;
    let resources = conn.randr_get_screen_resources_current(root)?.reply()?;
    let primary = conn
        .randr_get_output_primary(root)?
        .reply()
        .map(|r| r.output)
        .unwrap_or(0);

    let mut displays = Vec::new();
    for output in &resources.outputs {
        let info = conn.randr_get_output_info(*output, resources.config_timestamp)?;
        let Ok(info) = info.reply() else { continue };
        if info.connection != randr::Connection::CONNECTED || info.crtc == 0 {
            continue;
        }
        let name = String::from_utf8_lossy(&info.name).into_owned();
        // Gamma support is what colour temperature needs: only a CRTC with a
        // nonzero gamma table can take ramps at all.
        let supports_gamma = if info.crtc == 0 {
            false
        } else {
            match conn.randr_get_crtc_gamma_size(info.crtc) {
                Ok(cookie) => cookie.reply().map(|r| r.size > 0).unwrap_or(false),
                Err(_) => false,
            }
        };
        let identity = conn
            .intern_atom(true, b"EDID")
            .ok()
            .and_then(|c| c.reply().ok())
            .filter(|atom| atom.atom != 0)
            .and_then(|atom| {
                conn.randr_get_output_property(*output, atom.atom, 0u32, 0, 32, false, false)
                    .ok()
            })
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|property| crate::ddc::edid_identity(&property.data));
        let supports_hardware =
            output_supports_hardware(&name, identity.as_deref(), ddc_connectors);
        displays.push(
            DisplayInfo::new(
                output.to_string(),
                name.clone(),
                format!("x11-{output}"),
                *output == primary,
                supports_hardware || supports_gamma,
                supports_gamma,
            )
            .with_brightness_modes(supports_hardware, supports_gamma),
        );
    }
    // A count-based fallback is safe only when RandR agrees with DRM about
    // the complete external output set (including active virtual outputs).
    let is_external = |display: &DisplayInfo| {
        !["eDP", "LVDS", "DSI"]
            .iter()
            .any(|prefix| display.name.starts_with(prefix))
    };
    let external_count = displays.iter().filter(|d| is_external(d)).count();
    if external_count > 0
        && ddc_connectors.contains(&format!("all-external-outputs:{external_count}"))
    {
        for display in displays.iter_mut().filter(|d| is_external(d)) {
            display.supports_hardware_brightness = true;
            display.supports_brightness = true;
        }
    }
    // No primary reported: treat the first output as primary.
    if !primary_valid(&displays) && !displays.is_empty() {
        displays[0].is_primary = true;
    }
    Ok(displays)
}

fn primary_valid(displays: &[DisplayInfo]) -> bool {
    displays.iter().any(|d| d.is_primary)
}

/// EDID is authoritative when present; aliases are a fallback for older drivers.
fn output_supports_hardware(
    name: &str,
    identity: Option<&str>,
    capabilities: &HashSet<String>,
) -> bool {
    if let Some(identity) = identity {
        if capabilities.iter().any(|key| key.starts_with("edid-")) {
            return capabilities.contains(identity);
        }
    }
    let normalize = |s: &str| {
        s.replace("-A-", "-")
            .replace("DisplayPort-", "DP-")
            .replace('-', "")
    };
    capabilities.contains(name)
        || capabilities
            .iter()
            .filter(|key| {
                !key.starts_with("card")
                    && !key.starts_with("edid-")
                    && !key.starts_with("all-external-outputs:")
            })
            .any(|key| normalize(key) == normalize(name))
}

#[cfg(test)]
mod capability_tests {
    use super::*;
    #[test]
    fn edid_identity_crosses_driver_names_and_overrules_misleading_aliases() {
        let capabilities = HashSet::from(["edid-supported".to_owned(), "HDMI-A-1".to_owned()]);
        assert!(output_supports_hardware(
            "DP-0",
            Some("edid-supported"),
            &capabilities
        ));
        assert!(!output_supports_hardware(
            "HDMI-A-1",
            Some("edid-unsupported"),
            &capabilities
        ));
        assert!(output_supports_hardware("HDMI-1", None, &capabilities));
        assert!(output_supports_hardware("HDMI1", None, &capabilities));
    }
}
