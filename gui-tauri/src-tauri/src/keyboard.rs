use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use zbus::blocking::{Connection as BlockingConnection, Proxy as BlockingProxy};

use crate::dbus::AcerControlDBus;

pub const KEYBOARD_STATE_EVENT: &str = "keyboard-state-changed";

const DEFAULT_SLEEP_PRESET_SECS: u32 = 5;
const FADE_DURATION: Duration = Duration::from_millis(900);
const FADE_TICK: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardState {
    pub available: bool,
    pub daemon_connected: bool,
    pub daemon_version: Option<String>,
    pub brightness_step: u8,
    pub brightness_raw: u8,
    pub color: [u8; 3],
    pub effect: String,
    pub sleep_timer_enabled: bool,
    pub sleep_timer_seconds: u32,
    pub idle_dimmed: bool,
    pub idle_status: String,
    pub idle_monitor_supported: bool,
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
            brightness_step: 0,
            brightness_raw: 0,
            color: [255, 255, 255],
            effect: "static".into(),
            sleep_timer_enabled: false,
            sleep_timer_seconds: DEFAULT_SLEEP_PRESET_SECS,
            idle_dimmed: false,
            idle_status: "Idle dimming disabled".into(),
            idle_monitor_supported: false,
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

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("acer-control-center")
}

fn timeout_path() -> PathBuf {
    config_dir().join("keyboard-idle-timeout")
}

fn preset_path() -> PathBuf {
    config_dir().join("keyboard-idle-preset")
}

fn load_sleep_timer_config() -> (bool, u32) {
    let active_secs = std::fs::read_to_string(timeout_path())
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|s| *s <= 86_400)
        .unwrap_or(0);

    if active_secs > 0 {
        return (true, active_secs);
    }

    let preset_secs = std::fs::read_to_string(preset_path())
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|s| (1..=86_400).contains(s))
        .unwrap_or(DEFAULT_SLEEP_PRESET_SECS);

    (false, preset_secs)
}

fn save_sleep_timer_config(enabled: bool, seconds: u32) -> Result<(), String> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed creating config dir: {e}"))?;

    let clamped_preset = seconds.clamp(1, 86_400);
    let tmp_preset = preset_path().with_extension("tmp");
    std::fs::write(&tmp_preset, format!("{clamped_preset}\n"))
        .and_then(|()| std::fs::rename(&tmp_preset, preset_path()))
        .map_err(|e| format!("Failed saving idle preset: {e}"))?;

    let active_value = if enabled { clamped_preset } else { 0 };
    let tmp_active = timeout_path().with_extension("tmp");
    std::fs::write(&tmp_active, format!("{active_value}\n"))
        .and_then(|()| std::fs::rename(&tmp_active, timeout_path()))
        .map_err(|e| format!("Failed saving idle timeout: {e}"))?;

    Ok(())
}

fn query_session_idle_seconds(session_conn: &mut Option<BlockingConnection>) -> Result<u32, String> {
    if session_conn.is_none() {
        *session_conn = Some(
            BlockingConnection::session()
                .map_err(|e| format!("Session D-Bus unavailable: {e}"))?,
        );
    }
    let conn = session_conn.as_ref().expect("session bus initialized");

    if let Ok(mutter) = BlockingProxy::new(
        conn,
        "org.gnome.Mutter.IdleMonitor",
        "/org/gnome/Mutter/IdleMonitor/Core",
        "org.gnome.Mutter.IdleMonitor",
    ) {
        if let Ok(idle_ms) = mutter.call::<_, _, u64>("GetIdletime", &()) {
            return Ok((idle_ms / 1000).min(u32::MAX as u64) as u32);
        }
    }

    for (service, path) in [
        ("org.freedesktop.ScreenSaver", "/org/freedesktop/ScreenSaver"),
        ("org.freedesktop.ScreenSaver", "/ScreenSaver"),
        ("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver"),
    ] {
        if let Ok(proxy) = BlockingProxy::new(conn, service, path, "org.freedesktop.ScreenSaver") {
            if let Ok(idle_secs) = proxy.call::<_, _, u32>("GetSessionIdleTime", &()) {
                return Ok(idle_secs);
            }
        }
    }

    Err("No supported session idle monitor interface available".into())
}

