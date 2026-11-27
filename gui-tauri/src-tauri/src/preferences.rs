use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use zbus::blocking::{Connection as DbusConnection, MessageIterator};
use zbus::zvariant::{OwnedValue, Value};
use zbus::MatchRule;

pub const PREFERENCES_UPDATED_EVENT: &str = "preferences-updated";

const SCHEMA_MUTTER: &str = "org.gnome.mutter";
const KEY_OVERLAY_KEY: &str = "overlay-key";

const SCHEMA_TOUCHPAD: &str = "org.gnome.desktop.peripherals.touchpad";
const KEY_SEND_EVENTS: &str = "send-events";

const SCHEMA_PRIVACY: &str = "org.gnome.desktop.privacy";
const KEY_DISABLE_CAMERA: &str = "disable-camera";

const RFKILL_DEST: &str = "org.gnome.SettingsDaemon.Rfkill";
const RFKILL_PATH: &str = "/org/gnome/SettingsDaemon/Rfkill";
const RFKILL_IFACE: &str = "org.gnome.SettingsDaemon.Rfkill";
const DBUS_PROPS_IFACE: &str = "org.freedesktop.DBus.Properties";

const ACER_IO_PATH: &str = "/dev/acer_io";
const UINPUT_PATH: &str = "/dev/uinput";

const IOCTL_MAGIC: libc::c_ulong = 0xEC;
const MAGIC_READ_CL: libc::c_ulong = IOCTL_MAGIC + 1;
const MAGIC_WRITE_CL: libc::c_ulong = IOCTL_MAGIC + 2;

const fn ioctl_ior(ty: libc::c_ulong, nr: libc::c_ulong, size: usize) -> libc::c_ulong {
    (2 << 30) | ((size as libc::c_ulong) << 16) | (ty << 8) | nr
}

const fn ioctl_iow(ty: libc::c_ulong, nr: libc::c_ulong, size: usize) -> libc::c_ulong {
    (1 << 30) | ((size as libc::c_ulong) << 16) | (ty << 8) | nr
}

// Match driver/include/acer_io_ioctl.h (_IOR/_IOW use int32_t* pointer size on LP64)
const PTR_ARG_SIZE: usize = std::mem::size_of::<*const i32>();
const R_CL_WEBCAM_SW: libc::c_ulong = ioctl_ior(MAGIC_READ_CL, 0x13, PTR_ARG_SIZE);
const R_CL_TOUCHPAD_SW: libc::c_ulong = ioctl_ior(MAGIC_READ_CL, 0x15, PTR_ARG_SIZE);
const W_CL_WEBCAM_SW: libc::c_ulong = ioctl_iow(MAGIC_WRITE_CL, 0x12, PTR_ARG_SIZE);
const W_CL_TOUCHPAD_SW: libc::c_ulong = ioctl_iow(MAGIC_WRITE_CL, 0x14, PTR_ARG_SIZE);

// Linux input / uinput constants (x86_64)
const UI_SET_EVBIT: libc::c_ulong = 0x4004_5564;
const UI_SET_KEYBIT: libc::c_ulong = 0x4004_5565;
const UI_DEV_SETUP: libc::c_ulong = 0x405c_5503;
const UI_DEV_CREATE: libc::c_ulong = 0x5501;
const UI_DEV_DESTROY: libc::c_ulong = 0x5502;

const BUS_VIRTUAL: u16 = 0x06;
const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const SYN_REPORT: u16 = 0x00;

pub const KEY_CAPSLOCK: u16 = 58;
pub const KEY_NUMLOCK: u16 = 69;

const UINPUT_WARMUP_MS: u64 = 1200;

#[repr(C)]
struct UinputSetup {
    id_bustype: u16,
    id_vendor: u16,
    id_product: u16,
    id_version: u16,
    name: [u8; 80],
    ff_effects_max: u32,
}

