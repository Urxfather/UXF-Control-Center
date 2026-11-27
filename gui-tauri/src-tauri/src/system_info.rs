use serde::Serialize;
use std::collections::HashSet;
use std::ffi::CStr;
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::telemetry::SystemIdentity;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareBiosInfo {
    pub manufacturer: String,
    pub product_model: String,
    pub board_name: String,
    pub bios_vendor: String,
    pub bios_version: String,
    pub bios_date: String,
    pub boot_mode: String,
    pub secure_boot: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsKernelInfo {
    pub distribution: String,
    pub kernel_release: String,
    pub architecture: String,
    pub display_server: String,
    pub hostname: String,
    pub uptime_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessorGraphicsInfo {
    pub cpu_model: String,
    pub physical_cores: Option<u32>,
    pub logical_threads: Option<u32>,
    pub max_clock_ghz: Option<f32>,
    pub discrete_gpu: String,
    pub discrete_gpu_driver: Option<String>,
    pub integrated_gpu: String,
    pub integrated_gpu_driver: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStorageInfo {
    pub ram_total_mb: u64,
    pub ram_total_display: String,
    pub swap_display: String,
    pub primary_drive: String,
    pub root_filesystem: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInformationReport {
    pub hardware: HardwareBiosInfo,
    pub os_kernel: OsKernelInfo,
    pub processor_graphics: ProcessorGraphicsInfo,
    pub memory_storage: MemoryStorageInfo,
    pub collected_at_ms: u64,
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("Not Applicable")
        || trimmed.eq_ignore_ascii_case("To Be Filled By O.E.M.")
        || trimmed.eq_ignore_ascii_case("Default string")
        || trimmed.eq_ignore_ascii_case("None")
    {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Runs a subprocess with a strict wall-clock timeout so no slow or hung CLI
/// can block diagnostics or freeze workers.
pub(crate) fn run_command_with_timeout(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> Option<Output> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    }
}

/// Fast zero-subprocess uname wrapper returning `(release, machine)`.
pub(crate) fn kernel_uname_info() -> (String, String) {
    let mut uts: libc::utsname = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::uname(&mut uts) };
    if rc != 0 {
        return ("unknown".into(), "unknown".into());
    }
    let release = unsafe { CStr::from_ptr(uts.release.as_ptr()) }
        .to_string_lossy()
        .trim()
        .to_string();
    let machine = unsafe { CStr::from_ptr(uts.machine.as_ptr()) }
        .to_string_lossy()
        .trim()
        .to_string();
    (release, machine)
}

/// Reads UEFI SecureBoot efivar directly (0.05ms, no subprocess required).
/// Returns `Some(true)` if enabled, `Some(false)` if disabled, `None` if unavailable.
pub(crate) fn probe_secure_boot_efi() -> Option<bool> {
    let entries = fs::read_dir("/sys/firmware/efi/efivars").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("SecureBoot-") {
            if let Ok(bytes) = fs::read(entry.path()) {
                // EFI variable payload is 4-byte attributes followed by 1-byte value
                if bytes.len() >= 5 {
                    return Some(bytes[4] == 1);
                }
            }
        }
    }
    None
}

pub(crate) fn read_uptime_seconds() -> Option<u64> {
    let text = fs::read_to_string("/proc/uptime").ok()?;
    let first = text.split_whitespace().next()?;
    let secs: f64 = first.parse().ok()?;
    Some(secs.max(0.0) as u64)
}

fn read_os_distribution(arch: &str) -> String {
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        let mut pretty = None;
        let mut name = None;
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("PRETTY_NAME=") {
                pretty = Some(rest.trim_matches('"').to_string());
            } else if let Some(rest) = trimmed.strip_prefix("NAME=") {
                name = Some(rest.trim_matches('"').to_string());
            }
        }
        if let Some(dist) = pretty.or(name) {
            let concise = dist.replace(" (Workstation Edition)", " Workstation");
            if !arch.is_empty() && arch != "unknown" && !concise.contains(arch) {
                return format!("{concise} · {arch}");
            }
            return concise;
        }
    }
    format!("Linux ({arch})")
}

fn detect_display_server() -> String {
    let session_type = std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let proto = if session_type == "wayland" || std::env::var_os("WAYLAND_DISPLAY").is_some() {
        "Wayland"
    } else if session_type == "x11" || std::env::var_os("DISPLAY").is_some() {
        "X11"
    } else {
        "Headless / Unknown"
    };

    if desktop.to_ascii_uppercase().contains("GNOME") {
        if let Some(out) = run_command_with_timeout(
            "gnome-shell",
            &["--version"],
            Duration::from_millis(800),
        ) {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                // e.g. "GNOME Shell 51.1"
                if let Some(ver) = text.strip_prefix("GNOME Shell ") {
                    return format!("{proto} (GNOME {})", ver.trim());
                }
            }
        }
        return format!("{proto} (GNOME)");
    }

    if !desktop.trim().is_empty() {
        format!("{proto} ({})", desktop.trim())
    } else {
        proto.to_string()
    }
}

