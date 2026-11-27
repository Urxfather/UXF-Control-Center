use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::acercontrol::AcerControlLib;
use crate::dbus::AcerControlDBus;
use crate::system_info::{
    kernel_uname_info, now_ms, probe_secure_boot_efi, read_trimmed, run_command_with_timeout,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Passed,
    Warning,
    Failed,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSubsystemCard {
    pub id: String,
    pub title: String,
    pub status: HealthStatus,
    pub status_label: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthItem {
    pub id: String,
    pub name: String,
    pub status: HealthStatus,
    pub summary: String,
    pub detail: String,
    pub repair_action: Option<String>,
    pub checked_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthReport {
    pub overall_status: HealthStatus,
    pub hardware_model: String,
    pub hardware_compatible: bool,
    pub kernel_version: String,
    pub checked_at_ms: u64,
    pub healthy_count: usize,
    pub total_count: usize,
    pub daemon_version: Option<String>,
    pub daemon_pid: Option<u32>,
    pub acpi_interface: Option<String>,
    pub subsystems: Vec<HealthSubsystemCard>,
    pub items: Vec<HealthItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseHistoryItem {
    pub version: String,
    pub name: String,
    pub published_at: Option<String>,
    pub is_installed: bool,
    pub notes_summary: Option<String>,
    pub html_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub download_url: Option<String>,
    pub release_notes: Option<String>,
    pub release_name: Option<String>,
    pub published_at: Option<String>,
    pub asset_size_bytes: Option<u64>,
    pub update_command: String,
    pub has_dnf_repo: bool,
    pub checked_at_ms: u64,
    pub history: Vec<ReleaseHistoryItem>,
    pub error: Option<String>,
}

pub fn check_system_health() -> SystemHealthReport {
    let scan_started_ms = now_ms();

    // Fast synchronous local reads (< 1ms total)
    let board_name = read_trimmed("/sys/class/dmi/id/board_name").unwrap_or_default();
    let product_name = read_trimmed("/sys/class/dmi/id/product_name").unwrap_or_default();
    let sys_vendor = read_trimmed("/sys/class/dmi/id/sys_vendor").unwrap_or_default();

    let is_acer = sys_vendor.to_ascii_lowercase().contains("acer");
    let is_verified_model =
        board_name == "AlgAl15g53" || product_name.contains("A715-79G");
    let display_model = if !board_name.is_empty() && board_name != product_name {
        format!("{product_name} ({board_name})")
    } else if !product_name.is_empty() {
        product_name.clone()
    } else {
        format!("{sys_vendor} System").trim().to_string()
    };

    let (uname, _) = kernel_uname_info();
    let modules_out = fs::read_to_string("/proc/modules").unwrap_or_default();
    let acer_keyboard_loaded = modules_out.contains("acer_keyboard");
    let acer_acpi_loaded = modules_out.contains("acer_acpi");
    let acer_io_loaded = modules_out.contains("acer_io");
    let acer_wmi_loaded = modules_out.contains("acer_wmi");

    let dev_io = Path::new("/dev/acer_io");
    let dev_exists = dev_io.exists();
    let dev_rw = dev_exists
        && fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(dev_io)
            .is_ok();

    let nvidia_ver = read_trimmed("/sys/module/nvidia/version");
    let i915_loaded = Path::new("/sys/module/i915").exists();
    let xe_loaded = Path::new("/sys/module/xe").exists();
    let amdgpu_loaded = Path::new("/sys/module/amdgpu").exists();
    let sb_efi = probe_secure_boot_efi();

    // Run slower subprocess + IPC probes concurrently with timeouts via std::thread::scope
    let (
        dkms_probe,
        sb_mok_probe,
        services_probe,
        dbus_probe,
        acpi_iface_probe,
    ) = thread::scope(|s| {
        let dkms_handle = s.spawn(|| {
            let out = run_command_with_timeout(
                "dkms",
                &["status", "-m", "acer-keyboard"],
                Duration::from_secs(3),
            );
            (out, now_ms())
        });

        let sb_handle = s.spawn(|| {
            let sb_enabled = match sb_efi {
                Some(v) => v,
                None => run_command_with_timeout(
                    "mokutil",
                    &["--sb-state"],
                    Duration::from_secs(2),
                )
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .to_ascii_lowercase()
                        .contains("secureboot enabled")
                })
                .unwrap_or(false),
            };
            let mok_enrolled = if sb_enabled && Path::new("/var/lib/dkms/mok.pub").exists() {
                run_command_with_timeout(
                    "mokutil",
                    &["--test-key", "/var/lib/dkms/mok.pub"],
                    Duration::from_secs(2),
                )
                .map(|o| {
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    stdout.contains("already enrolled")
                })
                .unwrap_or(false)
            } else {
                false
            };
            (sb_enabled, mok_enrolled, now_ms())
        });

        let svc_handle = s.spawn(|| {
            let is_active_out = run_command_with_timeout(
                "systemctl",
                &[
                    "is-active",
                    "acer-control-daemon.service",
                    "polkit.service",
                    "systemd-udevd.service",
                ],
                Duration::from_secs(2),
            );
            let pid_out = run_command_with_timeout(
                "systemctl",
                &["show", "acer-control-daemon.service", "-p", "MainPID", "--value"],
                Duration::from_secs(2),
            );
            (is_active_out, pid_out, now_ms())
        });

        let dbus_handle = s.spawn(|| {
            let t0 = Instant::now();
            let res = match AcerControlDBus::new() {
                Ok(client) => match client.version() {
                    Ok(v) => {
                        let rtt_ms = t0.elapsed().as_secs_f64() * 1000.0;
                        let fan_ok = client.fan_max_selected().is_ok();
                        Ok((v, rtt_ms, fan_ok))
                    }
                    Err(e) => Err(format!("GetVersion failed: {e}")),
                },
                Err(e) => Err(e),
            };
            (res, now_ms())
        });

        let iface_handle = s.spawn(|| {
            if !dev_rw {
                return (None, None);
            }
            AcerControlLib::load()
                .ok()
                .and_then(|lib| {
                    lib.open_context()
                        .ok()
                        .map(|ctx| (ctx.driver_interface(), ctx.driver_version()))
                })
                .unwrap_or((None, None))
        });

        (
            dkms_handle.join().unwrap_or((None, scan_started_ms)),
            sb_handle.join().unwrap_or((false, false, scan_started_ms)),
            svc_handle.join().unwrap_or((None, None, scan_started_ms)),
            dbus_handle.join().unwrap_or((Err("D-Bus probe panicked".into()), scan_started_ms)),
            iface_handle.join().unwrap_or((None, None)),
        )
    });

    let (acpi_iface_probe, drv_ver_probe) = acpi_iface_probe;

    let mut items = Vec::new();
    let mut subsystems = Vec::new();
    let mut overall_failed = false;
    let mut overall_warning = false;

    // 1. Hardware Detection
    let (hw_status, hw_label, hw_summary, hw_detail) = if is_verified_model {
        (
            HealthStatus::Passed,
            "Passed",
            format!("{display_model} — Verified Acer Platform"),
            "DMI and SMBIOS hardware profile matched. Full support for acer_acpi EC backend, RGB keyboard, and fan control.".into(),
        )
    } else if is_acer {
        overall_warning = true;
        (
            HealthStatus::Warning,
            "Warning",
            format!("{display_model} — Compatible Acer Device"),
            "Acer chassis detected. Hardware support depends on active acer_acpi or acer_wmi kernel backend compatibility.".into(),
        )
    } else {
        overall_warning = true;
        (
            HealthStatus::Info,
            "Limited",
            format!("{sys_vendor} {product_name}").trim().to_string(),
            "Non-Acer motherboard detected. Vendor-specific EC fan and keyboard backlight interfaces are unavailable.".into(),
        )
    };

    subsystems.push(HealthSubsystemCard {
        id: "hardware".into(),
        title: "Hardware Detection".into(),
        status: hw_status.clone(),
        status_label: hw_label.into(),
        description: if is_verified_model {
            format!("{display_model} chassis and DMI tables verified.")
        } else {
            hw_summary.clone()
        },
    });
    items.push(HealthItem {
        id: "hardware".into(),
        name: "DMI / SMBIOS Hardware Probe".into(),
        status: hw_status,
        summary: hw_summary,
        detail: hw_detail,
        repair_action: None,
        checked_at_ms: scan_started_ms,
    });

    // 2. Acer ACPI Backend + Kernel Modules
    let resolved_iface = acpi_iface_probe.clone().or_else(|| {
        if acer_acpi_loaded {
            Some("acer_acpi".into())
        } else if acer_wmi_loaded {
            Some("acer_wmi".into())
        } else {
            None
        }
    });

    let (acpi_status, acpi_label, acpi_desc, mod_summary, mod_detail, mod_repair) =
        if acer_keyboard_loaded && (acer_acpi_loaded || acer_io_loaded || acer_wmi_loaded) && dev_rw {
            let iface_str = resolved_iface.as_deref().unwrap_or("acer_acpi");
            let drv_ver_line = drv_ver_probe
                .as_deref()
                .map(|v| format!("\nDriver module version: {v}"))
                .unwrap_or_default();
            (
                HealthStatus::Passed,
                "Active",
                format!("{iface_str} thermal telemetry and /dev/acer_io interface responding."),
                format!("Kernel drivers active ({iface_str})"),
                format!("Kernel: {uname}\nModules loaded: acer_keyboard, acer_acpi ({acer_acpi_loaded}), acer_io ({acer_io_loaded}), acer_wmi ({acer_wmi_loaded})\nActive IOCTL interface: {iface_str}{drv_ver_line}"),
                None,
            )
        } else if acer_keyboard_loaded && dev_exists && !dev_rw {
            overall_warning = true;
            (
                HealthStatus::Warning,
                "Restricted",
                "/dev/acer_io exists but current user lacks write permissions.".into(),
                "Driver loaded with restricted device node permissions".into(),
                format!("Kernel: {uname}\nAdd user to acercontrol group: sudo usermod -aG acercontrol $USER"),
                Some("sudo usermod -aG acercontrol $USER".into()),
            )
        } else if !is_acer {
            (
                HealthStatus::Info,
                "Unavailable",
                "Acer EC hardware backend not applicable on non-Acer systems.".into(),
                "Acer hardware drivers not loaded".into(),
                format!("Kernel: {uname}\nNo Acer platform EC interface detected."),
                None,
            )
        } else {
            overall_failed = true;
            (
                HealthStatus::Failed,
                "Offline",
                "Required Acer kernel modules or /dev/acer_io not responding.".into(),
                "Kernel modules not loaded".into(),
                format!("Kernel: {uname}\nacer_keyboard ({acer_keyboard_loaded}), acer_acpi ({acer_acpi_loaded}), acer_io ({acer_io_loaded})\nRun: sudo modprobe acer_keyboard acer_acpi acer_io"),
                Some("sudo modprobe acer_keyboard acer_acpi acer_io".into()),
            )
        };

    subsystems.push(HealthSubsystemCard {
        id: "acpi_backend".into(),
        title: "Acer ACPI Backend".into(),
        status: acpi_status.clone(),
        status_label: acpi_label.into(),
        description: acpi_desc,
    });
    items.push(HealthItem {
        id: "kernel_modules".into(),
        name: "ACPI EC & IOCTL Interface".into(),
        status: acpi_status,
        summary: mod_summary,
        detail: mod_detail,
        repair_action: mod_repair,
        checked_at_ms: now_ms(),
    });

    // 3. DKMS Module Status
    let (dkms_opt, dkms_ts) = dkms_probe;
    let dkms_dir_built =
        Path::new(&format!("/var/lib/dkms/acer-keyboard/0.1.1/{uname}")).exists();
    let (dkms_status, dkms_label, dkms_desc, dkms_sum, dkms_det, dkms_repair) = match dkms_opt {
        Some(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let built_for_running = stdout.lines().any(|l| l.contains(&uname) && l.contains("installed"))
                || (stdout.contains("installed") && dkms_dir_built);

            if built_for_running {
                (
                    HealthStatus::Passed,
                    "Loaded",
                    format!("acer-keyboard v0.1.1 built and installed for {uname}."),
                    "DKMS driver installed for active kernel".into(),
                    stdout,
                    None,
                )
            } else if stdout.contains("installed") {
                overall_warning = true;
                (
                    HealthStatus::Warning,
                    "Partial",
                    format!("DKMS registered, but module not built specifically for {uname}."),
                    "DKMS build missing for running kernel".into(),
                    stdout,
                    Some("sudo dkms autoinstall".into()),
                )
            } else if !stdout.is_empty() {
                overall_warning = true;
                (
                    HealthStatus::Warning,
                    "Registered",
                    "DKMS source registered; compilation required for active kernel.".into(),
                    "DKMS module not built".into(),
                    stdout,
                    Some("sudo dkms autoinstall".into()),
                )
            } else {
                overall_failed = true;
                (
                    HealthStatus::Failed,
                    "Missing",
                    "acer-keyboard driver not registered in DKMS.".into(),
                    "DKMS registration missing".into(),
                    "Run: sudo dkms add -m acer-keyboard -v 0.1.1 && sudo dkms install -m acer-keyboard -v 0.1.1".into(),
                    Some("sudo dkms add -m acer-keyboard -v 0.1.1 && sudo dkms autoinstall".into()),
                )
            }
        }
        None if dkms_dir_built => (
            HealthStatus::Passed,
            "Loaded",
            format!("acer-keyboard 0.1.1 verified in /var/lib/dkms for {uname}."),
            "DKMS kernel module present".into(),
            format!("Verified artifact directory /var/lib/dkms/acer-keyboard/0.1.1/{uname}"),
            None,
        ),
        None => {
            overall_warning = true;
            (
                HealthStatus::Warning,
                "Timeout",
                "DKMS CLI check timed out or is unavailable.".into(),
                "DKMS CLI unavailable".into(),
                "Cannot query dkms binary within timeout.".into(),
                None,
            )
        }
    };

    subsystems.push(HealthSubsystemCard {
        id: "dkms".into(),
        title: "DKMS Module Status".into(),
        status: dkms_status.clone(),
        status_label: dkms_label.into(),
        description: dkms_desc,
    });
    items.push(HealthItem {
        id: "dkms".into(),
        name: "DKMS Kernel Module Integrity".into(),
        status: dkms_status,
        summary: dkms_sum,
        detail: dkms_det,
        repair_action: dkms_repair,
        checked_at_ms: dkms_ts,
    });

    // 4. Daemon & D-Bus
    let (dbus_res, dbus_ts) = dbus_probe;
    let (svc_active_opt, svc_pid_opt, svc_ts) = services_probe;
    let daemon_pid = svc_pid_opt
        .as_ref()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u32>().ok())
        .filter(|&pid| pid > 0);

    let (dbus_status, dbus_label, dbus_desc, dbus_sum, dbus_det, dbus_rep, daemon_ver) =
        match dbus_res {
            Ok((ver, rtt_ms, _fan_ok)) => {
                let pid_text = daemon_pid
                    .map(|p| format!(" (PID {p})"))
                    .unwrap_or_default();
                (
                    HealthStatus::Passed,
                    "Connected",
                    format!("io.github.stanlysilas.AcerControl v{ver} active{pid_text}."),
                    format!("Connected (v{ver})"),
                    format!("Round-trip IPC latency {rtt_ms:.2} ms"),
                    None,
                    Some(ver),
                )
            }
            Err(e) => {
                overall_failed = true;
                (
                    HealthStatus::Failed,
                    "Offline",
                    "Cannot reach acer-control-daemon on the system D-Bus.".into(),
                    "Daemon unreachable on system bus".into(),
                    format!("Cannot connect to io.github.stanlysilas.AcerControl: {e}"),
                    Some("sudo systemctl start acer-control-daemon.service".into()),
                    None,
                )
            }
        };

    subsystems.push(HealthSubsystemCard {
        id: "daemon_dbus".into(),
        title: "Daemon & D-Bus".into(),
        status: dbus_status.clone(),
        status_label: dbus_label.into(),
        description: dbus_desc,
    });
    items.push(HealthItem {
        id: "daemon".into(),
        name: "D-Bus System Socket & Daemon IPC".into(),
        status: dbus_status,
        summary: dbus_sum,
        detail: dbus_det,
        repair_action: dbus_rep,
        checked_at_ms: dbus_ts,
    });

    // 5. System Services
    let svc_names = [
        "acer-control-daemon.service",
        "polkit.service",
        "systemd-udevd.service",
    ];
    let (svc_status, svc_label, svc_desc, svc_sum, svc_det, svc_rep) = match &svc_active_opt {
        Some(out) => {
            let states: Vec<String> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .collect();
            let inactive: Vec<&str> = svc_names
                .iter()
                .enumerate()
                .filter_map(|(idx, &name)| {
                    if states.get(idx).map(|s| s == "active").unwrap_or(false) {
                        None
                    } else {
                        Some(name)
                    }
                })
                .collect();

            if inactive.is_empty() {
                (
                    HealthStatus::Passed,
                    "Running",
                    "acer-control-daemon, polkit, and systemd-udevd operational.".into(),
                    "All core system services active".into(),
                    "acer-control-daemon: active · polkit: active · systemd-udevd: active".into(),
                    None,
                )
            } else {
                overall_failed = true;
                let missing = inactive.join(", ");
                (
                    HealthStatus::Failed,
                    "Degraded",
                    format!("Inactive services: {missing}."),
                    format!("Service(s) not running: {missing}"),
                    format!("Run: sudo systemctl start {}", inactive[0]),
                    Some(format!("sudo systemctl start {}", inactive[0])),
                )
            }
        }
        None => {
            overall_warning = true;
            (
                HealthStatus::Warning,
                "Unknown",
                "Unable to query systemd service units within timeout.".into(),
                "Systemd status check failed".into(),
                "systemctl is-active probe did not return a status list.".into(),
                None,
            )
        }
    };

    subsystems.push(HealthSubsystemCard {
        id: "system_services".into(),
        title: "System Services".into(),
        status: svc_status.clone(),
        status_label: svc_label.into(),
        description: svc_desc,
    });
    items.push(HealthItem {
        id: "services".into(),
        name: "Systemd Units & Policy Services".into(),
        status: svc_status,
        summary: svc_sum,
        detail: svc_det,
        repair_action: svc_rep,
        checked_at_ms: svc_ts,
    });

    // 6. Driver Compatibility, Secure Boot & Permissions
    let (sb_enabled, mok_enrolled, sb_ts) = sb_mok_probe;
    let mut gpu_drv_parts = Vec::new();
    if let Some(nv) = nvidia_ver.as_deref() {
        gpu_drv_parts.push(format!("NVIDIA {nv}"));
    }
    if xe_loaded && i915_loaded {
        gpu_drv_parts.push("Intel i915/xe".into());
    } else if i915_loaded {
        gpu_drv_parts.push("Intel i915".into());
    } else if xe_loaded {
        gpu_drv_parts.push("Intel xe".into());
    }
    if amdgpu_loaded {
        gpu_drv_parts.push("AMD amdgpu".into());
    }
    let gpu_compat_text = if gpu_drv_parts.is_empty() {
        "No discrete/integrated GPU kernel driver detected".to_string()
    } else {
        format!("{} kernel drivers verified.", gpu_drv_parts.join(" & "))
    };

    let (compat_status, compat_label, compat_desc) = if sb_enabled
        && Path::new("/var/lib/dkms/mok.pub").exists()
        && !mok_enrolled
    {
        overall_warning = true;
        (
            HealthStatus::Warning,
            "MOK Pending",
            format!("{gpu_compat_text} Secure Boot enabled; DKMS MOK enrollment pending."),
        )
    } else if !dev_rw && dev_exists {
        overall_warning = true;
        (
            HealthStatus::Warning,
            "Permissions",
            format!("{gpu_compat_text} /dev/acer_io lacks group write access."),
        )
    } else {
        (HealthStatus::Passed, "Compatible", gpu_compat_text.clone())
    };

    subsystems.push(HealthSubsystemCard {
        id: "driver_compat".into(),
        title: "Driver Compatibility".into(),
        status: compat_status.clone(),
        status_label: compat_label.into(),
        description: compat_desc,
    });

    // Detailed items for Secure Boot & Device Permissions in the diagnostic log table
    if sb_enabled {
        let (sb_st, sb_sum, sb_det, sb_rep) = if mok_enrolled {
            (
                HealthStatus::Passed,
                "Secure Boot enabled with enrolled MOK key".into(),
                "Kernel module signing verified via Machine Owner Key (/var/lib/dkms/mok.pub).".into(),
                None,
            )
        } else if Path::new("/var/lib/dkms/mok.pub").exists() {
            (
                HealthStatus::Warning,
                "Secure Boot active; MOK enrollment pending".into(),
                "Run: sudo mokutil --import /var/lib/dkms/mok.pub && reboot".into(),
                Some("sudo mokutil --import /var/lib/dkms/mok.pub".into()),
            )
        } else {
            (
                HealthStatus::Info,
                "Secure Boot enabled".into(),
                "Kernel modules operating under system Secure Boot policy.".into(),
                None,
            )
        };
        items.push(HealthItem {
            id: "secure_boot".into(),
            name: "Secure Boot & MOK Signing Key".into(),
            status: sb_st,
            summary: sb_sum,
            detail: sb_det,
            repair_action: sb_rep,
            checked_at_ms: sb_ts,
        });
    }

    let (perm_st, perm_sum, perm_det, perm_rep) = if dev_rw {
        (
            HealthStatus::Passed,
            "/dev/acer_io accessible with read/write permissions".into(),
            "Authorized via udev / 'acercontrol' group rules.".into(),
            None,
        )
    } else if dev_exists {
        (
            HealthStatus::Warning,
            "/dev/acer_io lacks write permissions".into(),
            "Run: sudo usermod -aG acercontrol $USER (and re-login)".into(),
            Some("sudo usermod -aG acercontrol $USER".into()),
        )
    } else {
        (
            HealthStatus::Info,
            "/dev/acer_io character device absent".into(),
            "Character device node missing; EC fan controls may be unavailable on this hardware.".into(),
            None,
        )
    };
    items.push(HealthItem {
        id: "permissions".into(),
        name: "Device Node & Udev Permissions".into(),
        status: perm_st,
        summary: perm_sum,
        detail: perm_det,
        repair_action: perm_rep,
        checked_at_ms: now_ms(),
    });

    let healthy_count = subsystems
        .iter()
        .filter(|c| c.status == HealthStatus::Passed)
        .count();
    let total_count = subsystems.len();

    let overall_status = if overall_failed {
        HealthStatus::Failed
    } else if overall_warning {
        HealthStatus::Warning
    } else {
        HealthStatus::Passed
    };

    SystemHealthReport {
        overall_status,
        hardware_model: display_model,
        hardware_compatible: is_verified_model || is_acer,
        kernel_version: uname,
        checked_at_ms: now_ms(),
        healthy_count,
        total_count,
        daemon_version: daemon_ver,
        daemon_pid,
        acpi_interface: resolved_iface,
        subsystems,
        items,
    }
}

/// Represents a structured parsed version, supporting RPM-style release numbers
/// (e.g., "2.0.2-2", "2.0.2-1", "v2.0.3-1", "2.0.2-2.fc45").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedVersion {
    pub numbers: Vec<u32>,
    pub release: Option<u32>,
}

