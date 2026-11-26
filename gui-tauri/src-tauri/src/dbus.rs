use serde::Serialize;
use zbus::blocking::{Connection, Proxy};

const SERVICE: &str = "io.github.stanlysilas.AcerControl";
const PATH: &str = "/io/github/stanlysilas/AcerControl";
const INTERFACE: &str = "io.github.stanlysilas.AcerControl";

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

pub struct AcerControlDBus {
    connection: Connection,
}

impl AcerControlDBus {
    pub fn new() -> Result<Self, String> {
        let connection = Connection::system()
            .map_err(|e| format!("Failed to connect to System D-Bus: {e}"))?;
        Ok(Self { connection })
    }

    fn proxy(&self) -> Result<Proxy<'_>, String> {
        Proxy::new(&self.connection, SERVICE, PATH, INTERFACE)
            .map_err(|e| format!("Failed to create daemon D-Bus proxy: {e}"))
    }

    pub fn version(&self) -> Result<String, String> {
        let proxy = self.proxy()?;
        proxy
            .call("GetVersion", &())
            .map_err(|e| format!("Daemon unreachable (GetVersion failed): {e}"))
    }

    pub fn fan_max_selected(&self) -> Result<bool, String> {
        let proxy = self.proxy()?;
        proxy
            .call("GetFanMaxSelected", &())
            .map_err(|e| format!("GetFanMaxSelected failed: {e}"))
    }

    pub fn set_fan_auto(&self) -> Result<(), String> {
        let proxy = self.proxy()?;
        proxy
            .call::<_, _, ()>("SetFanAuto", &())
            .map_err(|e| format!("SetFanAuto failed: {e}"))
    }

    pub fn set_fan_max(&self) -> Result<(), String> {
        let proxy = self.proxy()?;
        proxy
            .call::<_, _, ()>("SetFanMax", &())
            .map_err(|e| format!("SetFanMax failed: {e}"))
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
        let proxy = self.proxy()?;
        proxy
            .call("GetKeyboardBrightness", &())
            .map_err(|e| format!("GetKeyboardBrightness failed: {e}"))
    }

    pub fn set_keyboard_brightness(&self, value: u8) -> Result<(), String> {
        let proxy = self.proxy()?;
        proxy
            .call::<_, _, ()>("SetKeyboardBrightness", &(value))
            .map_err(|e| format!("SetKeyboardBrightness failed: {e}"))
    }

    pub fn set_keyboard_brightness_transient(&self, value: u8) -> Result<(), String> {
        let proxy = self.proxy()?;
        proxy
            .call::<_, _, ()>("SetKeyboardBrightnessTransient", &(value))
            .map_err(|e| format!("SetKeyboardBrightnessTransient failed: {e}"))
    }

    pub fn keyboard_color(&self) -> Result<(u8, u8, u8), String> {
        let proxy = self.proxy()?;
        proxy
            .call("GetKeyboardColor", &())
            .map_err(|e| format!("GetKeyboardColor failed: {e}"))
    }

    pub fn set_keyboard_color(&self, r: u8, g: u8, b: u8) -> Result<(), String> {
        let proxy = self.proxy()?;
        proxy
            .call::<_, _, ()>("SetKeyboardColor", &(r, g, b))
            .map_err(|e| format!("SetKeyboardColor failed: {e}"))
    }

    pub fn start_breathing(&self) -> Result<(), String> {
        let proxy = self.proxy()?;
        proxy
            .call::<_, _, ()>("StartBreathing", &())
            .map_err(|e| format!("StartBreathing failed: {e}"))
    }

    pub fn start_rainbow(&self) -> Result<(), String> {
        let proxy = self.proxy()?;
        proxy
            .call::<_, _, ()>("StartRainbow", &())
            .map_err(|e| format!("StartRainbow failed: {e}"))
    }

    pub fn stop_animation(&self) -> Result<(), String> {
        let proxy = self.proxy()?;
        proxy
            .call::<_, _, ()>("StopAnimation", &())
            .map_err(|e| format!("StopAnimation failed: {e}"))
    }

    pub fn breathing_selected(&self) -> Result<bool, String> {
        let proxy = self.proxy()?;
        proxy
            .call("GetBreathingSelected", &())
            .map_err(|e| format!("GetBreathingSelected failed: {e}"))
    }

    pub fn rainbow_selected(&self) -> Result<bool, String> {
        let proxy = self.proxy()?;
        proxy
            .call("GetRainbowSelected", &())
            .map_err(|e| format!("GetRainbowSelected failed: {e}"))
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
