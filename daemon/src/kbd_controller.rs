use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::animation::{
    breathing_color, mono_now_ms, rainbow_rgb, smooth_fade_value, KeyboardDaemonSnapshot,
    KeyboardEffect, FADE_DURATION_MS,
};
use crate::state::UserState;

const WORKER_ACTIVE_TICK: Duration = Duration::from_millis(20);
const SELF_ECHO_TTL_MS: u64 = 1_500;
const SAVE_DEBOUNCE_MS: u64 = 400;
const WAKE_IDLE_THRESHOLD_MS: u64 = 450;

/// Abstracts physical sysfs + UPower + state persistence for deterministic unit testing.
pub trait KeyboardPlatformIo: Send + Sync {
    fn read_brightness_sysfs(&self) -> Result<u8, String>;
    fn write_brightness_synced(&self, value: u8) -> Result<(), String>;
    fn write_brightness_transient(&self, value: u8) -> Result<(), String>;
    fn write_color(&self, r: u8, g: u8, b: u8) -> Result<(), String>;
    fn save_user_state(&self, state: UserState) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeKind {
    DimOut,
    WakeIn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FadeState {
    from: u8,
    to: u8,
    start_mono_ms: u64,
    duration_ms: u32,
    kind: FadeKind,
}

struct KbdInner {
    brightness_target: u8,
    brightness_effective: u8,
    color: (u8, u8, u8),
    last_written_color: Option<(u8, u8, u8)>,
    effect: KeyboardEffect,
    anim_epoch_ms: u64,
    sleep_timer_enabled: bool,
    sleep_timer_seconds: u32,
    idle_dimmed: bool,
    fade: Option<FadeState>,
    expected_upower_echoes: VecDeque<(u8, u64)>,
    dirty_save: bool,
    last_save_mono_ms: u64,
}

/// Daemon-resident controller owning RGB animations, smooth fade, inactivity sleep timer, and UPower sync.
pub struct KeyboardController {
    io: Arc<dyn KeyboardPlatformIo>,
    inner: Mutex<KbdInner>,
    wake_cvar: Condvar,
    stop: Arc<AtomicBool>,
    last_input_mono_ms: Arc<AtomicU64>,
    idle_dimmed_flag: Arc<AtomicBool>,
    state_notify: tokio::sync::watch::Sender<u64>,
    state_seq: AtomicU64,
    worker_handle: Mutex<Option<JoinHandle<()>>>,
    input_handle: Mutex<Option<JoinHandle<()>>>,
}

impl KeyboardController {
    pub fn new(
        io: Arc<dyn KeyboardPlatformIo>,
        initial_state: UserState,
        live_input_monitor: bool,
    ) -> Arc<Self> {
        Self::build(io, initial_state, true, live_input_monitor)
    }

    #[cfg(test)]
    fn new_for_test(io: Arc<dyn KeyboardPlatformIo>, initial_state: UserState) -> Arc<Self> {
        Self::build(io, initial_state, false, false)
    }

    fn build(
        io: Arc<dyn KeyboardPlatformIo>,
        initial_state: UserState,
        start_worker: bool,
        live_input_monitor: bool,
    ) -> Arc<Self> {
        let now = mono_now_ms();
        let initial_effect = if initial_state.rainbow {
            KeyboardEffect::Rainbow
        } else if initial_state.breathing && initial_state.brightness > 0 {
            KeyboardEffect::Breathing
        } else {
            KeyboardEffect::Static
        };

        let (state_notify, _rx) = tokio::sync::watch::channel(0_u64);

        let controller = Arc::new(Self {
            io,
            inner: Mutex::new(KbdInner {
                brightness_target: initial_state.brightness,
                brightness_effective: initial_state.brightness,
                color: (initial_state.r, initial_state.g, initial_state.b),
                last_written_color: None,
                effect: initial_effect,
                anim_epoch_ms: now,
                sleep_timer_enabled: false,
                sleep_timer_seconds: 5,
                idle_dimmed: false,
                fade: None,
                expected_upower_echoes: VecDeque::new(),
                dirty_save: false,
                last_save_mono_ms: now,
            }),
            wake_cvar: Condvar::new(),
            stop: Arc::new(AtomicBool::new(false)),
            last_input_mono_ms: Arc::new(AtomicU64::new(now)),
            idle_dimmed_flag: Arc::new(AtomicBool::new(false)),
            state_notify,
            state_seq: AtomicU64::new(0),
            worker_handle: Mutex::new(None),
            input_handle: Mutex::new(None),
        });

        if start_worker {
            controller.spawn_worker();
        }
        if live_input_monitor {
            controller.spawn_input_monitor();
        }
        controller
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.state_notify.subscribe()
    }

    fn emit_change(&self) {
        let next = self.state_seq.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        let _ = self.state_notify.send(next);
    }

    fn persist_state_locked(&self, guard: &mut KbdInner, now: u64, force: bool) {
        if !force && now.saturating_sub(guard.last_save_mono_ms) < SAVE_DEBOUNCE_MS {
            guard.dirty_save = true;
            return;
        }
        guard.dirty_save = false;
        guard.last_save_mono_ms = now;
        let st = UserState {
            r: guard.color.0,
            g: guard.color.1,
            b: guard.color.2,
            brightness: guard.brightness_target,
            fan_max: false,
            breathing: guard.effect == KeyboardEffect::Breathing,
            rainbow: guard.effect == KeyboardEffect::Rainbow,
        };
        let _ = self.io.save_user_state(st);
    }

    fn push_self_echo(guard: &mut KbdInner, value: u8, now: u64) {
        while let Some(&(_, ts)) = guard.expected_upower_echoes.front() {
            if now.saturating_sub(ts) > SELF_ECHO_TTL_MS {
                guard.expected_upower_echoes.pop_front();
            } else {
                break;
            }
        }
        if guard.expected_upower_echoes.len() >= 16 {
            guard.expected_upower_echoes.pop_front();
        }
        guard.expected_upower_echoes.push_back((value, now));
    }

    pub fn snapshot(&self) -> KeyboardDaemonSnapshot {
        let now = mono_now_ms();
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (fade_from, fade_to, fade_start_mono_ms, fade_duration_ms) = match guard.fade {
            Some(f) => (f.from, f.to, f.start_mono_ms, f.duration_ms),
            None => (
                guard.brightness_effective,
                guard.brightness_effective,
                0,
                0,
            ),
        };
        KeyboardDaemonSnapshot {
            available: true,
            brightness_target: guard.brightness_target,
            brightness_effective: guard.brightness_effective,
            r: guard.color.0,
            g: guard.color.1,
            b: guard.color.2,
            effect: guard.effect.as_str().into(),
            sleep_timer_enabled: guard.sleep_timer_enabled,
            sleep_timer_seconds: guard.sleep_timer_seconds,
            idle_dimmed: guard.idle_dimmed,
            fade_from,
            fade_to,
            fade_start_mono_ms,
            fade_duration_ms,
            anim_epoch_ms: guard.anim_epoch_ms,
            mono_now_ms: now,
        }
    }

    /// Reads authoritative brightness (querying live hardware/sysfs if static and un-faded).
    pub fn get_brightness(&self) -> Result<u8, String> {
        let (idle_dimmed, fading) = {
            let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            (guard.idle_dimmed, guard.fade.is_some())
        };
        if !idle_dimmed && !fading {
            if let Ok(actual) = self.io.read_brightness_sysfs() {
                if let Ok(mut guard) = self.inner.lock() {
                    if !guard.idle_dimmed && guard.fade.is_none() && guard.brightness_effective != actual
                    {
                        guard.brightness_effective = actual;
                        guard.brightness_target = actual;
                    }
                }
                return Ok(actual);
            }
        }
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        Ok(guard.brightness_effective)
    }

    pub fn get_color(&self) -> Result<(u8, u8, u8), String> {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        Ok(guard.color)
    }

    pub fn breathing_selected(&self) -> bool {
        self.inner
            .lock()
            .map(|g| g.effect == KeyboardEffect::Breathing)
            .unwrap_or(false)
    }

    pub fn rainbow_selected(&self) -> bool {
        self.inner
            .lock()
            .map(|g| g.effect == KeyboardEffect::Rainbow)
            .unwrap_or(false)
    }

    pub fn set_brightness(&self, value: u8, commit: bool) -> Result<(), String> {
        let now = mono_now_ms();
        self.last_input_mono_ms.store(now, Ordering::Relaxed);
        self.idle_dimmed_flag.store(false, Ordering::Relaxed);

        let (color_to_apply, apply_color) = {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.fade = None;
            guard.idle_dimmed = false;
            guard.brightness_target = value;
            guard.brightness_effective = value;

            if value == 0 {
                // Explicit OFF takes priority: cancel pending timer and animations.
                guard.sleep_timer_enabled = false;
                guard.effect = KeyboardEffect::Static;
            }
            if commit {
                Self::push_self_echo(&mut guard, value, now);
                self.persist_state_locked(&mut guard, now, true);
            } else {
                self.persist_state_locked(&mut guard, now, false);
            }
            let c = guard.color;
            let static_mode = guard.effect == KeyboardEffect::Static;
            if static_mode {
                guard.last_written_color = Some(c);
            }
            (c, static_mode && value > 0)
        };

        if apply_color {
            let _ = self.io.write_color(color_to_apply.0, color_to_apply.1, color_to_apply.2);
        }

        let res = if commit {
            self.io.write_brightness_synced(value)
        } else {
            self.io.write_brightness_transient(value)
        };

        self.wake_cvar.notify_all();
        self.emit_change();
        res
    }

    pub fn set_brightness_transient(&self, value: u8) -> Result<(), String> {
        self.io.write_brightness_transient(value)
    }

    pub fn set_color(&self, r: u8, g: u8, b: u8, commit: bool) -> Result<(), String> {
        let now = mono_now_ms();
        let write_direct = {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.color = (r, g, b);
            if guard.effect == KeyboardEffect::Rainbow {
                guard.effect = KeyboardEffect::Static;
            }
            self.persist_state_locked(&mut guard, now, commit);
            let is_static = guard.effect == KeyboardEffect::Static;
            if is_static {
                guard.last_written_color = Some((r, g, b));
            }
            is_static
        };

        let res = if write_direct {
            self.io.write_color(r, g, b)
        } else {
            Ok(())
        };

        self.wake_cvar.notify_all();
        self.emit_change();
        res
    }

    pub fn start_breathing(&self) -> Result<(), String> {
        self.start_effect(KeyboardEffect::Breathing)
    }

    pub fn start_rainbow(&self) -> Result<(), String> {
        self.start_effect(KeyboardEffect::Rainbow)
    }

    fn start_effect(&self, effect: KeyboardEffect) -> Result<(), String> {
        let now = mono_now_ms();
        self.last_input_mono_ms.store(now, Ordering::Relaxed);
        self.idle_dimmed_flag.store(false, Ordering::Relaxed);

        let needs_wake_brightness = {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let mut turn_on = false;
            if guard.brightness_target == 0 {
                guard.brightness_target = 255;
                turn_on = true;
            }
            if guard.idle_dimmed || turn_on {
                guard.brightness_effective = guard.brightness_target;
                turn_on = true;
            }
            guard.fade = None;
            guard.idle_dimmed = false;
            guard.effect = effect;
            guard.anim_epoch_ms = now;
            guard.last_written_color = None;
            let target_b = guard.brightness_target;
            if turn_on {
                Self::push_self_echo(&mut guard, target_b, now);
            }
            self.persist_state_locked(&mut guard, now, true);
            if turn_on {
                Some(guard.brightness_target)
            } else {
                None
            }
        };

        if let Some(val) = needs_wake_brightness {
            self.io.write_brightness_synced(val)?;
        }

        self.wake_cvar.notify_all();
        self.emit_change();
        Ok(())
    }

    pub fn stop_animation(&self) -> Result<(), String> {
        let now = mono_now_ms();
        let color = {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.effect = KeyboardEffect::Static;
            guard.last_written_color = Some(guard.color);
            self.persist_state_locked(&mut guard, now, true);
            guard.color
        };

        let res = self.io.write_color(color.0, color.1, color.2);
        self.wake_cvar.notify_all();
        self.emit_change();
        res
    }

    pub fn set_sleep_timer(&self, enabled: bool, seconds: u32) -> Result<(), String> {
        let now = mono_now_ms();
        let clamped_secs = seconds.clamp(1, 86_400);
        self.last_input_mono_ms.store(now, Ordering::Relaxed);

        let restore_fade_needed = {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.sleep_timer_seconds = clamped_secs;
            guard.sleep_timer_enabled = enabled;

            if !enabled {
                let was_dimmed_or_fading = guard.idle_dimmed
                    || guard.fade.is_some_and(|f| f.kind == FadeKind::DimOut);
                guard.idle_dimmed = false;
                self.idle_dimmed_flag.store(false, Ordering::Relaxed);
                if was_dimmed_or_fading && guard.brightness_target > 0 {
                    guard.fade = Some(FadeState {
                        from: guard.brightness_effective,
                        to: guard.brightness_target,
                        start_mono_ms: now,
                        duration_ms: FADE_DURATION_MS,
                        kind: FadeKind::WakeIn,
                    });
                    true
                } else {
                    guard.fade = None;
                    false
                }
            } else {
                // New timer replaces any previous idle/dim cycle with a fresh deadline from now.
                guard.idle_dimmed = false;
                self.idle_dimmed_flag.store(false, Ordering::Relaxed);
                if guard.brightness_effective != guard.brightness_target && guard.brightness_target > 0 {
                    guard.fade = Some(FadeState {
                        from: guard.brightness_effective,
                        to: guard.brightness_target,
                        start_mono_ms: now,
                        duration_ms: FADE_DURATION_MS,
                        kind: FadeKind::WakeIn,
                    });
                } else {
                    guard.fade = None;
                }
                false
            }
        };

        let _ = restore_fade_needed;
        self.wake_cvar.notify_all();
        self.emit_change();
        Ok(())
    }

    /// Invoked by UPower watcher when external brightness change arrives on System D-Bus.
    pub fn on_upower_brightness_changed(&self, ext_raw: u8) {
        let now = mono_now_ms();
        let (accepted, restore_color) = {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());

            // Consume matching self-triggered UPower write echo if present.
            let mut matched_idx = None;
            for (idx, &(val, ts)) in guard.expected_upower_echoes.iter().enumerate() {
                if val == ext_raw && now.saturating_sub(ts) <= SELF_ECHO_TTL_MS {
                    matched_idx = Some(idx);
                    break;
                }
            }
            if let Some(idx) = matched_idx {
                guard.expected_upower_echoes.remove(idx);
                (false, None)
            } else {
                Self::apply_external_brightness_locked(
                    &self.io,
                    &self.last_input_mono_ms,
                    &self.idle_dimmed_flag,
                    &mut guard,
                    ext_raw,
                    now,
                );
                let col = if guard.effect == KeyboardEffect::Static && ext_raw > 0 {
                    Some(guard.color)
                } else {
                    None
                };
                (true, col)
            }
        };

        if accepted {
            if let Some((r, g, b)) = restore_color {
                let _ = self.io.write_color(r, g, b);
            }
            self.wake_cvar.notify_all();
            self.emit_change();
        }
    }

    /// Invoked when GNOME Quick Settings explicitly switches keyboard backlight OFF (`0%`).
    pub fn notify_external_off(&self) -> Result<(), String> {
        let now = mono_now_ms();
        {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            Self::apply_external_brightness_locked(
                &self.io,
                &self.last_input_mono_ms,
                &self.idle_dimmed_flag,
                &mut guard,
                0,
                now,
            );
            Self::push_self_echo(&mut guard, 0, now);
        }
        let _ = self.io.write_brightness_synced(0);
        self.wake_cvar.notify_all();
        self.emit_change();
        Ok(())
    }

    fn apply_external_brightness_locked(
        io: &Arc<dyn KeyboardPlatformIo>,
        last_input: &AtomicU64,
        idle_dimmed_flag: &AtomicBool,
        guard: &mut KbdInner,
        ext_raw: u8,
        now: u64,
    ) {
        last_input.store(now, Ordering::Relaxed);
        idle_dimmed_flag.store(false, Ordering::Relaxed);
        guard.fade = None;
        guard.idle_dimmed = false;
        guard.brightness_target = ext_raw;
        guard.brightness_effective = ext_raw;

        if ext_raw == 0 {
            // Priority rule: External OFF cancels pending sleep timer & stops animation so backlight never auto-restores.
            guard.sleep_timer_enabled = false;
            guard.effect = KeyboardEffect::Static;
        }

        guard.dirty_save = false;
        guard.last_save_mono_ms = now;
        let _ = io.save_user_state(UserState {
            r: guard.color.0,
            g: guard.color.1,
            b: guard.color.2,
            brightness: ext_raw,
            fan_max: false,
            breathing: guard.effect == KeyboardEffect::Breathing,
            rainbow: guard.effect == KeyboardEffect::Rainbow,
        });
    }

    /// Injects synthetic user activity timestamp for deterministic testing or fallback.
    pub fn record_user_activity_at(&self, mono_ms: u64) {
        self.last_input_mono_ms.store(mono_ms, Ordering::Relaxed);
        if self.idle_dimmed_flag.load(Ordering::Relaxed) {
            self.wake_cvar.notify_all();
        }
    }

    /// Advances one worker evaluation tick at explicit monotonic timestamp (shared by live loop & tests).
    pub fn step_at(&self, now: u64) -> Duration {
        let mut emit_signal = false;
        let mut sysfs_transient_brightness: Option<u8> = None;
        let mut sysfs_color: Option<(u8, u8, u8)> = None;
        let mut next_wait = Duration::from_secs(60);

        {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());

            // 1. Flush debounced disk persistence when idle after slider drags.
            if guard.dirty_save && now.saturating_sub(guard.last_save_mono_ms) >= SAVE_DEBOUNCE_MS {
                self.persist_state_locked(&mut guard, now, true);
            }

            // 2. Evaluate inactivity sleep timer transitions.
            if guard.sleep_timer_enabled && guard.brightness_target > 0 {
                let last_act = self.last_input_mono_ms.load(Ordering::Relaxed);
                let idle_ms = now.saturating_sub(last_act);
                let timeout_ms = (guard.sleep_timer_seconds as u64).saturating_mul(1000);

                if !guard.idle_dimmed
                    && guard.fade.is_none()
                    && idle_ms >= timeout_ms
                {
                    guard.idle_dimmed = true;
                    self.idle_dimmed_flag.store(true, Ordering::Relaxed);
                    guard.fade = Some(FadeState {
                        from: guard.brightness_effective,
                        to: 0,
                        start_mono_ms: now,
                        duration_ms: FADE_DURATION_MS,
                        kind: FadeKind::DimOut,
                    });
                    emit_signal = true;
                } else if (guard.idle_dimmed
                    || guard.fade.is_some_and(|f| f.kind == FadeKind::DimOut))
                    && idle_ms < WAKE_IDLE_THRESHOLD_MS
                {
                    guard.idle_dimmed = false;
                    self.idle_dimmed_flag.store(false, Ordering::Relaxed);
                    guard.fade = Some(FadeState {
                        from: guard.brightness_effective,
                        to: guard.brightness_target,
                        start_mono_ms: now,
                        duration_ms: FADE_DURATION_MS,
                        kind: FadeKind::WakeIn,
                    });
                    emit_signal = true;
                } else if !guard.idle_dimmed && guard.fade.is_none() {
                    let remain = timeout_ms.saturating_sub(idle_ms).max(20);
                    next_wait = next_wait.min(Duration::from_millis(remain));
                }
            } else if !guard.sleep_timer_enabled && guard.idle_dimmed {
                guard.idle_dimmed = false;
                self.idle_dimmed_flag.store(false, Ordering::Relaxed);
            }

            // 3. Advance smooth cosine fade if active.
            if let Some(fade) = guard.fade {
                let elapsed = now.saturating_sub(fade.start_mono_ms);
                let next_val = smooth_fade_value(fade.from, fade.to, elapsed, fade.duration_ms);
                if next_val != guard.brightness_effective {
                    guard.brightness_effective = next_val;
                    if elapsed < fade.duration_ms as u64 {
                        sysfs_transient_brightness = Some(next_val);
                    }
                }
                if elapsed >= fade.duration_ms as u64 {
                    guard.brightness_effective = fade.to;
                    guard.fade = None;
                    // Inactivity fade uses transient sysfs writes so UPower retains the user's logical level
                    // and any UPower BrightnessChanged(0) signal is unambiguously an explicit OFF.
                    sysfs_transient_brightness = Some(fade.to);
                    emit_signal = true;
                } else {
                    next_wait = next_wait.min(WORKER_ACTIVE_TICK);
                }
            }

            // 4. Advance color animations strictly when hardware is visibly lit.
            if guard.brightness_effective > 0 && guard.effect != KeyboardEffect::Static {
                let anim_elapsed = now.saturating_sub(guard.anim_epoch_ms);
                let target_rgb = match guard.effect {
                    KeyboardEffect::Rainbow => rainbow_rgb(anim_elapsed),
                    KeyboardEffect::Breathing => breathing_color(guard.color, anim_elapsed),
                    KeyboardEffect::Static => guard.color,
                };
                if guard.last_written_color != Some(target_rgb) {
                    guard.last_written_color = Some(target_rgb);
                    sysfs_color = Some(target_rgb);
                }
                next_wait = next_wait.min(WORKER_ACTIVE_TICK);
            }
        }

        // Perform I/O cleanly outside the mutex lock.
        if let Some((r, g, b)) = sysfs_color {
            let _ = self.io.write_color(r, g, b);
        }
        if let Some(val) = sysfs_transient_brightness {
            let _ = self.io.write_brightness_transient(val);
        }
        if emit_signal {
            self.emit_change();
        }

        next_wait
    }

