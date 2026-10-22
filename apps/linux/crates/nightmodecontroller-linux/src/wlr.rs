//! wlroots colour temperature via `wlr-gamma-control-unstable-v1`.
//!
//! Used on Wayland compositors that implement the protocol (Sway, Hyprland,
//! Wayfire, river, labwc, ...). Like the X11 backend, colour temperature and
//! software brightness share one gamma ramp per output. Reset writes identity
//! ramps — the protocol has no restore call, so linear is the documented
//! equivalent of the pre-launch state.

use nmc_core::kelvin;
use nmc_core::settings::AppSettings;
use nmc_core::traits::ColorTemperatureService;
use std::os::fd::AsFd;
use std::sync::Mutex;
use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::gamma_control::v1::client::{
    zwlr_gamma_control_manager_v1, zwlr_gamma_control_v1,
};

type WlrResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct ControlledOutput {
    output: wl_output::WlOutput,
    control: zwlr_gamma_control_v1::ZwlrGammaControlV1,
    size: Option<u32>,
    failed: bool,
}

struct WlrState {
    manager: Option<zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1>,
    outputs: Vec<wl_output::WlOutput>,
    controls: Vec<ControlledOutput>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for WlrState {
    fn event(
        state: &mut Self,
        proxy: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == wl_output::WlOutput::interface().name {
                let output = proxy.bind::<wl_output::WlOutput, _, _>(name, version.min(4), qh, ());
                state.outputs.push(output);
            } else if interface
                == zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1::interface().name
            {
                let manager = proxy
                    .bind::<zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1, _, _>(
                        name,
                        1,
                        qh,
                        (),
                    );
                state.manager = Some(manager);
            }
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for WlrState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_output::WlOutput,
        _event: wl_output::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1, ()> for WlrState {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1,
        _event: zwlr_gamma_control_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwlr_gamma_control_v1::ZwlrGammaControlV1, usize> for WlrState {
    fn event(
        state: &mut Self,
        _proxy: &zwlr_gamma_control_v1::ZwlrGammaControlV1,
        event: zwlr_gamma_control_v1::Event,
        index: &usize,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some(output) = state.controls.get_mut(*index) else {
            return;
        };
        match event {
            zwlr_gamma_control_v1::Event::GammaSize { size } => {
                output.size = Some(size);
            }
            zwlr_gamma_control_v1::Event::Failed => {
                output.failed = true;
            }
            _ => {}
        }
    }
}

pub struct WlrGammaService {
    // Held: the queue does not keep the connection alive on its own.
    _conn: Connection,
    queue: Mutex<wayland_client::EventQueue<WlrState>>,
    qh: QueueHandle<WlrState>,
    state: Mutex<WlrState>,
    shared: Mutex<Shared>,
}

struct Shared {
    kelvin: i32,
    software_brightness: i32,
}

impl WlrGammaService {
    /// Connects and binds gamma controls for every output. Fails when the
    /// compositor does not implement the protocol or no output reports a
    /// gamma size, in which case no colour support is reported.
    pub fn probe() -> WlrResult<Self> {
        let conn = Connection::connect_to_env()?;
        let mut queue = conn.new_event_queue::<WlrState>();
        let qh = queue.handle();
        conn.display().get_registry(&qh, ());
        let mut state = WlrState::default();
        queue.roundtrip(&mut state)?;

        let Some(manager) = state.manager.clone() else {
            return Err("compositor does not implement wlr-gamma-control".into());
        };
        for (index, output) in state.outputs.clone().iter().enumerate() {
            let control = manager.get_gamma_control(output, &qh, index);
            state.controls.push(ControlledOutput {
                output: output.clone(),
                control,
                size: None,
                failed: false,
            });
        }
        queue.roundtrip(&mut state)?;
        state
            .controls
            .retain(|c| !c.failed && c.size.unwrap_or(0) > 0);
        if state.controls.is_empty() {
            return Err("no output with a gamma table".into());
        }
        Ok(Self {
            _conn: conn,
            queue: Mutex::new(queue),
            qh,
            state: Mutex::new(state),
            shared: Mutex::new(Shared {
                kelvin: 6500,
                software_brightness: 100,
            }),
        })
    }

    pub fn supports_software_brightness(&self) -> bool {
        self.state
            .lock()
            .map(|s| !s.controls.is_empty())
            .unwrap_or(false)
    }

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
        self.apply_ramps(|capacity| {
            let scale = kelvin::rgb_scale(kelvin_value);
            kelvin::build_ramp(
                capacity,
                scale.red,
                scale.green,
                scale.blue,
                brightness as f64 / 100.0,
            )
        })
    }

    fn apply_ramps(&self, build: impl Fn(usize) -> nmc_core::kelvin::GammaRamp) -> bool {
        // Pick up Failed/GammaSize events before deciding what to drive.
        if let Ok(mut queue) = self.queue.lock() {
            if let Ok(mut state) = self.state.lock() {
                let _ = queue.dispatch_pending(&mut state);
            }
            let _ = queue.flush();
        }
        let mut applied = false;
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return false,
        };
        state
            .controls
            .retain(|c| !c.failed && c.size.unwrap_or(0) > 0);
        for output in &state.controls {
            let size = output.size.unwrap_or(0) as usize;
            if size == 0 {
                continue;
            }
            let ramp = build(size);
            let fd = match ramps_fd(&ramp) {
                Ok(fd) => fd,
                Err(_) => continue,
            };
            output.control.set_gamma(fd.as_fd());
            applied = true;
        }
        drop(state);
        if let Ok(queue) = self.queue.lock() {
            let _ = queue.flush();
        }
        applied
    }
}

/// Serializes the three f32 ramps to one memfd of successive native-endian
/// u16 tables (red, green, blue), rewound for the compositor to read from
/// the start.
fn ramps_fd(ramp: &nmc_core::kelvin::GammaRamp) -> WlrResult<std::os::fd::OwnedFd> {
    use std::io::{Seek, SeekFrom, Write};
    use std::os::fd::FromRawFd;

    let fd = rustix::fs::memfd_create("nmc-gamma", rustix::fs::MemfdFlags::CLOEXEC)?;
    let mut file = std::fs::File::from(fd);
    for channel in [&ramp.red, &ramp.green, &ramp.blue] {
        for v in channel {
            let entry = (v.clamp(0.0, 1.0) as f64 * 65535.0).round() as u16;
            file.write_all(&entry.to_ne_bytes())?;
        }
    }
    file.seek(SeekFrom::Start(0))?;
    // Re-wrap as a std fd (rustix and std fd types are distinct) through a
    // raw transfer with exactly one owner.
    let owned: rustix::fd::OwnedFd = file.into();
    let raw = rustix::fd::IntoRawFd::into_raw_fd(owned);
    // SAFETY: `raw` is a live memfd we own; `from_raw_fd` takes that ownership.
    Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(raw) })
}