fn detect_cpu_topology() -> (Option<u32>, Option<u32>, Option<f32>) {
    let mut logical_threads: u32 = 0;
    let mut core_pairs: HashSet<(String, String)> = HashSet::new();

    if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
        let mut phys_id = String::from("0");
        let mut core_id: Option<String> = None;

        for line in cpuinfo.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if let Some(cid) = core_id.take() {
                    core_pairs.insert((phys_id.clone(), cid));
                }
                phys_id = String::from("0");
                continue;
            }
            if trimmed.starts_with("processor") && trimmed.contains(':') {
                logical_threads += 1;
            } else if let Some(rest) = trimmed.strip_prefix("physical id") {
                if let Some(v) = rest.split(':').nth(1) {
                    phys_id = v.trim().to_string();
                }
            } else if let Some(rest) = trimmed.strip_prefix("core id") {
                if let Some(v) = rest.split(':').nth(1) {
                    core_id = Some(v.trim().to_string());
                }
            }
        }
        if let Some(cid) = core_id {
            core_pairs.insert((phys_id, cid));
        }
    }

    let physical_cores = if !core_pairs.is_empty() {
        Some(core_pairs.len() as u32)
    } else {
        None
    };

    let logical = if logical_threads > 0 {
        Some(logical_threads)
    } else {
        None
    };

    let max_ghz = read_trimmed("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
        .and_then(|s| s.parse::<u64>().ok())
        .map(|khz| (khz as f32) / 1_000_000.0);

    (physical_cores, logical, max_ghz)
}