fn probe_idle_monitor_supported() -> bool {
    let mut conn = None;
    query_session_idle_seconds(&mut conn).is_ok()
}

fn query_hardware_snapshot(idle_dimmed: bool, idle_status: String, idle_monitor_supported: bool) -> KeyboardState {
    let (sleep_timer_enabled, sleep_timer_seconds) = load_sleep_timer_config();
    let boot_reason = Some(
        "Pre-boot lighting override is not exposed by Acer Aspire 7 A715-79G firmware, acer_keyboard driver, or daemon".into(),
    );

    let client = match AcerControlDBus::new() {
        Ok(c) => c,
        Err(err) => {
            return KeyboardState {
                sleep_timer_enabled,
                sleep_timer_seconds,
                idle_dimmed,
                idle_status,
                idle_monitor_supported,
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
                sleep_timer_enabled,
                sleep_timer_seconds,
                idle_dimmed,
                idle_status,
                idle_monitor_supported,
                boot_effect_reason: boot_reason,
                error: Some(err),
                ..KeyboardState::default()
            };
        }
    };

    let introspect = client.introspect_xml().unwrap_or_default();
    let rainbow_supported = introspect.contains("StartRainbow") && introspect.contains("GetRainbowSelected");
    let rainbow_reason = if rainbow_supported {
        None
    } else {
        Some(format!(
            "Rainbow requires StartRainbow/GetRainbowSelected on root acer-control-daemon (absent in installed v{}); 1-zone RGB hardware has no firmware rainbow EC mode, kbd_backlight_mode sysfs node, or /dev/acer_io LED control",
            daemon_version.as_deref().unwrap_or("unknown")
        ))
    };
    let breathing_supported = introspect.is_empty() || introspect.contains("StartBreathing");

    let brightness_raw = match client.keyboard_brightness() {
        Ok(b) => b,
        Err(err) => {
            return KeyboardState {
                available: false,
                daemon_connected: true,
                daemon_version,
                sleep_timer_enabled,
                sleep_timer_seconds,
                idle_dimmed,
                idle_status,
                idle_monitor_supported,
                breathing_supported,
                rainbow_supported,
                rainbow_reason,
                boot_effect_supported: false,
                boot_effect_reason: boot_reason,
                zones_label: "Single-Zone RGB".into(),
                error: Some(err),
                ..KeyboardState::default()
            };
        }
    };

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
        brightness_step: raw_to_step(brightness_raw),
        brightness_raw,
        color,
        effect,
        sleep_timer_enabled,
        sleep_timer_seconds,
        idle_dimmed,
        idle_status,
        idle_monitor_supported,
        breathing_supported,
        rainbow_supported,
        rainbow_reason,
        boot_effect_supported: false,
        boot_effect_reason: boot_reason,
        zones_label: "Single-Zone RGB".into(),
        error: None,
    }
}

enum IdleCmd {
    Configure { enabled: bool, seconds: u32 },
    SetBaseBrightness(u8),
    Stop,
}

pub struct KeyboardManager {
    state: Arc<RwLock<KeyboardState>>,
    op_lock: Mutex<()>,
    idle_tx: Mutex<Option<mpsc::Sender<IdleCmd>>>,
    stop: Arc<AtomicBool>,
    idle_status: Arc<Mutex<String>>,
    idle_dimmed: Arc<AtomicBool>,
    idle_monitor_supported: bool,
    app_handle: Mutex<Option<AppHandle>>,
}

impl KeyboardManager {
    pub fn new() -> Arc<Self> {
        let idle_monitor_supported = probe_idle_monitor_supported();
        let (enabled, seconds) = load_sleep_timer_config();
        let initial_idle_status = if !enabled {
            "Idle dimming off".to_string()
        } else if idle_monitor_supported {
            format!("Fades after {seconds}s idle")
        } else {
            "Session idle monitor unavailable".to_string()
        };

        let initial_state = query_hardware_snapshot(
            false,
            initial_idle_status.clone(),
            idle_monitor_supported,
        );
        let initial_brightness = initial_state.brightness_raw;

        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let idle_dimmed = Arc::new(AtomicBool::new(false));
        let idle_status = Arc::new(Mutex::new(initial_idle_status));

        let mgr = Arc::new(Self {
            state: Arc::new(RwLock::new(initial_state)),
            op_lock: Mutex::new(()),
            idle_tx: Mutex::new(Some(tx)),
            stop,
            idle_status,
            idle_dimmed,
            idle_monitor_supported,
            app_handle: Mutex::new(None),
        });

        Self::spawn_idle_worker(Arc::clone(&mgr), rx, enabled, seconds, initial_brightness);
        mgr
    }

