//! DDC/CI hardware brightness over Linux `/dev/i2c-*`.
//!
//! Mirrors the macOS DDC services: every I2C bus is probed with the MCCS
//! luminance query, and only buses that answer take brightness writes — a
//! panel that does not answer must not be reported as brightness-capable.
//! Slider changes are coalesced through a 40 ms debounce into one I2C
//! transaction per bus.

use i2cdev::core::{I2CMessage, I2CTransfer};
use i2cdev::linux::{LinuxI2CBus, LinuxI2CError, LinuxI2CMessage};
use moonglow_core::ddc::{self, CHIP_ADDRESS, DEBOUNCE_DELAY, MAX_ATTEMPTS, WRITE_DELAY};
use moonglow_core::settings::AppSettings;
use moonglow_core::traits::BrightnessService;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};

/// How long the probe read reply is (MCCS get-VCP response).
const PROBE_REPLY_LEN: usize = 11;

/// Default manual brightness, matching `AppSettings` (kept literal: the value
/// is the contract, and core owns the constant).
const DEFAULT_BRIGHTNESS: i32 = 80;

/// Validates an MCCS luminance probe reply, mirroring the macOS Intel probe.
fn valid_probe_reply(response: &[u8]) -> bool {
    if response.len() != PROBE_REPLY_LEN {
        return false;
    }
    let expected_checksum = response[..PROBE_REPLY_LEN - 1]
        .iter()
        .fold(0x50u8, |acc, b| acc ^ b);
    response[2] == 0x02
        && response[3] == 0x00
        && response[4] == 0x10
        && response[PROBE_REPLY_LEN - 1] == expected_checksum
}

/// Outcome of probing one I2C bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeOutcome {
    /// The panel answered the luminance query.
    Answered,
    /// The bus opened but nothing answered (no monitor, DDC/CI off in the OSD).
    NoReply,
    /// The bus could not be opened for lack of permission (`/dev/i2c-*` is
    /// root-only without the udev rule + `i2c` group).
    AccessDenied,
}

/// Probes one I2C bus for a DDC/CI luminance response.
fn probe_bus(path: &Path) -> ProbeOutcome {
    let mut bus = match LinuxI2CBus::new(path) {
        Ok(bus) => bus,
        Err(e) => {
            // Root-only device nodes surface as EACCES/EPERM or an io permission
            // error; that case is actionable (i2c group + udev rule) and worth
            // naming instead of lumping in with "no reply".
            let denied = matches!(e, LinuxI2CError::Errno(13) | LinuxI2CError::Errno(1))
                || matches!(e, LinuxI2CError::Io(ref io) if io.kind() == std::io::ErrorKind::PermissionDenied);
            return if denied {
                ProbeOutcome::AccessDenied
            } else {
                ProbeOutcome::NoReply
            };
        }
    };
    let payload = ddc::linux_probe_payload();
    for _ in 0..MAX_ATTEMPTS {
        std::thread::sleep(WRITE_DELAY);
        let mut write = LinuxI2CMessage::write(&payload).with_address(CHIP_ADDRESS);
        if bus.transfer(std::slice::from_mut(&mut write)).is_err() {
            continue;
        }
        std::thread::sleep(WRITE_DELAY);
        let mut reply = [0u8; PROBE_REPLY_LEN];
        let mut read = LinuxI2CMessage::read(&mut reply).with_address(CHIP_ADDRESS);
        if bus.transfer(std::slice::from_mut(&mut read)).is_ok() && valid_probe_reply(&reply) {
            return ProbeOutcome::Answered;
        }
    }
    ProbeOutcome::NoReply
}

/// Explains an empty probe result, or `None` when there is nothing actionable
/// (a panel answered, or no I2C buses exist at all).
fn probe_note(buses: usize, denied: usize, answering: usize) -> Option<String> {
    if answering > 0 || buses == 0 {
        return None;
    }
    if denied > 0 {
        return Some(format!(
            "Found {buses} I2C bus(es) but cannot open them (permission denied). Join the i2c group and install the udev rule, then log out and back in."
        ));
    }
    Some(
        "No panel answered the DDC/CI probe. Check the monitor's DDC/CI setting and cable."
            .to_string(),
    )
}