    fn spawn_worker(self: &Arc<Self>) {
        let ctrl = Arc::clone(self);
        let handle = thread::Builder::new()
            .name("uxf-daemon-kbd".into())
            .spawn(move || {
                while !ctrl.stop.load(Ordering::Relaxed) {
                    let wait = ctrl.step_at(mono_now_ms());
                    let guard = ctrl.inner.lock().unwrap_or_else(|e| e.into_inner());
                    if ctrl.stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let _ = ctrl.wake_cvar.wait_timeout(guard, wait);
                }
            })
            .ok();
        if let Ok(mut g) = self.worker_handle.lock() {
            *g = handle;
        }
    }

    fn spawn_input_monitor(self: &Arc<Self>) {
        let ctrl = Arc::clone(self);
        let handle = thread::Builder::new()
            .name("uxf-input-idle".into())
            .spawn(move || {
                run_evdev_idle_monitor(ctrl);
            })
            .ok();
        if let Ok(mut g) = self.input_handle.lock() {
            *g = handle;
        }
    }
}

impl Drop for KeyboardController {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.wake_cvar.notify_all();
        if let Ok(mut guard) = self.worker_handle.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
        if let Ok(mut guard) = self.input_handle.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
    }
}

/// Discovers keyboard, mouse, and touchpad `/dev/input/event*` nodes passively (excluding virtual UXF devices).
fn discover_input_event_devices() -> Vec<File> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Ok(text) = fs::read_to_string("/proc/bus/input/devices") {
        let mut current_name = String::new();
        for line in text.lines() {
            if line.starts_with("N: Name=") {
                current_name = line.to_string();
            } else if line.starts_with("H: Handlers=") {
                let is_relevant = (line.contains("kbd") || line.contains("mouse"))
                    && !current_name.contains("uxf-lock-key-helper")
                    && !current_name.contains("Power Button")
                    && !current_name.contains("Sleep Button");
                if is_relevant {
                    for token in line.split_whitespace() {
                        if token.starts_with("event") {
                            paths.push(Path::new("/dev/input").join(token));
                        }
                    }
                }
            } else if line.trim().is_empty() {
                current_name.clear();
            }
        }
    }

    paths
        .into_iter()
        .filter_map(|p| {
            let file = OpenOptions::new().read(true).open(&p).ok()?;
            unsafe {
                let fd = file.as_raw_fd();
                let flags = libc::fcntl(fd, libc::F_GETFL);
                if flags >= 0 {
                    libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }
            }
            Some(file)
        })
        .collect()
}

