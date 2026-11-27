mod acercontrol;
mod dbus;
mod desktop_controls;
mod health;
#[cfg(test)]
mod health_test;
mod keyboard;
mod preferences;
mod system_center;
mod system_info;
mod telemetry;

use std::process::Command;
use std::sync::Arc;
use tauri::State;

use dbus::{AcerControlDBus, FanStatus};
use desktop_controls::{DesktopControlsManager, DesktopControlsState};
use health::{SystemHealthReport, UpdateCheckResult};
use keyboard::{KeyboardManager, KeyboardState};
use preferences::{PreferencesManager, PreferencesState};
use system_center::{
    CachedHealthEnvelope, CachedUpdatesEnvelope, SystemCenterManager,
    SystemCenterOverviewSnapshot,
};
use system_info::SystemInformationReport;
use telemetry::{SystemIdentity, TelemetryCollector, TelemetrySnapshot};

#[tauri::command]
async fn get_app_version(sc: State<'_, Arc<SystemCenterManager>>) -> Result<String, String> {
    Ok(sc.installed_version())
}

#[tauri::command]
fn get_capture_mode() -> bool {
    std::env::var("UXF_GALLERY_CAPTURE").is_ok()
}

#[tauri::command]
fn report_capture_step(step: String) {
    if let Ok(mut file) = std::fs::File::create("/tmp/uxf_capture_step.txt") {
        use std::io::Write;
        let _ = writeln!(file, "{step}");
    }
}

#[tauri::command]
async fn get_system_health(
    sc: State<'_, Arc<SystemCenterManager>>,
) -> Result<SystemHealthReport, String> {
    let mgr = Arc::clone(sc.inner());
    tauri::async_runtime::spawn_blocking(move || {
        mgr.get_or_refresh_health(true)
            .report
            .ok_or_else(|| "System health report unavailable".to_string())
    })
    .await
    .map_err(|e| format!("System health task failed: {e}"))?
}

#[tauri::command]
async fn get_system_health_cached(
    sc: State<'_, Arc<SystemCenterManager>>,
) -> Result<CachedHealthEnvelope, String> {
    Ok(sc.peek_health())
}

#[tauri::command]
async fn refresh_system_health(
    force: Option<bool>,
    sc: State<'_, Arc<SystemCenterManager>>,
) -> Result<CachedHealthEnvelope, String> {
    let mgr = Arc::clone(sc.inner());
    let force_run = force.unwrap_or(true);
    tauri::async_runtime::spawn_blocking(move || mgr.get_or_refresh_health(force_run))
        .await
        .map_err(|e| format!("Refresh health task failed: {e}"))
}

#[tauri::command]
async fn get_system_center_overview(
    sc: State<'_, Arc<SystemCenterManager>>,
) -> Result<SystemCenterOverviewSnapshot, String> {
    let mgr = Arc::clone(sc.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.get_overview_snapshot())
        .await
        .map_err(|e| format!("Overview snapshot task failed: {e}"))
}

#[tauri::command]
async fn get_updates_cached(
    sc: State<'_, Arc<SystemCenterManager>>,
) -> Result<CachedUpdatesEnvelope, String> {
    Ok(sc.peek_updates())
}

#[tauri::command]
async fn check_updates(
    current_version: String,
    force: Option<bool>,
    sc: State<'_, Arc<SystemCenterManager>>,
) -> Result<UpdateCheckResult, String> {
    let mgr = Arc::clone(sc.inner());
    let force_run = force.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || mgr.check_updates_dedup(&current_version, force_run))
        .await
        .map_err(|e| format!("Update check task failed: {e}"))
}

#[tauri::command]
async fn get_system_information(
    force: Option<bool>,
    sc: State<'_, Arc<SystemCenterManager>>,
    collector: State<'_, TelemetryCollector>,
) -> Result<SystemInformationReport, String> {
    let mgr = Arc::clone(sc.inner());
    let identity = collector.identity();
    let force_run = force.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || mgr.get_system_information(&identity, force_run))
        .await
        .map_err(|e| format!("System information task failed: {e}"))
}