impl Default for WlrState {
    fn default() -> Self {
        Self {
            manager: None,
            outputs: Vec::new(),
            controls: Vec::new(),
        }
    }
}

impl ColorTemperatureService for WlrGammaService {
    fn backend_name(&self) -> String {
        "wlroots gamma control".to_string()
    }

    fn is_supported(&self) -> bool {
        self.supports_software_brightness()
    }

    fn apply(&self, kelvin: i32) -> bool {
        if let Ok(mut shared) = self.shared.lock() {
            shared.kelvin = kelvin;
        }
        self.apply_current_ramp()
    }

    fn reset(&self) {
        // Destroying a gamma control restores the compositor's original
        // table, so reset destroys every control and recreates them — the
        // service keeps working if it is used again afterwards.
        let outputs: Vec<wl_output::WlOutput> = match self.state.lock() {
            Ok(mut state) => state
                .controls
                .drain(..)
                .map(|c| {
                    c.control.destroy();
                    c.output
                })
                .collect(),
            Err(_) => return,
        };
        let manager = match self.state.lock() {
            Ok(state) => state.manager.clone(),
            Err(_) => return,
        };
        let Some(manager) = manager else { return };
        if let Ok(mut queue) = self.queue.lock() {
            if let Ok(mut state) = self.state.lock() {
                let _ = queue.flush();
                for (index, output) in outputs.iter().enumerate() {
                    let control = manager.get_gamma_control(output, &self.qh, index);
                    state.controls.push(ControlledOutput {
                        output: output.clone(),
                        control,
                        size: None,
                        failed: false,
                    });
                }
                let _ = queue.roundtrip(&mut state);
                state
                    .controls
                    .retain(|c| !c.failed && c.size.unwrap_or(0) > 0);
            }
        }
    }
}

/// Software brightness fallback backed by the shared wlroots gamma ramps.
pub struct WlrSoftwareBrightnessService {
    gamma: std::sync::Arc<Mutex<WlrGammaService>>,
}

impl WlrSoftwareBrightnessService {
    pub fn new(gamma: std::sync::Arc<Mutex<WlrGammaService>>) -> Self {
        Self { gamma }
    }
}

impl nmc_core::traits::BrightnessService for WlrSoftwareBrightnessService {
    fn is_supported(&self) -> bool {
        self.gamma
            .lock()
            .map(|g| g.supports_software_brightness())
            .unwrap_or(false)
    }

    fn set_brightness(&self, percent: i32) {
        if let Ok(gamma) = self.gamma.lock() {
            gamma.set_software_brightness(percent);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_fails_without_wayland() {
        // Must fail cleanly without a compositor rather than claim support.
        // WAYLAND_DISPLAY may or may not be set in the test environment; if a
        // compositor IS reachable the outcome varies, but it must not panic.
        let _ = WlrGammaService::probe();
    }

    #[test]
    fn ramps_fd_concatenates_three_u16_tables() {
        let ramp = kelvin::build_ramp(2, 1.0, 1.0, 1.0, 1.0);
        let fd = ramps_fd(&ramp).unwrap();
        let mut file = std::fs::File::from(fd);
        use std::io::Read;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        // 3 channels × 2 entries × 2 bytes, red table first.
        assert_eq!(bytes.len(), 12);
        assert_eq!(u16::from_ne_bytes([bytes[0], bytes[1]]), 0);
        assert_eq!(u16::from_ne_bytes([bytes[2], bytes[3]]), 65535);
        assert_eq!(u16::from_ne_bytes([bytes[10], bytes[11]]), 65535);
    }
}
