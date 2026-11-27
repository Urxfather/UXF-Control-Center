mod animation;
mod backend;
mod dbus;
mod ffi;
mod hardware;
mod kbd_controller;
mod service;
mod state;
mod sysfs;
mod upower;

mod models;

use std::sync::Arc;

use anyhow::{bail, Result};
use backend::HardwareBackend;
use dbus::AcerControlDBus;
use hardware::HardwarePlatform;
use models::alg_al15g_53::AlgAl15g53Backend;
use service::AcerService;
use upower::{watch_upower_signals, UPowerBridge};
use zbus::connection::Builder;

const DBUS_PATH: &str = "/io/github/stanlysilas/AcerControl";

#[tokio::main]
async fn main() -> Result<()> {
    let hardware = hardware::detect().map_err(|error| anyhow::anyhow!(error))?;

    println!("Detected: {} {}", hardware.vendor, hardware.product_name);
    println!("Board: {} {}", hardware.board_vendor, hardware.board_name);
    println!("Interface: {}", hardware.interface_name);
    println!("Platform: {:?}", hardware.platform);

    if !hardware.supported {
        bail!(
            "Unsupported hardware: {} {}",
            hardware.vendor,
            hardware.product_name
        );
    }

    let upower = Arc::new(UPowerBridge::new());

    let backend = match hardware.platform {
        HardwarePlatform::AlgAl15g53 => {
            Arc::new(
                AlgAl15g53Backend::new_async(Arc::clone(&upower))
                    .await
                    .map_err(anyhow::Error::msg)?,
            )
        }
        HardwarePlatform::Unknown => {
            bail!(
                "Unsupported hardware platform: {} {}",
                hardware.vendor,
                hardware.product_name
            );
        }
    };

    let hardware_backend: Arc<dyn HardwareBackend> = backend.clone();
    let animation_backend: Arc<dyn crate::animation::HardwareAnimationBackend> = backend.clone();
    let service = Arc::new(AcerService::new(hardware_backend, animation_backend));
    let controller = backend.controller();
    let upower_sync = backend.upower_sync();

    let connection = Builder::system()?
        .name("io.github.stanlysilas.AcerControl")?
        .serve_at(DBUS_PATH, AcerControlDBus::new(service.clone(), upower_sync))?
        .build()
        .await?;

    // Sync once UPower finishes initializing at boot without stalling D-Bus registration.
    let backend_boot = backend.clone();
    tokio::spawn(async move {
        backend_boot.sync_boot_upower().await;
    });

    // Reactively receive external GNOME/UPower brightness changes (including external OFF).
    let ctrl_upower = Arc::clone(&controller);
    tokio::spawn(async move {
        watch_upower_signals(move |raw| {
            ctrl_upower.on_upower_brightness_changed(raw);
        })
        .await;
    });

    // Broadcast KeyboardStateChanged signal on System D-Bus whenever daemon state updates.
    let conn_sig = connection.clone();
    let ctrl_sig = Arc::clone(&controller);
    tokio::spawn(async move {
        let mut rx = ctrl_sig.subscribe();
        while rx.changed().await.is_ok() {
            let snap = ctrl_sig.snapshot();
            if let Ok(iface_ref) = conn_sig
                .object_server()
                .interface::<_, AcerControlDBus>(DBUS_PATH)
                .await
            {
                let _ = AcerControlDBus::keyboard_state_changed(
                    iface_ref.signal_context(),
                    snap.available,
                    snap.brightness_target,
                    snap.brightness_effective,
                    snap.r,
                    snap.g,
                    snap.b,
                    &snap.effect,
                    snap.sleep_timer_enabled,
                    snap.sleep_timer_seconds,
                    snap.idle_dimmed,
                    snap.fade_from,
                    snap.fade_to,
                    snap.fade_start_mono_ms,
                    snap.fade_duration_ms,
                    snap.anim_epoch_ms,
                    snap.mono_now_ms,
                )
                .await;
            }
        }
    });

    println!("Acer Control Daemon started.");

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("Received SIGINT, initiating clean shutdown...");
        }
        _ = sigterm.recv() => {
            println!("Received SIGTERM, initiating clean shutdown...");
        }
    }

    // Normal daemon shutdown sequence:
    // 1. Drop controller cleanly on shutdown after saving state; do NOT turn ON OFF lights.
    println!("Synchronously resetting fan mode to Auto...");
    if let Err(error) = service.set_fan_auto() {
        eprintln!("Failed to reset fan mode to Auto during shutdown: {error}");
    }

    println!("Acer Control Daemon terminated cleanly.");
    Ok(())
}