    pub fn bind_app(&self, app: AppHandle) {
        if let Ok(mut guard) = self.app_handle.lock() {
            *guard = Some(app);
        }
    }

    pub fn refresh_state(&self) -> KeyboardState {
        let dimmed = self.idle_dimmed.load(Ordering::Relaxed);
        let status = self
            .idle_status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "Idle status unavailable".into());
        let next = query_hardware_snapshot(dimmed, status, self.idle_monitor_supported);
        if let Ok(mut guard) = self.state.write() {
            *guard = next.clone();
        }
        next
    }

    fn emit_snapshot(&self, snapshot: &KeyboardState) {
        if let Ok(guard) = self.app_handle.lock() {
            if let Some(app) = guard.as_ref() {
                let _ = app.emit(KEYBOARD_STATE_EVENT, snapshot.clone());
            }
        }
    }

    pub fn set_brightness_step(&self, step: u8) -> Result<KeyboardState, String> {
        let _guard = self.op_lock.lock().map_err(|_| "Keyboard lock poisoned".to_string())?;
        let raw = step_to_raw(step.min(4));
        let client = AcerControlDBus::new()?;
        let was_breathing = client.breathing_selected().unwrap_or(false);

        client.set_keyboard_brightness(raw)?;
        if was_breathing && raw > 0 {
            let _ = client.start_breathing();
        }
        if let Ok(tx_guard) = self.idle_tx.lock() {
            if let Some(tx) = tx_guard.as_ref() {
                let _ = tx.send(IdleCmd::SetBaseBrightness(raw));
            }
        }
        self.idle_dimmed.store(false, Ordering::Relaxed);
        Ok(self.refresh_state())
    }

    pub fn set_color(&self, r: u8, g: u8, b: u8) -> Result<KeyboardState, String> {
        let _guard = self.op_lock.lock().map_err(|_| "Keyboard lock poisoned".to_string())?;
        let client = AcerControlDBus::new()?;
        let was_breathing = client.breathing_selected().unwrap_or(false);
        let brightness = client.keyboard_brightness().unwrap_or(255);

        client.set_keyboard_color(r, g, b)?;
        if was_breathing && brightness > 0 {
            let _ = client.start_breathing();
        }
        Ok(self.refresh_state())
    }

    pub fn set_effect(&self, effect: &str) -> Result<KeyboardState, String> {
        let _guard = self.op_lock.lock().map_err(|_| "Keyboard lock poisoned".to_string())?;
        let normalized = effect.trim().to_ascii_lowercase();
        let client = AcerControlDBus::new()?;

        match normalized.as_str() {
            "static" => client.stop_animation()?,
            "breathing" => {
                let brightness = client.keyboard_brightness().unwrap_or(255);
                if brightness == 0 {
                    client.set_keyboard_brightness(step_to_raw(4))?;
                }
                client.start_breathing()?;
            }
            "rainbow" => {
                let introspect = client.introspect_xml().unwrap_or_default();
                if introspect.contains("StartRainbow") && introspect.contains("GetRainbowSelected") {
                    let brightness = client.keyboard_brightness().unwrap_or(255);
                    if brightness == 0 {
                        client.set_keyboard_brightness(step_to_raw(4))?;
                    }
                    client.start_rainbow()?;
                } else {
                    return Err(
                        "Rainbow spectrum mode requires StartRainbow/GetRainbowSelected on acer-control-daemon (absent in installed binary; no firmware EC or /dev/acer_io LED interface available)"
                            .into(),
                    );
                }
            }
            other => return Err(format!("Unsupported keyboard lighting effect: {other}")),
        }
        Ok(self.refresh_state())
    }

    pub fn set_sleep_timer(&self, enabled: bool, seconds: u32) -> Result<KeyboardState, String> {
        let _guard = self.op_lock.lock().map_err(|_| "Keyboard lock poisoned".to_string())?;
        if enabled && !self.idle_monitor_supported {
            return Err("Session idle monitor unavailable on current desktop session".into());
        }
        let clamped_secs = seconds.clamp(1, 86_400);
        save_sleep_timer_config(enabled, clamped_secs)?;

        let status_text = if enabled {
            format!("Fades after {clamped_secs}s idle")
        } else {
            "Idle dimming off".to_string()
        };
        if let Ok(mut s) = self.idle_status.lock() {
            *s = status_text;
        }
        if !enabled {
            self.idle_dimmed.store(false, Ordering::Relaxed);
        }

        if let Ok(tx_guard) = self.idle_tx.lock() {
            if let Some(tx) = tx_guard.as_ref() {
                let _ = tx.send(IdleCmd::Configure {
                    enabled,
                    seconds: clamped_secs,
                });
            }
        }
        Ok(self.refresh_state())
    }

    fn spawn_idle_worker(
        mgr: Arc<Self>,
        rx: mpsc::Receiver<IdleCmd>,
        mut enabled: bool,
        mut timeout_secs: u32,
        mut base_brightness: u8,
    ) {
        let _ = thread::Builder::new()
            .name("uxf-kbd-idle".into())
            .spawn(move || {
                let mut session_conn: Option<BlockingConnection> = None;
                let mut daemon_bus: Option<AcerControlDBus> = None;
                let mut is_idle = false;
                let mut idle_hits = 0_u8;
                let mut active_hits = 0_u8;
                let mut current = base_brightness as f32;
                let mut trans_from = current;
                let mut trans_to = current;
                let mut trans_started = Instant::now();
                let mut last_sent = Some(base_brightness);
                let mut next_poll = Instant::now();

                while !mgr.stop.load(Ordering::Relaxed) {
                    let wait = if !enabled && (current - base_brightness as f32).abs() < 0.5 {
                        Duration::from_millis(400)
                    } else {
                        FADE_TICK
                    };

                    match rx.recv_timeout(wait) {
                        Ok(IdleCmd::Configure { enabled: next_en, seconds }) => {
                            enabled = next_en;
                            timeout_secs = seconds.max(1);
                            if !enabled {
                                is_idle = false;
                                idle_hits = 0;
                                active_hits = 0;
                            }
                            next_poll = Instant::now();
                        }
                        Ok(IdleCmd::SetBaseBrightness(raw)) => {
                            base_brightness = raw;
                            current = raw as f32;
                            trans_from = current;
                            trans_to = if is_idle && enabled { 0.0 } else { current };
                            trans_started = Instant::now();
                            last_sent = Some(raw);
                        }
                        Ok(IdleCmd::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }

                    let now = Instant::now();
                    if enabled && base_brightness > 0 && now >= next_poll {
                        match query_session_idle_seconds(&mut session_conn) {
                            Ok(idle_secs) => {
                                if is_idle {
                                    if idle_secs <= 1 {
                                        active_hits = active_hits.saturating_add(1);
                                        if active_hits >= 2 {
                                            is_idle = false;
                                            idle_hits = 0;
                                        }
                                    } else {
                                        active_hits = 0;
                                    }
                                } else if idle_secs >= timeout_secs {
                                    idle_hits = idle_hits.saturating_add(1);
                                    if idle_hits >= 2 {
                                        is_idle = true;
                                        active_hits = 0;
                                    }
                                } else {
                                    idle_hits = 0;
                                }
                                let new_status = if is_idle {
                                    "Dimmed after inactivity".to_string()
                                } else {
                                    format!("Fades after {timeout_secs}s idle")
                                };
                                let prev_dimmed = mgr.idle_dimmed.swap(is_idle, Ordering::Relaxed);
                                if let Ok(mut s) = mgr.idle_status.lock() {
                                    *s = new_status;
                                }
                                if prev_dimmed != is_idle {
                                    let snap = mgr.refresh_state();
                                    mgr.emit_snapshot(&snap);
                                }
                            }
                            Err(_) => {
                                session_conn = None;
                            }
                        }
                        next_poll = now + Duration::from_millis(500);
                    }

                    let target = if enabled && is_idle {
                        0.0
                    } else {
                        base_brightness as f32
                    };
                    if (target - trans_to).abs() > 0.01 {
                        trans_from = current;
                        trans_to = target;
                        trans_started = now;
                    }

                    let progress = (now.duration_since(trans_started).as_secs_f32()
                        / FADE_DURATION.as_secs_f32())
                    .clamp(0.0, 1.0);
                    current = trans_from + (trans_to - trans_from) * progress;
                    let output = current.round().clamp(0.0, 255.0) as u8;
                    if last_sent != Some(output) {
                        if daemon_bus.is_none() {
                            daemon_bus = AcerControlDBus::new().ok();
                        }
                        if let Some(bus) = daemon_bus.as_ref() {
                            if bus.set_keyboard_brightness_transient(output).is_ok() {
                                last_sent = Some(output);
                            } else {
                                daemon_bus = None;
                            }
                        }
                    }
                }

                if is_idle && base_brightness > 0 {
                    if let Ok(bus) = AcerControlDBus::new() {
                        let _ = bus.set_keyboard_brightness_transient(base_brightness);
                    }
                }
            });
    }
}

