use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use zbus::blocking::fdo::DBusProxy;
use zbus::blocking::{Connection as BlockingConnection, MessageIterator, Proxy as BlockingProxy};
use zbus::message::Type as MessageType;
use zbus::zvariant::OwnedValue;
use zbus::MatchRule;

pub const DESKTOP_CONTROLS_EVENT: &str = "desktop-controls-changed";
pub const BLUE_LIGHT_MIN_KELVIN: u32 = 1700;
pub const BLUE_LIGHT_MAX_KELVIN: u32 = 4700;
const PA_VOLUME_NORM: f64 = 65536.0;
const PA_CHANNELS_MAX: usize = 32;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrightnessState {
    pub available: bool,
    pub percent: u8,
    pub value: i32,
    pub min: i32,
    pub max: i32,
    pub connector: Option<String>,
    pub backend: String,
    pub error: Option<String>,
}

impl BrightnessState {
    fn unavailable(err: impl Into<String>) -> Self {
        Self {
            available: false,
            percent: 0,
            value: 0,
            min: 0,
            max: 0,
            connector: None,
            backend: "unavailable".into(),
            error: Some(err.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlueLightState {
    pub available: bool,
    pub level: f32,
    pub kelvin: u32,
    pub min_kelvin: u32,
    pub max_kelvin: u32,
    pub enabled: bool,
    pub active: bool,
    pub schedule_automatic: bool,
    pub schedule_from: f64,
    pub schedule_to: f64,
    pub status_reason: String,
    pub error: Option<String>,
}

impl BlueLightState {
    fn unavailable(err: impl Into<String>) -> Self {
        Self {
            available: false,
            level: 0.0,
            kelvin: BLUE_LIGHT_MAX_KELVIN,
            min_kelvin: BLUE_LIGHT_MIN_KELVIN,
            max_kelvin: BLUE_LIGHT_MAX_KELVIN,
            enabled: false,
            active: false,
            schedule_automatic: false,
            schedule_from: 0.0,
            schedule_to: 0.0,
            status_reason: "unavailable".into(),
            error: Some(err.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeState {
    pub available: bool,
    pub percent: u8,
    pub muted: bool,
    pub sink_name: Option<String>,
    pub sink_description: Option<String>,
    pub error: Option<String>,
}

impl VolumeState {
    fn unavailable(err: impl Into<String>) -> Self {
        Self {
            available: false,
            percent: 0,
            muted: false,
            sink_name: None,
            sink_description: None,
            error: Some(err.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopControlsState {
    pub brightness: BrightnessState,
    pub blue_light: BlueLightState,
    pub volume: VolumeState,
}

pub fn kelvin_to_level(kelvin: u32) -> f32 {
    let span = (BLUE_LIGHT_MAX_KELVIN - BLUE_LIGHT_MIN_KELVIN) as f32;
    let clamped = kelvin.clamp(BLUE_LIGHT_MIN_KELVIN, BLUE_LIGHT_MAX_KELVIN) as f32;
    let raw = (BLUE_LIGHT_MAX_KELVIN as f32 - clamped) * 10.0 / span;
    (raw * 10.0).round() / 10.0
}

pub fn level_to_kelvin(level: f32) -> u32 {
    let clamped = level.clamp(0.0, 10.0) as f64;
    let span = (BLUE_LIGHT_MAX_KELVIN - BLUE_LIGHT_MIN_KELVIN) as f64;
    let kelvin = (BLUE_LIGHT_MAX_KELVIN as f64 - (clamped / 10.0) * span).round() as u32;
    kelvin.clamp(BLUE_LIGHT_MIN_KELVIN, BLUE_LIGHT_MAX_KELVIN)
}

pub fn map_percent_to_range(percent: u8, min: i32, max: i32) -> i32 {
    if max <= min {
        return min;
    }
    let clamped_pct = percent.min(100) as f64 / 100.0;
    let raw = min as f64 + clamped_pct * (max - min) as f64;
    (raw.round() as i32).clamp(min, max)
}

pub fn map_range_to_percent(value: i32, min: i32, max: i32) -> u8 {
    if max <= min {
        return 0;
    }
    let clamped = value.clamp(min, max);
    let pct = ((clamped - min) as f64 / (max - min) as f64) * 100.0;
    pct.round().clamp(0.0, 100.0) as u8
}

// ============================================================================
// Screen Brightness (Mutter DisplayConfig primary + logind Session fallback)
// ============================================================================

const MUTTER_DEST: &str = "org.gnome.Mutter.DisplayConfig";
const MUTTER_PATH: &str = "/org/gnome/Mutter/DisplayConfig";

type MutterBacklightTuple = (u32, Vec<HashMap<String, OwnedValue>>);

fn owned_value_as_string(v: &OwnedValue) -> Option<String> {
    <&str>::try_from(v).ok().map(ToOwned::to_owned)
}

fn owned_value_as_bool(v: &OwnedValue) -> Option<bool> {
    bool::try_from(v).ok()
}

fn owned_value_as_i32(v: &OwnedValue) -> Option<i32> {
    i32::try_from(v)
        .ok()
        .or_else(|| u32::try_from(v).ok().and_then(|x| i32::try_from(x).ok()))
}

fn select_active_backlight(
    entries: Vec<HashMap<String, OwnedValue>>,
) -> Option<(String, i32, i32, i32)> {
    let mut chosen: Option<(String, i32, i32, i32)> = None;
    for entry in entries {
        let active = entry.get("active").and_then(owned_value_as_bool).unwrap_or(true);
        if !active {
            continue;
        }
        let Some(connector) = entry.get("connector").and_then(owned_value_as_string) else {
            continue;
        };
        let min = entry.get("min").and_then(owned_value_as_i32).unwrap_or(0);
        let max = entry.get("max").and_then(owned_value_as_i32).unwrap_or(0);
        let value = entry.get("value").and_then(owned_value_as_i32).unwrap_or(min);
        if max <= min {
            continue;
        }
        let is_edp = connector.starts_with("eDP");
        if is_edp || chosen.is_none() {
            chosen = Some((connector, min, max, value));
            if is_edp {
                break;
            }
        }
    }
    chosen
}

fn read_mutter_backlight(conn: &BlockingConnection) -> Result<BrightnessState, String> {
    let proxy = BlockingProxy::new(conn, MUTTER_DEST, MUTTER_PATH, MUTTER_DEST)
        .map_err(|e| format!("Failed to connect to Mutter DisplayConfig: {e}"))?;

    let (serial, entries): MutterBacklightTuple = proxy
        .get_property("Backlight")
        .map_err(|e| format!("Mutter Backlight property unavailable: {e}"))?;

    let Some((connector, min, max, value)) = select_active_backlight(entries) else {
        return Err("No active internal backlight reported by Mutter DisplayConfig".into());
    };

    let _ = serial;
    Ok(BrightnessState {
        available: true,
        percent: map_range_to_percent(value, min, max),
        value,
        min,
        max,
        connector: Some(connector),
        backend: "mutter".into(),
        error: None,
    })
}

fn set_mutter_backlight(conn: &BlockingConnection, percent: u8) -> Result<BrightnessState, String> {
    let proxy = BlockingProxy::new(conn, MUTTER_DEST, MUTTER_PATH, MUTTER_DEST)
        .map_err(|e| format!("Failed to create Mutter DisplayConfig proxy: {e}"))?;

    for attempt in 0..2 {
        let (serial, entries): MutterBacklightTuple = proxy
            .get_property("Backlight")
            .map_err(|e| format!("Failed to read Mutter Backlight serial: {e}"))?;

        let Some((connector, min, max, _)) = select_active_backlight(entries) else {
            return Err("No active backlight connector available".into());
        };

        let target_value = map_percent_to_range(percent, min, max);
        match proxy.call::<_, _, ()>("SetBacklight", &(serial, connector.as_str(), target_value)) {
            Ok(()) => {
                // Re-read Mutter's authoritative Backlight property to confirm actual persisted state.
                if let Ok(confirmed) = read_mutter_backlight(conn) {
                    return Ok(confirmed);
                }
                return Ok(BrightnessState {
                    available: true,
                    percent: map_range_to_percent(target_value, min, max),
                    value: target_value,
                    min,
                    max,
                    connector: Some(connector),
                    backend: "mutter".into(),
                    error: None,
                });
            }
            Err(_) if attempt == 0 => continue,
            Err(err) => {
                return Err(format!("Mutter SetBacklight failed: {err}"));
            }
        }
    }

    Err("Mutter SetBacklight failed after serial refresh".into())
}

fn find_sysfs_backlight_dir() -> Option<(String, PathBuf)> {
    let preferred = Path::new("/sys/class/backlight/intel_backlight");
    if preferred.exists() {
        return Some(("intel_backlight".into(), preferred.to_path_buf()));
    }
    let entries = fs::read_dir("/sys/class/backlight").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.join("brightness").exists() && path.join("max_brightness").exists() {
            return Some((name, path));
        }
    }
    None
}

fn read_sysfs_brightness() -> Result<(String, i32, i32, i32), String> {
    let Some((name, dir)) = find_sysfs_backlight_dir() else {
        return Err("No backlight interface found under /sys/class/backlight".into());
    };
    let max: i32 = fs::read_to_string(dir.join("max_brightness"))
        .map_err(|e| format!("Failed reading max_brightness: {e}"))?
        .trim()
        .parse()
        .map_err(|e| format!("Invalid max_brightness value: {e}"))?;
    let current: i32 = fs::read_to_string(dir.join("actual_brightness"))
        .or_else(|_| fs::read_to_string(dir.join("brightness")))
        .map_err(|e| format!("Failed reading current brightness: {e}"))?
        .trim()
        .parse()
        .map_err(|e| format!("Invalid current brightness value: {e}"))?;

    if max <= 0 {
        return Err("Invalid hardware max_brightness <= 0".into());
    }
    let safe_min = (max / 100).max(1);
    Ok((name, safe_min, max, current))
}

fn query_brightness_with_conn(session_conn: Option<&BlockingConnection>) -> BrightnessState {
    if let Some(conn) = session_conn {
        if let Ok(state) = read_mutter_backlight(conn) {
            return state;
        }
    } else if let Ok(conn) = BlockingConnection::session() {
        if let Ok(state) = read_mutter_backlight(&conn) {
            return state;
        }
    }

    match read_sysfs_brightness() {
        Ok((name, min, max, current)) => {
            // Confirm logind session bus is reachable so fallback writes can work cleanly.
            let logind_ok = BlockingConnection::system()
                .and_then(|sys_conn| {
                    BlockingProxy::new(
                        &sys_conn,
                        "org.freedesktop.login1",
                        "/org/freedesktop/login1/session/auto",
                        "org.freedesktop.login1.Session",
                    )
                    .and_then(|p| p.get_property::<bool>("Active"))
                })
                .unwrap_or(false);

            if logind_ok {
                BrightnessState {
                    available: true,
                    percent: map_range_to_percent(current, min, max),
                    value: current,
                    min,
                    max,
                    connector: Some(name),
                    backend: "login1".into(),
                    error: None,
                }
            } else {
                BrightnessState::unavailable("GNOME DisplayConfig and logind brightness unavailable")
            }
        }
        Err(err) => BrightnessState::unavailable(err),
    }
}

fn query_brightness_blocking() -> BrightnessState {
    query_brightness_with_conn(None)
}

fn set_brightness_blocking(percent: u8) -> Result<BrightnessState, String> {
    let clamped = percent.min(100);
    if let Ok(session_conn) = BlockingConnection::session() {
        if read_mutter_backlight(&session_conn).is_ok() {
            // Never bypass an active Mutter backlight controller with direct sysfs/logind writes.
            return set_mutter_backlight(&session_conn, clamped);
        }
    }

    let (name, min, max, _) = read_sysfs_brightness()?;
    set_login1_brightness(&name, min, max, clamped)
}

fn set_login1_brightness(name: &str, min: i32, max: i32, percent: u8) -> Result<BrightnessState, String> {
    let sys_conn = BlockingConnection::system()
        .map_err(|e| format!("Failed connecting to System D-Bus for logind fallback: {e}"))?;
    let proxy = BlockingProxy::new(
        &sys_conn,
        "org.freedesktop.login1",
        "/org/freedesktop/login1/session/auto",
        "org.freedesktop.login1.Session",
    )
    .map_err(|e| format!("Failed creating logind Session proxy: {e}"))?;

    let target = map_percent_to_range(percent, min, max);
    proxy
        .call::<_, _, ()>("SetBrightness", &("backlight", name, target as u32))
        .map_err(|e| format!("logind SetBrightness failed: {e}"))?;

    Ok(BrightnessState {
        available: true,
        percent: map_range_to_percent(target, min, max),
        value: target,
        min,
        max,
        connector: Some(name.to_string()),
        backend: "login1".into(),
        error: None,
    })
}

// ============================================================================
// Blue Light Filter (In-Process GIO GSettings + gsd-color D-Bus, schedule-safe)
// ============================================================================

const GSD_COLOR_DEST: &str = "org.gnome.SettingsDaemon.Color";
const GSD_COLOR_PATH: &str = "/org/gnome/SettingsDaemon/Color";
const NIGHT_LIGHT_SCHEMA: &str = "org.gnome.settings-daemon.plugins.color";

type FnGSettingsNew = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type FnGSettingsGetBoolean = unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int;
type FnGSettingsSetBoolean = unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_int;
type FnGSettingsGetUint = unsafe extern "C" fn(*mut c_void, *const c_char) -> u32;
type FnGSettingsSetUint = unsafe extern "C" fn(*mut c_void, *const c_char, u32) -> c_int;
type FnGSettingsGetDouble = unsafe extern "C" fn(*mut c_void, *const c_char) -> f64;
type FnGSettingsSync = unsafe extern "C" fn();
type FnGObjectUnref = unsafe extern "C" fn(*mut c_void);

struct GioSettingsLib {
    gio_handle: *mut c_void,
    gobj_handle: *mut c_void,
    settings_new: FnGSettingsNew,
    get_boolean: FnGSettingsGetBoolean,
    set_boolean: FnGSettingsSetBoolean,
    get_uint: FnGSettingsGetUint,
    set_uint: FnGSettingsSetUint,
    get_double: FnGSettingsGetDouble,
    sync_fn: FnGSettingsSync,
    unref_fn: FnGObjectUnref,
}

unsafe impl Send for GioSettingsLib {}
unsafe impl Sync for GioSettingsLib {}

impl GioSettingsLib {
    fn load() -> Result<Self, String> {
        let gio_handle = unsafe {
            libc::dlopen(
                b"libgio-2.0.so.0\0".as_ptr() as *const c_char,
                libc::RTLD_LAZY | libc::RTLD_GLOBAL,
            )
        };
        if gio_handle.is_null() {
            return Err("libgio-2.0.so.0 unavailable".into());
        }

        let gobj_handle = unsafe {
            libc::dlopen(
                b"libgobject-2.0.so.0\0".as_ptr() as *const c_char,
                libc::RTLD_LAZY | libc::RTLD_GLOBAL,
            )
        };

        macro_rules! sym {
            ($handle:expr, $name:expr) => {{
                let ptr = unsafe { libc::dlsym($handle, $name.as_ptr() as *const c_char) };
                if ptr.is_null() {
                    if !gobj_handle.is_null() {
                        unsafe { libc::dlclose(gobj_handle) };
                    }
                    unsafe { libc::dlclose(gio_handle) };
                    return Err(format!(
                        "Symbol {} missing in GIO",
                        String::from_utf8_lossy($name).trim_end_matches('\0')
                    ));
                }
                unsafe { std::mem::transmute(ptr) }
            }};
        }

        let settings_new: FnGSettingsNew = sym!(gio_handle, b"g_settings_new\0");
        let get_boolean: FnGSettingsGetBoolean = sym!(gio_handle, b"g_settings_get_boolean\0");
        let set_boolean: FnGSettingsSetBoolean = sym!(gio_handle, b"g_settings_set_boolean\0");
        let get_uint: FnGSettingsGetUint = sym!(gio_handle, b"g_settings_get_uint\0");
        let set_uint: FnGSettingsSetUint = sym!(gio_handle, b"g_settings_set_uint\0");
        let get_double: FnGSettingsGetDouble = sym!(gio_handle, b"g_settings_get_double\0");
        let sync_fn: FnGSettingsSync = sym!(gio_handle, b"g_settings_sync\0");
        let unref_handle = if !gobj_handle.is_null() {
            gobj_handle
        } else {
            gio_handle
        };
        let unref_fn: FnGObjectUnref = sym!(unref_handle, b"g_object_unref\0");

        Ok(Self {
            gio_handle,
            gobj_handle,
            settings_new,
            get_boolean,
            set_boolean,
            get_uint,
            set_uint,
            get_double,
            sync_fn,
            unref_fn,
        })
    }

    fn with_night_light_settings<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(*mut c_void) -> T,
    {
        let schema_c = CString::new(NIGHT_LIGHT_SCHEMA).map_err(|e| e.to_string())?;
        let obj = unsafe { (self.settings_new)(schema_c.as_ptr()) };
        if obj.is_null() {
            return Err("GSettings schema org.gnome.settings-daemon.plugins.color missing".into());
        }
        let out = f(obj);
        unsafe { (self.unref_fn)(obj) };
        Ok(out)
    }

    fn read_night_light_settings(&self) -> Result<(bool, u32, bool, f64, f64), String> {
        self.with_night_light_settings(|obj| unsafe {
            let k_enabled = b"night-light-enabled\0".as_ptr() as *const c_char;
            let k_temp = b"night-light-temperature\0".as_ptr() as *const c_char;
            let k_auto = b"night-light-schedule-automatic\0".as_ptr() as *const c_char;
            let k_from = b"night-light-schedule-from\0".as_ptr() as *const c_char;
            let k_to = b"night-light-schedule-to\0".as_ptr() as *const c_char;

            let enabled = (self.get_boolean)(obj, k_enabled) != 0;
            let temp = (self.get_uint)(obj, k_temp);
            let schedule_auto = (self.get_boolean)(obj, k_auto) != 0;
            let from = (self.get_double)(obj, k_from);
            let to = (self.get_double)(obj, k_to);
            (enabled, temp, schedule_auto, from, to)
        })
    }

    /// Updates Night Light enabled state and optionally Kelvin temperature without altering
    /// the user's automatic/manual schedule keys. Returns `true` if `night-light-enabled`
    /// changed state.
    fn apply_blue_light_settings(
        &self,
        kelvin: Option<u32>,
        target_enabled: bool,
    ) -> Result<bool, String> {
        let clamped_opt = kelvin.map(|k| k.clamp(BLUE_LIGHT_MIN_KELVIN, BLUE_LIGHT_MAX_KELVIN));
        let (ok, toggled) = self.with_night_light_settings(|obj| unsafe {
            let k_enabled = b"night-light-enabled\0".as_ptr() as *const c_char;
            let k_temp = b"night-light-temperature\0".as_ptr() as *const c_char;

            let was_enabled = (self.get_boolean)(obj, k_enabled) != 0;
            let mut changed_enabled = false;
            let mut enable_ok = true;
            if was_enabled != target_enabled {
                enable_ok = (self.set_boolean)(obj, k_enabled, if target_enabled { 1 } else { 0 }) != 0;
                changed_enabled = enable_ok;
            }
            let temp_ok = match clamped_opt {
                Some(clamped) => (self.set_uint)(obj, k_temp, clamped) != 0,
                None => true,
            };
            (self.sync_fn)();
            (enable_ok && temp_ok, changed_enabled)
        })?;
        if ok {
            Ok(toggled)
        } else {
            Err("Failed writing Night Light configuration to GSettings".into())
        }
    }
}

impl Drop for GioSettingsLib {
    fn drop(&mut self) {
        if !self.gobj_handle.is_null() {
            unsafe { libc::dlclose(self.gobj_handle) };
        }
        if !self.gio_handle.is_null() {
            unsafe { libc::dlclose(self.gio_handle) };
        }
    }
}

fn query_blue_light_with_conn(
    gio: Option<&GioSettingsLib>,
    existing_conn: Option<&BlockingConnection>,
) -> BlueLightState {
    let Some(gio_lib) = gio else {
        return BlueLightState::unavailable("GIO library unavailable for GNOME Night Light");
    };

    let (enabled, raw_kelvin, schedule_automatic, schedule_from, schedule_to) =
        match gio_lib.read_night_light_settings() {
            Ok(tuple) => tuple,
            Err(err) => return BlueLightState::unavailable(err),
        };

    let owned_conn;
    let session_conn = match existing_conn {
        Some(c) => c,
        None => match BlockingConnection::session() {
            Ok(c) => {
                owned_conn = c;
                &owned_conn
            }
            Err(err) => {
                return BlueLightState {
                    available: false,
                    level: kelvin_to_level(raw_kelvin),
                    kelvin: raw_kelvin,
                    min_kelvin: BLUE_LIGHT_MIN_KELVIN,
                    max_kelvin: BLUE_LIGHT_MAX_KELVIN,
                    enabled,
                    active: false,
                    schedule_automatic,
                    schedule_from,
                    schedule_to,
                    status_reason: "unavailable".into(),
                    error: Some(format!("Session D-Bus unavailable: {err}")),
                };
            }
        },
    };

    let supported = BlockingProxy::new(session_conn, MUTTER_DEST, MUTTER_PATH, MUTTER_DEST)
        .and_then(|p| p.get_property::<bool>("NightLightSupported"))
        .unwrap_or(true);

    if !supported {
        return BlueLightState::unavailable("Night Light not supported on current display");
    }

    let color_proxy = match BlockingProxy::new(
        session_conn,
        GSD_COLOR_DEST,
        GSD_COLOR_PATH,
        GSD_COLOR_DEST,
    ) {
        Ok(p) => p,
        Err(err) => {
            return BlueLightState::unavailable(format!("gsd-color proxy failed: {err}"));
        }
    };

    let active = match color_proxy.get_property::<bool>("NightLightActive") {
        Ok(v) => v,
        Err(err) => {
            return BlueLightState::unavailable(format!("GNOME Color daemon unreachable: {err}"));
        }
    };

    let disabled_until_tomorrow = color_proxy
        .get_property::<bool>("DisabledUntilTomorrow")
        .unwrap_or(false);

    let status_reason = if active {
        "active"
    } else if !enabled {
        "disabled"
    } else if disabled_until_tomorrow {
        "paused_until_tomorrow"
    } else {
        "schedule_inactive"
    };

    BlueLightState {
        available: true,
        level: if enabled {
            kelvin_to_level(raw_kelvin)
        } else {
            0.0
        },
        kelvin: raw_kelvin,
        min_kelvin: BLUE_LIGHT_MIN_KELVIN,
        max_kelvin: BLUE_LIGHT_MAX_KELVIN,
        enabled,
        active,
        schedule_automatic,
        schedule_from,
        schedule_to,
        status_reason: status_reason.into(),
        error: None,
    }
}

fn query_blue_light_blocking(gio: Option<&GioSettingsLib>) -> BlueLightState {
    query_blue_light_with_conn(gio, None)
}

fn set_blue_light_blocking(gio: Option<&GioSettingsLib>, level: f32) -> Result<BlueLightState, String> {
    let gio_lib = gio.ok_or_else(|| "GIO library unavailable for Night Light".to_string())?;
    let clamped_level = level.clamp(0.0, 10.0);
    let session_conn = BlockingConnection::session().ok();

    if clamped_level <= 0.05 {
        // Intensity 0 disables Blue Light Filter completely (neutral default display temperature)
        // while preserving saved temperature preset and GNOME schedule keys.
        let toggled_off = gio_lib.apply_blue_light_settings(None, false)?;
        let mut state = query_blue_light_with_conn(Some(gio_lib), session_conn.as_ref());
        if toggled_off && state.active {
            for _ in 0..8 {
                thread::sleep(Duration::from_millis(25));
                state = query_blue_light_with_conn(Some(gio_lib), session_conn.as_ref());
                if !state.active {
                    break;
                }
            }
        }
        return Ok(state);
    }

    let kelvin = level_to_kelvin(clamped_level);
    // Activate GNOME Night Light if currently disabled while preserving user schedule settings intact.
    let toggled_on = gio_lib.apply_blue_light_settings(Some(kelvin), true)?;

    let mut cleared_pause = false;
    if let Some(conn) = session_conn.as_ref() {
        if let Ok(color_proxy) = BlockingProxy::new(conn, GSD_COLOR_DEST, GSD_COLOR_PATH, GSD_COLOR_DEST) {
            if color_proxy.get_property::<bool>("DisabledUntilTomorrow").unwrap_or(false) {
                if color_proxy.set_property("DisabledUntilTomorrow", false).is_ok() {
                    cleared_pause = true;
                }
            }
        }
    }

    let mut state = query_blue_light_with_conn(Some(gio_lib), session_conn.as_ref());
    // gsd-color reacts asynchronously to GSettings/dconf sync; briefly observe initial activation.
    if (toggled_on || cleared_pause) && !state.active {
        for _ in 0..8 {
            thread::sleep(Duration::from_millis(25));
            state = query_blue_light_with_conn(Some(gio_lib), session_conn.as_ref());
            if state.active {
                break;
            }
        }
    }
    Ok(state)
}

// ============================================================================
// System Volume (In-Process libpulse.so.0 over PipeWire PulseAudio compat)
// ============================================================================

#[repr(C)]
#[derive(Clone, Copy)]
struct PaSampleSpec {
    format: c_int,
    rate: u32,
    channels: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PaChannelMap {
    channels: u8,
    map: [c_int; PA_CHANNELS_MAX],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PaCvolume {
    channels: u8,
    values: [u32; PA_CHANNELS_MAX],
}

#[repr(C)]
struct PaServerInfo {
    user_name: *const c_char,
    host_name: *const c_char,
    server_version: *const c_char,
    server_name: *const c_char,
    sample_spec: PaSampleSpec,
    default_sink_name: *const c_char,
    default_source_name: *const c_char,
    cookie: u32,
    channel_map: PaChannelMap,
}

#[repr(C)]
struct PaSinkInfo {
    name: *const c_char,
    index: u32,
    description: *const c_char,
    sample_spec: PaSampleSpec,
    channel_map: PaChannelMap,
    owner_module: u32,
    volume: PaCvolume,
    mute: c_int,
}

const PA_CONTEXT_READY: c_int = 4;
const PA_CONTEXT_FAILED: c_int = 5;
const PA_CONTEXT_TERMINATED: c_int = 6;
const PA_SUBSCRIPTION_MASK_SINK: u32 = 0x0001;
const PA_SUBSCRIPTION_MASK_SERVER: u32 = 0x0080;
const PA_SUBSCRIPTION_EVENT_FACILITY_MASK: u32 = 0x000F;
const PA_SUBSCRIPTION_EVENT_SINK: u32 = 0x0000;
const PA_SUBSCRIPTION_EVENT_SERVER: u32 = 0x0007;
const PA_OPERATION_RUNNING: c_int = 0;

type PaContextNotifyCb = Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>;
type PaContextSubscribeCb = Option<unsafe extern "C" fn(*mut c_void, u32, u32, *mut c_void)>;
type PaServerInfoCb = Option<unsafe extern "C" fn(*mut c_void, *const PaServerInfo, *mut c_void)>;
type PaSinkInfoCb = Option<unsafe extern "C" fn(*mut c_void, *const PaSinkInfo, c_int, *mut c_void)>;
type PaSuccessCb = Option<unsafe extern "C" fn(*mut c_void, c_int, *mut c_void)>;

type FnPaMainloopNew = unsafe extern "C" fn() -> *mut c_void;
type FnPaMainloopFree = unsafe extern "C" fn(*mut c_void);
type FnPaMainloopStart = unsafe extern "C" fn(*mut c_void) -> c_int;
type FnPaMainloopStop = unsafe extern "C" fn(*mut c_void);
type FnPaMainloopLock = unsafe extern "C" fn(*mut c_void);
type FnPaMainloopUnlock = unsafe extern "C" fn(*mut c_void);
type FnPaMainloopWait = unsafe extern "C" fn(*mut c_void);
type FnPaMainloopSignal = unsafe extern "C" fn(*mut c_void, c_int);
type FnPaMainloopGetApi = unsafe extern "C" fn(*mut c_void) -> *mut c_void;

type FnPaContextNew = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;
type FnPaContextUnref = unsafe extern "C" fn(*mut c_void);
type FnPaContextConnect =
    unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *const c_void) -> c_int;
type FnPaContextDisconnect = unsafe extern "C" fn(*mut c_void);
type FnPaContextGetState = unsafe extern "C" fn(*const c_void) -> c_int;
type FnPaContextSetStateCb =
    unsafe extern "C" fn(*mut c_void, PaContextNotifyCb, *mut c_void);
type FnPaContextSetSubscribeCb =
    unsafe extern "C" fn(*mut c_void, PaContextSubscribeCb, *mut c_void);
type FnPaContextSubscribe =
    unsafe extern "C" fn(*mut c_void, u32, PaSuccessCb, *mut c_void) -> *mut c_void;
type FnPaContextGetServerInfo =
    unsafe extern "C" fn(*mut c_void, PaServerInfoCb, *mut c_void) -> *mut c_void;
type FnPaContextGetSinkByName =
    unsafe extern "C" fn(*mut c_void, *const c_char, PaSinkInfoCb, *mut c_void) -> *mut c_void;
type FnPaContextSetSinkVolByName = unsafe extern "C" fn(
    *mut c_void,
    *const c_char,
    *const PaCvolume,
    PaSuccessCb,
    *mut c_void,
) -> *mut c_void;

type FnPaOperationGetState = unsafe extern "C" fn(*const c_void) -> c_int;
type FnPaOperationUnref = unsafe extern "C" fn(*mut c_void);
type FnPaCvolumeAvg = unsafe extern "C" fn(*const PaCvolume) -> u32;
type FnPaCvolumeSet = unsafe extern "C" fn(*mut PaCvolume, u32, u32) -> *mut PaCvolume;

struct LibPulseApi {
    lib_handle: *mut c_void,
    mainloop_new: FnPaMainloopNew,
    mainloop_free: FnPaMainloopFree,
    mainloop_start: FnPaMainloopStart,
    mainloop_stop: FnPaMainloopStop,
    mainloop_lock: FnPaMainloopLock,
    mainloop_unlock: FnPaMainloopUnlock,
    mainloop_wait: FnPaMainloopWait,
    mainloop_signal: FnPaMainloopSignal,
    mainloop_get_api: FnPaMainloopGetApi,
    context_new: FnPaContextNew,
    context_unref: FnPaContextUnref,
    context_connect: FnPaContextConnect,
    context_disconnect: FnPaContextDisconnect,
    context_get_state: FnPaContextGetState,
    context_set_state_cb: FnPaContextSetStateCb,
    context_set_subscribe_cb: FnPaContextSetSubscribeCb,
    context_subscribe: FnPaContextSubscribe,
    context_get_server_info: FnPaContextGetServerInfo,
    context_get_sink_by_name: FnPaContextGetSinkByName,
    context_set_sink_vol_by_name: FnPaContextSetSinkVolByName,
    op_get_state: FnPaOperationGetState,
    op_unref: FnPaOperationUnref,
    cvolume_avg: FnPaCvolumeAvg,
    cvolume_set: FnPaCvolumeSet,
}

unsafe impl Send for LibPulseApi {}
unsafe impl Sync for LibPulseApi {}

impl LibPulseApi {
    fn load() -> Result<Self, String> {
        let lib_handle = unsafe {
            libc::dlopen(
                b"libpulse.so.0\0".as_ptr() as *const c_char,
                libc::RTLD_LAZY,
            )
        };
        if lib_handle.is_null() {
            return Err("libpulse.so.0 unavailable".into());
        }

        macro_rules! sym {
            ($name:expr) => {{
                let ptr = unsafe { libc::dlsym(lib_handle, $name.as_ptr() as *const c_char) };
                if ptr.is_null() {
                    unsafe { libc::dlclose(lib_handle) };
                    return Err(format!(
                        "Symbol {} missing in libpulse.so.0",
                        String::from_utf8_lossy($name).trim_end_matches('\0')
                    ));
                }
                unsafe { std::mem::transmute(ptr) }
            }};
        }

        Ok(Self {
            lib_handle,
            mainloop_new: sym!(b"pa_threaded_mainloop_new\0"),
            mainloop_free: sym!(b"pa_threaded_mainloop_free\0"),
            mainloop_start: sym!(b"pa_threaded_mainloop_start\0"),
            mainloop_stop: sym!(b"pa_threaded_mainloop_stop\0"),
            mainloop_lock: sym!(b"pa_threaded_mainloop_lock\0"),
            mainloop_unlock: sym!(b"pa_threaded_mainloop_unlock\0"),
            mainloop_wait: sym!(b"pa_threaded_mainloop_wait\0"),
            mainloop_signal: sym!(b"pa_threaded_mainloop_signal\0"),
            mainloop_get_api: sym!(b"pa_threaded_mainloop_get_api\0"),
            context_new: sym!(b"pa_context_new\0"),
            context_unref: sym!(b"pa_context_unref\0"),
            context_connect: sym!(b"pa_context_connect\0"),
            context_disconnect: sym!(b"pa_context_disconnect\0"),
            context_get_state: sym!(b"pa_context_get_state\0"),
            context_set_state_cb: sym!(b"pa_context_set_state_callback\0"),
            context_set_subscribe_cb: sym!(b"pa_context_set_subscribe_callback\0"),
            context_subscribe: sym!(b"pa_context_subscribe\0"),
            context_get_server_info: sym!(b"pa_context_get_server_info\0"),
            context_get_sink_by_name: sym!(b"pa_context_get_sink_info_by_name\0"),
            context_set_sink_vol_by_name: sym!(b"pa_context_set_sink_volume_by_name\0"),
            op_get_state: sym!(b"pa_operation_get_state\0"),
            op_unref: sym!(b"pa_operation_unref\0"),
            cvolume_avg: sym!(b"pa_cvolume_avg\0"),
            cvolume_set: sym!(b"pa_cvolume_set\0"),
        })
    }
}

impl Drop for LibPulseApi {
    fn drop(&mut self) {
        if !self.lib_handle.is_null() {
            unsafe { libc::dlclose(self.lib_handle) };
        }
    }
}

struct PulseCallbackSignal {
    api: *const LibPulseApi,
    mainloop: *mut c_void,
    event_tx: mpsc::Sender<()>,
}

unsafe extern "C" fn pa_state_cb(_ctx: *mut c_void, userdata: *mut c_void) {
    if userdata.is_null() {
        return;
    }
    unsafe {
        let sig = &*(userdata as *const PulseCallbackSignal);
        ((*sig.api).mainloop_signal)(sig.mainloop, 0);
        let _ = sig.event_tx.send(());
    }
}

unsafe extern "C" fn pa_subscribe_cb(
    _ctx: *mut c_void,
    t: u32,
    _idx: u32,
    userdata: *mut c_void,
) {
    if userdata.is_null() {
        return;
    }
    let facility = t & PA_SUBSCRIPTION_EVENT_FACILITY_MASK;
    if facility == PA_SUBSCRIPTION_EVENT_SINK || facility == PA_SUBSCRIPTION_EVENT_SERVER {
        unsafe {
            let sig = &*(userdata as *const PulseCallbackSignal);
            let _ = sig.event_tx.send(());
        }
    }
}

struct ServerInfoCapture {
    api: *const LibPulseApi,
    mainloop: *mut c_void,
    default_sink: Option<String>,
    done: bool,
}

unsafe extern "C" fn on_server_info(
    _ctx: *mut c_void,
    info: *const PaServerInfo,
    userdata: *mut c_void,
) {
    unsafe {
        let cap = &mut *(userdata as *mut ServerInfoCapture);
        if !info.is_null() && !(*info).default_sink_name.is_null() {
            let sink = CStr::from_ptr((*info).default_sink_name)
                .to_string_lossy()
                .into_owned();
            if !sink.is_empty() {
                cap.default_sink = Some(sink);
            }
        }
        cap.done = true;
        ((*cap.api).mainloop_signal)(cap.mainloop, 0);
    }
}

struct SinkInfoCapture {
    api: *const LibPulseApi,
    mainloop: *mut c_void,
    found: bool,
    channels: u8,
    percent: u8,
    muted: bool,
    description: Option<String>,
}

unsafe extern "C" fn on_sink_info(
    _ctx: *mut c_void,
    info: *const PaSinkInfo,
    eol: c_int,
    userdata: *mut c_void,
) {
    unsafe {
        let cap = &mut *(userdata as *mut SinkInfoCapture);
        if !info.is_null() && eol == 0 {
            let vol_ptr: *const PaCvolume = &(*info).volume;
            let avg = ((*cap.api).cvolume_avg)(vol_ptr);
            let pct = ((avg as f64 / PA_VOLUME_NORM) * 100.0)
                .round()
                .clamp(0.0, 100.0) as u8;
            cap.found = true;
            cap.channels = (*info).volume.channels.max(1);
            cap.percent = pct;
            cap.muted = (*info).mute != 0;
            if !(*info).description.is_null() {
                cap.description = Some(
                    CStr::from_ptr((*info).description)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
        if eol != 0 {
            ((*cap.api).mainloop_signal)(cap.mainloop, 0);
        }
    }
}

struct SimpleOpDone {
    api: *const LibPulseApi,
    mainloop: *mut c_void,
    success: bool,
    done: bool,
}

unsafe extern "C" fn on_simple_op_complete(
    _ctx: *mut c_void,
    success: c_int,
    userdata: *mut c_void,
) {
    unsafe {
        let cap = &mut *(userdata as *mut SimpleOpDone);
        cap.success = success != 0;
        cap.done = true;
        ((*cap.api).mainloop_signal)(cap.mainloop, 0);
    }
}

struct ActivePulseConn {
    api: Arc<LibPulseApi>,
    mainloop: *mut c_void,
    context: *mut c_void,
    _cb_signal: Box<PulseCallbackSignal>,
}

unsafe impl Send for ActivePulseConn {}
unsafe impl Sync for ActivePulseConn {}

impl ActivePulseConn {
    fn connect(api: Arc<LibPulseApi>, event_tx: mpsc::Sender<()>) -> Result<Self, String> {
        unsafe {
            let mainloop = (api.mainloop_new)();
            if mainloop.is_null() {
                return Err("pa_threaded_mainloop_new failed".into());
            }
            let ml_api = (api.mainloop_get_api)(mainloop);
            let app_name = b"UXF-Control-Center\0";
            let context = (api.context_new)(ml_api, app_name.as_ptr() as *const c_char);
            if context.is_null() {
                (api.mainloop_free)(mainloop);
                return Err("pa_context_new failed".into());
            }

            let mut cb_signal = Box::new(PulseCallbackSignal {
                api: Arc::as_ptr(&api),
                mainloop,
                event_tx,
            });
            let sig_ptr: *mut c_void = (&mut *cb_signal as *mut PulseCallbackSignal).cast();

            (api.context_set_state_cb)(context, Some(pa_state_cb), sig_ptr);
            (api.context_set_subscribe_cb)(context, Some(pa_subscribe_cb), sig_ptr);

            if (api.mainloop_start)(mainloop) < 0 {
                (api.context_unref)(context);
                (api.mainloop_free)(mainloop);
                return Err("pa_threaded_mainloop_start failed".into());
            }

            (api.mainloop_lock)(mainloop);
            let conn_rc = (api.context_connect)(context, std::ptr::null(), 0, std::ptr::null());
            if conn_rc < 0 {
                (api.mainloop_unlock)(mainloop);
                (api.mainloop_stop)(mainloop);
                (api.context_unref)(context);
                (api.mainloop_free)(mainloop);
                return Err("Failed connecting to PipeWire/PulseAudio socket".into());
            }

            let start = Instant::now();
            loop {
                let st = (api.context_get_state)(context);
                if st == PA_CONTEXT_READY {
                    break;
                }
                if st == PA_CONTEXT_FAILED || st == PA_CONTEXT_TERMINATED {
                    (api.mainloop_unlock)(mainloop);
                    (api.context_disconnect)(context);
                    (api.mainloop_stop)(mainloop);
                    (api.context_unref)(context);
                    (api.mainloop_free)(mainloop);
                    return Err("PipeWire/PulseAudio connection rejected or terminated".into());
                }
                if start.elapsed() > Duration::from_secs(2) {
                    (api.mainloop_unlock)(mainloop);
                    (api.context_disconnect)(context);
                    (api.mainloop_stop)(mainloop);
                    (api.context_unref)(context);
                    (api.mainloop_free)(mainloop);
                    return Err("Timed out waiting for PipeWire/PulseAudio ready".into());
                }
                (api.mainloop_wait)(mainloop);
            }

            let sub_op = (api.context_subscribe)(
                context,
                PA_SUBSCRIPTION_MASK_SINK | PA_SUBSCRIPTION_MASK_SERVER,
                None,
                std::ptr::null_mut(),
            );
            if !sub_op.is_null() {
                (api.op_unref)(sub_op);
            }
            (api.mainloop_unlock)(mainloop);

            Ok(Self {
                api,
                mainloop,
                context,
                _cb_signal: cb_signal,
            })
        }
    }

    fn is_ready(&self) -> bool {
        unsafe {
            (self.api.mainloop_lock)(self.mainloop);
            let st = (self.api.context_get_state)(self.context);
            (self.api.mainloop_unlock)(self.mainloop);
            st == PA_CONTEXT_READY
        }
    }

    fn wait_op_locked(&self, op: *mut c_void) {
        if op.is_null() {
            return;
        }
        unsafe {
            let start = Instant::now();
            while (self.api.op_get_state)(op) == PA_OPERATION_RUNNING {
                if start.elapsed() > Duration::from_millis(800) {
                    break;
                }
                (self.api.mainloop_wait)(self.mainloop);
            }
            (self.api.op_unref)(op);
        }
    }

    fn query_sink_and_channels(&self) -> Result<(VolumeState, String, u8), String> {
        if !self.is_ready() {
            return Err("Audio server disconnected".into());
        }

        let default_sink = unsafe {
            let mut cap = ServerInfoCapture {
                api: Arc::as_ptr(&self.api),
                mainloop: self.mainloop,
                default_sink: None,
                done: false,
            };
            (self.api.mainloop_lock)(self.mainloop);
            let op = (self.api.context_get_server_info)(
                self.context,
                Some(on_server_info),
                (&mut cap as *mut ServerInfoCapture).cast(),
            );
            self.wait_op_locked(op);
            (self.api.mainloop_unlock)(self.mainloop);
            cap.default_sink
        };

        let Some(sink_name) = default_sink else {
            return Err("No default audio output device active".into());
        };
        let sink_c = CString::new(sink_name.as_str()).map_err(|e| e.to_string())?;

        let sink_cap = unsafe {
            let mut cap = SinkInfoCapture {
                api: Arc::as_ptr(&self.api),
                mainloop: self.mainloop,
                found: false,
                channels: 2,
                percent: 0,
                muted: false,
                description: None,
            };
            (self.api.mainloop_lock)(self.mainloop);
            let op = (self.api.context_get_sink_by_name)(
                self.context,
                sink_c.as_ptr(),
                Some(on_sink_info),
                (&mut cap as *mut SinkInfoCapture).cast(),
            );
            self.wait_op_locked(op);
            (self.api.mainloop_unlock)(self.mainloop);
            cap
        };

        if !sink_cap.found {
            return Err("Default audio output sink not found".into());
        }

        let state = VolumeState {
            available: true,
            percent: sink_cap.percent,
            muted: sink_cap.muted,
            sink_name: Some(sink_name.clone()),
            sink_description: sink_cap.description,
            error: None,
        };
        Ok((state, sink_name, sink_cap.channels))
    }

    fn set_volume_percent(&self, percent: u8) -> Result<VolumeState, String> {
        let clamped = percent.min(100);
        let (_, sink_name, channels) = self.query_sink_and_channels()?;
        let sink_c = CString::new(sink_name.as_str()).map_err(|e| e.to_string())?;

        let pa_vol = ((clamped as f64 / 100.0) * PA_VOLUME_NORM).round() as u32;
        let mut cvol = PaCvolume {
            channels: 0,
            values: [0; PA_CHANNELS_MAX],
        };
        unsafe {
            (self.api.cvolume_set)(&mut cvol, channels.max(1) as u32, pa_vol);
        }

        let success = unsafe {
            let mut done = SimpleOpDone {
                api: Arc::as_ptr(&self.api),
                mainloop: self.mainloop,
                success: false,
                done: false,
            };
            (self.api.mainloop_lock)(self.mainloop);
            let op = (self.api.context_set_sink_vol_by_name)(
                self.context,
                sink_c.as_ptr(),
                &cvol,
                Some(on_simple_op_complete),
                (&mut done as *mut SimpleOpDone).cast(),
            );
            self.wait_op_locked(op);
            (self.api.mainloop_unlock)(self.mainloop);
            done.success
        };

        if !success {
            return Err("Failed setting PipeWire/PulseAudio sink volume".into());
        }

        let (updated, _, _) = self.query_sink_and_channels()?;
        Ok(updated)
    }
}

impl Drop for ActivePulseConn {
    fn drop(&mut self) {
        unsafe {
            (self.api.mainloop_lock)(self.mainloop);
            (self.api.context_set_state_cb)(self.context, None, std::ptr::null_mut());
            (self.api.context_set_subscribe_cb)(self.context, None, std::ptr::null_mut());
            (self.api.context_disconnect)(self.context);
            (self.api.mainloop_unlock)(self.mainloop);
            (self.api.mainloop_stop)(self.mainloop);
            (self.api.context_unref)(self.context);
            (self.api.mainloop_free)(self.mainloop);
        }
    }
}

struct PulseAudioService {
    api: Option<Arc<LibPulseApi>>,
    conn: Mutex<Option<ActivePulseConn>>,
    event_tx: mpsc::Sender<()>,
}

impl PulseAudioService {
    fn new(event_tx: mpsc::Sender<()>) -> Self {
        let api = LibPulseApi::load().ok().map(Arc::new);
        let initial_conn = api
            .as_ref()
            .and_then(|a| ActivePulseConn::connect(Arc::clone(a), event_tx.clone()).ok());
        Self {
            api,
            conn: Mutex::new(initial_conn),
            event_tx,
        }
    }

    fn ensure_connected_query(&self) -> VolumeState {
        let Some(api) = &self.api else {
            return VolumeState::unavailable("libpulse.so.0 unavailable");
        };

        let Ok(mut guard) = self.conn.lock() else {
            return VolumeState::unavailable("Audio lock poisoned");
        };

        if let Some(existing) = guard.as_ref() {
            if existing.is_ready() {
                if let Ok((state, _, _)) = existing.query_sink_and_channels() {
                    return state;
                }
            }
            *guard = None;
        }

        match ActivePulseConn::connect(Arc::clone(api), self.event_tx.clone()) {
            Ok(new_conn) => {
                let state = match new_conn.query_sink_and_channels() {
                    Ok((s, _, _)) => s,
                    Err(err) => VolumeState::unavailable(err),
                };
                *guard = Some(new_conn);
                state
            }
            Err(err) => VolumeState::unavailable(err),
        }
    }

    fn set_volume_percent(&self, percent: u8) -> Result<VolumeState, String> {
        let api = self
            .api
            .as_ref()
            .ok_or_else(|| "libpulse.so.0 unavailable".to_string())?;
        let mut guard = self
            .conn
            .lock()
            .map_err(|_| "Audio lock poisoned".to_string())?;

        if guard.as_ref().is_some_and(|c| !c.is_ready()) {
            *guard = None;
        }

        if guard.is_none() {
            *guard = Some(ActivePulseConn::connect(
                Arc::clone(api),
                self.event_tx.clone(),
            )?);
        }

        let conn = guard.as_ref().unwrap();
        match conn.set_volume_percent(percent) {
            Ok(state) => Ok(state),
            Err(_) => {
                // Attempt once on fresh connection in case PipeWire restarted.
                *guard = None;
                let fresh = ActivePulseConn::connect(Arc::clone(api), self.event_tx.clone())?;
                let res = fresh.set_volume_percent(percent);
                *guard = Some(fresh);
                res
            }
        }
    }
}

// ============================================================================
// Unified Desktop Controls Manager + Event-Driven Watchers
// ============================================================================

pub struct DesktopControlsManager {
    state: Arc<RwLock<DesktopControlsState>>,
    gio: Option<Arc<GioSettingsLib>>,
    pulse: Arc<PulseAudioService>,
    brightness_lock: Mutex<()>,
    blue_light_lock: Mutex<()>,
    stop: Arc<AtomicBool>,
    pulse_wake: Arc<(Mutex<bool>, Condvar)>,
}

impl DesktopControlsManager {
    pub fn new() -> Arc<Self> {
        let gio = GioSettingsLib::load().ok().map(Arc::new);
        let (pa_tx, pa_rx) = mpsc::channel::<()>();
        let pulse = Arc::new(PulseAudioService::new(pa_tx));

        let initial = DesktopControlsState {
            brightness: query_brightness_blocking(),
            blue_light: query_blue_light_blocking(gio.as_deref()),
            volume: pulse.ensure_connected_query(),
        };

        let mgr = Arc::new(Self {
            state: Arc::new(RwLock::new(initial)),
            gio,
            pulse,
            brightness_lock: Mutex::new(()),
            blue_light_lock: Mutex::new(()),
            stop: Arc::new(AtomicBool::new(false)),
            pulse_wake: Arc::new((Mutex::new(false), Condvar::new())),
        });

        Self::spawn_pulse_event_loop(Arc::clone(&mgr), pa_rx);
        mgr
    }

    pub fn start_event_watchers(self: &Arc<Self>, app: AppHandle) {
        self.spawn_dbus_watcher(app.clone());
        self.spawn_pulse_emitter(app);
    }

    pub fn snapshot(&self) -> DesktopControlsState {
        self.state.read().map(|s| s.clone()).unwrap_or_else(|_| DesktopControlsState {
            brightness: BrightnessState::unavailable("State lock poisoned"),
            blue_light: BlueLightState::unavailable("State lock poisoned"),
            volume: VolumeState::unavailable("State lock poisoned"),
        })
    }

    pub fn refresh_all(&self) -> DesktopControlsState {
        let brightness = query_brightness_blocking();
        let blue_light = query_blue_light_blocking(self.gio.as_deref());
        let volume = self.pulse.ensure_connected_query();
        let next = DesktopControlsState {
            brightness,
            blue_light,
            volume,
        };
        if let Ok(mut guard) = self.state.write() {
            *guard = next.clone();
        }
        next
    }

    pub fn set_brightness(&self, percent: u8) -> Result<DesktopControlsState, String> {
        let _guard = self
            .brightness_lock
            .lock()
            .map_err(|_| "Brightness lock poisoned".to_string())?;
        let updated = set_brightness_blocking(percent)?;
        if let Ok(mut guard) = self.state.write() {
            guard.brightness = updated;
            return Ok(guard.clone());
        }
        Ok(self.snapshot())
    }

    pub fn set_blue_light(&self, level: f32) -> Result<DesktopControlsState, String> {
        let _guard = self
            .blue_light_lock
            .lock()
            .map_err(|_| "Blue light lock poisoned".to_string())?;
        let updated = set_blue_light_blocking(self.gio.as_deref(), level)?;
        if let Ok(mut guard) = self.state.write() {
            guard.blue_light = updated;
            return Ok(guard.clone());
        }
        Ok(self.snapshot())
    }

    pub fn set_volume(&self, percent: u8) -> Result<DesktopControlsState, String> {
        let updated = self.pulse.set_volume_percent(percent)?;
        if let Ok(mut guard) = self.state.write() {
            guard.volume = updated;
            return Ok(guard.clone());
        }
        Ok(self.snapshot())
    }

    fn spawn_pulse_event_loop(mgr: Arc<Self>, pa_rx: mpsc::Receiver<()>) {
        let _ = thread::Builder::new()
            .name("uxf-pulse-events".into())
            .spawn(move || {
                while !mgr.stop.load(Ordering::Relaxed) {
                    match pa_rx.recv_timeout(Duration::from_secs(5)) {
                        Ok(()) => {
                            // Coalesce bursty PulseAudio subscribe notifications briefly.
                            while pa_rx.try_recv().is_ok() {}
                            let vol = mgr.pulse.ensure_connected_query();
                            if let Ok(mut guard) = mgr.state.write() {
                                guard.volume = vol;
                            }
                            let (lock, cvar) = &*mgr.pulse_wake;
                            if let Ok(mut dirty) = lock.lock() {
                                *dirty = true;
                                cvar.notify_all();
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            // Reconnect only if audio was disconnected/unavailable.
                            let needs_retry = mgr
                                .state
                                .read()
                                .map(|s| !s.volume.available)
                                .unwrap_or(false);
                            if needs_retry {
                                let vol = mgr.pulse.ensure_connected_query();
                                if vol.available {
                                    if let Ok(mut guard) = mgr.state.write() {
                                        guard.volume = vol;
                                    }
                                    let (lock, cvar) = &*mgr.pulse_wake;
                                    if let Ok(mut dirty) = lock.lock() {
                                        *dirty = true;
                                        cvar.notify_all();
                                    }
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            });
    }

    fn spawn_pulse_emitter(&self, app: AppHandle) {
        let state = Arc::clone(&self.state);
        let wake = Arc::clone(&self.pulse_wake);
        let stop = Arc::clone(&self.stop);

        let _ = thread::Builder::new()
            .name("uxf-pulse-emit".into())
            .spawn(move || {
                let (lock, cvar) = &*wake;
                while !stop.load(Ordering::Relaxed) {
                    let mut dirty = match lock.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    while !*dirty && !stop.load(Ordering::Relaxed) {
                        let (next, _) = match cvar.wait_timeout(dirty, Duration::from_secs(2)) {
                            Ok(res) => res,
                            Err(_) => return,
                        };
                        dirty = next;
                    }
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    *dirty = false;
                    drop(dirty);

                    if let Ok(snapshot) = state.read().map(|s| s.clone()) {
                        let _ = app.emit(DESKTOP_CONTROLS_EVENT, snapshot);
                    }
                }
            });
    }

    fn spawn_dbus_watcher(&self, app: AppHandle) {
        let state = Arc::clone(&self.state);
        let gio = self.gio.clone();
        let stop = Arc::clone(&self.stop);

        let _ = thread::Builder::new()
            .name("uxf-dbus-controls-watch".into())
            .spawn(move || {
                let Ok(conn) = BlockingConnection::session() else {
                    return;
                };
                {
                    let Ok(dbus_proxy) = DBusProxy::new(&conn) else {
                        return;
                    };

                    let rules = [
                        MatchRule::builder()
                            .msg_type(MessageType::Signal)
                            .path(MUTTER_PATH)
                            .ok()
                            .map(|b| b.build()),
                        MatchRule::builder()
                            .msg_type(MessageType::Signal)
                            .path("/org/gnome/Shell/Brightness")
                            .ok()
                            .map(|b| b.build()),
                        MatchRule::builder()
                            .msg_type(MessageType::Signal)
                            .path(GSD_COLOR_PATH)
                            .ok()
                            .map(|b| b.build()),
                        MatchRule::builder()
                            .msg_type(MessageType::Signal)
                            .interface("ca.desrt.dconf.Writer")
                            .and_then(|b| b.member("Notify"))
                            .ok()
                            .map(|b| b.build()),
                    ];

                    for rule in rules.into_iter().flatten() {
                        let _ = dbus_proxy.add_match_rule(rule);
                    }
                }

                let iter = MessageIterator::from(conn);
                let query_conn = BlockingConnection::session().ok();
                for msg_res in iter {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let Ok(msg) = msg_res else {
                        continue;
                    };
                    let header = msg.header();
                    let path = header.path().map(|p| p.as_str()).unwrap_or("");
                    let member = header.member().map(|m| m.as_str()).unwrap_or("");

                    let mut changed = false;
                    if path == MUTTER_PATH || path == "/org/gnome/Shell/Brightness" {
                        let next_brightness = query_brightness_with_conn(query_conn.as_ref());
                        if let Ok(mut guard) = state.write() {
                            if guard.brightness != next_brightness {
                                guard.brightness = next_brightness;
                                changed = true;
                            }
                        }
                    } else if path == GSD_COLOR_PATH {
                        let next_blue = query_blue_light_with_conn(gio.as_deref(), query_conn.as_ref());
                        if let Ok(mut guard) = state.write() {
                            if guard.blue_light != next_blue {
                                guard.blue_light = next_blue;
                                changed = true;
                            }
                        }
                    } else if member == "Notify" {
                        // Filter dconf writer notifications strictly to color settings prefix.
                        if let Ok((prefix, _, _)) =
                            msg.body().deserialize::<(String, Vec<String>, String)>()
                        {
                            if prefix.starts_with("/org/gnome/settings-daemon/plugins/color/") {
                                let next_blue =
                                    query_blue_light_with_conn(gio.as_deref(), query_conn.as_ref());
                                if let Ok(mut guard) = state.write() {
                                    if guard.blue_light != next_blue {
                                        guard.blue_light = next_blue;
                                        changed = true;
                                    }
                                }
                            }
                        }
                    }

                    if changed {
                        if let Ok(snapshot) = state.read().map(|s| s.clone()) {
                            let _ = app.emit(DESKTOP_CONTROLS_EVENT, snapshot);
                        }
                    }
                }
            });
    }
}

impl Drop for DesktopControlsManager {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let (_, cvar) = &*self.pulse_wake;
        cvar.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kelvin_and_brightness_mappings_stay_within_bounds() {
        assert_eq!(level_to_kelvin(0.0), 4700);
        assert_eq!(level_to_kelvin(5.0), 3200);
        assert_eq!(level_to_kelvin(10.0), 1700);
        assert_eq!(level_to_kelvin(-2.0), 4700);
        assert_eq!(level_to_kelvin(15.0), 1700);
        assert_eq!(kelvin_to_level(4700), 0.0);
        assert_eq!(kelvin_to_level(1700), 10.0);

        assert_eq!(map_percent_to_range(0, 480, 48000), 480);
        assert_eq!(map_percent_to_range(100, 480, 48000), 48000);
        assert_eq!(map_range_to_percent(480, 480, 48000), 0);
        assert_eq!(map_range_to_percent(48000, 480, 48000), 100);
    }

    #[test]
    fn read_only_desktop_controls_probe_succeeds() {
        let mgr = DesktopControlsManager::new();
        let snap = mgr.snapshot();
        assert!(snap.brightness.available, "Expected brightness available: {:?}", snap.brightness.error);
        assert!(snap.brightness.min >= 480 && snap.brightness.max >= 48000);
        assert!(snap.brightness.value >= snap.brightness.min);

        assert!(snap.blue_light.available, "Expected Night Light GSettings available: {:?}", snap.blue_light.error);
        assert!(!snap.blue_light.status_reason.is_empty());

        assert!(snap.volume.available, "Expected PipeWire/libpulse sink available: {:?}", snap.volume.error);
        assert!(snap.volume.sink_name.is_some());
        assert!(snap.volume.percent <= 100);
    }

    #[test]
    #[ignore = "mutates live display brightness and Night Light temporarily; run explicitly"]
    fn live_brightness_and_blue_light_hardware_verification() {
        struct RestoreGuard {
            orig_raw: i32,
            orig_enabled: bool,
            orig_kelvin: u32,
        }
        impl Drop for RestoreGuard {
            fn drop(&mut self) {
                if let Ok(conn) = BlockingConnection::session() {
                    if let Ok(p) = BlockingProxy::new(&conn, MUTTER_DEST, MUTTER_PATH, MUTTER_DEST) {
                        if let Ok((serial, entries)) = p.get_property::<MutterBacklightTuple>("Backlight") {
                            if let Some((connector, _, _, _)) = select_active_backlight(entries) {
                                let _ = p.call::<_, _, ()>("SetBacklight", &(serial, connector.as_str(), self.orig_raw));
                            }
                        }
                    }
                }
                if let Ok(gio) = GioSettingsLib::load() {
                    let _ = gio.with_night_light_settings(|obj| unsafe {
                        let k_en = b"night-light-enabled\0".as_ptr() as *const c_char;
                        let k_tmp = b"night-light-temperature\0".as_ptr() as *const c_char;
                        (gio.set_boolean)(obj, k_en, if self.orig_enabled { 1 } else { 0 });
                        (gio.set_uint)(obj, k_tmp, self.orig_kelvin);
                        (gio.sync_fn)();
                    });
                }
            }
        }

        let mgr = DesktopControlsManager::new();
        let orig = mgr.snapshot();
        assert!(orig.brightness.available && orig.blue_light.available);

        let conn = BlockingConnection::session().expect("session bus");
        let mutter_proxy = BlockingProxy::new(&conn, MUTTER_DEST, MUTTER_PATH, MUTTER_DEST).unwrap();
        let (orig_serial, orig_entries): MutterBacklightTuple =
            mutter_proxy.get_property("Backlight").unwrap();
        let (orig_conn, orig_min, orig_max, orig_raw) = select_active_backlight(orig_entries).unwrap();
        assert!(orig_min >= 480 && orig_max >= 48000);

        let _restore = RestoreGuard {
            orig_raw,
            orig_enabled: orig.blue_light.enabled,
            orig_kelvin: orig.blue_light.kelvin,
        };

        // 1. Brightness UXF -> Mutter -> Sysfs verification & min/max safety bounds
        let low = mgr.set_brightness(0).expect("set 0%");
        assert_eq!(low.brightness.value, low.brightness.min);
        assert_eq!(low.brightness.backend, "mutter");

        let mid = mgr.set_brightness(50).expect("set 50%");
        assert_eq!(mid.brightness.percent, 50);
        assert_eq!(mid.brightness.backend, "mutter");
        let (mid_serial, mid_entries): MutterBacklightTuple =
            mutter_proxy.get_property("Backlight").unwrap();
        let (mid_conn, _, _, mid_mutter_raw) = select_active_backlight(mid_entries).unwrap();
        assert_eq!(mid_mutter_raw, mid.brightness.value);
        assert_eq!(mid_conn, orig_conn);

        // Mutter updates Backlight synchronously and writes sysfs asynchronously via mutter-backlight-helper.
        let mut sysfs_mid = -1;
        for _ in 0..30 {
            if let Ok(raw) = fs::read_to_string("/sys/class/backlight/intel_backlight/brightness") {
                if let Ok(parsed) = raw.trim().parse::<i32>() {
                    sysfs_mid = parsed;
                    if sysfs_mid == mid.brightness.value {
                        break;
                    }
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
        println!(
            "LIVE_BRIGHTNESS_VERIFY: orig_serial={orig_serial} mid_serial={mid_serial} connector={mid_conn} mutter={} sysfs={sysfs_mid}",
            mid.brightness.value
        );
        assert_eq!(sysfs_mid, mid.brightness.value);

        // 2. Blue Light activation + 1700K <-> 4700K + schedule preservation
        let gio = GioSettingsLib::load().unwrap();
        let orig_bl = orig.blue_light.clone();
        let color_proxy = BlockingProxy::new(&conn, GSD_COLOR_DEST, GSD_COLOR_PATH, GSD_COLOR_DEST).unwrap();

        // Reproduce night-light-enabled=false before adjusting Blue Light Filter
        gio.with_night_light_settings(|obj| unsafe {
            let k_en = b"night-light-enabled\0".as_ptr() as *const c_char;
            (gio.set_boolean)(obj, k_en, 0);
            (gio.sync_fn)();
        }).unwrap();
        thread::sleep(Duration::from_millis(100));
        assert!(!color_proxy.get_property::<bool>("NightLightActive").unwrap());

        let warm = mgr.set_blue_light(10.0).expect("set 10 -> 1700K");
        assert!(warm.blue_light.enabled, "Night Light should enable on adjustment");
        assert_eq!(warm.blue_light.kelvin, 1700);
        assert_eq!(warm.blue_light.schedule_automatic, orig_bl.schedule_automatic);
        assert_eq!(warm.blue_light.schedule_from, orig_bl.schedule_from);
        assert_eq!(warm.blue_light.schedule_to, orig_bl.schedule_to);

        // Check actual gsd-color NightLightActive & live smeared Temperature moving toward 1700K
        if !warm.blue_light.schedule_automatic && warm.blue_light.schedule_from == warm.blue_light.schedule_to {
            assert!(warm.blue_light.active, "NightLightActive should be true when schedule permits");
            let mut temp_warm = color_proxy.get_property::<u32>("Temperature").unwrap();
            for _ in 0..35 {
                thread::sleep(Duration::from_millis(200));
                temp_warm = color_proxy.get_property::<u32>("Temperature").unwrap();
                if temp_warm <= 2200 {
                    break;
                }
            }
            let active_now = color_proxy.get_property::<bool>("NightLightActive").unwrap();
            println!("LIVE_NIGHT_LIGHT_1700K: active={active_now} smeared_temp={temp_warm}K");
            assert!(active_now);
            assert!(temp_warm < 3500, "Expected display temperature to drop toward 1700K, got {temp_warm}K");

            let mild = mgr.set_blue_light(0.5).expect("set 0.5 -> 4550K");
            assert_eq!(mild.blue_light.kelvin, 4550);
            assert!(mild.blue_light.active);
            let mut temp_mild = color_proxy.get_property::<u32>("Temperature").unwrap();
            for _ in 0..35 {
                thread::sleep(Duration::from_millis(200));
                temp_mild = color_proxy.get_property::<u32>("Temperature").unwrap();
                if temp_mild >= 4200 {
                    break;
                }
            }
            println!("LIVE_NIGHT_LIGHT_MILD: active={} smeared_temp={temp_mild}K", mild.blue_light.active);
            assert!(temp_mild > temp_warm, "Expected 4550K target ({temp_mild}K) warmer-to-cooler increase vs {temp_warm}K");

            let off = mgr.set_blue_light(0.0).expect("set 0 -> OFF");
            assert!(!off.blue_light.enabled, "Intensity 0 should disable Night Light");
            assert!(!off.blue_light.active, "Intensity 0 should make NightLightActive=false");
            assert_eq!(off.blue_light.level, 0.0);
            assert_eq!(off.blue_light.status_reason, "disabled");
            assert_eq!(off.blue_light.schedule_automatic, orig_bl.schedule_automatic);
            assert_eq!(off.blue_light.schedule_from, orig_bl.schedule_from);
            assert_eq!(off.blue_light.schedule_to, orig_bl.schedule_to);
            let mut temp_off = color_proxy.get_property::<u32>("Temperature").unwrap();
            for _ in 0..35 {
                thread::sleep(Duration::from_millis(200));
                temp_off = color_proxy.get_property::<u32>("Temperature").unwrap();
                if temp_off >= 6200 {
                    break;
                }
            }
            println!("LIVE_NIGHT_LIGHT_ZERO_OFF: enabled={} active={} smeared_temp={temp_off}K", off.blue_light.enabled, off.blue_light.active);
            assert!(temp_off > temp_mild, "Expected neutral temperature climb toward 6500K after setting 0, got {temp_off}K");
        }

        // Restore original Night Light enabled + temperature exactly
        gio.with_night_light_settings(|obj| unsafe {
            let k_en = b"night-light-enabled\0".as_ptr() as *const c_char;
            let k_tmp = b"night-light-temperature\0".as_ptr() as *const c_char;
            (gio.set_boolean)(obj, k_en, if orig_bl.enabled { 1 } else { 0 });
            (gio.set_uint)(obj, k_tmp, orig_bl.kelvin);
            (gio.sync_fn)();
        }).unwrap();
    }
}