fn detect_integrated_gpu_and_drivers() -> (String, Option<String>, Option<String>) {
    let mut igpu_name: Option<String> = None;
    let mut igpu_driver: Option<String> = None;
    let mut dgpu_driver: Option<String> = read_trimmed("/sys/module/nvidia/version")
        .map(|v| format!("nvidia {v}"));

    if let Ok(entries) = fs::read_dir("/sys/bus/pci/devices") {
        for entry in entries.flatten() {
            let dir = entry.path();
            let class = read_trimmed(dir.join("class")).unwrap_or_default();
            if !class.starts_with("0x03") {
                continue;
            }
            let vendor = read_trimmed(dir.join("vendor"))
                .unwrap_or_default()
                .to_ascii_lowercase();
            let driver = fs::read_link(dir.join("driver"))
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));

            if vendor == "0x8086" {
                igpu_driver = driver;
            } else if vendor == "0x10de" && dgpu_driver.is_none() {
                dgpu_driver = driver;
            } else if vendor == "0x1002" && igpu_driver.is_none() {
                igpu_driver = driver;
            }
        }
    }

    if let Some(out) = run_command_with_timeout("lspci", &[], Duration::from_millis(800)) {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let is_display = line.contains("VGA compatible controller")
                    || line.contains("Display controller");
                if is_display && line.contains("Intel") {
                    if let Some(raw) = line.split(':').nth(2) {
                        let clean = raw
                            .split("(rev")
                            .next()
                            .unwrap_or(raw)
                            .trim()
                            .to_string();
                        if !clean.is_empty() {
                            igpu_name = Some(clean);
                            break;
                        }
                    }
                }
            }
            if igpu_name.is_none() {
                for line in text.lines() {
                    let is_display = line.contains("VGA compatible controller")
                        || line.contains("Display controller");
                    if is_display && line.contains("AMD") && line.contains("Radeon Graphics") {
                        if let Some(raw) = line.split(':').nth(2) {
                            let clean = raw
                                .split("(rev")
                                .next()
                                .unwrap_or(raw)
                                .trim()
                                .to_string();
                            if !clean.is_empty() {
                                igpu_name = Some(clean);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let igpu = match (igpu_name, igpu_driver.as_deref()) {
        (Some(name), Some(drv)) if name.contains('[') => {
            if let (Some(start), Some(end)) = (name.find('['), name.find(']')) {
                if end > start + 1 {
                    format!("{} ({drv})", &name[start + 1..end])
                } else {
                    format!("{name} ({drv})")
                }
            } else {
                format!("{name} ({drv})")
            }
        }
        (Some(name), Some(drv)) => format!("{name} ({drv})"),
        (Some(name), None) => name,
        (None, Some(drv)) => format!("Integrated Graphics ({drv})"),
        (None, None) => "Unavailable".into(),
    };

    (igpu, igpu_driver, dgpu_driver)
}

fn detect_ram_summary() -> (u64, String) {
    let mut total_kb = 0u64;
    if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                break;
            }
        }
    }
    let ram_total_mb = total_kb / 1024;
    let display = if ram_total_mb > 0 {
        let gib = (ram_total_mb as f64) / 1024.0;
        format!("{gib:.1} GB ({ram_total_mb} MB)")
    } else {
        "Unavailable".into()
    };
    (ram_total_mb, display)
}

fn detect_swap_summary() -> String {
    let Ok(text) = fs::read_to_string("/proc/swaps") else {
        return "Unavailable".into();
    };
    let mut parts = Vec::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 4 {
            let dev_name = Path::new(cols[0])
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| cols[0].to_string());
            let size_kb: u64 = cols[2].parse().unwrap_or(0);
            let used_kb: u64 = cols[3].parse().unwrap_or(0);
            let size_gb = (size_kb as f64) / (1024.0 * 1024.0);
            let used_gb = (used_kb as f64) / (1024.0 * 1024.0);
            parts.push(format!("{size_gb:.1} GB {dev_name} ({used_gb:.1} GB used)"));
        }
    }
    if parts.is_empty() {
        "None configured".into()
    } else {
        parts.join(", ")
    }
}

fn detect_storage_summary() -> (String, String) {
    let mut root_dev: Option<String> = None;
    let mut root_fs: Option<String> = None;

    if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
        for line in mounts.lines() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 3 && cols[1] == "/" {
                root_dev = Some(cols[0].to_string());
                root_fs = Some(cols[2].to_string());
                break;
            }
        }
    }

    let (disk_used_gb, disk_total_gb) = unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        let root_c = b"/\0";
        if libc::statvfs(root_c.as_ptr() as *const libc::c_char, &mut stat) == 0 {
            let block_size = stat.f_frsize as u64;
            let total_bytes = (stat.f_blocks as u64).saturating_mul(block_size);
            let avail_bytes = (stat.f_bavail as u64).saturating_mul(block_size);
            let used_bytes = total_bytes.saturating_sub(avail_bytes);
            (
                used_bytes / (1024 * 1024 * 1024),
                total_bytes / (1024 * 1024 * 1024),
            )
        } else {
            (0, 0)
        }
    };

    let root_fs_str = match (root_fs, disk_total_gb > 0) {
        (Some(fs_name), true) => format!("{fs_name} ({disk_used_gb} / {disk_total_gb} GB used)"),
        (Some(fs_name), false) => fs_name,
        (None, true) => format!("{disk_used_gb} / {disk_total_gb} GB used"),
        (None, false) => "Unavailable".into(),
    };

    let mut primary_drive = String::from("Unavailable");
    if let Some(dev_path) = root_dev.as_deref() {
        let dev_base = Path::new(dev_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Map partition name (e.g. nvme0n1p3 -> nvme0n1, sda2 -> sda)
        let block_dev = if dev_base.starts_with("nvme") {
            if let Some(idx) = dev_base.rfind('p') {
                if dev_base[idx + 1..].chars().all(|c| c.is_ascii_digit()) {
                    dev_base[..idx].to_string()
                } else {
                    dev_base.clone()
                }
            } else {
                dev_base.clone()
            }
        } else {
            dev_base.trim_end_matches(|c: char| c.is_ascii_digit()).to_string()
        };

        let sys_block = Path::new("/sys/block").join(&block_dev);
        if sys_block.exists() {
            let model = read_trimmed(sys_block.join("device/model"));
            let sectors = read_trimmed(sys_block.join("size"))
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            let size_gb = (sectors as f64 * 512.0) / 1_000_000_000.0;
            primary_drive = match (model, size_gb > 0.1) {
                (Some(m), true) => format!("{m} {size_gb:.1} GB (/dev/{block_dev})"),
                (Some(m), false) => format!("{m} (/dev/{block_dev})"),
                (None, true) => format!("/dev/{block_dev} ({size_gb:.1} GB)"),
                (None, false) => format!("/dev/{block_dev}"),
            };
        } else if !dev_path.is_empty() {
            primary_drive = dev_path.to_string();
        }
    }

    (primary_drive, root_fs_str)
}

