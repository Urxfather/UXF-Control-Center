use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use zbus::blocking::fdo::DBusProxy;
use zbus::blocking::{Connection as BlockingConnection, MessageIterator, Proxy as BlockingProxy};
use zbus::message::Type as MessageType;
use zbus::zvariant::{OwnedValue, Value};
use zbus::MatchRule;

use crate::dbus::{AcerControlDBus, DaemonKeyboardFullState};

pub const KEYBOARD_STATE_EVENT: &str = "keyboard-state-changed";

const DEFAULT_SLEEP_PRESET_SECS: u32 = 5;
const DAEMON_SERVICE: &str = "io.github.stanlysilas.AcerControl";
const DAEMON_PATH: &str = "/io/github/stanlysilas/AcerControl";
const UPOWER_DEST: &str = "org.freedesktop.UPower";
const UPOWER_KBD_PATH: &str = "/org/freedesktop/UPower/KbdBacklight";
const UPOWER_KBD_IFACE: &str = "org.freedesktop.UPower.KbdBacklight";
const GSD_POWER_DEST: &str = "org.gnome.SettingsDaemon.Power";
const GSD_POWER_PATH: &str = "/org/gnome/SettingsDaemon/Power";
const GSD_KBD_IFACE: &str = "org.gnome.SettingsDaemon.Power.Keyboard";
const SELF_GSD_ECHO_TTL_MS: u64 = 1_500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardState {
    pub available: bool,
    pub daemon_connected: bool,
    pub daemon_version: Option<String>,
    pub daemon_supports_full_sync: bool,
    pub brightness_step: u8,
    pub brightness_raw: u8,
    pub brightness_target: u8,
    pub max_brightness: u8,
    pub color: [u8; 3],
    pub effect: String,
    pub sleep_timer_enabled: bool,
    pub sleep_timer_seconds: u32,
    pub idle_dimmed: bool,
    pub idle_status: String,
    pub idle_monitor_supported: bool,
    pub fade_from: u8,
    pub fade_to: u8,
    pub fade_start_mono_ms: u64,
    pub fade_duration_ms: u32,
    pub anim_epoch_ms: u64,
    pub mono_now_ms: u64,
    pub breathing_supported: bool,
    pub rainbow_supported: bool,
    pub rainbow_reason: Option<String>,
    pub boot_effect_supported: bool,
    pub boot_effect_reason: Option<String>,
    pub zones_label: String,
    pub error: Option<String>,
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            available: false,
            daemon_connected: false,
            daemon_version: None,
            daemon_supports_full_sync: false,
            brightness_step: 0,
            brightness_raw: 0,
            brightness_target: 0,
            max_brightness: 255,
            color: [255, 255, 255],
            effect: "static".into(),
            sleep_timer_enabled: false,
            sleep_timer_seconds: DEFAULT_SLEEP_PRESET_SECS,
            idle_dimmed: false,
            idle_status: "Idle dimming off".into(),
            idle_monitor_supported: true,
            fade_from: 0,
            fade_to: 0,
            fade_start_mono_ms: 0,
            fade_duration_ms: 0,
            anim_epoch_ms: 0,
            mono_now_ms: mono_now_ms(),
            breathing_supported: false,
            rainbow_supported: false,
            rainbow_reason: None,
            boot_effect_supported: false,
            boot_effect_reason: Some(
                "Pre-boot lighting override is not exposed by Aspire 7 A715-79G firmware, acer_keyboard driver, or daemon".into(),
            ),
            zones_label: "Single-Zone RGB".into(),
            error: None,
        }
    }
}