#[repr(C)]
struct InputEvent {
    tv_sec: i64,
    tv_usec: i64,
    type_: u16,
    code: u16,
    value: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceControlState {
    pub id: String,
    pub label: String,
    pub supported: bool,
    pub active: Option<bool>,
    pub source: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesState {
    pub items: Vec<PreferenceControlState>,
    pub sampled_at_ms: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn parse_gsettings_string(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('\'') && trimmed.ends_with('\'') {
        trimmed[1..trimmed.len() - 1].replace("\\'", "'")
    } else {
        trimmed.to_owned()
    }
}

pub fn parse_gsettings_bool(raw: &str) -> Result<bool, String> {
    match raw.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("Unexpected GSettings boolean output: {other}")),
    }
}

fn gsettings_get(schema: &str, key: &str) -> Result<String, String> {
    let out = Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .map_err(|e| format!("Failed to run gsettings get {schema} {key}: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "gsettings get {schema} {key} failed ({}): {}",
            out.status,
            stderr.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn gsettings_set(schema: &str, key: &str, value: &str) -> Result<(), String> {
    let out = Command::new("gsettings")
        .args(["set", schema, key, value])
        .output()
        .map_err(|e| format!("Failed to run gsettings set {schema} {key}: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "gsettings set {schema} {key} {value} failed ({}): {}",
            out.status,
            stderr.trim()
        ));
    }
    Ok(())
}

fn acer_io_read_switch(request: libc::c_ulong) -> Result<bool, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(ACER_IO_PATH)
        .map_err(|e| format!("Cannot open {ACER_IO_PATH}: {e}"))?;
    let mut buf: i64 = 0;
    let rc = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            request as libc::Ioctl,
            &mut buf as *mut i64,
        )
    };
    if rc < 0 {
        return Err(format!(
            "acer_io read ioctl 0x{request:x} failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok((buf as i32) != 0)
}

fn acer_io_write_switch(request: libc::c_ulong, enabled: bool) -> Result<(), String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(ACER_IO_PATH)
        .map_err(|e| format!("Cannot open {ACER_IO_PATH}: {e}"))?;
    let mut arg: i64 = if enabled { 1 } else { 0 };
    let rc = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            request as libc::Ioctl,
            &mut arg as *mut i64,
        )
    };
    if rc < 0 {
        return Err(format!(
            "acer_io write ioctl 0x{request:x} failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn parse_input_led_rank(name: &str) -> (u32, &str) {
    if let Some(rest) = name.strip_prefix("input") {
        if let Some((num_part, _)) = rest.split_once("::") {
            if let Ok(n) = num_part.parse::<u32>() {
                return (n, name);
            }
        }
    }
    (u32::MAX, name)
}

fn find_lock_led_path_in(leds_dir: &Path, suffix: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(leds_dir).ok()?;
    let mut candidates: Vec<(u32, String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(suffix) {
            continue;
        }
        let brightness = entry.path().join("brightness");
        if brightness.is_file() {
            let (rank, _) = parse_input_led_rank(&name);
            candidates.push((rank, name, brightness));
        }
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    candidates.into_iter().next().map(|(_, _, p)| p)
}

fn read_led_state(suffix: &str) -> Option<bool> {
    let path = find_lock_led_path_in(Path::new("/sys/class/leds"), suffix)?;
    let raw = fs::read_to_string(path).ok()?;
    match raw.trim() {
        "0" => Some(false),
        other => other.parse::<u32>().ok().map(|v| v > 0),
    }
}

fn uinput_writable() -> bool {
    OpenOptions::new().write(true).open(UINPUT_PATH).is_ok()
}

struct LockKeyRequest {
    keycode: u16,
    reply: mpsc::Sender<Result<(), String>>,
}

struct UinputLockWorker {
    tx: Option<mpsc::Sender<LockKeyRequest>>,
    shutdown: Arc<AtomicBool>,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
}

impl UinputLockWorker {
    fn new_live() -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        if !uinput_writable() {
            return Self {
                tx: None,
                shutdown,
                handle: Mutex::new(None),
            };
        }

        let (tx, rx) = mpsc::channel::<LockKeyRequest>();
        let shutdown_worker = Arc::clone(&shutdown);
        let join = thread::Builder::new()
            .name("uxf-uinput-locks".into())
            .spawn(move || Self::run_worker(rx, shutdown_worker))
            .ok();

        Self {
            tx: if join.is_some() { Some(tx) } else { None },
            shutdown,
            handle: Mutex::new(join),
        }
    }

    #[cfg(test)]
    fn new_headless() -> Self {
        Self {
            tx: None,
            shutdown: Arc::new(AtomicBool::new(true)),
            handle: Mutex::new(None),
        }
    }

    fn is_available(&self) -> bool {
        self.tx.is_some()
    }

    fn tap_key(&self, keycode: u16) -> Result<(), String> {
        let Some(tx) = &self.tx else {
            return Err("Virtual input device (/dev/uinput) is unavailable for key injection".into());
        };
        let (reply_tx, reply_rx) = mpsc::channel();
        tx.send(LockKeyRequest {
            keycode,
            reply: reply_tx,
        })
        .map_err(|_| "Virtual lock-key worker has stopped".to_owned())?;
        reply_rx
            .recv_timeout(Duration::from_secs(4))
            .map_err(|e| format!("Timed out waiting for virtual lock key injection: {e}"))?
    }

    fn run_worker(rx: mpsc::Receiver<LockKeyRequest>, shutdown: Arc<AtomicBool>) {
        let mut dev: Option<UinputVirtualKbd> = match UinputVirtualKbd::open() {
            Ok(d) => Some(d),
            Err(err) => {
                eprintln!("uinput lock worker init failed: {err}");
                None
            }
        };

        while !shutdown.load(Ordering::Relaxed) {
            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(req) => {
                    if dev.is_none() {
                        dev = UinputVirtualKbd::open().ok();
                    }
                    let res = match dev.as_mut() {
                        Some(kbd) => kbd.tap(req.keycode),
                        None => Err("Failed to initialize /dev/uinput keyboard device".into()),
                    };
                    let _ = req.reply.send(res);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        drop(dev);
    }
}

impl Drop for UinputLockWorker {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.tx.take();
        if let Ok(mut guard) = self.handle.lock() {
            if let Some(join) = guard.take() {
                let _ = join.join();
            }
        }
    }
}

struct UinputVirtualKbd {
    file: File,
    ready_at: Instant,
}

impl UinputVirtualKbd {
    fn open() -> Result<Self, String> {
        let file = OpenOptions::new()
            .write(true)
            .open(UINPUT_PATH)
            .map_err(|e| format!("Failed to open {UINPUT_PATH}: {e}"))?;
        let fd = file.as_raw_fd();

        unsafe {
            if libc::ioctl(fd, UI_SET_EVBIT as libc::Ioctl, EV_KEY as libc::c_int) < 0 {
                return Err(format!(
                    "UI_SET_EVBIT failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            for code in [KEY_CAPSLOCK, KEY_NUMLOCK] {
                if libc::ioctl(fd, UI_SET_KEYBIT as libc::Ioctl, code as libc::c_int) < 0 {
                    return Err(format!(
                        "UI_SET_KEYBIT({code}) failed: {}",
                        std::io::Error::last_os_error()
                    ));
                }
            }

            let mut setup = UinputSetup {
                id_bustype: BUS_VIRTUAL,
                id_vendor: 0x1d6b,
                id_product: 0x0104,
                id_version: 1,
                name: [0u8; 80],
                ff_effects_max: 0,
            };
            let label = b"uxf-lock-key-helper";
            setup.name[..label.len()].copy_from_slice(label);

            if libc::ioctl(fd, UI_DEV_SETUP as libc::Ioctl, &setup as *const UinputSetup) < 0 {
                return Err(format!(
                    "UI_DEV_SETUP failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            if libc::ioctl(fd, UI_DEV_CREATE as libc::Ioctl) < 0 {
                return Err(format!(
                    "UI_DEV_CREATE failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }

        Ok(Self {
            file,
            ready_at: Instant::now() + Duration::from_millis(UINPUT_WARMUP_MS),
        })
    }

    fn write_event(&mut self, type_: u16, code: u16, value: i32) -> Result<(), String> {
        let ev = InputEvent {
            tv_sec: 0,
            tv_usec: 0,
            type_,
            code,
            value,
        };
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (&ev as *const InputEvent).cast::<u8>(),
                std::mem::size_of::<InputEvent>(),
            )
        };
        self.file
            .write_all(bytes)
            .map_err(|e| format!("uinput write failed: {e}"))
    }

    fn tap(&mut self, keycode: u16) -> Result<(), String> {
        let now = Instant::now();
        if now < self.ready_at {
            thread::sleep(self.ready_at - now);
        }
        self.write_event(EV_KEY, keycode, 1)?;
        self.write_event(EV_SYN, SYN_REPORT, 0)?;
        thread::sleep(Duration::from_millis(40));
        self.write_event(EV_KEY, keycode, 0)?;
        self.write_event(EV_SYN, SYN_REPORT, 0)?;
        self.file
            .flush()
            .map_err(|e| format!("uinput flush failed: {e}"))?;
        thread::sleep(Duration::from_millis(120));
        Ok(())
    }
}

impl Drop for UinputVirtualKbd {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::ioctl(self.file.as_raw_fd(), UI_DEV_DESTROY as libc::Ioctl);
        }
    }
}

pub struct PreferencesManager {
    saved_overlay_key: Mutex<String>,
    action_lock: Mutex<()>,
    uinput_worker: UinputLockWorker,
    cached_state: Mutex<Option<PreferencesState>>,
    shutdown: Arc<AtomicBool>,
}

impl PreferencesManager {
    pub fn new() -> Self {
        Self {
            saved_overlay_key: Mutex::new("Super_L".to_owned()),
            action_lock: Mutex::new(()),
            uinput_worker: UinputLockWorker::new_live(),
            cached_state: Mutex::new(None),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    #[cfg(test)]
    pub fn new_headless() -> Self {
        Self {
            saved_overlay_key: Mutex::new("Super_L".to_owned()),
            action_lock: Mutex::new(()),
            uinput_worker: UinputLockWorker::new_headless(),
            cached_state: Mutex::new(None),
            shutdown: Arc::new(AtomicBool::new(true)),
        }
    }

    fn read_overlay_key_raw() -> Result<String, String> {
        gsettings_get(SCHEMA_MUTTER, KEY_OVERLAY_KEY)
    }

    fn read_super_lock(&self) -> PreferenceControlState {
        match Self::read_overlay_key_raw() {
            Ok(raw) => {
                let current = parse_gsettings_string(&raw);
                if !current.is_empty() {
                    if let Ok(mut guard) = self.saved_overlay_key.lock() {
                        *guard = current.clone();
                    }
                }
                PreferenceControlState {
                    id: "super-lock".into(),
                    label: "Super Lock".into(),
                    supported: true,
                    active: Some(current.is_empty()),
                    source: "GNOME Mutter overlay-key".into(),
                    reason: Some(
                        "Locks or unlocks the single Super key GNOME Overview trigger via org.gnome.mutter overlay-key."
                            .into(),
                    ),
                }
            }
            Err(err) => PreferenceControlState {
                id: "super-lock".into(),
                label: "Super Lock".into(),
                supported: false,
                active: None,
                source: "GNOME Mutter overlay-key".into(),
                reason: Some(err),
            },
        }
    }

    fn set_super_lock(&self, enabled: bool) -> Result<(), String> {
        if enabled {
            if let Ok(raw) = Self::read_overlay_key_raw() {
                let current = parse_gsettings_string(&raw);
                if !current.is_empty() {
                    if let Ok(mut guard) = self.saved_overlay_key.lock() {
                        *guard = current;
                    }
                }
            }
            gsettings_set(SCHEMA_MUTTER, KEY_OVERLAY_KEY, "")
        } else {
            let restore = self
                .saved_overlay_key
                .lock()
                .map(|g| g.clone())
                .unwrap_or_else(|_| "Super_L".to_owned());
            let target = if restore.is_empty() {
                "Super_L".to_owned()
            } else {
                restore
            };
            gsettings_set(SCHEMA_MUTTER, KEY_OVERLAY_KEY, &target)
        }
    }

    fn read_touchpad(&self) -> PreferenceControlState {
        let gsettings_state = gsettings_get(SCHEMA_TOUCHPAD, KEY_SEND_EVENTS)
            .map(|s| parse_gsettings_string(&s));
        let hw_state = acer_io_read_switch(R_CL_TOUCHPAD_SW).ok();

        match gsettings_state {
            Ok(mode) => {
                let desktop_enabled = mode != "disabled";
                let active = desktop_enabled && hw_state.unwrap_or(true);
                PreferenceControlState {
                    id: "touchpad".into(),
                    label: "Touchpad".into(),
                    supported: true,
                    active: Some(active),
                    source: "GNOME Peripherals Touchpad".into(),
                    reason: Some(format!(
                        "Toggles touchpad input via org.gnome.desktop.peripherals.touchpad send-events (current: {mode})."
                    )),
                }
            }
            Err(err) => PreferenceControlState {
                id: "touchpad".into(),
                label: "Touchpad".into(),
                supported: false,
                active: hw_state,
                source: "GNOME Peripherals Touchpad".into(),
                reason: Some(err),
            },
        }
    }

    fn set_touchpad(&self, enabled: bool) -> Result<(), String> {
        if enabled {
            if matches!(acer_io_read_switch(R_CL_TOUCHPAD_SW), Ok(false)) {
                let _ = acer_io_write_switch(W_CL_TOUCHPAD_SW, true);
            }
            gsettings_set(SCHEMA_TOUCHPAD, KEY_SEND_EVENTS, "enabled")
        } else {
            gsettings_set(SCHEMA_TOUCHPAD, KEY_SEND_EVENTS, "disabled")
        }
    }

    fn read_camera(&self) -> PreferenceControlState {
        let hw_res = acer_io_read_switch(R_CL_WEBCAM_SW);
        let gnome_disabled = gsettings_get(SCHEMA_PRIVACY, KEY_DISABLE_CAMERA)
            .and_then(|s| parse_gsettings_bool(&s));

        match (hw_res, gnome_disabled) {
            (Ok(hw_on), Ok(disabled)) => PreferenceControlState {
                id: "camera".into(),
                label: "Camera".into(),
                supported: true,
                active: Some(hw_on && !disabled),
                source: "/dev/acer_io + GNOME Privacy".into(),
                reason: Some(if hw_on && disabled {
                    "Camera hardware is on, but GNOME Privacy disable-camera is active."
                        .into()
                } else if hw_on {
                    "Camera enabled via Acer EC USB switch and GNOME Privacy."
                        .into()
                } else {
                    "Camera hardware USB power switch is off via /dev/acer_io."
                        .into()
                }),
            },
            (Ok(hw_on), Err(_)) => PreferenceControlState {
                id: "camera".into(),
                label: "Camera".into(),
                supported: true,
                active: Some(hw_on),
                source: "/dev/acer_io WEBCAM_SW".into(),
                reason: Some(
                    "Controls internal camera hardware switch via /dev/acer_io.".into(),
                ),
            },
            (Err(_), Ok(disabled)) => PreferenceControlState {
                id: "camera".into(),
                label: "Camera".into(),
                supported: true,
                active: Some(!disabled),
                source: "GNOME Privacy disable-camera".into(),
                reason: Some(
                    "Controls camera availability via org.gnome.desktop.privacy disable-camera."
                        .into(),
                ),
            },
            (Err(hw_err), Err(gs_err)) => PreferenceControlState {
                id: "camera".into(),
                label: "Camera".into(),
                supported: false,
                active: None,
                source: "Unavailable".into(),
                reason: Some(format!("Camera control unavailable ({hw_err}; {gs_err})")),
            },
        }
    }

    fn set_camera(&self, enabled: bool) -> Result<(), String> {
        let hw_available = acer_io_read_switch(R_CL_WEBCAM_SW).is_ok();
        if enabled {
            if matches!(
                gsettings_get(SCHEMA_PRIVACY, KEY_DISABLE_CAMERA)
                    .and_then(|s| parse_gsettings_bool(&s)),
                Ok(true)
            ) {
                gsettings_set(SCHEMA_PRIVACY, KEY_DISABLE_CAMERA, "false")?;
            }
            if hw_available {
                acer_io_write_switch(W_CL_WEBCAM_SW, true)?;
                thread::sleep(Duration::from_millis(120));
            }
            Ok(())
        } else if hw_available {
            acer_io_write_switch(W_CL_WEBCAM_SW, false)?;
            thread::sleep(Duration::from_millis(120));
            Ok(())
        } else {
            gsettings_set(SCHEMA_PRIVACY, KEY_DISABLE_CAMERA, "true")
        }
    }

    fn rfkill_get_prop(conn: &DbusConnection, prop: &str) -> Result<bool, String> {
        let msg = conn
            .call_method(
                Some(RFKILL_DEST),
                RFKILL_PATH,
                Some(DBUS_PROPS_IFACE),
                "Get",
                &(RFKILL_IFACE, prop),
            )
            .map_err(|e| format!("D-Bus Get {RFKILL_IFACE}.{prop} failed: {e}"))?;
        let val: OwnedValue = msg
            .body()
            .deserialize()
            .map_err(|e| format!("Invalid D-Bus property {prop}: {e}"))?;
        bool::try_from(val).map_err(|e| format!("Property {prop} is not boolean: {e}"))
    }

    fn rfkill_set_airplane(conn: &DbusConnection, enabled: bool) -> Result<(), String> {
        let val = Value::from(enabled);
        conn.call_method(
            Some(RFKILL_DEST),
            RFKILL_PATH,
            Some(DBUS_PROPS_IFACE),
            "Set",
            &(RFKILL_IFACE, "AirplaneMode", val),
        )
        .map_err(|e| format!("D-Bus Set {RFKILL_IFACE}.AirplaneMode failed: {e}"))?;
        Ok(())
    }

    fn read_airplane(&self) -> PreferenceControlState {
        let conn = match DbusConnection::session() {
            Ok(c) => c,
            Err(e) => {
                return PreferenceControlState {
                    id: "airplane".into(),
                    label: "Airplane".into(),
                    supported: false,
                    active: None,
                    source: RFKILL_IFACE.into(),
                    reason: Some(format!("Session D-Bus unavailable: {e}")),
                }
            }
        };

        let has_airplane = Self::rfkill_get_prop(&conn, "HasAirplaneMode").unwrap_or(false);
        let hw_locked = Self::rfkill_get_prop(&conn, "HardwareAirplaneMode").unwrap_or(false);
        let active = Self::rfkill_get_prop(&conn, "AirplaneMode");

        match active {
            Ok(is_on) if has_airplane => PreferenceControlState {
                id: "airplane".into(),
                label: "Airplane".into(),
                supported: !hw_locked || !is_on,
                active: Some(is_on),
                source: RFKILL_IFACE.into(),
                reason: Some(if hw_locked {
                    "Hardware wireless kill switch is active and overrides software Airplane Mode."
                        .into()
                } else {
                    "Controls wireless rfkill radios via GNOME Settings Daemon Rfkill D-Bus."
                        .into()
                }),
            },
            Ok(_) => PreferenceControlState {
                id: "airplane".into(),
                label: "Airplane".into(),
                supported: false,
                active: None,
                source: RFKILL_IFACE.into(),
                reason: Some("GNOME Rfkill reports no airplane-mode capable radios.".into()),
            },
            Err(err) => PreferenceControlState {
                id: "airplane".into(),
                label: "Airplane".into(),
                supported: false,
                active: None,
                source: RFKILL_IFACE.into(),
                reason: Some(err),
            },
        }
    }

    fn set_airplane(&self, enabled: bool) -> Result<(), String> {
        let conn =
            DbusConnection::session().map_err(|e| format!("Session D-Bus unavailable: {e}"))?;
        if !Self::rfkill_get_prop(&conn, "HasAirplaneMode").unwrap_or(false) {
            return Err("System does not report Airplane Mode radio control support.".into());
        }
        if !enabled && Self::rfkill_get_prop(&conn, "HardwareAirplaneMode").unwrap_or(false) {
            return Err("Hardware rfkill switch is active; release the hardware switch first.".into());
        }
        Self::rfkill_set_airplane(&conn, enabled)
    }

    fn read_lock_led(&self, id: &str, label: &str, suffix: &str) -> PreferenceControlState {
        let state = read_led_state(suffix);
        let writable = self.uinput_worker.is_available();
        match (state, writable) {
            (Some(active), true) => PreferenceControlState {
                id: id.into(),
                label: label.into(),
                supported: true,
                active: Some(active),
                source: format!("kernel LED ({suffix}) + /dev/uinput"),
                reason: Some(format!(
                    "Reads live keyboard {label} LED and toggles state via virtual Wayland input event."
                )),
            },
            (Some(active), false) => PreferenceControlState {
                id: id.into(),
                label: label.into(),
                supported: false,
                active: Some(active),
                source: format!("kernel LED ({suffix}, read-only)"),
                reason: Some(format!(
                    "Reads live {label} LED, but toggling requires /dev/uinput write permission."
                )),
            },
            (None, _) => PreferenceControlState {
                id: id.into(),
                label: label.into(),
                supported: false,
                active: None,
                source: "kernel LED sysfs".into(),
                reason: Some(format!(
                    "No readable kernel LED device found matching *{suffix}."
                )),
            },
        }
    }

    fn set_lock_led(&self, suffix: &str, keycode: u16, label: &str, target: bool) -> Result<(), String> {
        let current = read_led_state(suffix)
            .ok_or_else(|| format!("Cannot read current {label} LED state from sysfs"))?;
        if current == target {
            return Ok(());
        }
        self.uinput_worker.tap_key(keycode)?;
        // Verify resulting LED state cleanly without faking success
        for _ in 0..8 {
            if read_led_state(suffix) == Some(target) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err(format!(
            "{label} state did not transition to {} after key event on Wayland",
            if target { "ON" } else { "OFF" }
        ))
    }

    fn read_scroll_lock(&self) -> PreferenceControlState {
        PreferenceControlState {
            id: "scr-lock".into(),
            label: "Scroll Lock".into(),
            supported: false,
            active: None,
            source: "GNOME Wayland XKB".into(),
            reason: Some(
                "Unsupported on default GNOME Wayland XKB keymap: Scroll Lock is not bound to an active modifier/LED indicator."
                    .into(),
            ),
        }
    }

    pub fn get_state(&self) -> PreferencesState {
        let state = PreferencesState {
            items: vec![
                self.read_super_lock(),
                PreferenceControlState {
                    id: "super-fn-swap".into(),
                    label: "Super-Fn Swap".into(),
                    supported: false,
                    active: None,
                    source: "Firmware / EC".into(),
                    reason: Some(
                        "Unsupported: Fn modifier handling is private to EC firmware and this hardware exposes no Super-Fn swap interface."
                            .into(),
                    ),
                },
                PreferenceControlState {
                    id: "fn-lock".into(),
                    label: "Fn Lock".into(),
                    supported: false,
                    active: None,
                    source: "Firmware / EC".into(),
                    reason: Some(
                        "Unsupported: Fn Lock is managed internally by EC/BIOS firmware with no exposed Linux ACPI/WMI switch."
                            .into(),
                    ),
                },
                self.read_camera(),
                self.read_touchpad(),
                self.read_airplane(),
                self.read_lock_led("num-lock", "Num Lock", "::numlock"),
                self.read_scroll_lock(),
                self.read_lock_led("caps-lock", "Caps Lock", "::capslock"),
            ],
            sampled_at_ms: now_ms(),
        };

        if let Ok(mut guard) = self.cached_state.lock() {
            *guard = Some(state.clone());
        }
        state
    }

    pub fn set_preference(&self, id: &str, enabled: bool) -> Result<PreferencesState, String> {
        let _lock = self
            .action_lock
            .lock()
            .map_err(|_| "Preferences lock poisoned".to_owned())?;

        match id {
            "super-lock" => self.set_super_lock(enabled)?,
            "touchpad" => self.set_touchpad(enabled)?,
            "camera" => self.set_camera(enabled)?,
            "airplane" => self.set_airplane(enabled)?,
            "num-lock" => {
                self.set_lock_led("::numlock", KEY_NUMLOCK, "Num Lock", enabled)?;
            }
            "caps-lock" => {
                self.set_lock_led("::capslock", KEY_CAPSLOCK, "Caps Lock", enabled)?;
            }
            "super-fn-swap" | "fn-lock" | "scr-lock" => {
                let current = self.get_state();
                let msg = current
                    .items
                    .iter()
                    .find(|item| item.id == id)
                    .and_then(|item| item.reason.clone())
                    .unwrap_or_else(|| format!("Preference '{id}' is not supported on this platform"));
                return Err(msg);
            }
            other => return Err(format!("Unknown preference id: {other}")),
        }

        Ok(self.get_state())
    }

    pub fn toggle_preference(&self, id: &str) -> Result<PreferencesState, String> {
        let current = self.get_state();
        let item = current
            .items
            .iter()
            .find(|item| item.id == id)
            .ok_or_else(|| format!("Unknown preference id: {id}"))?;
        if !item.supported {
            return Err(item
                .reason
                .clone()
                .unwrap_or_else(|| format!("Preference '{}' is unsupported", item.label)));
        }
        let target = !item.active.unwrap_or(false);
        self.set_preference(id, target)
    }

    pub fn start_watchers(self: &Arc<Self>, app_handle: AppHandle) {
        let (notify_tx, notify_rx) = mpsc::channel::<()>();

        // 1. Dispatcher thread: coalesces bursts cleanly and emits only on real state diff
        {
            let mgr = Arc::clone(self);
            let app = app_handle.clone();
            let shutdown = Arc::clone(&self.shutdown);
            let _ = thread::Builder::new()
                .name("uxf-pref-sync".into())
                .spawn(move || {
                    let mut last_signature: Option<BTreeMap<String, (bool, Option<bool>)>> = None;
                    let mut last_leds = (read_led_state("::numlock"), read_led_state("::capslock"));

                    while !shutdown.load(Ordering::Relaxed) {
                        let triggered = match notify_rx.recv_timeout(Duration::from_millis(350)) {
                            Ok(()) => {
                                while notify_rx.try_recv().is_ok() {}
                                thread::sleep(Duration::from_millis(40));
                                while notify_rx.try_recv().is_ok() {}
                                true
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                let now_leds =
                                    (read_led_state("::numlock"), read_led_state("::capslock"));
                                if now_leds != last_leds {
                                    last_leds = now_leds;
                                    true
                                } else {
                                    false
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        };

                        if triggered {
                            last_leds = (read_led_state("::numlock"), read_led_state("::capslock"));
                            let snapshot = mgr.get_state();
                            let sig: BTreeMap<String, (bool, Option<bool>)> = snapshot
                                .items
                                .iter()
                                .map(|i| (i.id.clone(), (i.supported, i.active)))
                                .collect();
                            if last_signature.as_ref() != Some(&sig) {
                                last_signature = Some(sig);
                                let _ = app.emit(PREFERENCES_UPDATED_EVENT, &snapshot);
                            }
                        }
                    }
                });
        }

        // 2. DConf watch thread for GNOME Mutter, Touchpad, Camera Privacy, and Keyboard NumLock
        {
            let tx = notify_tx.clone();
            let shutdown = Arc::clone(&self.shutdown);
            let _ = thread::Builder::new()
                .name("uxf-dconf-watch".into())
                .spawn(move || Self::watch_dconf(tx, shutdown));
        }

        // 3. Session D-Bus PropertiesChanged signal watch for org.gnome.SettingsDaemon.Rfkill
        {
            let tx = notify_tx.clone();
            let shutdown = Arc::clone(&self.shutdown);
            let _ = thread::Builder::new()
                .name("uxf-rfkill-watch".into())
                .spawn(move || Self::watch_rfkill_dbus(tx, shutdown));
        }

        // 4. Kernel NETLINK_KOBJECT_UEVENT watch for external camera hotplug / EC switch changes
        {
            let tx = notify_tx;
            let shutdown = Arc::clone(&self.shutdown);
            let _ = thread::Builder::new()
                .name("uxf-uevent-watch".into())
                .spawn(move || Self::watch_camera_uevents(tx, shutdown));
        }
    }

    fn watch_dconf(tx: mpsc::Sender<()>, shutdown: Arc<AtomicBool>) {
        let mut child: Option<Child> = Command::new("dconf")
            .args(["watch", "/org/gnome/"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok();

        if let Some(ref mut proc) = child {
            if let Some(stdout) = proc.stdout.take() {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    if shutdown.load(Ordering::Relaxed) {
                        break;
                    }
                    if line.contains("overlay-key")
                        || line.contains("send-events")
                        || line.contains("disable-camera")
                        || line.contains("numlock-state")
                    {
                        if tx.send(()).is_err() {
                            break;
                        }
                    }
                }
            }
            let _ = proc.kill();
            let _ = proc.wait();
        }
    }

    fn watch_rfkill_dbus(tx: mpsc::Sender<()>, shutdown: Arc<AtomicBool>) {
        let Ok(conn) = DbusConnection::session() else {
            return;
        };
        let Ok(rule) = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(DBUS_PROPS_IFACE)
            .and_then(|b| b.member("PropertiesChanged"))
            .and_then(|b| b.path(RFKILL_PATH))
            .map(|b| b.build())
        else {
            return;
        };

        let Ok(iter) = MessageIterator::for_match_rule(rule, &conn, Some(32)) else {
            return;
        };

        for msg in iter {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            if msg.is_ok() && tx.send(()).is_err() {
                break;
            }
        }
    }

    fn watch_camera_uevents(tx: mpsc::Sender<()>, shutdown: Arc<AtomicBool>) {
        unsafe {
            let sock = libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
                libc::NETLINK_KOBJECT_UEVENT,
            );
            if sock < 0 {
                return;
            }
            let mut addr: libc::sockaddr_nl = std::mem::zeroed();
            addr.nl_family = libc::AF_NETLINK as libc::sa_family_t;
            addr.nl_groups = 1; // kernel uevents
            if libc::bind(
                sock,
                (&addr as *const libc::sockaddr_nl).cast::<libc::sockaddr>(),
                std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
            ) < 0
            {
                libc::close(sock);
                return;
            }

            let tv = libc::timeval {
                tv_sec: 1,
                tv_usec: 0,
            };
            let _ = libc::setsockopt(
                sock,
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                (&tv as *const libc::timeval).cast::<libc::c_void>(),
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            );

            let mut buf = [0u8; 4096];
            while !shutdown.load(Ordering::Relaxed) {
                let n = libc::recv(sock, buf.as_mut_ptr().cast::<libc::c_void>(), buf.len(), 0);
                if n > 0 {
                    let msg = String::from_utf8_lossy(&buf[..n as usize]);
                    if msg.contains("SUBSYSTEM=video4linux") || msg.contains("SUBSYSTEM=usb") {
                        if tx.send(()).is_err() {
                            break;
                        }
                    }
                }
            }
            libc::close(sock);
        }
    }
}

impl Drop for PreferencesManager {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gsettings_strings_and_booleans_accurately() {
        assert_eq!(parse_gsettings_string("'Super_L'"), "Super_L");
        assert_eq!(parse_gsettings_string("''"), "");
        assert_eq!(parse_gsettings_string("  'enabled'  "), "enabled");
        assert_eq!(parse_gsettings_bool("true\n"), Ok(true));
        assert_eq!(parse_gsettings_bool("false"), Ok(false));
        assert!(parse_gsettings_bool("invalid").is_err());
    }

    #[test]
    fn sorts_input_leds_numerically_so_input4_precedes_input26() {
        let base = std::env::temp_dir().join(format!("uxf-led-rank-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("input26::numlock")).unwrap();
        fs::create_dir_all(base.join("input4::numlock")).unwrap();
        fs::write(base.join("input26::numlock/brightness"), "1\n").unwrap();
        fs::write(base.join("input4::numlock/brightness"), "0\n").unwrap();

        let picked = find_lock_led_path_in(&base, "::numlock").unwrap();
        assert!(picked.ends_with("input4::numlock/brightness"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn snapshot_contains_all_nine_linux_labeled_preferences_and_blocks_unsupported() {
        let mgr = PreferencesManager::new_headless();
        let snap = mgr.get_state();
        let labels: Vec<&str> = snap.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "Super Lock",
                "Super-Fn Swap",
                "Fn Lock",
                "Camera",
                "Touchpad",
                "Airplane",
                "Num Lock",
                "Scroll Lock",
                "Caps Lock"
            ]
        );

        let err_swap = mgr.toggle_preference("super-fn-swap").unwrap_err();
        assert!(err_swap.contains("Unsupported"));
        let err_fn = mgr.toggle_preference("fn-lock").unwrap_err();
        assert!(err_fn.contains("Unsupported"));
        let err_scr = mgr.toggle_preference("scr-lock").unwrap_err();
        assert!(err_scr.contains("Scroll Lock"));
        let err_unknown = mgr.set_preference("does-not-exist", true).unwrap_err();
        assert!(err_unknown.contains("Unknown"));
    }

    #[test]
    fn serializes_camel_case_contract_for_frontend() {
        let item = PreferenceControlState {
            id: "super-lock".into(),
            label: "Super Lock".into(),
            supported: true,
            active: Some(false),
            source: "GNOME Mutter overlay-key".into(),
            reason: None,
        };
        let state = PreferencesState {
            items: vec![item],
            sampled_at_ms: 12345,
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"sampledAtMs\":12345"));
        assert!(json.contains("\"super-lock\""));
        assert!(json.contains("\"Super Lock\""));
    }

    #[test]
    #[ignore = "mutates live GNOME Mutter overlay-key, Touchpad, Camera USB hardware, Num Lock, and Caps Lock temporarily; restores original state"]
    fn live_preferences_hardware_and_desktop_round_trip_verification() {
        let mgr = PreferencesManager::new();
        let initial = mgr.get_state();
        let find_active = |state: &PreferencesState, id: &str| {
            state
                .items
                .iter()
                .find(|i| i.id == id)
                .and_then(|i| i.active)
        };

        let orig_super = find_active(&initial, "super-lock").expect("super-lock readable");
        let orig_cam = find_active(&initial, "camera").expect("camera readable");
        let orig_tp = find_active(&initial, "touchpad").expect("touchpad readable");
        let orig_num = find_active(&initial, "num-lock").expect("num-lock readable");
        let orig_caps = find_active(&initial, "caps-lock").expect("caps-lock readable");

        // 1. Super Lock round-trip
        let s1 = mgr.set_preference("super-lock", !orig_super).unwrap();
        assert_eq!(find_active(&s1, "super-lock"), Some(!orig_super));
        let s2 = mgr.set_preference("super-lock", orig_super).unwrap();
        assert_eq!(find_active(&s2, "super-lock"), Some(orig_super));

        // 2. Touchpad round-trip
        let t1 = mgr.set_preference("touchpad", !orig_tp).unwrap();
        assert_eq!(find_active(&t1, "touchpad"), Some(!orig_tp));
        let t2 = mgr.set_preference("touchpad", orig_tp).unwrap();
        assert_eq!(find_active(&t2, "touchpad"), Some(orig_tp));

        // 3. Camera round-trip (verify /dev/video0 hardware toggle too when camera toggled)
        let c1 = mgr.set_preference("camera", !orig_cam).unwrap();
        assert_eq!(find_active(&c1, "camera"), Some(!orig_cam));
        thread::sleep(Duration::from_millis(350));
        let c2 = mgr.set_preference("camera", orig_cam).unwrap();
        assert_eq!(find_active(&c2, "camera"), Some(orig_cam));

        // 4. Num Lock round-trip (waits for uinput worker warmup automatically)
        let n1 = mgr.set_preference("num-lock", !orig_num).unwrap();
        assert_eq!(find_active(&n1, "num-lock"), Some(!orig_num));
        let n2 = mgr.set_preference("num-lock", orig_num).unwrap();
        assert_eq!(find_active(&n2, "num-lock"), Some(orig_num));

        // 5. Caps Lock round-trip
        let cap1 = mgr.set_preference("caps-lock", !orig_caps).unwrap();
        assert_eq!(find_active(&cap1, "caps-lock"), Some(!orig_caps));
        let cap2 = mgr.set_preference("caps-lock", orig_caps).unwrap();
        assert_eq!(find_active(&cap2, "caps-lock"), Some(orig_caps));
    }
}