pub fn collect_system_information(identity: &SystemIdentity) -> SystemInformationReport {
    let manufacturer =
        read_trimmed("/sys/class/dmi/id/sys_vendor").unwrap_or_else(|| "Unavailable".into());
    let product_name =
        read_trimmed("/sys/class/dmi/id/product_name").unwrap_or_else(|| "Unavailable".into());
    let board_name =
        read_trimmed("/sys/class/dmi/id/board_name").unwrap_or_else(|| product_name.clone());

    let product_model = if board_name != product_name
        && !board_name.is_empty()
        && board_name != "Unavailable"
    {
        format!("{product_name} ({board_name})")
    } else {
        product_name
    };

    let bios_vendor =
        read_trimmed("/sys/class/dmi/id/bios_vendor").unwrap_or_else(|| "Unavailable".into());
    let raw_bios_ver = read_trimmed("/sys/class/dmi/id/bios_version");
    let bios_date = read_trimmed("/sys/class/dmi/id/bios_date").unwrap_or_default();
    let bios_version = match (raw_bios_ver, bios_vendor.as_str()) {
        (Some(ver), "Unavailable") => ver,
        (Some(ver), vendor) => format!("{vendor} {ver}"),
        (None, _) => "Unavailable".into(),
    };

    let efi_exists = Path::new("/sys/firmware/efi").exists();
    let boot_mode = if efi_exists {
        "UEFI".to_string()
    } else {
        "Legacy BIOS".to_string()
    };

    let secure_boot = match probe_secure_boot_efi() {
        Some(true) => "Enabled".into(),
        Some(false) => "Disabled".into(),
        None if !efi_exists => "Not Applicable (Legacy)".into(),
        None => "Unavailable".into(),
    };

    let (kernel_release, architecture) = kernel_uname_info();
    let distribution = read_os_distribution(&architecture);
    let hostname =
        read_trimmed("/proc/sys/kernel/hostname").unwrap_or_else(|| "localhost".into());
    let display_server = detect_display_server();
    let uptime_seconds = read_uptime_seconds();

    let (physical_cores, logical_threads, max_clock_ghz) = detect_cpu_topology();
    let (integrated_gpu, integrated_gpu_driver, discrete_gpu_driver) =
        detect_integrated_gpu_and_drivers();
    let (ram_total_mb, ram_total_display) = detect_ram_summary();
    let swap_display = detect_swap_summary();
    let (primary_drive, root_filesystem) = detect_storage_summary();

    SystemInformationReport {
        hardware: HardwareBiosInfo {
            manufacturer,
            product_model,
            board_name,
            bios_vendor,
            bios_version,
            bios_date,
            boot_mode,
            secure_boot,
        },
        os_kernel: OsKernelInfo {
            distribution,
            kernel_release,
            architecture,
            display_server,
            hostname,
            uptime_seconds,
        },
        processor_graphics: ProcessorGraphicsInfo {
            cpu_model: identity.cpu_model.clone(),
            physical_cores,
            logical_threads,
            max_clock_ghz,
            discrete_gpu: identity.gpu_model.clone(),
            discrete_gpu_driver,
            integrated_gpu,
            integrated_gpu_driver,
        },
        memory_storage: MemoryStorageInfo {
            ram_total_mb,
            ram_total_display,
            swap_display,
            primary_drive,
            root_filesystem,
        },
        collected_at_ms: now_ms(),
    }
}