fn run_evdev_idle_monitor(ctrl: Arc<KeyboardController>) {
    let mut files = discover_input_event_devices();
    let mut last_scan = mono_now_ms();
    let mut buf = [0_u8; 1024];

    while !ctrl.stop.load(Ordering::Relaxed) {
        let now = mono_now_ms();
        if files.is_empty() || now.saturating_sub(last_scan) >= 10_000 {
            files = discover_input_event_devices();
            last_scan = now;
        }

        if files.is_empty() {
            thread::sleep(Duration::from_millis(500));
            continue;
        }

        let mut pollfds: Vec<libc::pollfd> = files
            .iter()
            .map(|f| libc::pollfd {
                fd: f.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            })
            .collect();

        let rc = unsafe {
            libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, 250)
        };
        if rc < 0 {
            thread::sleep(Duration::from_millis(250));
            continue;
        }

        let mut had_activity = false;
        let mut need_rescan = false;
        for (idx, pfd) in pollfds.iter().enumerate() {
            if (pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL)) != 0 {
                need_rescan = true;
            }
            if (pfd.revents & libc::POLLIN) != 0 {
                let mut f = &files[idx];
                while let Ok(n) = f.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    had_activity = true;
                }
            }
        }

        if need_rescan {
            files.clear();
        }

        if had_activity {
            ctrl.record_user_activity_at(mono_now_ms());
            // Coalesce high-frequency mouse events cleanly (10 Hz wake check max).
            thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::RAINBOW_CYCLE_MS;
    use std::sync::atomic::AtomicUsize;

    struct MockIo {
        sysfs_brightness: Mutex<u8>,
        upower_brightness: Mutex<u8>,
        color: Mutex<(u8, u8, u8)>,
        saved: Mutex<Option<UserState>>,
        brightness_history: Mutex<Vec<u8>>,
        color_history: Mutex<Vec<(u8, u8, u8)>>,
        sync_calls: AtomicUsize,
    }

    impl MockIo {
        fn new(brightness: u8, color: (u8, u8, u8)) -> Arc<Self> {
            Arc::new(Self {
                sysfs_brightness: Mutex::new(brightness),
                upower_brightness: Mutex::new(brightness),
                color: Mutex::new(color),
                saved: Mutex::new(None),
                brightness_history: Mutex::new(Vec::new()),
                color_history: Mutex::new(Vec::new()),
                sync_calls: AtomicUsize::new(0),
            })
        }
    }

    impl KeyboardPlatformIo for MockIo {
        fn read_brightness_sysfs(&self) -> Result<u8, String> {
            Ok(*self.sysfs_brightness.lock().unwrap())
        }
        fn write_brightness_synced(&self, value: u8) -> Result<(), String> {
            self.sync_calls.fetch_add(1, Ordering::Relaxed);
            *self.sysfs_brightness.lock().unwrap() = value;
            *self.upower_brightness.lock().unwrap() = value;
            self.brightness_history.lock().unwrap().push(value);
            Ok(())
        }
        fn write_brightness_transient(&self, value: u8) -> Result<(), String> {
            *self.sysfs_brightness.lock().unwrap() = value;
            self.brightness_history.lock().unwrap().push(value);
            Ok(())
        }
        fn write_color(&self, r: u8, g: u8, b: u8) -> Result<(), String> {
            *self.color.lock().unwrap() = (r, g, b);
            self.color_history.lock().unwrap().push((r, g, b));
            Ok(())
        }
        fn save_user_state(&self, state: UserState) -> Result<(), String> {
            *self.saved.lock().unwrap() = Some(state);
            Ok(())
        }
    }

    #[test]
    fn rainbow_spectrum_never_pulses_brightness_and_stays_independent_from_breathing() {
        for t in (0..RAINBOW_CYCLE_MS).step_by(125) {
            let (r, g, b) = rainbow_rgb(t);
            let max_c = r.max(g).max(b);
            let min_c = r.min(g).min(b);
            assert_eq!(max_c, 255, "Rainbow must stay at full saturation/intensity at t={t}");
            assert_eq!(min_c, 0, "Rainbow must be pure hue without dimming or desaturation at t={t}");
        }

        let io = MockIo::new(200, (10, 20, 30));
        let ctrl = KeyboardController::new_for_test(io.clone(), UserState::defaults(10, 20, 30, 200));
        ctrl.start_rainbow().unwrap();
        assert!(ctrl.rainbow_selected());
        assert!(!ctrl.breathing_selected());
        ctrl.step_at(mono_now_ms() + 1_200);
        // Hardware brightness stays at exact user level 200 while cycling rainbow colors.
        assert_eq!(ctrl.get_brightness().unwrap(), 200);
        assert_eq!(*io.sysfs_brightness.lock().unwrap(), 200);

        ctrl.start_breathing().unwrap();
        assert!(ctrl.breathing_selected());
        assert!(!ctrl.rainbow_selected());
    }

    #[test]
    fn smooth_fade_out_curve_is_monotonic_and_matches_preview() {
        let mut prev = 255_u8;
        let mut steps = Vec::new();
        for ms in (0..=FADE_DURATION_MS as u64).step_by(20) {
            let v = smooth_fade_value(255, 0, ms, FADE_DURATION_MS);
            assert!(v <= prev, "Fade out must decrease monotonically: {prev} -> {v} at {ms}ms");
            prev = v;
            steps.push(v);
        }
        assert_eq!(*steps.first().unwrap(), 255);
        assert_eq!(*steps.last().unwrap(), 0);
        assert!(steps.windows(2).filter(|w| w[0] != w[1]).count() > 25);
    }

    #[test]
    fn timer_continues_without_gui_and_external_off_cancels_restoration() {
        let io = MockIo::new(255, (0, 200, 212));
        let ctrl = KeyboardController::new_for_test(io.clone(), UserState::defaults(0, 200, 212, 255));

        // 1. Arm a 5-second sleep timer cleanly in daemon memory.
        ctrl.set_sleep_timer(true, 5).unwrap();
        let snap0 = ctrl.snapshot();
        assert!(snap0.sleep_timer_enabled);
        assert_eq!(snap0.sleep_timer_seconds, 5);

        // 2. Simulate external GNOME Quick Settings OFF (SetBrightness 0 on UPower) at t+1s.
        *io.sysfs_brightness.lock().unwrap() = 0;
        *io.upower_brightness.lock().unwrap() = 0;
        ctrl.on_upower_brightness_changed(0);

        let snap_off = ctrl.snapshot();
        assert_eq!(snap_off.brightness_target, 0);
        assert_eq!(snap_off.brightness_effective, 0);
        assert!(!snap_off.sleep_timer_enabled, "External OFF must cancel active timer");
        assert!(!snap_off.idle_dimmed);

        // 3. Even well past 5s expiration and subsequent user input, backlight stays OFF (never restores 255).
        ctrl.step_at(snap_off.mono_now_ms + 7_000);
        ctrl.record_user_activity_at(snap_off.mono_now_ms + 7_100);
        ctrl.step_at(snap_off.mono_now_ms + 7_120);
        assert_eq!(*io.sysfs_brightness.lock().unwrap(), 0);
        assert_eq!(ctrl.snapshot().brightness_target, 0);

        // 4. Turn ON again via GNOME Quick Settings (128), arm 5s timer, let timer expire cleanly in daemon.
        *io.sysfs_brightness.lock().unwrap() = 128;
        *io.upower_brightness.lock().unwrap() = 128;
        ctrl.on_upower_brightness_changed(128);
        assert_eq!(ctrl.snapshot().brightness_target, 128);

        ctrl.set_sleep_timer(true, 5).unwrap();
        let t0 = ctrl.snapshot().mono_now_ms;
        ctrl.record_user_activity_at(t0);
        ctrl.step_at(t0 + 5_050);
        assert!(ctrl.snapshot().idle_dimmed, "Timer should start fade after 5s idle");
        // Advance through full 900ms fade.
        for offset in (5_070..=6_050).step_by(20) {
            ctrl.step_at(t0 + offset);
        }
        assert_eq!(*io.sysfs_brightness.lock().unwrap(), 0);
        // UPower remains at user's logical brightness (128) during temporary idle dim.
        assert_eq!(*io.upower_brightness.lock().unwrap(), 128);

        // While dimmed to 0 by timer, if user clicks GNOME Quick Settings OFF, restoration must be cancelled!
        ctrl.on_upower_brightness_changed(0);
        ctrl.record_user_activity_at(t0 + 6_500);
        for offset in (6_520..=7_600).step_by(20) {
            ctrl.step_at(t0 + offset);
        }
        assert_eq!(
            *io.sysfs_brightness.lock().unwrap(),
            0,
            "Explicit external OFF while dimmed must prevent later brightness restoration"
        );
    }

    #[test]
    fn self_upower_echo_is_ignored_and_subsequent_external_zero_cancels() {
        let io = MockIo::new(200, (255, 0, 0));
        let ctrl = KeyboardController::new_for_test(io.clone(), UserState::defaults(255, 0, 0, 200));
        ctrl.set_sleep_timer(true, 5).unwrap();
        // UXF commits brightness=100 (pushes ONE self-echo for 100), timer remains enabled.
        ctrl.set_brightness(100, true).unwrap();
        ctrl.on_upower_brightness_changed(100);
        assert!(ctrl.snapshot().sleep_timer_enabled);
        assert_eq!(ctrl.snapshot().brightness_target, 100);

        // External OFF from GNOME Quick Settings immediately cancels timer & zeroes brightness.
        ctrl.on_upower_brightness_changed(0);
        assert!(!ctrl.snapshot().idle_dimmed);
        assert!(!ctrl.snapshot().sleep_timer_enabled);
        assert_eq!(ctrl.snapshot().brightness_target, 0);
    }
}