fn write_brightness_once(bus: &mut LinuxI2CBus, percent: i32) -> bool {
    let payload = ddc::build_linux_write_payload(percent);
    // DDC/CI is lossy: a single dropped transaction should not leave the
    // panel at the old brightness.
    for _ in 0..MAX_ATTEMPTS {
        std::thread::sleep(WRITE_DELAY);
        let mut msg = LinuxI2CMessage::write(&payload).with_address(CHIP_ADDRESS);
        if bus.transfer(std::slice::from_mut(&mut msg)).is_ok() {
            return true;
        }
    }
    false
}

/// Lists candidate DDC buses under a device directory (normally `/dev`).
fn enumerate_buses(dev_dir: &Path) -> Vec<PathBuf> {
    let mut buses = Vec::new();
    let Ok(entries) = std::fs::read_dir(dev_dir) else {
        return buses;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("i2c-") && name[4..].chars().all(|c| c.is_ascii_digit()) {
            buses.push(entry.path());
        }
    }
    buses.sort();
    buses
}

/// DDC/CI brightness across every answering I2C bus.
pub struct LinuxDdcService {
    inner: Mutex<Inner>,
}

struct Inner {
    supported: bool,
    /// Why the probe came back empty, if anything actionable was found.
    note: Option<String>,
    /// Shared with the worker thread: the latest requested brightness.
    pending_brightness: Arc<Mutex<i32>>,
    notify_tx: Option<mpsc::Sender<()>>,
    worker: Option<std::thread::JoinHandle<()>>,
    disposed: bool,
}

impl LinuxDdcService {
    /// Probes every `/dev/i2c-*` bus. Construction never fails; when nothing
    /// answers, the service reports unsupported.
    pub fn probe() -> Self {
        Self::probe_under(Path::new("/dev"))
    }

    pub fn probe_under(dev_dir: &Path) -> Self {
        let buses = enumerate_buses(dev_dir);
        let mut answering = Vec::new();
        let mut denied = 0;
        for path in &buses {
            match probe_bus(path) {
                ProbeOutcome::Answered => answering.push(path.clone()),
                ProbeOutcome::AccessDenied => denied += 1,
                ProbeOutcome::NoReply => {}
            }
        }
        let note = probe_note(buses.len(), denied, answering.len());
        if let Some(note) = &note {
            eprintln!("brightness: DDC/CI unavailable ({note})");
        }
        Self::with_buses(answering, note)
    }

    fn with_buses(buses: Vec<PathBuf>, note: Option<String>) -> Self {
        let supported = !buses.is_empty();
        let (notify_tx, notify_rx) = mpsc::channel::<()>();
        // The worker owns its bus handles and applies the latest pending
        // value after the debounce window, draining coalesced slider events.
        let pending = Arc::new(Mutex::new(DEFAULT_BRIGHTNESS));
        let worker_pending = pending.clone();
        let worker = std::thread::Builder::new()
            .name("moonglow-ddc".to_string())
            .spawn(move || {
                let mut open: Vec<LinuxI2CBus> = buses
                    .iter()
                    .filter_map(|p| LinuxI2CBus::new(p).ok())
                    .collect();
                if open.is_empty() {
                    return;
                }
                while notify_rx.recv().is_ok() {
                    std::thread::sleep(DEBOUNCE_DELAY);
                    // Drain coalesced notifications; only the latest value matters.
                    while notify_rx.try_recv().is_ok() {}
                    let percent = worker_pending.lock().map(|p| *p).unwrap_or(80);
                    for bus in &mut open {
                        let _ = write_brightness_once(bus, percent);
                    }
                }
            })
            .ok();
        Self {
            inner: Mutex::new(Inner {
                supported: supported && worker.is_some(),
                note,
                pending_brightness: pending,
                notify_tx: Some(notify_tx),
                worker,
                disposed: false,
            }),
        }
    }

    /// Re-runs the bus probe, e.g. after the display set changed. Disposes
    /// the old worker first, then rebuilds; a no-op once disposed for quit.
    fn reprobe_under(&mut self, dev_dir: &Path) {
        if self.inner.lock().map(|i| i.disposed).unwrap_or(true) {
            return;
        }
        self.dispose();
        *self = Self::probe_under(dev_dir);
    }

