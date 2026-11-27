use std::fs;
use std::os::raw::c_int;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::animation::{HardwareAnimationBackend, KeyboardDaemonSnapshot};
use crate::backend::{HardwareBackend, HardwareCapabilities};
use crate::ffi;
use crate::kbd_controller::{KeyboardController, KeyboardPlatformIo};
use crate::state::{StateStore, UserState};
use crate::sysfs::SysfsBackend;
use crate::upower::{UPowerBridge, UPowerSyncHandle};

const DAEMON_BOOT_MARKER: &str = "/run/acer-control-daemon.initialized";

struct SystemKeyboardIo {
    sysfs: SysfsBackend,
    upower_sync: UPowerSyncHandle,
    state: StateStore,
}

impl KeyboardPlatformIo for SystemKeyboardIo {
    fn read_brightness_sysfs(&self) -> Result<u8, String> {
        self.sysfs.get_keyboard_brightness()
    }

    fn write_brightness_synced(&self, value: u8) -> Result<(), String> {
        // Always update physical sysfs immediately, then enqueue async UPower sync without blocking.
        let sysfs_res = self.sysfs.set_keyboard_brightness(value);
        let _ = self.upower_sync.set_brightness_nonblocking(value);
        sysfs_res
    }

    fn write_brightness_transient(&self, value: u8) -> Result<(), String> {
        self.sysfs.set_keyboard_brightness(value)
    }

    fn write_color(&self, r: u8, g: u8, b: u8) -> Result<(), String> {
        self.sysfs.set_keyboard_color(r, g, b)
    }

    fn save_user_state(&self, state: UserState) -> Result<(), String> {
        self.state.save(state)
    }
}

pub struct AlgAl15g53Backend {
    context: *mut ffi::AcerContext,
    controller: Arc<KeyboardController>,
    upower: Arc<UPowerBridge>,
    upower_sync: UPowerSyncHandle,
    fan_max: AtomicBool,
}

impl AlgAl15g53Backend {
    pub async fn new_async(upower: Arc<UPowerBridge>) -> Result<Self, String> {
        let context = unsafe { ffi::acer_open() };
        if context.is_null() {
            return Err("Failed to open Acer Control Center context".into());
        }

        let sysfs = SysfsBackend::new();
        let state = StateStore::new()?;
        let upower_sync = upower.spawn_actor();

        let live_color = sysfs.get_keyboard_color().unwrap_or((255, 255, 255));
        let live_brightness = match upower.get_brightness().await {
            Ok(val) => val,
            Err(err) => {
                eprintln!("Initial UPower brightness probe deferred ({err}); falling back to sysfs");
                sysfs.get_keyboard_brightness().unwrap_or(100)
            }
        };

        let saved = match state.load() {
            Ok(saved) => saved,
            Err(error) => {
                eprintln!("Could not restore saved preferences: {error}");
                None
            }
        };

        let is_restart_in_same_boot = Path::new(DAEMON_BOOT_MARKER).exists();
        let initial = if is_restart_in_same_boot {
            // Preserve live hardware/UPower brightness across mid-session daemon restarts.
            let base = saved.unwrap_or_else(|| {
                UserState::defaults(live_color.0, live_color.1, live_color.2, live_brightness)
            });
            UserState {
                r: base.r,
                g: base.g,
                b: base.b,
                brightness: live_brightness,
                fan_max: false,
                breathing: live_brightness > 0 && base.breathing,
                rainbow: live_brightness > 0 && base.rainbow,
            }
        } else {
            let _ = fs::write(DAEMON_BOOT_MARKER, b"1\n");
            saved.unwrap_or_else(|| {
                UserState::defaults(live_color.0, live_color.1, live_color.2, live_brightness)
            })
        };

        if saved.is_some() {
            let _ = sysfs.set_keyboard_color(initial.r, initial.g, initial.b);
            let _ = sysfs.set_keyboard_brightness(initial.brightness);
            if let Err(err) = upower.set_brightness(initial.brightness).await {
                eprintln!("Initial UPower SetBrightness deferred ({err}); sync_boot_upower will retry");
            }
        }

        let platform_io: Arc<dyn KeyboardPlatformIo> = Arc::new(SystemKeyboardIo {
            sysfs,
            upower_sync: upower_sync.clone(),
            state,
        });

        let controller = KeyboardController::new(platform_io, initial, true);

        let backend = Self {
            context,
            controller,
            upower,
            upower_sync,
            fan_max: AtomicBool::new(false),
        };

        // Fan safety rule: Always initialize the fan mode to Auto when daemon starts.
        if let Err(error) = backend.set_fan_auto() {
            eprintln!("Could not initialize fan mode to Auto: {error}");
        }

        Ok(backend)
    }