impl ParsedVersion {
    pub fn parse(s: &str) -> Self {
        let trimmed = s.trim().trim_start_matches(|c| c == 'v' || c == 'V');
        let parts: Vec<&str> = trimmed.splitn(2, '-').collect();
        let ver_part = parts[0];
        let rel_part = parts.get(1);

        let numbers: Vec<u32> = ver_part
            .split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .collect();

        let release = rel_part.and_then(|r| {
            let digits: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse::<u32>().ok()
        });

        Self { numbers, release }
    }

    /// Returns true if `remote` is strictly newer than `self` (local).
    pub fn is_remote_newer(&self, remote: &Self) -> bool {
        let max_len = self.numbers.len().max(remote.numbers.len());
        for i in 0..max_len {
            let l = self.numbers.get(i).copied().unwrap_or(0);
            let r = remote.numbers.get(i).copied().unwrap_or(0);
            if r > l {
                return true;
            } else if r < l {
                return false;
            }
        }

        let l_rel = self.release.unwrap_or(1);
        let r_rel = remote.release.unwrap_or(1);
        r_rel > l_rel
    }
}

/// Dynamically queries the installed package version.
pub fn detect_installed_version() -> String {
    if let Ok(out) = Command::new("rpm")
        .args(["-q", "--qf", "%{VERSION}-%{RELEASE}", "uxf-control-center"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() && !s.contains("not installed") && !s.contains("error") {
                return s;
            }
        }
    }

    if let Ok(out) = Command::new("rpm")
        .args(["-q", "--qf", "%{VERSION}-%{RELEASE}", "acer-control-center"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() && !s.contains("not installed") && !s.contains("error") {
                return s;
            }
        }
    }

    option_env!("UXF_PKG_FULL_VERSION")
        .unwrap_or("2.0.2-4")
        .to_string()
}