    /// Test hook sharing the debounce/worker path without hardware. Not used
    /// in production; production always goes through [`LinuxDdcService::probe`].
    #[cfg(test)]
    fn unsupported() -> Self {
        let (notify_tx, notify_rx) = mpsc::channel::<()>();
        drop(notify_rx);
        Self {
            inner: Mutex::new(Inner {
                supported: false,
                note: None,
                pending_brightness: Arc::new(Mutex::new(DEFAULT_BRIGHTNESS)),
                notify_tx: Some(notify_tx),
                worker: None,
                disposed: false,
            }),
        }
    }
}

impl BrightnessService for LinuxDdcService {
    fn is_supported(&self) -> bool {
        self.inner
            .lock()
            .map(|i| i.supported && !i.disposed)
            .unwrap_or(false)
    }

    /// Queues a brightness write, coalescing rapid slider changes into one I2C
    /// transaction per bus.
    fn set_brightness(&self, percent: i32) {
        let tx = match self.inner.lock() {
            Ok(inner) => {
                if !inner.supported || inner.disposed {
                    return;
                }
                if let Ok(mut pending) = inner.pending_brightness.lock() {
                    *pending = AppSettings::clamp_brightness(percent);
                }
                inner.notify_tx.clone()
            }
            Err(_) => return,
        };
        let _ = tx.as_ref().map(|tx| tx.send(()));
    }

    fn status_detail(&self) -> Option<String> {
        self.inner
            .lock()
            .ok()
            .and_then(|i| if i.supported { None } else { i.note.clone() })
    }

    fn dispose(&mut self) {
        let mut worker = None;
        if let Ok(mut inner) = self.inner.lock() {
            if inner.disposed {
                return;
            }
            inner.disposed = true;
            inner.supported = false;
            inner.notify_tx.take();
            worker = inner.worker.take();
        }
        if let Some(handle) = worker {
            let _ = handle.join();
        }
    }

    fn reprobe(&mut self) {
        self.reprobe_under(Path::new("/dev"));
    }

    fn needs_reprobe(&self) -> bool {
        self.inner
            .lock()
            .map(|i| !i.supported && !i.disposed)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed_probe_replies() {
        assert!(!valid_probe_reply(&[]));
        assert!(!valid_probe_reply(&[0u8; 11]));
        // Correct shape but wrong checksum.
        let mut reply = [0u8; 11];
        reply[2] = 0x02;
        reply[3] = 0x00;
        reply[4] = 0x10;
        assert!(!valid_probe_reply(&reply));
        // Fix the checksum: accepted.
        let cks = reply[..10].iter().fold(0x50u8, |acc, b| acc ^ b);
        reply[10] = cks;
        assert!(valid_probe_reply(&reply));
    }

    #[test]
    fn enumerates_i2c_devices_only() {
        let dir = std::env::temp_dir().join(format!(
            "moonglow-i2c-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["i2c-0", "i2c-12", "ttyS0", "i2c-x"] {
            std::fs::write(dir.join(name), []).unwrap();
        }
        let buses = enumerate_buses(&dir);
        assert_eq!(buses.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_bus_dir_is_unsupported() {
        let service = LinuxDdcService::probe_under(Path::new("/nonexistent-dev"));
        assert!(!service.is_supported());
        // Setting brightness while unsupported is a silent no-op.
        service.set_brightness(50);
    }

    #[test]
    fn reprobe_without_hardware_stays_unsupported() {
        let mut service = LinuxDdcService::unsupported();
        service.reprobe_under(Path::new("/nonexistent-dev"));
        assert!(!service.is_supported());
        // Still safe to drive afterwards.
        service.set_brightness(50);
        // And never resurrects a disposed service.
        service.dispose();
        service.reprobe_under(Path::new("/nonexistent-dev"));
        assert!(!service.is_supported());
    }

    #[test]
    fn probe_note_names_permission_denied() {
        assert_eq!(probe_note(0, 0, 0), None);
        assert_eq!(probe_note(5, 0, 2), None);
        let denied = probe_note(5, 5, 0).unwrap();
        assert!(denied.contains("permission denied"), "{denied}");
        assert!(denied.contains("i2c group"), "{denied}");
        let silent = probe_note(2, 0, 0).unwrap();
        assert!(silent.contains("DDC/CI"), "{silent}");
    }

    #[test]
    fn dispose_is_idempotent() {
        let mut service = LinuxDdcService::unsupported();
        service.dispose();
        service.dispose();
        assert!(!service.is_supported());
    }
}
