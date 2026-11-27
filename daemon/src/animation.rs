use std::f64::consts::PI;
use std::time::{SystemTime, UNIX_EPOCH};

pub const FADE_DURATION_MS: u32 = 900;
pub const RAINBOW_CYCLE_MS: u64 = 5_000;
pub const BREATH_CYCLE_MS: u64 = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardEffect {
    Static,
    Breathing,
    Rainbow,
}

impl KeyboardEffect {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Breathing => "breathing",
            Self::Rainbow => "rainbow",
        }
    }
}

/// Authoritative daemon keyboard state shared atomically over D-Bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardDaemonSnapshot {
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

impl KeyboardDaemonSnapshot {
    pub fn as_dbus_tuple(
        &self,
    ) -> (
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
    ) {
        (
            self.available,
            self.brightness_target,
            self.brightness_effective,
            self.r,
            self.g,
            self.b,
            self.effect.clone(),
            self.sleep_timer_enabled,
            self.sleep_timer_seconds,
            self.idle_dimmed,
            self.fade_from,
            self.fade_to,
            self.fade_start_mono_ms,
            self.fade_duration_ms,
            self.anim_epoch_ms,
            self.mono_now_ms,
        )
    }
}

/// Returns Linux `CLOCK_MONOTONIC` milliseconds so daemon and frontend preview stay phase-locked.
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

/// Pure full-saturation RGB spectrum wheel without brightness pulsing.
pub fn rainbow_rgb(elapsed_ms: u64) -> (u8, u8, u8) {
    let phase = (elapsed_ms % RAINBOW_CYCLE_MS) as f64 / RAINBOW_CYCLE_MS as f64;
    let scaled = phase * 6.0;
    let sector = (scaled.floor() as u32) % 6;
    let fraction = ((scaled - scaled.floor()) * 255.0).round().clamp(0.0, 255.0) as u8;
    match sector {
        0 => (255, fraction, 0),
        1 => (255 - fraction, 255, 0),
        2 => (0, 255, fraction),
        3 => (0, 255 - fraction, 255),
        4 => (fraction, 0, 255),
        _ => (255, 0, 255 - fraction),
    }
}

/// Smooth cosine breathing modulation starting cleanly at 100% intensity.
pub fn breathing_scale(elapsed_ms: u64) -> f64 {
    let phase = (elapsed_ms % BREATH_CYCLE_MS) as f64 / BREATH_CYCLE_MS as f64;
    let cosine = 0.5 + 0.5 * (2.0 * PI * phase).cos();
    0.06 + 0.94 * cosine
}

/// Applies smooth breathing modulation to a base RGB color.
pub fn breathing_color(base: (u8, u8, u8), elapsed_ms: u64) -> (u8, u8, u8) {
    let scale = breathing_scale(elapsed_ms);
    (
        ((base.0 as f64) * scale).round().clamp(0.0, 255.0) as u8,
        ((base.1 as f64) * scale).round().clamp(0.0, 255.0) as u8,
        ((base.2 as f64) * scale).round().clamp(0.0, 255.0) as u8,
    )
}

/// Shared ease-in-out cosine fade curve used identically by physical hardware and GUI preview.
pub fn smooth_fade_value(from: u8, to: u8, elapsed_ms: u64, duration_ms: u32) -> u8 {
    if duration_ms == 0 || elapsed_ms >= duration_ms as u64 {
        return to;
    }
    let t = (elapsed_ms as f64 / duration_ms as f64).clamp(0.0, 1.0);
    let eased = 0.5 - 0.5 * (PI * t).cos();
    let val = from as f64 + (to as f64 - from as f64) * eased;
    val.round().clamp(0.0, 255.0) as u8
}

pub trait HardwareAnimationBackend: Send + Sync {
    fn start_breathing(&self) -> Result<(), String>;
    fn start_rainbow(&self) -> Result<(), String> {
        Err("Rainbow animation is not supported by this backend".into())
    }
    fn stop_animation(&self) -> Result<(), String>;
    fn breathing_selected(&self) -> bool {
        false
    }
    fn rainbow_selected(&self) -> bool {
        false
    }
    fn set_brightness_commit(&self, value: u8, commit: bool) -> Result<(), String>;
    fn set_color_commit(&self, r: u8, g: u8, b: u8, commit: bool) -> Result<(), String>;
    fn set_sleep_timer(&self, enabled: bool, seconds: u32) -> Result<(), String>;
    fn notify_external_off(&self) -> Result<(), String>;
    fn get_full_state(&self) -> KeyboardDaemonSnapshot;
}