pub fn mono_now_ms() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    if rc == 0 && ts.tv_sec >= 0 && ts.tv_nsec >= 0 {
        (ts.tv_sec as u64)
            .saturating_mul(1000)
            .saturating_add((ts.tv_nsec as u64) / 1_000_000)
    } else {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

pub fn step_to_raw(step: u8) -> u8 {
    match step {
        0 => 0,
        1 => 64,
        2 => 128,
        3 => 192,
        _ => 255,
    }
}

pub fn raw_to_step(raw: u8) -> u8 {
    match raw {
        0 => 0,
        1..=96 => 1,
        97..=160 => 2,
        161..=224 => 3,
        _ => 4,
    }
}

pub fn raw_to_gsd_percent(raw: u8, max_brightness: u8) -> i32 {
    if raw == 0 || max_brightness == 0 {
        return 0;
    }
    let max_f = max_brightness as u32;
    let pct = ((raw as u32 * 100) + (max_f / 2)) / max_f;
    pct.clamp(1, 100) as i32
}

pub fn gsd_percent_to_raw(pct: i32, max_brightness: u8) -> u8 {
    let clamped = pct.clamp(0, 100) as u32;
    if clamped == 0 {
        return 0;
    }
    let raw = (clamped * max_brightness as u32 + 50) / 100;
    raw.clamp(1, max_brightness as u32) as u8
}

fn read_sysfs_keyboard_brightness() -> Option<u8> {
    std::fs::read_to_string("/sys/class/leds/rgb:kbd_backlight/brightness")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
}

fn read_sysfs_keyboard_max_brightness() -> u8 {
    std::fs::read_to_string("/sys/class/leds/rgb:kbd_backlight/max_brightness")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .filter(|m| *m > 0)
        .unwrap_or(255)
}

#[cfg(test)]
fn read_upower_brightness(sys_conn: &BlockingConnection) -> Option<u8> {
    let proxy = BlockingProxy::new(sys_conn, UPOWER_DEST, UPOWER_KBD_PATH, UPOWER_KBD_IFACE).ok()?;
    let val: i32 = proxy.call("GetBrightness", &()).ok()?;
    Some(val.clamp(0, 255) as u8)
}

fn read_gsd_kbd_percent(session_conn: &BlockingConnection) -> Option<i32> {
    let proxy = BlockingProxy::new(
        session_conn,
        GSD_POWER_DEST,
        GSD_POWER_PATH,
        "org.freedesktop.DBus.Properties",
    )
    .ok()?;
    let val: OwnedValue = proxy.call("Get", &(GSD_KBD_IFACE, "Brightness")).ok()?;
    i32::try_from(&val).ok()
}

fn set_gsd_kbd_percent(session_conn: &BlockingConnection, pct: i32) -> Result<(), String> {
    let proxy = BlockingProxy::new(
        session_conn,
        GSD_POWER_DEST,
        GSD_POWER_PATH,
        "org.freedesktop.DBus.Properties",
    )
    .map_err(|e| format!("Failed creating GSD Power proxy: {e}"))?;
    proxy
        .call::<_, _, ()>("Set", &(GSD_KBD_IFACE, "Brightness", Value::from(pct)))
        .map_err(|e| format!("GSD Power Keyboard.Brightness Set failed: {e}"))
}

fn format_idle_status(enabled: bool, dimmed: bool, seconds: u32, supported: bool) -> String {
    if !supported {
        return "Daemon sleep timer unavailable".into();
    }
    if dimmed {
        return "Dimmed after inactivity".into();
    }
    if enabled {
        return format!("Fades after {seconds}s idle");
    }
    "Idle dimming off".into()
}

fn build_state_from_full(
    full: DaemonKeyboardFullState,
    daemon_version: Option<String>,
    max_brightness: u8,
) -> KeyboardState {
    KeyboardState {
        available: full.available,
        daemon_connected: true,
        daemon_version,
        daemon_supports_full_sync: true,
        brightness_step: raw_to_step(full.brightness_effective),
        brightness_raw: full.brightness_effective,
        brightness_target: full.brightness_target,
        max_brightness,
        color: [full.r, full.g, full.b],
        effect: full.effect,
        sleep_timer_enabled: full.sleep_timer_enabled,
        sleep_timer_seconds: full.sleep_timer_seconds.max(1),
        idle_dimmed: full.idle_dimmed,
        idle_status: format_idle_status(
            full.sleep_timer_enabled,
            full.idle_dimmed,
            full.sleep_timer_seconds.max(1),
            true,
        ),
        idle_monitor_supported: true,
        fade_from: full.fade_from,
        fade_to: full.fade_to,
        fade_start_mono_ms: full.fade_start_mono_ms,
        fade_duration_ms: full.fade_duration_ms,
        anim_epoch_ms: full.anim_epoch_ms,
        mono_now_ms: full.mono_now_ms,
        breathing_supported: true,
        rainbow_supported: true,
        rainbow_reason: None,
        boot_effect_supported: false,
        boot_effect_reason: Some(
            "Pre-boot lighting override is not exposed by Acer Aspire 7 A715-79G firmware, acer_keyboard driver, or daemon".into(),
        ),
        zones_label: "Single-Zone RGB".into(),
        error: None,
    }
}

fn query_hardware_snapshot(cached_introspect: &Mutex<Option<(String, String)>>) -> KeyboardState {
    let max_brightness = read_sysfs_keyboard_max_brightness();
    let boot_reason = Some(
        "Pre-boot lighting override is not exposed by Acer Aspire 7 A715-79G firmware, acer_keyboard driver, or daemon".into(),
    );

    let client = match AcerControlDBus::new() {
        Ok(c) => c,
        Err(err) => {
            return KeyboardState {
                max_brightness,
                boot_effect_reason: boot_reason,
                error: Some(err),
                ..KeyboardState::default()
            };
        }
    };

    let daemon_version = match client.version() {
        Ok(v) => Some(v),
        Err(err) => {
            return KeyboardState {
                max_brightness,
                boot_effect_reason: boot_reason,
                error: Some(err),
                ..KeyboardState::default()
            };
        }
    };

    // Fast atomic path when upgraded daemon exposes GetKeyboardFullState.
    if let Ok(full) = client.keyboard_full_state() {
        return build_state_from_full(full, daemon_version, max_brightness);
    }

    // Backward-compatible path when talking to an older installed daemon binary.
    let introspect = {
        let mut guard = cached_introspect.lock().unwrap_or_else(|e| e.into_inner());
        let ver_key = daemon_version.clone().unwrap_or_default();
        if let Some((cached_ver, xml)) = guard.as_ref() {
            if cached_ver == &ver_key {
                xml.clone()
            } else {
                let fresh = client.introspect_xml().unwrap_or_default();
                *guard = Some((ver_key, fresh.clone()));
                fresh
            }
        } else {
            let fresh = client.introspect_xml().unwrap_or_default();
            *guard = Some((ver_key, fresh.clone()));
            fresh
        }
    };

    let rainbow_supported = introspect.contains("StartRainbow") && introspect.contains("GetRainbowSelected");
    let rainbow_reason = if rainbow_supported {
        None
    } else {
        Some(format!(
            "Rainbow requires StartRainbow/GetRainbowSelected on root acer-control-daemon (absent in installed v{})",
            daemon_version.as_deref().unwrap_or("unknown")
        ))
    };
    let breathing_supported = introspect.is_empty() || introspect.contains("StartBreathing");

    let brightness_raw = read_sysfs_keyboard_brightness()
        .or_else(|| client.keyboard_brightness().ok())
        .unwrap_or(0);
    let color = client
        .keyboard_color()
        .map(|(r, g, b)| [r, g, b])
        .unwrap_or([255, 255, 255]);
    let rainbow_active = rainbow_supported && client.rainbow_selected().unwrap_or(false);
    let breathing_active = !rainbow_active && client.breathing_selected().unwrap_or(false);
    let effect = if rainbow_active {
        "rainbow".into()
    } else if breathing_active {
        "breathing".into()
    } else {
        "static".into()
    };

    KeyboardState {
        available: true,
        daemon_connected: true,
        daemon_version,
        daemon_supports_full_sync: introspect.contains("SetKeyboardSleepTimer"),
        brightness_step: raw_to_step(brightness_raw),
        brightness_raw,
        brightness_target: brightness_raw,
        max_brightness,
        color,
        effect,
        sleep_timer_enabled: false,
        sleep_timer_seconds: DEFAULT_SLEEP_PRESET_SECS,
        idle_dimmed: false,
        idle_status: if introspect.contains("SetKeyboardSleepTimer") {
            "Idle dimming off".into()
        } else {
            "Daemon upgrade needed for background sleep timer".into()
        },
        idle_monitor_supported: introspect.contains("SetKeyboardSleepTimer"),
        fade_from: brightness_raw,
        fade_to: brightness_raw,
        fade_start_mono_ms: 0,
        fade_duration_ms: 0,
        anim_epoch_ms: 0,
        mono_now_ms: mono_now_ms(),
        breathing_supported,
        rainbow_supported,
        rainbow_reason,
        boot_effect_supported: false,
        boot_effect_reason: boot_reason,
        zones_label: "Single-Zone RGB".into(),
        error: None,
    }
}

pub struct KeyboardManager {
    state: Arc<RwLock<KeyboardState>>,
    op_lock: Mutex<()>,
    introspect_cache: Mutex<Option<(String, String)>>,
    last_non_zero_brightness: Mutex<u8>,
    gsd_sync_tx: Mutex<Option<mpsc::Sender<u8>>>,
    expected_gsd_echoes: Arc<Mutex<VecDeque<(i32, u64)>>>,
    stop: Arc<AtomicBool>,
    app_handle: Mutex<Option<AppHandle>>,
}

impl KeyboardManager {
    pub fn new() -> Arc<Self> {
        let mgr = Arc::new(Self {
            state: Arc::new(RwLock::new(KeyboardState::default())),
            op_lock: Mutex::new(()),
            introspect_cache: Mutex::new(None),
            last_non_zero_brightness: Mutex::new(255),
            gsd_sync_tx: Mutex::new(None),
            expected_gsd_echoes: Arc::new(Mutex::new(VecDeque::new())),
            stop: Arc::new(AtomicBool::new(false)),
            app_handle: Mutex::new(None),
        });
        mgr
    }

    /// Binds Tauri AppHandle and starts non-blocking D-Bus watchers cleanly after window init.
    pub fn start_watchers(self: &Arc<Self>, app: AppHandle) {
        if let Ok(mut guard) = self.app_handle.lock() {
            *guard = Some(app);
        }

        self.spawn_gsd_sync_worker();
        self.spawn_system_dbus_watcher();
        self.spawn_session_gsd_watcher();

        // Initial non-blocking state hydration + one-shot GNOME Quick Settings alignment.
        let mgr = Arc::clone(self);
        let _ = thread::Builder::new()
            .name("uxf-kbd-init".into())
            .spawn(move || {
                let snap = mgr.refresh_state();
                mgr.emit_snapshot(&snap);
                if snap.available && !snap.idle_dimmed {
                    if let Ok(session) = BlockingConnection::session() {
                        let target_pct = raw_to_gsd_percent(snap.brightness_target, snap.max_brightness);
                        if read_gsd_kbd_percent(&session) != Some(target_pct) {
                            mgr.push_gsd_echo(target_pct);
                            let _ = set_gsd_kbd_percent(&session, target_pct);
                        }
                    }
                }
            });
    }

    fn push_gsd_echo(&self, pct: i32) {
        let now = mono_now_ms();
        if let Ok(mut g) = self.expected_gsd_echoes.lock() {
            while let Some(&(_, ts)) = g.front() {
                if now.saturating_sub(ts) > SELF_GSD_ECHO_TTL_MS {
                    g.pop_front();
                } else {
                    break;
                }
            }
            if g.len() >= 16 {
                g.pop_front();
            }
            g.push_back((pct, now));
        }
    }

    fn enqueue_gsd_sync(&self, raw: u8) {
        if let Ok(guard) = self.gsd_sync_tx.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(raw);
            }
        }
    }

    fn spawn_gsd_sync_worker(self: &Arc<Self>) {
        let (tx, rx) = mpsc::channel::<u8>();
        if let Ok(mut g) = self.gsd_sync_tx.lock() {
            *g = Some(tx);
        }
        let mgr = Arc::clone(self);
        let _ = thread::Builder::new()
            .name("uxf-gsd-kbd-sync".into())
            .spawn(move || {
                let mut session: Option<BlockingConnection> = None;
                while !mgr.stop.load(Ordering::Relaxed) {
                    let Ok(mut raw) = rx.recv() else {
                        break;
                    };
                    // Coalesce burst updates and keep latest requested brightness.
                    while let Ok(newer) = rx.try_recv() {
                        raw = newer;
                    }
                    if mgr.stop.load(Ordering::Relaxed) {
                        break;
                    }
                    if session.is_none() {
                        session = BlockingConnection::session().ok();
                    }
                    let Some(conn) = session.as_ref() else {
                        continue;
                    };
                    let max_b = mgr.snapshot().max_brightness.max(1);
                    let target_pct = raw_to_gsd_percent(raw, max_b);
                    if read_gsd_kbd_percent(conn) != Some(target_pct) {
                        mgr.push_gsd_echo(target_pct);
                        if set_gsd_kbd_percent(conn, target_pct).is_err() {
                            session = None;
                        }
                    }
                }
            });
    }

    fn spawn_system_dbus_watcher(self: &Arc<Self>) {
        let mgr = Arc::clone(self);
        let _ = thread::Builder::new()
            .name("uxf-kbd-sys-watch".into())
            .spawn(move || {
                let mut backoff_ms = 250_u64;
                while !mgr.stop.load(Ordering::Relaxed) {
                    let Ok(conn) = BlockingConnection::system() else {
                        thread::sleep(Duration::from_millis(backoff_ms));
                        backoff_ms = (backoff_ms * 2).min(4_000);
                        continue;
                    };
                    backoff_ms = 250;

                    if let Ok(dbus_proxy) = DBusProxy::new(&conn) {
                        let rules = [
                            MatchRule::builder()
                                .msg_type(MessageType::Signal)
                                .interface(DAEMON_SERVICE)
                                .and_then(|b| b.member("KeyboardStateChanged"))
                                .and_then(|b| b.path(DAEMON_PATH))
                                .ok()
                                .map(|b| b.build()),
                            MatchRule::builder()
                                .msg_type(MessageType::Signal)
                                .interface(UPOWER_KBD_IFACE)
                                .and_then(|b| b.member("BrightnessChangedWithSource"))
                                .and_then(|b| b.path(UPOWER_KBD_PATH))
                                .ok()
                                .map(|b| b.build()),
                        ];
                        for rule in rules.into_iter().flatten() {
                            let _ = dbus_proxy.add_match_rule(rule);
                        }
                    }

                    let iter = MessageIterator::from(conn);
                    for msg_res in iter {
                        if mgr.stop.load(Ordering::Relaxed) {
                            break;
                        }
                        let Ok(msg) = msg_res else {
                            break;
                        };
                        let header = msg.header();
                        let iface = header.interface().map(|i| i.as_str()).unwrap_or("");
                        let member = header.member().map(|m| m.as_str()).unwrap_or("");

                        if iface == DAEMON_SERVICE && member == "KeyboardStateChanged" {
                            if let Ok(tuple) = msg.body().deserialize::<(
                                bool,
                                u8,
                                u8,
                                u8,
                                u8,
                                u8,
                                String,
                                bool,
                                u32,
                                bool,
                                u8,
                                u8,
                                u64,
                                u32,
                                u64,
                                u64,
                            )>() {
                                let full = DaemonKeyboardFullState::from(tuple);
                                let ver = mgr.snapshot().daemon_version;
                                let max_b = read_sysfs_keyboard_max_brightness();
                                let snap = build_state_from_full(full, ver, max_b);
                                mgr.update_cached_and_emit(snap);
                            }
                        } else if iface == UPOWER_KBD_IFACE
                            && member == "BrightnessChangedWithSource"
                        {
                            // Fallback signal handling if daemon lacks KeyboardStateChanged.
                            if let Ok((raw, _src)) = msg.body().deserialize::<(i32, String)>() {
                                let cur = mgr.snapshot();
                                if !cur.daemon_supports_full_sync {
                                    let clamped = raw.clamp(0, cur.max_brightness as i32) as u8;
                                    let next = KeyboardState {
                                        brightness_raw: clamped,
                                        brightness_target: clamped,
                                        brightness_step: raw_to_step(clamped),
                                        mono_now_ms: mono_now_ms(),
                                        ..cur
                                    };
                                    mgr.update_cached_and_emit(next);
                                }
                            }
                        }
                    }

                    thread::sleep(Duration::from_millis(250));
                }
            });
    }

    fn spawn_session_gsd_watcher(self: &Arc<Self>) {
        let mgr = Arc::clone(self);
        let _ = thread::Builder::new()
            .name("uxf-kbd-gsd-watch".into())
            .spawn(move || {
                while !mgr.stop.load(Ordering::Relaxed) {
                    let Ok(conn) = BlockingConnection::session() else {
                        thread::sleep(Duration::from_secs(1));
                        continue;
                    };

                    if let Ok(dbus_proxy) = DBusProxy::new(&conn) {
                        if let Ok(rule) = MatchRule::builder()
                            .msg_type(MessageType::Signal)
                            .interface(GSD_KBD_IFACE)
                            .and_then(|b| b.member("BrightnessChanged"))
                            .and_then(|b| b.path(GSD_POWER_PATH))
                            .map(|b| b.build())
                        {
                            let _ = dbus_proxy.add_match_rule(rule);
                        }
                    }

                    let iter = MessageIterator::from(conn);
                    for msg_res in iter {
                        if mgr.stop.load(Ordering::Relaxed) {
                            break;
                        }
                        let Ok(msg) = msg_res else {
                            break;
                        };
                        let header = msg.header();
                        if header.interface().map(|i| i.as_str()) == Some(GSD_KBD_IFACE)
                            && header.member().map(|m| m.as_str()) == Some("BrightnessChanged")
                        {
                            if let Ok((pct, _source)) = msg.body().deserialize::<(i32, String)>() {
                                let now = mono_now_ms();
                                let is_self = {
                                    let mut g = mgr
                                        .expected_gsd_echoes
                                        .lock()
                                        .unwrap_or_else(|e| e.into_inner());
                                    let pos = g.iter().position(|&(p, ts)| {
                                        p == pct && now.saturating_sub(ts) <= SELF_GSD_ECHO_TTL_MS
                                    });
                                    if let Some(idx) = pos {
                                        g.remove(idx);
                                        true
                                    } else {
                                        false
                                    }
                                };

                                if is_self {
                                    continue;
                                }

                                if pct == 0 {
                                    if let Ok(client) = AcerControlDBus::new() {
                                        if client.notify_external_off().is_err() {
                                            let _ = client.set_keyboard_brightness(0);
                                        }
                                    }
                                    let snap = mgr.refresh_state();
                                    mgr.emit_snapshot(&snap);
                                } else {
                                    let max_b = mgr.snapshot().max_brightness.max(1);
                                    let target_raw = gsd_percent_to_raw(pct, max_b);
                                    if !mgr.snapshot().daemon_supports_full_sync {
                                        if let Ok(client) = AcerControlDBus::new() {
                                            let _ = client.set_keyboard_brightness(target_raw);
                                        }
                                    }
                                    let snap = mgr.refresh_state();
                                    mgr.emit_snapshot(&snap);
                                }
                            }
                        }
                    }

                    thread::sleep(Duration::from_millis(300));
                }
            });
    }

    pub fn snapshot(&self) -> KeyboardState {
        self.state.read().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn refresh_state(&self) -> KeyboardState {
        let next = query_hardware_snapshot(&self.introspect_cache);
        self.update_cached_and_emit(next.clone());
        next
    }

    fn update_cached_and_emit(&self, next: KeyboardState) {
        if next.brightness_target > 0 {
            if let Ok(mut last) = self.last_non_zero_brightness.lock() {
                *last = next.brightness_target;
            }
        }
        if let Ok(mut guard) = self.state.write() {
            *guard = next.clone();
        }
        self.emit_snapshot(&next);
    }

    fn emit_snapshot(&self, snapshot: &KeyboardState) {
        if let Ok(guard) = self.app_handle.lock() {
            if let Some(app) = guard.as_ref() {
                let _ = app.emit(KEYBOARD_STATE_EVENT, snapshot.clone());
            }
        }
    }

    pub fn set_brightness_raw(&self, raw: u8, commit: bool) -> Result<KeyboardState, String> {
        let _guard = self
            .op_lock
            .lock()
            .map_err(|_| "Keyboard lock poisoned".to_string())?;
        let client = AcerControlDBus::new()?;
        client.set_keyboard_brightness_commit(raw, commit)?;

        if !self.snapshot().daemon_supports_full_sync {
            // Sync UPower directly if talking to legacy daemon without UPower sync.
            if commit {
                if let Ok(sys_conn) = BlockingConnection::system() {
                    if let Ok(proxy) = BlockingProxy::new(
                        &sys_conn,
                        UPOWER_DEST,
                        UPOWER_KBD_PATH,
                        UPOWER_KBD_IFACE,
                    ) {
                        let _ = proxy.call::<_, _, ()>("SetBrightness", &(raw as i32));
                    }
                }
            }
        }

        if commit {
            self.enqueue_gsd_sync(raw);
        }

        if raw > 0 {
            if let Ok(mut last) = self.last_non_zero_brightness.lock() {
                *last = raw;
            }
        }

        Ok(self.refresh_state())
    }

    pub fn set_brightness_step(&self, step: u8) -> Result<KeyboardState, String> {
        self.set_brightness_raw(step_to_raw(step.min(4)), true)
    }

    pub fn toggle_backlight(&self) -> Result<KeyboardState, String> {
        let cur = self.snapshot();
        let target = if cur.brightness_raw > 0 || cur.brightness_target > 0 {
            0
        } else {
            self.last_non_zero_brightness
                .lock()
                .map(|g| (*g).max(1))
                .unwrap_or(255)
        };
        self.set_brightness_raw(target, true)
    }

    pub fn set_color(&self, r: u8, g: u8, b: u8, commit: Option<bool>) -> Result<KeyboardState, String> {
        let _guard = self
            .op_lock
            .lock()
            .map_err(|_| "Keyboard lock poisoned".to_string())?;
        let client = AcerControlDBus::new()?;
        let do_commit = commit.unwrap_or(true);
        client.set_keyboard_color_commit(r, g, b, do_commit)?;
        Ok(self.refresh_state())
    }

    pub fn set_effect(&self, effect: &str) -> Result<KeyboardState, String> {
        let _guard = self
            .op_lock
            .lock()
            .map_err(|_| "Keyboard lock poisoned".to_string())?;
        let normalized = effect.trim().to_ascii_lowercase();
        let client = AcerControlDBus::new()?;

        match normalized.as_str() {
            "static" => client.stop_animation()?,
            "breathing" => {
                let b = read_sysfs_keyboard_brightness()
                    .or_else(|| client.keyboard_brightness().ok())
                    .unwrap_or(255);
                if b == 0 {
                    client.set_keyboard_brightness_commit(255, true)?;
                    self.enqueue_gsd_sync(255);
                }
                client.start_breathing()?;
            }
            "rainbow" => {
                let b = read_sysfs_keyboard_brightness()
                    .or_else(|| client.keyboard_brightness().ok())
                    .unwrap_or(255);
                if b == 0 {
                    client.set_keyboard_brightness_commit(255, true)?;
                    self.enqueue_gsd_sync(255);
                }
                client.start_rainbow()?;
            }
            other => return Err(format!("Unsupported keyboard lighting effect: {other}")),
        }
        Ok(self.refresh_state())
    }

    pub fn set_sleep_timer(&self, enabled: bool, seconds: u32) -> Result<KeyboardState, String> {
        let _guard = self
            .op_lock
            .lock()
            .map_err(|_| "Keyboard lock poisoned".to_string())?;
        let clamped_secs = seconds.clamp(1, 86_400);
        let client = AcerControlDBus::new()?;
        client.set_keyboard_sleep_timer(enabled, clamped_secs)?;
        Ok(self.refresh_state())
    }
}