    pub fn controller(&self) -> Arc<KeyboardController> {
        Arc::clone(&self.controller)
    }

    pub fn upower_sync(&self) -> UPowerSyncHandle {
        self.upower_sync.clone()
    }

    /// Ensures UPower and sysfs match once UPower finishes booting concurrently with daemon.
    pub async fn sync_boot_upower(&self) {
        for _ in 0..10 {
            if let Ok(up_val) = self.upower.get_brightness().await {
                let snap = self.controller.snapshot();
                if !snap.idle_dimmed && up_val != snap.brightness_effective {
                    let _ = self.upower.set_brightness(snap.brightness_effective).await;
                }
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }
}

impl Drop for AlgAl15g53Backend {
    fn drop(&mut self) {
        if !self.context.is_null() {
            let _ = unsafe { ffi::acer_fan_auto(self.context) };
            unsafe {
                ffi::acer_close(self.context);
            }
        }
    }
}

unsafe impl Send for AlgAl15g53Backend {}
unsafe impl Sync for AlgAl15g53Backend {}

impl HardwareBackend for AlgAl15g53Backend {
    fn capabilities(&self) -> HardwareCapabilities {
        HardwareCapabilities {
            rgb_keyboard: true,
            rgb_animations: true,
            breathing: true,
            wave: false,
        }
    }

    fn get_keyboard_brightness(&self) -> Result<u8, String> {
        self.controller.get_brightness()
    }

    fn set_keyboard_brightness(&self, value: u8) -> Result<(), String> {
        self.controller.set_brightness(value, true)
    }

    fn set_keyboard_brightness_transient(&self, value: u8) -> Result<(), String> {
        self.controller.set_brightness_transient(value)
    }

    fn get_keyboard_color(&self) -> Result<(u8, u8, u8), String> {
        self.controller.get_color()
    }

    fn set_keyboard_color(&self, r: u8, g: u8, b: u8) -> Result<(), String> {
        self.controller.set_color(r, g, b, true)
    }

    fn get_cpu_temperature(&self) -> Result<i32, String> {
        let mut temperature: c_int = 0;
        let result = unsafe { ffi::acer_cpu_temperature(self.context, &mut temperature) };
        if result < 0 {
            return Err("Failed to read CPU temperature".into());
        }
        Ok(temperature)
    }

    fn get_gpu_temperature(&self) -> Result<i32, String> {
        let mut temperature: c_int = 0;
        let result = unsafe { ffi::acer_gpu_temperature(self.context, &mut temperature) };
        if result < 0 {
            return Err("Failed to read GPU temperature".into());
        }
        Ok(temperature)
    }

    fn set_fan_max(&self) -> Result<(), String> {
        let result = unsafe { ffi::acer_fan_max(self.context) };
        if result < 0 {
            return Err("Failed to set maximum fan mode".into());
        }
        self.fan_max.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn set_fan_auto(&self) -> Result<(), String> {
        let result = unsafe { ffi::acer_fan_auto(self.context) };
        if result < 0 {
            return Err("Failed to restore automatic fan mode".into());
        }
        self.fan_max.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn fan_max_selected(&self) -> bool {
        self.fan_max.load(Ordering::Relaxed)
    }
}

impl HardwareAnimationBackend for AlgAl15g53Backend {
    fn start_breathing(&self) -> Result<(), String> {
        self.controller.start_breathing()
    }

    fn start_rainbow(&self) -> Result<(), String> {
        self.controller.start_rainbow()
    }

    fn stop_animation(&self) -> Result<(), String> {
        self.controller.stop_animation()
    }

    fn breathing_selected(&self) -> bool {
        self.controller.breathing_selected()
    }

    fn rainbow_selected(&self) -> bool {
        self.controller.rainbow_selected()
    }

    fn set_brightness_commit(&self, value: u8, commit: bool) -> Result<(), String> {
        self.controller.set_brightness(value, commit)
    }

    fn set_color_commit(&self, r: u8, g: u8, b: u8, commit: bool) -> Result<(), String> {
        self.controller.set_color(r, g, b, commit)
    }

    fn set_sleep_timer(&self, enabled: bool, seconds: u32) -> Result<(), String> {
        self.controller.set_sleep_timer(enabled, seconds)
    }

    fn notify_external_off(&self) -> Result<(), String> {
        self.controller.notify_external_off()
    }

    fn get_full_state(&self) -> KeyboardDaemonSnapshot {
        self.controller.snapshot()
    }
}