#[tauri::command]
async fn open_external_url(url: String) -> Result<(), String> {
    let trimmed = url.trim().to_string();
    const ALLOWED_PREFIX: &str = "https://github.com/Urxfather/UXF-Control-Center";
    if !trimmed.starts_with(ALLOWED_PREFIX) {
        return Err("Blocked URL outside approved repository scope".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        Command::new("xdg-open")
            .arg(&trimmed)
            .spawn()
            .map_err(|e| format!("Failed to open browser: {e}"))?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Open URL task failed: {e}"))?
}

#[tauri::command]
async fn get_system_identity(collector: State<'_, TelemetryCollector>) -> Result<SystemIdentity, String> {
    Ok(collector.identity())
}

#[tauri::command]
async fn get_telemetry_snapshot(
    collector: State<'_, TelemetryCollector>,
) -> Result<TelemetrySnapshot, String> {
    Ok(collector.snapshot())
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
async fn set_fan_mode(
    mode: String,
    sc: State<'_, Arc<SystemCenterManager>>,
) -> Result<FanStatus, String> {
    let normalized = mode.trim().to_ascii_lowercase();
    if normalized != "auto" && normalized != "max" {
        return Err(format!("Unsupported fan mode requested: {mode}"));
    }

    let mgr = Arc::clone(sc.inner());
    let mode_label = normalized.clone();
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
        mgr.record_activity(
            "fan",
            format!(
                "Cooling fan mode switched to {}",
                if mode_label == "max" { "Maximum" } else { "Auto" }
            ),
            "acer-control-daemon",
            "info",
        );
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
async fn set_keyboard_brightness(
    raw: u8,
    commit: Option<bool>,
    keyboard: State<'_, Arc<KeyboardManager>>,
    sc: State<'_, Arc<SystemCenterManager>>,
) -> Result<KeyboardState, String> {
    let mgr = Arc::clone(keyboard.inner());
    let sc_mgr = Arc::clone(sc.inner());
    let do_commit = commit.unwrap_or(true);
    let res = tauri::async_runtime::spawn_blocking(move || mgr.set_brightness_raw(raw, do_commit))
        .await
        .map_err(|e| format!("Set keyboard brightness task failed: {e}"))??;
    if do_commit {
        sc_mgr.record_activity(
            "kbd-bright",
            if raw == 0 {
                "Keyboard backlight turned OFF".into()
            } else {
                format!(
                    "Keyboard backlight brightness set to {}/{}",
                    raw, res.max_brightness
                )
            },
            "acer-control-daemon",
            "info",
        );
    }
    Ok(res)
}

#[tauri::command]
async fn toggle_keyboard_backlight(
    keyboard: State<'_, Arc<KeyboardManager>>,
    sc: State<'_, Arc<SystemCenterManager>>,
) -> Result<KeyboardState, String> {
    let mgr = Arc::clone(keyboard.inner());
    let sc_mgr = Arc::clone(sc.inner());
    let res = tauri::async_runtime::spawn_blocking(move || mgr.toggle_backlight())
        .await
        .map_err(|e| format!("Toggle keyboard backlight task failed: {e}"))??;
    sc_mgr.record_activity(
        "kbd-bright",
        if res.brightness_target == 0 {
            "Keyboard backlight turned OFF".into()
        } else {
            format!(
                "Keyboard backlight turned ON ({}/{})",
                res.brightness_target, res.max_brightness
            )
        },
        "acer-control-daemon",
        "info",
    );
    Ok(res)
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
    commit: Option<bool>,
    keyboard: State<'_, Arc<KeyboardManager>>,
) -> Result<KeyboardState, String> {
    let mgr = Arc::clone(keyboard.inner());
    tauri::async_runtime::spawn_blocking(move || mgr.set_color(r, g, b, commit))
        .await
        .map_err(|e| format!("Set keyboard color task failed: {e}"))?
}

#[tauri::command]
async fn set_keyboard_effect(
    effect: String,
    keyboard: State<'_, Arc<KeyboardManager>>,
    sc: State<'_, Arc<SystemCenterManager>>,
) -> Result<KeyboardState, String> {
    let mgr = Arc::clone(keyboard.inner());
    let sc_mgr = Arc::clone(sc.inner());
    let eff_label = effect.clone();
    let res = tauri::async_runtime::spawn_blocking(move || mgr.set_effect(&effect))
        .await
        .map_err(|e| format!("Set keyboard effect task failed: {e}"))??;
    sc_mgr.record_activity(
        "kbd",
        format!("Keyboard RGB lighting effect set to {eff_label}"),
        "acer-control-daemon",
        "info",
    );
    Ok(res)
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
    let collector_identity = collector.identity_handle();
    let controls = DesktopControlsManager::new_deferred();
    let controls_for_setup = Arc::clone(&controls);
    let keyboard = KeyboardManager::new();
    let keyboard_for_setup = Arc::clone(&keyboard);
    let preferences = Arc::new(PreferencesManager::new());
    let preferences_for_setup = Arc::clone(&preferences);
    let system_center = SystemCenterManager::new();
    let system_center_for_setup = Arc::clone(&system_center);

    tauri::Builder::default()
        .manage(collector)
        .manage(controls)
        .manage(keyboard)
        .manage(preferences)
        .manage(system_center)
        .setup(move |app| {
            let handle = app.handle().clone();
            controls_for_setup.start_event_watchers(handle.clone());
            keyboard_for_setup.start_watchers(handle.clone());
            preferences_for_setup.start_watchers(handle.clone());
            system_center_for_setup.bind_app(handle);
            system_center_for_setup.start_warmup(collector_identity);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_capture_mode,
            report_capture_step,
            get_app_version,
            get_system_health,
            get_system_health_cached,
            refresh_system_health,
            get_system_center_overview,
            get_updates_cached,
            check_updates,
            get_system_information,
            open_external_url,
            get_system_identity,
            get_telemetry_snapshot,
            get_fan_status,
            set_fan_mode,
            get_desktop_controls_state,
            set_screen_brightness,
            set_blue_light_filter,
            set_system_volume,
            get_keyboard_state,
            set_keyboard_brightness,
            toggle_keyboard_backlight,
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