pub fn is_semver_greater(local: &str, remote: &str) -> bool {
    let local_v = ParsedVersion::parse(local);
    let remote_v = ParsedVersion::parse(remote);
    local_v.is_remote_newer(&remote_v)
}

fn summarize_release_notes(body: Option<&str>) -> Option<String> {
    let raw = body?;
    for line in raw.lines() {
        let cleaned = line.trim().trim_start_matches(['-', '*', '#']).trim();
        if cleaned.is_empty()
            || cleaned.starts_with("```")
            || cleaned.starts_with("sudo ")
            || cleaned.starts_with("SHA-256")
            || cleaned.starts_with("Target OS")
            || cleaned.starts_with("File:")
        {
            continue;
        }
        return Some(cleaned.chars().take(120).collect());
    }
    None
}

pub fn check_app_update(current_version: &str) -> UpdateCheckResult {
    let checked_at_ms = now_ms();
    let effective_version =
        if current_version.trim().is_empty() || current_version.trim() == "1.0.0" {
            detect_installed_version()
        } else if !current_version.contains('-') {
            let detected = detect_installed_version();
            if detected.starts_with(current_version.trim()) && detected.contains('-') {
                detected
            } else {
                current_version.trim().to_string()
            }
        } else {
            current_version.trim().to_string()
        };

    let has_dnf_repo = Path::new("/etc/yum.repos.d")
        .read_dir()
        .map(|entries| {
            entries.flatten().any(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.contains("uxf") || name.contains("acer-control")
            })
        })
        .unwrap_or(false);

    let default_update_command = if has_dnf_repo {
        "sudo dnf upgrade uxf-control-center".to_string()
    } else {
        "Download RPM from GitHub Releases and run: sudo dnf install ./uxf-control-center-*.rpm"
            .to_string()
    };

    let local_fallback_history = vec![ReleaseHistoryItem {
        version: effective_version.clone(),
        name: format!("UXF-Control-Center v{effective_version}"),
        published_at: None,
        is_installed: true,
        notes_summary: Some("Current installed build".into()),
        html_url: Some("https://github.com/Urxfather/UXF-Control-Center/releases".into()),
    }];

    // Single fast request fetching up to 6 releases (covers latest stable + update history!)
    let api_url = "https://api.github.com/repos/Urxfather/UXF-Control-Center/releases?per_page=6";
    let mut output = run_command_with_timeout(
        "curl",
        &[
            "-sS",
            "--happy-eyeballs-timeout-ms",
            "200",
            "--connect-timeout",
            "5",
            "--max-time",
            "9",
            "-A",
            "UXF-Control-Center",
            "-H",
            "Accept: application/vnd.github.v3+json",
            api_url,
        ],
        Duration::from_secs(10),
    );

    // If dual-stack connection or timeout fails (curl exit 7 or 28), fall back once to IPv4.
    if output.as_ref().is_none_or(|o| {
        !o.status.success() && matches!(o.status.code(), Some(7) | Some(28))
    }) {
        if let Some(v4_out) = run_command_with_timeout(
            "curl",
            &[
                "-4",
                "-sS",
                "--connect-timeout",
                "4",
                "--max-time",
                "6",
                "-A",
                "UXF-Control-Center",
                "-H",
                "Accept: application/vnd.github.v3+json",
                api_url,
            ],
            Duration::from_secs(7),
        ) {
            output = Some(v4_out);
        }
    }

    let Some(out) = output else {
        return UpdateCheckResult {
            current_version: effective_version,
            latest_version: None,
            update_available: false,
            download_url: None,
            release_notes: None,
            release_name: None,
            published_at: None,
            asset_size_bytes: None,
            update_command: default_update_command,
            has_dnf_repo,
            checked_at_ms,
            history: local_fallback_history,
            error: Some("Network request to GitHub Releases timed out".into()),
        };
    };

    if !out.status.success() {
        return UpdateCheckResult {
            current_version: effective_version,
            latest_version: None,
            update_available: false,
            download_url: None,
            release_notes: None,
            release_name: None,
            published_at: None,
            asset_size_bytes: None,
            update_command: default_update_command,
            has_dnf_repo,
            checked_at_ms,
            history: local_fallback_history,
            error: Some("Unable to reach GitHub Releases API".into()),
        };
    }

    let Ok(json_str) = String::from_utf8(out.stdout) else {
        return UpdateCheckResult {
            current_version: effective_version,
            latest_version: None,
            update_available: false,
            download_url: None,
            release_notes: None,
            release_name: None,
            published_at: None,
            asset_size_bytes: None,
            update_command: default_update_command,
            has_dnf_repo,
            checked_at_ms,
            history: local_fallback_history,
            error: Some("Invalid UTF-8 response from GitHub Releases".into()),
        };
    };

    let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) else {
        return UpdateCheckResult {
            current_version: effective_version,
            latest_version: None,
            update_available: false,
            download_url: None,
            release_notes: None,
            release_name: None,
            published_at: None,
            asset_size_bytes: None,
            update_command: default_update_command,
            has_dnf_repo,
            checked_at_ms,
            history: local_fallback_history,
            error: Some("Unable to parse GitHub Releases JSON payload".into()),
        };
    };

    // GitHub API may return an error object (e.g. rate limit exceeded)
    if let Some(msg) = val.get("message").and_then(|m| m.as_str()) {
        return UpdateCheckResult {
            current_version: effective_version,
            latest_version: None,
            update_available: false,
            download_url: None,
            release_notes: None,
            release_name: None,
            published_at: None,
            asset_size_bytes: None,
            update_command: default_update_command,
            has_dnf_repo,
            checked_at_ms,
            history: local_fallback_history,
            error: Some(format!("GitHub API: {msg}")),
        };
    }

    let Some(arr) = val.as_array() else {
        return UpdateCheckResult {
            current_version: effective_version,
            latest_version: None,
            update_available: false,
            download_url: None,
            release_notes: None,
            release_name: None,
            published_at: None,
            asset_size_bytes: None,
            update_command: default_update_command,
            has_dnf_repo,
            checked_at_ms,
            history: local_fallback_history,
            error: Some("Unexpected response format from GitHub Releases".into()),
        };
    };

    let stable: Vec<&serde_json::Value> = arr
        .iter()
        .filter(|rel| {
            let draft = rel.get("draft").and_then(|d| d.as_bool()).unwrap_or(false);
            let pre = rel
                .get("prerelease")
                .and_then(|p| p.as_bool())
                .unwrap_or(false);
            !draft && !pre && rel.get("tag_name").and_then(|t| t.as_str()).is_some()
        })
        .collect();

    let mut history: Vec<ReleaseHistoryItem> = stable
        .iter()
        .take(5)
        .filter_map(|rel| {
            let tag = rel.get("tag_name")?.as_str()?;
            let ver = tag.trim_start_matches(|c| c == 'v' || c == 'V').to_string();
            let is_installed = !is_semver_greater(&effective_version, &ver)
                && !is_semver_greater(&ver, &effective_version);
            Some(ReleaseHistoryItem {
                version: ver.clone(),
                name: rel
                    .get("name")
                    .and_then(|n| n.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("v{ver}")),
                published_at: rel
                    .get("published_at")
                    .and_then(|p| p.as_str())
                    .map(|s| s.chars().take(10).collect()),
                is_installed,
                notes_summary: if is_installed {
                    Some("Current installed build".into())
                } else {
                    summarize_release_notes(rel.get("body").and_then(|b| b.as_str()))
                },
                html_url: rel
                    .get("html_url")
                    .and_then(|u| u.as_str())
                    .map(|s| s.to_string()),
            })
        })
        .collect();

    if history.is_empty() {
        history = local_fallback_history;
    }

    if let Some(latest_rel) = stable.first() {
        let tag = latest_rel
            .get("tag_name")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let latest_ver = tag.trim_start_matches(|c| c == 'v' || c == 'V').to_string();
        let is_newer = is_semver_greater(&effective_version, &latest_ver);

        let rpm_asset = latest_rel
            .get("assets")
            .and_then(|a| a.as_array())
            .and_then(|assets| {
                assets.iter().find(|item| {
                    item.get("name")
                        .and_then(|n| n.as_str())
                        .map(|n| n.ends_with(".rpm") && n.contains("x86_64"))
                        .unwrap_or(false)
                })
            });

        let asset_size_bytes = rpm_asset
            .and_then(|a| a.get("size"))
            .and_then(|s| s.as_u64());
        let download_url = if is_newer {
            rpm_asset
                .and_then(|a| a.get("browser_download_url"))
                .and_then(|u| u.as_str())
                .map(|s| s.to_string())
        } else {
            rpm_asset
                .and_then(|a| a.get("browser_download_url"))
                .and_then(|u| u.as_str())
                .map(|s| s.to_string())
        };

        let body = latest_rel
            .get("body")
            .and_then(|b| b.as_str())
            .map(|s| s.to_string());
        let release_name = latest_rel
            .get("name")
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());
        let published_at = latest_rel
            .get("published_at")
            .and_then(|p| p.as_str())
            .map(|s| s.chars().take(10).collect());

        let update_cmd = if !is_newer {
            default_update_command
        } else if has_dnf_repo {
            "sudo dnf upgrade uxf-control-center".to_string()
        } else if let Some(ref url) = download_url {
            let filename = url.split('/').last().unwrap_or("uxf-control-center.rpm");
            format!("curl -LO \"{url}\" && sudo dnf install \"./{filename}\"")
        } else {
            default_update_command
        };

        return UpdateCheckResult {
            current_version: effective_version,
            latest_version: Some(latest_ver),
            update_available: is_newer,
            download_url: if is_newer { download_url } else { None },
            release_notes: body,
            release_name,
            published_at,
            asset_size_bytes,
            update_command: update_cmd,
            has_dnf_repo,
            checked_at_ms,
            history,
            error: None,
        };
    }

    UpdateCheckResult {
        current_version: effective_version,
        latest_version: None,
        update_available: false,
        download_url: None,
        release_notes: None,
        release_name: None,
        published_at: None,
        asset_size_bytes: None,
        update_command: default_update_command,
        has_dnf_repo,
        checked_at_ms,
        history,
        error: None,
    }
}
