use std::sync::Arc;

use zbus::{interface, SignalContext};

use crate::service::AcerService;
use crate::upower::UPowerSyncHandle;

pub struct AcerControlDBus {
    service: Arc<AcerService>,
    upower_sync: UPowerSyncHandle,
}

impl AcerControlDBus {
    pub fn new(service: Arc<AcerService>, upower_sync: UPowerSyncHandle) -> Self {
        Self {
            service,
            upower_sync,
        }
    }
}

#[interface(name = "io.github.stanlysilas.AcerControl")]
impl AcerControlDBus {
    async fn ping(&self) -> String {
        "pong".into()
    }

    async fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    async fn get_keyboard_brightness(&self) -> zbus::fdo::Result<u8> {
        self.service
            .get_keyboard_brightness()
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn set_keyboard_brightness(&self, value: u8) -> zbus::fdo::Result<()> {
        self.service
            .set_keyboard_brightness(value)
            .map_err(zbus::fdo::Error::Failed)?;
        let _ = self.upower_sync.flush().await;
        Ok(())
    }

    async fn set_keyboard_brightness_commit(
        &self,
        value: u8,
        commit: bool,
    ) -> zbus::fdo::Result<()> {
        self.service
            .set_keyboard_brightness_commit(value, commit)
            .map_err(zbus::fdo::Error::Failed)?;
        if commit {
            let _ = self.upower_sync.flush().await;
        }
        Ok(())
    }

    async fn set_keyboard_brightness_transient(&self, value: u8) -> zbus::fdo::Result<()> {
        self.service
            .set_keyboard_brightness_transient(value)
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn get_keyboard_color(&self) -> zbus::fdo::Result<(u8, u8, u8)> {
        self.service
            .get_keyboard_color()
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn set_keyboard_color(&self, r: u8, g: u8, b: u8) -> zbus::fdo::Result<()> {
        self.service
            .set_keyboard_color(r, g, b)
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn set_keyboard_color_commit(
        &self,
        r: u8,
        g: u8,
        b: u8,
        commit: bool,
    ) -> zbus::fdo::Result<()> {
        self.service
            .set_keyboard_color_commit(r, g, b, commit)
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn start_breathing(&self) -> zbus::fdo::Result<()> {
        self.service
            .start_breathing()
            .map_err(zbus::fdo::Error::Failed)?;
        let _ = self.upower_sync.flush().await;
        Ok(())
    }

    async fn start_rainbow(&self) -> zbus::fdo::Result<()> {
        self.service
            .start_rainbow()
            .map_err(zbus::fdo::Error::Failed)?;
        let _ = self.upower_sync.flush().await;
        Ok(())
    }

    async fn stop_animation(&self) -> zbus::fdo::Result<()> {
        self.service
            .stop_animation()
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn set_keyboard_sleep_timer(&self, enabled: bool, seconds: u32) -> zbus::fdo::Result<()> {
        self.service
            .set_keyboard_sleep_timer(enabled, seconds)
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn notify_external_off(&self) -> zbus::fdo::Result<()> {
        self.service
            .notify_external_off()
            .map_err(zbus::fdo::Error::Failed)?;
        let _ = self.upower_sync.flush().await;
        Ok(())
    }

    async fn get_keyboard_full_state(
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
        self.service.get_keyboard_full_state().as_dbus_tuple()
    }

    async fn set_fan_max(&self, #[zbus(signal_context)] ctxt: SignalContext<'_>) -> zbus::fdo::Result<()> {
        self.service
            .set_fan_max()
            .map_err(zbus::fdo::Error::Failed)?;
        let _ = Self::fan_mode_changed(&ctxt, true).await;
        Ok(())
    }

    async fn set_fan_auto(&self, #[zbus(signal_context)] ctxt: SignalContext<'_>) -> zbus::fdo::Result<()> {
        self.service
            .set_fan_auto()
            .map_err(zbus::fdo::Error::Failed)?;
        let _ = Self::fan_mode_changed(&ctxt, false).await;
        Ok(())
    }

    async fn get_fan_max_selected(&self) -> bool {
        self.service.fan_max_selected()
    }

    async fn get_breathing_selected(&self) -> bool {
        self.service.breathing_selected()
    }

    async fn get_rainbow_selected(&self) -> bool {
        self.service.rainbow_selected()
    }

    #[zbus(signal)]
    pub async fn keyboard_state_changed(
        ctxt: &SignalContext<'_>,
        available: bool,
        brightness_target: u8,
        brightness_effective: u8,
        r: u8,
        g: u8,
        b: u8,
        effect: &str,
        sleep_timer_enabled: bool,
        sleep_timer_seconds: u32,
        idle_dimmed: bool,
        fade_from: u8,
        fade_to: u8,
        fade_start_mono_ms: u64,
        fade_duration_ms: u32,
        anim_epoch_ms: u64,
        mono_now_ms: u64,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn fan_mode_changed(ctxt: &SignalContext<'_>, fan_max: bool) -> zbus::Result<()>;
}
