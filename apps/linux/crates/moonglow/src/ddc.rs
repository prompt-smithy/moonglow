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
use std::collections::{HashMap, HashSet};
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
    /// DRM connector names whose sysfs `ddc` link points at an answering I²C bus.
    supported_connectors: HashSet<String>,
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
        Self::probe_under_with_drm_dir(dev_dir, Path::new("/sys/class/drm"))
    }

    fn probe_under_with_drm_dir(dev_dir: &Path, drm_dir: &Path) -> Self {
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
        let edids = answering
            .iter()
            .filter_map(|bus| read_bus_edid(bus))
            .collect();
        let supported_connectors = connectors_for_answering_buses(drm_dir, &answering, &edids);
        Self::with_buses(answering, note, supported_connectors)
    }

    fn with_buses(
        buses: Vec<PathBuf>,
        note: Option<String>,
        supported_connectors: HashSet<String>,
    ) -> Self {
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
                supported_connectors,
                note,
                pending_brightness: pending,
                notify_tx: Some(notify_tx),
                worker,
                disposed: false,
            }),
        }
    }

    /// DRM connector names backed by a DDC bus that answered the luminance probe.
    pub fn supported_connectors(&self) -> HashSet<String> {
        self.inner
            .lock()
            .map(|inner| {
                if inner.supported {
                    inner.supported_connectors.clone()
                } else {
                    HashSet::new()
                }
            })
            .unwrap_or_default()
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
                supported_connectors: HashSet::new(),
                note: None,
                pending_brightness: Arc::new(Mutex::new(DEFAULT_BRIGHTNESS)),
                notify_tx: Some(notify_tx),
                worker: None,
                disposed: false,
            }),
        }
    }
}

/// Stable monitor identity shared by sysfs, I²C EDID and RandR. Reject malformed
/// EDIDs rather than allowing empty/default identities to claim unrelated outputs.
pub(crate) fn edid_identity(edid: &[u8]) -> Option<String> {
    if edid.len() < 128
        || edid[..8] != [0, 255, 255, 255, 255, 255, 255, 0]
        || edid[..128].iter().fold(0u8, |sum, b| sum.wrapping_add(*b)) != 0
    {
        return None;
    }
    Some(format!(
        "edid-{}",
        edid[8..18]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ))
}

fn read_bus_edid(path: &Path) -> Option<String> {
    let mut bus = LinuxI2CBus::new(path).ok()?;
    let offset = [0u8];
    let mut edid = [0u8; 128];
    let mut messages = [
        LinuxI2CMessage::write(&offset).with_address(0x50),
        LinuxI2CMessage::read(&mut edid).with_address(0x50),
    ];
    bus.transfer(&mut messages).ok()?;
    edid_identity(&edid)
}

