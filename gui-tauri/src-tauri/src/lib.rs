mod acercontrol;
mod dbus;
mod desktop_controls;
mod health;
#[cfg(test)]
mod health_test;
mod keyboard;
mod preferences;
mod telemetry;

use std::sync::Arc;
use tauri::State;

use dbus::{AcerControlDBus, FanStatus};
use desktop_controls::{DesktopControlsManager, DesktopControlsState};
use health::{check_app_update, check_system_health, detect_installed_version, SystemHealthReport, UpdateCheckResult};
use keyboard::{KeyboardManager, KeyboardState};
use preferences::{PreferencesManager, PreferencesState};
use telemetry::{SystemIdentity, TelemetryCollector, TelemetrySnapshot};

#[tauri::command]
fn get_app_version() -> String {
    detect_installed_version()
}

#[tauri::command]
fn get_system_health() -> SystemHealthReport {
    check_system_health()
}

#[tauri::command]
fn check_updates(current_version: String) -> UpdateCheckResult {
    check_app_update(&current_version)
}

#[tauri::command]
fn get_system_identity(collector: State<'_, TelemetryCollector>) -> SystemIdentity {
    collector.identity()
}

#[tauri::command]
fn get_telemetry_snapshot(collector: State<'_, TelemetryCollector>) -> TelemetrySnapshot {
    collector.snapshot()
}

#[tauri::command]
async fn get_fan_status() -> FanStatus {
    tauri::async_runtime::spawn_blocking(|| match AcerControlDBus::new() {
        Ok(client) => client.query_fan_status(),
        Err(err) => FanStatus::disconnected(err),
    })
    .await
    .unwrap_or_else(|e| FanStatus::disconnected(format!("Fan status task panicked: {e}")))
}

#[tauri::command]
async fn set_fan_mode(mode: String) -> Result<FanStatus, String> {
    let normalized = mode.trim().to_ascii_lowercase();
    if normalized != "auto" && normalized != "max" {
        return Err(format!("Unsupported fan mode requested: {mode}"));
    }

    tauri::async_runtime::spawn_blocking(move || {
        let client = AcerControlDBus::new()?;
        match normalized.as_str() {
            "auto" => client.set_fan_auto()?,
            "max" => client.set_fan_max()?,
            _ => unreachable!(),
        }
        let status = client.query_fan_status();
        if !status.daemon_connected {
            return Err(status
                .error
                .unwrap_or_else(|| "Daemon failed to confirm fan mode".into()));
        }
        Ok(status)
    })
    .await
    .map_err(|e| format!("Fan mode command failed: {e}"))?
}

#[tauri::command]
async fn get_desktop_controls_state(
    controls: State<'_, Arc<DesktopControlsManager>>,
) -> Result<DesktopControlsState, String> {
    let mgr = Arc::clone(controls.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.refresh_all())
        .await
        .map_err(|e| format!("Desktop controls refresh task failed: {e}"))
}

#[tauri::command]
async fn set_screen_brightness(
    percent: u8,
    controls: State<'_, Arc<DesktopControlsManager>>,
) -> Result<DesktopControlsState, String> {
    let mgr = Arc::clone(controls.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.set_brightness(percent))
        .await
        .map_err(|e| format!("Set brightness task failed: {e}"))?
}

#[tauri::command]
async fn set_blue_light_filter(
    level: f32,
    controls: State<'_, Arc<DesktopControlsManager>>,
) -> Result<DesktopControlsState, String> {
    let mgr = Arc::clone(controls.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.set_blue_light(level))
        .await
        .map_err(|e| format!("Set blue light filter task failed: {e}"))?
}

#[tauri::command]
async fn set_system_volume(
    percent: u8,
    controls: State<'_, Arc<DesktopControlsManager>>,
) -> Result<DesktopControlsState, String> {
    let mgr = Arc::clone(controls.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.set_volume(percent))
        .await
        .map_err(|e| format!("Set system volume task failed: {e}"))?
}

