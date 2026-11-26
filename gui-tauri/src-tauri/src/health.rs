use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use crate::dbus::AcerControlDBus;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Passed,
    Warning,
    Failed,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthItem {
    pub id: String,
    pub name: String,
    pub status: HealthStatus,
    pub summary: String,
    pub detail: String,
    pub repair_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthReport {
    pub overall_status: HealthStatus,
    pub hardware_model: String,
    pub hardware_compatible: bool,
    pub kernel_version: String,
    pub items: Vec<HealthItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub download_url: Option<String>,
    pub release_notes: Option<String>,
    pub update_command: String,
    pub has_dnf_repo: bool,
    pub error: Option<String>,
}

pub fn check_system_health() -> SystemHealthReport {
    let mut items = Vec::new();
    let mut overall_failed = false;
    let mut overall_warning = false;

    // 1. Hardware Model & Platform Detection
    let board_name = fs::read_to_string("/sys/class/dmi/id/board_name")
        .unwrap_or_default()
        .trim()
        .to_string();
    let product_name = fs::read_to_string("/sys/class/dmi/id/product_name")
        .unwrap_or_default()
        .trim()
        .to_string();
    let sys_vendor = fs::read_to_string("/sys/class/dmi/id/sys_vendor")
        .unwrap_or_default()
        .trim()
        .to_string();

    let is_acer = sys_vendor.to_ascii_lowercase().contains("acer");
    let is_verified_model = board_name == "AlgAl15g53" || product_name.contains("A715-79G");

    let (hw_status, hw_summary, hw_detail) = if is_verified_model {
        (
            HealthStatus::Passed,
            format!("{product_name} ({board_name}) — Verified Acer Platform"),
            "Target hardware profile matched. Full support for acer_acpi EC backend, RGB keyboard, and fan control.".into(),
        )
    } else if is_acer {
        overall_warning = true;
        (
            HealthStatus::Warning,
            format!("{product_name} ({board_name}) — Compatible Acer Device"),
            "Acer system detected. Basic ACPI and fan controls are supported depending on EC driver compatibility.".into(),
        )
    } else {
        overall_warning = true;
        (
            HealthStatus::Info,
            format!("{sys_vendor} {product_name}"),
            "Non-Acer or generic motherboard detected. Hardware fan and keyboard backlight features may be limited.".into(),
        )
    };

    items.push(HealthItem {
        id: "hardware".into(),
        name: "Hardware Platform".into(),
        status: hw_status,
        summary: hw_summary,
        detail: hw_detail,
        repair_action: None,
    });

    // 2. Kernel & Kernel Modules Check
    let uname = Command::new("uname")
        .arg("-r")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());

    let modules_out = fs::read_to_string("/proc/modules").unwrap_or_default();
    let acer_keyboard_loaded = modules_out.contains("acer_keyboard");
    let acer_acpi_loaded = modules_out.contains("acer_acpi");
    let acer_io_loaded = modules_out.contains("acer_io");

    let (mod_status, mod_summary, mod_detail, mod_repair) = if acer_keyboard_loaded && (acer_acpi_loaded || acer_io_loaded) {
        (
            HealthStatus::Passed,
            "Required kernel drivers loaded".into(),
            format!("Running kernel: {uname}\nLoaded modules: acer_keyboard, acer_acpi ({acer_acpi_loaded}), acer_io ({acer_io_loaded})"),
            None,
        )
    } else {
        overall_failed = true;
        (
            HealthStatus::Failed,
            "Kernel modules not loaded".into(),
            format!("Running kernel: {uname}\nDriver modules missing from kernel runtime.\nRun: sudo modprobe acer_keyboard acer_acpi acer_io"),
            Some("sudo modprobe acer_keyboard acer_acpi acer_io".into()),
        )
    };

    items.push(HealthItem {
        id: "kernel_modules".into(),
        name: "Kernel Driver Modules".into(),
        status: mod_status,
        summary: mod_summary,
        detail: mod_detail,
        repair_action: mod_repair,
    });

    // 3. DKMS Driver Status
    let dkms_status_cmd = Command::new("dkms")
        .args(["status", "-m", "acer-keyboard"])
        .output();
    let (dkms_st, dkms_sum, dkms_det, dkms_repair) = match dkms_status_cmd {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains("installed") {
                (
                    HealthStatus::Passed,
                    "DKMS driver installed and active".into(),
                    stdout.trim().to_string(),
                    None,
                )
            } else if !stdout.trim().is_empty() {
                overall_warning = true;
                (
                    HealthStatus::Warning,
                    "DKMS driver registered but not built for current kernel".into(),
                    stdout.trim().to_string(),
                    Some("sudo dkms autoinstall".into()),
                )
            } else {
                overall_failed = true;
                (
                    HealthStatus::Failed,
                    "acer-keyboard driver not registered with DKMS".into(),
                    "DKMS tree missing. Run: sudo dkms add -m acer-keyboard -v 0.1.1 && sudo dkms install -m acer-keyboard -v 0.1.1".into(),
                    Some("sudo dkms add -m acer-keyboard -v 0.1.1 && sudo dkms autoinstall".into()),
                )
            }
        }
        Err(_) => (
            HealthStatus::Info,
            "DKMS CLI not available".into(),
            "Cannot probe DKMS CLI directly. If modules are loaded, driver operates normally.".into(),
            None,
        ),
    };

    items.push(HealthItem {
        id: "dkms".into(),
        name: "DKMS Status".into(),
        status: dkms_st,
        summary: dkms_sum,
        detail: dkms_det,
        repair_action: dkms_repair,
    });

    // 4. Secure Boot & MOK Check
    let sb_state = Command::new("mokutil")
        .arg("--sb-state")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    if sb_state.to_ascii_lowercase().contains("secureboot enabled") {
        let mok_key_enrolled = Command::new("mokutil")
            .args(["--test-key", "/var/lib/dkms/mok.pub"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("already enrolled"))
            .unwrap_or(false);

        let (sb_st, sb_sum, sb_det, sb_rep) = if mok_key_enrolled {
            (
                HealthStatus::Passed,
                "Secure Boot enabled with enrolled MOK key".into(),
                "Kernel module signing verified via Machine Owner Key (MOK).".into(),
                None,
            )
        } else if Path::new("/var/lib/dkms/mok.pub").exists() {
            overall_warning = true;
            (
                HealthStatus::Warning,
                "Secure Boot active; MOK enrollment pending".into(),
                "DKMS key exists at /var/lib/dkms/mok.pub but requires enrollment.\nRun: sudo mokutil --import /var/lib/dkms/mok.pub && reboot".into(),
                Some("sudo mokutil --import /var/lib/dkms/mok.pub".into()),
            )
        } else {
            (
                HealthStatus::Info,
                "Secure Boot enabled".into(),
                "Kernel modules loaded successfully under current Secure Boot policy.".into(),
                None,
            )
        };

        items.push(HealthItem {
            id: "secure_boot".into(),
            name: "Secure Boot Verification".into(),
            status: sb_st,
            summary: sb_sum,
            detail: sb_det,
            repair_action: sb_rep,
        });
    }

    // 5. Control Daemon & D-Bus Service Check
    match AcerControlDBus::new() {
        Ok(client) => match client.version() {
            Ok(v) => {
                items.push(HealthItem {
                    id: "daemon".into(),
                    name: "Control Daemon (D-Bus)".into(),
                    status: HealthStatus::Passed,
                    summary: format!("Connected (v{v})"),
                    detail: "System D-Bus interface io.github.stanlysilas.AcerControl is active and responsive.".into(),
                    repair_action: None,
                });
            }
            Err(e) => {
                overall_failed = true;
                items.push(HealthItem {
                    id: "daemon".into(),
                    name: "Control Daemon (D-Bus)".into(),
                    status: HealthStatus::Failed,
                    summary: "D-Bus endpoint error".into(),
                    detail: format!("Daemon reached on bus but GetVersion failed: {e}"),
                    repair_action: Some("sudo systemctl restart acer-control-daemon.service".into()),
                });
            }
        },
        Err(e) => {
            overall_failed = true;
            items.push(HealthItem {
                id: "daemon".into(),
                name: "Control Daemon (D-Bus)".into(),
                status: HealthStatus::Failed,
                summary: "Daemon unreachable on system bus".into(),
                detail: format!("Cannot connect to io.github.stanlysilas.AcerControl: {e}\nTry: sudo systemctl start acer-control-daemon.service"),
                repair_action: Some("sudo systemctl start acer-control-daemon.service".into()),
            });
        }
    }

    // 6. Device Permissions (/dev/acer_io & /sys/class/leds)
    let dev_io = Path::new("/dev/acer_io");
    let dev_exists = dev_io.exists();
    let dev_rw = dev_exists && fs::OpenOptions::new().read(true).write(true).open(dev_io).is_ok();
    let (perm_st, perm_sum, perm_det, perm_rep) = if dev_rw {
        (
            HealthStatus::Passed,
            "/dev/acer_io accessible with read/write permissions".into(),
            "User is authorized via udev / 'acercontrol' group rules.".into(),
            None,
        )
    } else if dev_exists {
        overall_warning = true;
        (
            HealthStatus::Warning,
            "/dev/acer_io exists but lacks write permissions".into(),
            "Add current user to acercontrol group: sudo usermod -aG acercontrol $USER".into(),
            Some("sudo usermod -aG acercontrol $USER".into()),
        )
    } else {
        (
            HealthStatus::Info,
            "/dev/acer_io character device absent".into(),
            "Character device node not created. Fan speed IOCTL may use ACPI sysfs fallback.".into(),
            None,
        )
    };

    items.push(HealthItem {
        id: "permissions".into(),
        name: "Device Node & Permissions".into(),
        status: perm_st,
        summary: perm_sum,
        detail: perm_det,
        repair_action: perm_rep,
    });

    let overall_status = if overall_failed {
        HealthStatus::Failed
    } else if overall_warning {
        HealthStatus::Warning
    } else {
        HealthStatus::Passed
    };

    SystemHealthReport {
        overall_status,
        hardware_model: format!("{product_name} ({board_name})"),
        hardware_compatible: is_verified_model || is_acer,
        kernel_version: uname,
        items,
    }
}