/// Prefer monitor identity to connector spellings: NVIDIA, modesetting and
/// proprietary X11 drivers use different names for the same physical output.
fn connectors_for_answering_buses(
    drm_dir: &Path,
    buses: &[PathBuf],
    edids: &HashSet<String>,
) -> HashSet<String> {
    let answering: HashSet<String> = buses
        .iter()
        .filter_map(|path| path.file_name()?.to_str().map(str::to_owned))
        .collect();
    let mut connectors = edids.clone();
    if answering.is_empty() {
        return connectors;
    }
    let Ok(entries) = std::fs::read_dir(drm_dir) else {
        return connectors;
    };
    let externals: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let (_, suffix) = name.split_once('-')?;
            if !name.starts_with("card")
                || suffix.starts_with("eDP")
                || suffix.starts_with("LVDS")
                || std::fs::read_to_string(entry.path().join("status"))
                    .ok()?
                    .trim()
                    != "connected"
                || std::fs::read_to_string(entry.path().join("enabled"))
                    .is_ok_and(|v| v.trim() == "disabled")
            {
                return None;
            }
            Some((name.clone(), suffix.to_owned(), entry.path()))
        })
        .collect();
    let mut suffix_counts = HashMap::<String, usize>::new();
    for (_, suffix, _) in &externals {
        *suffix_counts.entry(suffix.clone()).or_default() += 1;
    }
    let mut matched = Vec::new();
    for (name, suffix, path) in &externals {
        let edid = std::fs::read(path.join("edid"))
            .ok()
            .and_then(|bytes| edid_identity(&bytes));
        let target = std::fs::canonicalize(path.join("ddc"))
            .ok()
            .or_else(|| std::fs::read_link(path.join("ddc")).ok());
        let bus = target.as_ref().and_then(|target| {
            target
                .components()
                .filter_map(|c| c.as_os_str().to_str())
                .rfind(|c| c.starts_with("i2c-") && c[4..].chars().all(|c| c.is_ascii_digit()))
        });
        if edid.as_ref().is_some_and(|key| edids.contains(key))
            || bus.is_some_and(|bus| answering.contains(bus))
        {
            matched.push((name, suffix, edid));
        }
    }
    // Some drivers expose neither a DDC link nor readable EDID. Only infer
    // complete external coverage when the two counts agree and nothing mapped.
    if matched.is_empty() && externals.len() == answering.len() {
        matched = externals
            .iter()
            .map(|(name, suffix, path)| {
                (
                    name,
                    suffix,
                    std::fs::read(path.join("edid"))
                        .ok()
                        .and_then(|b| edid_identity(&b)),
                )
            })
            .collect();
    }
    if !externals.is_empty() && matched.len() == externals.len() {
        connectors.insert(format!("all-external-outputs:{}", externals.len()));
    }
    for (name, suffix, edid) in matched {
        connectors.insert(name.clone());
        if let Some(edid) = edid {
            connectors.insert(edid);
        }
        if suffix_counts.get(suffix) == Some(&1) {
            connectors.insert(suffix.clone());
        }
    }
    connectors
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

    fn fake_edid(serial: u8) -> Vec<u8> {
        let mut edid = vec![0; 128];
        edid[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
        edid[8..12].copy_from_slice(&[0x12, 0x34, 0x56, 0x78]);
        edid[12] = serial;
        edid[127] = 0u8.wrapping_sub(edid[..127].iter().fold(0u8, |sum, b| sum.wrapping_add(*b)));
        edid
    }

    #[test]
    fn edid_maps_without_ddc_links_and_distinguishes_duplicate_suffixes() {
        let dir = std::env::temp_dir().join(format!("moonglow-edid-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (name, serial) in [("card0-DP-1", 1), ("card1-DP-1", 2)] {
            let path = dir.join(name);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("status"), "connected").unwrap();
            std::fs::write(path.join("edid"), fake_edid(serial)).unwrap();
        }
        let identity = edid_identity(&fake_edid(2)).unwrap();
        let capabilities = connectors_for_answering_buses(
            &dir,
            &[PathBuf::from("/dev/i2c-7")],
            &HashSet::from([identity.clone()]),
        );
        assert!(capabilities.contains("card1-DP-1"));
        assert!(capabilities.contains(&identity));
        assert!(!capabilities.contains("card0-DP-1"));
        assert!(!capabilities.contains("DP-1"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_ddc_and_edid_use_only_complete_external_coverage() {
        let dir =
            std::env::temp_dir().join(format!("moonglow-ddc-fallback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("card0-HDMI-A-1");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("status"), "connected").unwrap();
        let capabilities =
            connectors_for_answering_buses(&dir, &[PathBuf::from("/dev/i2c-7")], &HashSet::new());
        assert!(capabilities.contains("card0-HDMI-A-1"));
        std::fs::write(path.join("enabled"), "disabled").unwrap();
        assert!(connectors_for_answering_buses(
            &dir,
            &[PathBuf::from("/dev/i2c-7")],
            &HashSet::new()
        )
        .is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn muxed_ddc_link_uses_the_last_i2c_adapter() {
        let dir = std::env::temp_dir().join(format!("moonglow-ddc-mux-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for name in ["card0-DP-1", "card0-DP-2"] {
            let path = dir.join(name);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("status"), "connected").unwrap();
        }
        std::os::unix::fs::symlink("../../i2c-1/i2c-7", dir.join("card0-DP-1/ddc")).unwrap();
        let capabilities =
            connectors_for_answering_buses(&dir, &[PathBuf::from("/dev/i2c-7")], &HashSet::new());
        assert!(capabilities.contains("card0-DP-1"));
        assert!(!capabilities.contains("card0-DP-2"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
