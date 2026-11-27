use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use zbus::blocking::{Connection, Proxy};

const SERVICE: &str = "io.github.stanlysilas.AcerControl";
const PATH: &str = "/io/github/stanlysilas/AcerControl";
const INTERFACE: &str = "io.github.stanlysilas.AcerControl";
const DBUS_CALL_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FanStatus {
    pub mode: String,
    pub daemon_connected: bool,
    pub daemon_version: Option<String>,
    pub error: Option<String>,
}

impl FanStatus {
    pub fn disconnected(err: impl Into<String>) -> Self {
        Self {
            mode: "unavailable".into(),
            daemon_connected: false,
            daemon_version: None,
            error: Some(err.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DaemonKeyboardFullState {
    pub available: bool,
    pub brightness_target: u8,
    pub brightness_effective: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub effect: String,
    pub sleep_timer_enabled: bool,
    pub sleep_timer_seconds: u32,
    pub idle_dimmed: bool,
    pub fade_from: u8,
    pub fade_to: u8,
    pub fade_start_mono_ms: u64,
    pub fade_duration_ms: u32,
    pub anim_epoch_ms: u64,
    pub mono_now_ms: u64,
}

type FullStateTuple = (
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
);

impl From<FullStateTuple> for DaemonKeyboardFullState {
    fn from(t: FullStateTuple) -> Self {
        Self {
            available: t.0,
            brightness_target: t.1,
            brightness_effective: t.2,
            r: t.3,
            g: t.4,
            b: t.5,
            effect: t.6,
            sleep_timer_enabled: t.7,
            sleep_timer_seconds: t.8,
            idle_dimmed: t.9,
            fade_from: t.10,
            fade_to: t.11,
            fade_start_mono_ms: t.12,
            fade_duration_ms: t.13,
            anim_epoch_ms: t.14,
            mono_now_ms: t.15,
        }
    }
}

static SHARED_SYS_BUS: Mutex<Option<Connection>> = Mutex::new(None);

fn get_shared_system_connection() -> Result<Connection, String> {
    let mut guard = SHARED_SYS_BUS
        .lock()
        .map_err(|_| "System D-Bus pool lock poisoned".to_string())?;
    if let Some(conn) = guard.as_ref() {
        return Ok(conn.clone());
    }
    let conn = zbus::blocking::connection::Builder::system()
        .and_then(|b| b.method_timeout(DBUS_CALL_TIMEOUT).build())
        .map_err(|e| format!("Failed to connect to System D-Bus: {e}"))?;
    *guard = Some(conn.clone());
    Ok(conn)
}

fn invalidate_shared_system_connection() {
    if let Ok(mut guard) = SHARED_SYS_BUS.lock() {
        *guard = None;
    }
}

pub struct AcerControlDBus {
    connection: Connection,
}

impl AcerControlDBus {
    pub fn new() -> Result<Self, String> {
        let connection = get_shared_system_connection()?;
        Ok(Self { connection })
    }

    fn proxy(&self) -> Result<Proxy<'_>, String> {
        Proxy::new(&self.connection, SERVICE, PATH, INTERFACE)
            .map_err(|e| format!("Failed to create daemon D-Bus proxy: {e}"))
    }

    fn call_daemon<A, R>(&self, method: &str, args: &A) -> Result<R, String>
    where
        A: serde::ser::Serialize + zbus::zvariant::DynamicType,
        R: for<'d> serde::de::Deserialize<'d> + zbus::zvariant::Type,
    {
        match self.proxy()?.call(method, args) {
            Ok(v) => Ok(v),
            Err(err) => {
                invalidate_shared_system_connection();
                Err(format!("{method} failed: {err}"))
            }
        }
    }

    pub fn version(&self) -> Result<String, String> {
        self.call_daemon("GetVersion", &())
    }

    pub fn fan_max_selected(&self) -> Result<bool, String> {
        self.call_daemon("GetFanMaxSelected", &())
    }

    pub fn set_fan_auto(&self) -> Result<(), String> {
        self.call_daemon("SetFanAuto", &())
    }

    pub fn set_fan_max(&self) -> Result<(), String> {
        self.call_daemon("SetFanMax", &())
    }

    pub fn introspect_xml(&self) -> Result<String, String> {
        let proxy = Proxy::new(
            &self.connection,
            SERVICE,
            PATH,
            "org.freedesktop.DBus.Introspectable",
        )
        .map_err(|e| format!("Failed to create Introspectable proxy: {e}"))?;
        proxy
            .call("Introspect", &())
            .map_err(|e| format!("Introspect failed: {e}"))
    }

    pub fn keyboard_brightness(&self) -> Result<u8, String> {
        self.call_daemon("GetKeyboardBrightness", &())
    }

    pub fn set_keyboard_brightness(&self, value: u8) -> Result<(), String> {
        self.call_daemon("SetKeyboardBrightness", &(value))
    }

    pub fn set_keyboard_brightness_commit(&self, value: u8, commit: bool) -> Result<(), String> {
        match self.proxy()?.call::<_, _, ()>("SetKeyboardBrightnessCommit", &(value, commit)) {
            Ok(()) => Ok(()),
            Err(_) => {
                if commit {
                    self.set_keyboard_brightness(value)
                } else {
                    self.set_keyboard_brightness_transient(value)
                }
            }
        }
    }

    pub fn set_keyboard_brightness_transient(&self, value: u8) -> Result<(), String> {
        self.call_daemon("SetKeyboardBrightnessTransient", &(value))
    }

    pub fn keyboard_color(&self) -> Result<(u8, u8, u8), String> {
        self.call_daemon("GetKeyboardColor", &())
    }

    pub fn set_keyboard_color(&self, r: u8, g: u8, b: u8) -> Result<(), String> {
        self.call_daemon("SetKeyboardColor", &(r, g, b))
    }

    pub fn set_keyboard_color_commit(&self, r: u8, g: u8, b: u8, commit: bool) -> Result<(), String> {
        match self
            .proxy()?
            .call::<_, _, ()>("SetKeyboardColorCommit", &(r, g, b, commit))
        {
            Ok(()) => Ok(()),
            Err(_) => self.set_keyboard_color(r, g, b),
        }
    }

    pub fn start_breathing(&self) -> Result<(), String> {
        self.call_daemon("StartBreathing", &())
    }

    pub fn start_rainbow(&self) -> Result<(), String> {
        self.call_daemon("StartRainbow", &())
    }

    pub fn stop_animation(&self) -> Result<(), String> {
        self.call_daemon("StopAnimation", &())
    }

    pub fn breathing_selected(&self) -> Result<bool, String> {
        self.call_daemon("GetBreathingSelected", &())
    }

    pub fn rainbow_selected(&self) -> Result<bool, String> {
        self.call_daemon("GetRainbowSelected", &())
    }

    pub fn set_keyboard_sleep_timer(&self, enabled: bool, seconds: u32) -> Result<(), String> {
        self.call_daemon("SetKeyboardSleepTimer", &(enabled, seconds))
    }

    pub fn notify_external_off(&self) -> Result<(), String> {
        self.call_daemon("NotifyExternalOff", &())
    }

    pub fn keyboard_full_state(&self) -> Result<DaemonKeyboardFullState, String> {
        let tuple: FullStateTuple = self.call_daemon("GetKeyboardFullState", &())?;
        Ok(DaemonKeyboardFullState::from(tuple))
    }

    pub fn query_fan_status(&self) -> FanStatus {
        let version = match self.version() {
            Ok(v) => v,
            Err(err) => return FanStatus::disconnected(err),
        };

        match self.fan_max_selected() {
            Ok(is_max) => FanStatus {
                mode: if is_max { "max".into() } else { "auto".into() },
                daemon_connected: true,
                daemon_version: Some(version),
                error: None,
            },
            Err(err) => FanStatus {
                mode: "unavailable".into(),
                daemon_connected: false,
                daemon_version: Some(version),
                error: Some(err),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_fan_status_query_connects_read_only() {
        let client = AcerControlDBus::new().expect("System D-Bus connection should succeed");
        let status = client.query_fan_status();
        assert!(status.daemon_connected, "Daemon should be reachable: {:?}", status.error);
        assert!(status.daemon_version.is_some());
        assert!(status.mode == "auto" || status.mode == "max");
    }
}