#[tauri::command]
async fn get_keyboard_state(
    keyboard: State<'_, Arc<KeyboardManager>>,
) -> Result<KeyboardState, String> {
    let mgr = Arc::clone(keyboard.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.refresh_state())
        .await
        .map_err(|e| format!("Keyboard refresh task failed: {e}"))
}

#[tauri::command]
async fn set_keyboard_brightness_step(
    step: u8,
    keyboard: State<'_, Arc<KeyboardManager>>,
) -> Result<KeyboardState, String> {
    let mgr = Arc::clone(keyboard.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.set_brightness_step(step))
        .await
        .map_err(|e| format!("Set keyboard brightness task failed: {e}"))?
}

#[tauri::command]
async fn set_keyboard_color(
    r: u8,
    g: u8,
    b: u8,
    keyboard: State<'_, Arc<KeyboardManager>>,
) -> Result<KeyboardState, String> {
    let mgr = Arc::clone(keyboard.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.set_color(r, g, b))
        .await
        .map_err(|e| format!("Set keyboard color task failed: {e}"))?
}

#[tauri::command]
async fn set_keyboard_effect(
    effect: String,
    keyboard: State<'_, Arc<KeyboardManager>>,
) -> Result<KeyboardState, String> {
    let mgr = Arc::clone(keyboard.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.set_effect(&effect))
        .await
        .map_err(|e| format!("Set keyboard effect task failed: {e}"))?
}

#[tauri::command]
async fn set_keyboard_sleep_timer(
    enabled: bool,
    seconds: u32,
    keyboard: State<'_, Arc<KeyboardManager>>,
) -> Result<KeyboardState, String> {
    let mgr = Arc::clone(keyboard.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.set_sleep_timer(enabled, seconds))
        .await
        .map_err(|e| format!("Set keyboard sleep timer task failed: {e}"))?
}

#[tauri::command]
async fn get_preferences_state(
    preferences: State<'_, Arc<PreferencesManager>>,
) -> Result<PreferencesState, String> {
    let mgr = Arc::clone(preferences.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.get_state())
        .await
        .map_err(|e| format!("Get preferences task failed: {e}"))
}

#[tauri::command]
async fn set_preference(
    id: String,
    enabled: bool,
    preferences: State<'_, Arc<PreferencesManager>>,
) -> Result<PreferencesState, String> {
    let mgr = Arc::clone(preferences.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.set_preference(&id, enabled))
        .await
        .map_err(|e| format!("Set preference task failed: {e}"))?
}

#[tauri::command]
async fn toggle_preference(
    id: String,
    preferences: State<'_, Arc<PreferencesManager>>,
) -> Result<PreferencesState, String> {
    let mgr = Arc::clone(preferences.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.toggle_preference(&id))
        .await
        .map_err(|e| format!("Toggle preference task failed: {e}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let collector = TelemetryCollector::new();
    let controls = DesktopControlsManager::new();
    let controls_for_setup = Arc::clone(&controls);
    let keyboard = KeyboardManager::new();
    let keyboard_for_setup = Arc::clone(&keyboard);
    let preferences = Arc::new(PreferencesManager::new());
    let preferences_for_setup = Arc::clone(&preferences);

    tauri::Builder::default()
        .manage(collector)
        .manage(controls)
        .manage(keyboard)
        .manage(preferences)
        .setup(move |app| {
            let handle = app.handle().clone();
            controls_for_setup.start_event_watchers(handle.clone());
            keyboard_for_setup.bind_app(handle.clone());
            preferences_for_setup.start_watchers(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_version,
            get_system_health,
            check_updates,
            get_system_identity,
            get_telemetry_snapshot,
            get_fan_status,
            set_fan_mode,
            get_desktop_controls_state,
            set_screen_brightness,
            set_blue_light_filter,
            set_system_volume,
            get_keyboard_state,
            set_keyboard_brightness_step,
            set_keyboard_color,
            set_keyboard_effect,
            set_keyboard_sleep_timer,
            get_preferences_state,
            set_preference,
            toggle_preference,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