impl Drop for KeyboardManager {
    fn drop(&mut self) {
        // Closing GUI ONLY stops local D-Bus watcher loops — NEVER stops daemon timer or alters LEDs.
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_continuous_mapping_matches_gsd_percent() {
        assert_eq!(raw_to_gsd_percent(0, 255), 0);
        assert_eq!(raw_to_gsd_percent(1, 255), 1);
        assert_eq!(raw_to_gsd_percent(128, 255), 50);
        assert_eq!(raw_to_gsd_percent(255, 255), 100);

        assert_eq!(gsd_percent_to_raw(0, 255), 0);
        assert_eq!(gsd_percent_to_raw(1, 255), 3);
        assert_eq!(gsd_percent_to_raw(50, 255), 128);
        assert_eq!(gsd_percent_to_raw(100, 255), 255);
    }

    #[test]
    fn brightness_step_mapping_is_consistent() {
        assert_eq!(step_to_raw(0), 0);
        assert_eq!(step_to_raw(1), 64);
        assert_eq!(step_to_raw(2), 128);
        assert_eq!(step_to_raw(3), 192);
        assert_eq!(step_to_raw(4), 255);
        for s in 0..=4 {
            assert_eq!(raw_to_step(step_to_raw(s)), s);
        }
    }

    #[test]
    fn read_only_keyboard_daemon_probe_succeeds() {
        let mgr = KeyboardManager::new();
        let st = mgr.refresh_state();
        assert!(st.daemon_connected, "Acer daemon should be reachable: {:?}", st.error);
        assert!(st.available, "Keyboard RGB control should be available");
        assert_eq!(st.max_brightness, 255);
        assert!(st.brightness_raw <= st.max_brightness);
        assert!(st.effect == "static" || st.effect == "breathing" || st.effect == "rainbow");
        assert!(st.breathing_supported);
        assert!(st.rainbow_supported);
        assert!(!st.boot_effect_supported, "Pre-boot effect override is unsupported by firmware");
    }

    #[test]
    fn upower_and_sysfs_brightness_interfaces_align_on_host() {
        if let Some(sys_val) = read_sysfs_keyboard_brightness() {
            if let Ok(conn) = BlockingConnection::system() {
                if let Some(up_val) = read_upower_brightness(&conn) {
                    assert_eq!(
                        sys_val, up_val,
                        "UPower ({up_val}) and sysfs ({sys_val}) keyboard brightness diverged"
                    );
                }
            }
        }
    }
}