impl Drop for KeyboardManager {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(mut guard) = self.idle_tx.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(IdleCmd::Stop);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(st.brightness_step <= 4);
        assert!(st.effect == "static" || st.effect == "breathing" || st.effect == "rainbow");
        assert!(st.breathing_supported);
        if st.rainbow_supported {
            assert!(st.rainbow_reason.is_none());
        } else {
            assert!(st.rainbow_reason.is_some());
        }
        assert!(!st.boot_effect_supported, "Pre-boot effect override is unsupported by firmware");
    }

    #[test]
    #[ignore = "temporarily mutates physical keyboard LED color/brightness/breathing and restores original state"]
    fn live_keyboard_hardware_round_trip_verification() {
        struct KbdRestoreGuard {
            orig_raw: u8,
            orig_color: [u8; 3],
            orig_breathing: bool,
        }
        impl Drop for KbdRestoreGuard {
            fn drop(&mut self) {
                if let Ok(bus) = AcerControlDBus::new() {
                    let _ = bus.stop_animation();
                    let _ = bus.set_keyboard_color(self.orig_color[0], self.orig_color[1], self.orig_color[2]);
                    let _ = bus.set_keyboard_brightness(self.orig_raw);
                    if self.orig_breathing {
                        let _ = bus.start_breathing();
                    }
                }
            }
        }

        let mgr = KeyboardManager::new();
        let orig = mgr.refresh_state();
        assert!(orig.available);
        let _restore = KbdRestoreGuard {
            orig_raw: orig.brightness_raw,
            orig_color: orig.color,
            orig_breathing: orig.effect == "breathing",
        };

        // 1. Test Brightness steps 0..4 and sysfs readback
        let off = mgr.set_brightness_step(0).unwrap();
        assert_eq!(off.brightness_step, 0);
        assert_eq!(off.brightness_raw, 0);
        assert_eq!(
            std::fs::read_to_string("/sys/class/leds/rgb:kbd_backlight/brightness").unwrap().trim(),
            "0"
        );

        let lvl4 = mgr.set_brightness_step(4).unwrap();
        assert_eq!(lvl4.brightness_step, 4);
        assert_eq!(lvl4.brightness_raw, 255);
        assert_eq!(
            std::fs::read_to_string("/sys/class/leds/rgb:kbd_backlight/brightness").unwrap().trim(),
            "255"
        );

        // 2. Test RGB preset & custom RGB color update + sysfs readback
        let purple = mgr.set_color(147, 51, 234).unwrap();
        assert_eq!(purple.color, [147, 51, 234]);
        assert_eq!(
            std::fs::read_to_string("/sys/class/leds/rgb:kbd_backlight/multi_intensity").unwrap().trim(),
            "147 51 234"
        );

        // 3. Test Breathing start/stop & unsupported Rainbow rejection
        let breath = mgr.set_effect("breathing").unwrap();
        assert_eq!(breath.effect, "breathing");
        let stat = mgr.set_effect("static").unwrap();
        assert_eq!(stat.effect, "static");
        assert!(mgr.set_effect("rainbow").is_err());

        println!(
            "LIVE_KEYBOARD_VERIFY: daemon={} brightness=0..255 color=147,51,234 breathing=ok rainbow=rejected",
            orig.daemon_version.as_deref().unwrap_or("?")
        );
    }
}