/// Compares two semver strings (e.g. "2.0.2" vs "2.0.3").
/// Returns true if remote is strictly greater than local.
pub fn is_semver_greater(local: &str, remote: &str) -> bool {
    let parse_nums = |s: &str| -> Vec<u32> {
        s.trim_start_matches('v')
            .split(['.', '-'])
            .filter_map(|part| part.parse::<u32>().ok())
            .collect()
    };

    let local_parts = parse_nums(local);
    let remote_parts = parse_nums(remote);

    for (l, r) in local_parts.iter().zip(remote_parts.iter()) {
        if r > l {
            return true;
        } else if r < l {
            return false;
        }
    }
    remote_parts.len() > local_parts.len()
}

pub fn check_app_update(current_version: &str) -> UpdateCheckResult {
    // Check if a DNF repo configured for uxf-control-center exists in /etc/yum.repos.d/
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
        "Download RPM from GitHub Releases and run: sudo dnf install ./uxf-control-center-*.rpm".to_string()
    };

    // Query GitHub API for the latest release (safe read-only query with timeout)
    let output = Command::new("curl")
        .args([
            "-s",
            "--max-time",
            "5",
            "-H",
            "Accept: application/vnd.github.v3+json",
            "https://api.github.com/repos/Urxfather/UXF-Control-Center/releases/latest",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            if let Ok(json_str) = String::from_utf8(out.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let is_draft = val.get("draft").and_then(|d| d.as_bool()).unwrap_or(false);
                    let is_prerelease = val.get("prerelease").and_then(|p| p.as_bool()).unwrap_or(false);

                    if is_draft || is_prerelease {
                        return UpdateCheckResult {
                            current_version: current_version.to_string(),
                            latest_version: None,
                            update_available: false,
                            download_url: None,
                            release_notes: None,
                            update_command: default_update_command,
                            has_dnf_repo,
                            error: None,
                        };
                    }

                    if let Some(tag) = val.get("tag_name").and_then(|t| t.as_str()) {
                        let latest_ver = tag.trim_start_matches('v').to_string();
                        let is_newer = is_semver_greater(current_version, &latest_ver);

                        let download_url = val
                            .get("assets")
                            .and_then(|a| a.as_array())
                            .and_then(|arr| {
                                arr.iter().find_map(|item| {
                                    let name = item.get("name")?.as_str()?;
                                    if name.ends_with(".rpm") && name.contains("x86_64") {
                                        item.get("browser_download_url")?.as_str().map(|s| s.to_string())
                                    } else {
                                        None
                                    }
                                })
                            });

                        let body = val.get("body").and_then(|b| b.as_str()).map(|s| s.to_string());

                        let update_cmd = if has_dnf_repo {
                            "sudo dnf upgrade uxf-control-center".to_string()
                        } else if let Some(ref url) = download_url {
                            let filename = url.split('/').last().unwrap_or("uxf-control-center.rpm");
                            format!("curl -LO \"{url}\" && sudo dnf install \"./{filename}\"")
                        } else {
                            default_update_command
                        };

                        return UpdateCheckResult {
                            current_version: current_version.to_string(),
                            latest_version: Some(latest_ver),
                            update_available: is_newer,
                            download_url,
                            release_notes: body,
                            update_command: update_cmd,
                            has_dnf_repo,
                            error: None,
                        };
                    }
                }
            }
            UpdateCheckResult {
                current_version: current_version.to_string(),
                latest_version: None,
                update_available: false,
                download_url: None,
                release_notes: None,
                update_command: default_update_command,
                has_dnf_repo,
                error: Some("Unable to parse latest release response from GitHub".into()),
            }
        }
        Ok(_) | Err(_) => UpdateCheckResult {
            current_version: current_version.to_string(),
            latest_version: None,
            update_available: false,
            download_url: None,
            release_notes: None,
            update_command: default_update_command,
            has_dnf_repo,
            error: Some("Network request to GitHub Releases timed out or failed".into()),
        },
    }
}
