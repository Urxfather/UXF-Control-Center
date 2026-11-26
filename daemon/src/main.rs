mod animation;
mod backend;
mod dbus;
mod ffi;
mod hardware;
mod service;
mod state;
mod sysfs;

mod models;

use std::sync::Arc;

use anyhow::{bail, Result};
use backend::HardwareBackend;
use dbus::AcerControlDBus;
use hardware::HardwarePlatform;
use models::alg_al15g_53::AlgAl15g53Backend;
use service::AcerService;
use zbus::connection::Builder;

#[tokio::main]
async fn main() -> Result<()> {
    let hardware = hardware::detect()
        .map_err(|error| anyhow::anyhow!(error))?;

    println!(
        "Detected: {} {}",
        hardware.vendor,
        hardware.product_name
    );

    println!(
        "Board: {} {}",
        hardware.board_vendor,
        hardware.board_name
    );

    println!(
        "Interface: {}",
        hardware.interface_name
    );

    println!(
        "Platform: {:?}",
        hardware.platform
    );

    if !hardware.supported {
        bail!(
            "Unsupported hardware: {} {}",
            hardware.vendor,
            hardware.product_name
        );
    }

    let backend = match hardware.platform {
        HardwarePlatform::AlgAl15g53 => {
            Arc::new(
                    AlgAl15g53Backend::new()
                        .map_err(anyhow::Error::msg)?
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

    let animation_backend: Arc<dyn crate::animation::HardwareAnimationBackend> =
        backend.clone();

    let service = Arc::new(AcerService::new(
        hardware_backend,
        animation_backend,
    ));

    let _connection = Builder::system()?
        .name("io.github.stanlysilas.AcerControl")?
        .serve_at(
            "/io/github/stanlysilas/AcerControl",
            AcerControlDBus::new(service.clone()),
        )?
        .build()
        .await?;

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
    // 1. Stop and join animation workers first
    println!("Stopping animation workers...");
    let _ = service.stop_animation();

    // 2. Synchronously restore fan mode to Auto
    println!("Synchronously resetting fan mode to Auto...");
    if let Err(error) = service.set_fan_auto() {
        eprintln!("Failed to reset fan mode to Auto during shutdown: {error}");
    }

    println!("Acer Control Daemon terminated cleanly.");
    Ok(())
}
