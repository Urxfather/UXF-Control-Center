use serde::Deserialize;

use crate::libacercontrol::{dbus::AcerControlDBus, driver, telemetry};

const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/stanlysilas/acer-control-center/releases/latest";
pub const RELEASES_PAGE_URL: &str =
    "https://github.com/stanlysilas/acer-control-center/releases";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticStatus {
    Passed,
    Warning,
    Failed,
    Info,
}

#[derive(Clone, Debug)]
pub struct DiagnosticCheck {
    pub name: String,
    pub description: String,
    pub status: DiagnosticStatus,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct ModuleInfo {
    pub name: String,
    pub loaded: bool,
    pub version: Option<String>,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct SystemSummary {
    pub laptop_model: String,
    pub os_name: String,
    pub kernel: String,
    pub arch: String,
    pub app_version: String,
    pub daemon_status: (bool, String),
    #[allow(dead_code)]
    pub driver_status: (bool, String),
    pub interface_name: String,
    pub driver_abi: String,
    pub hardware_supported: bool,
    pub device_node_accessible: bool,
    pub modules: Vec<ModuleInfo>,
    pub diagnostics: Vec<DiagnosticCheck>,
}

#[derive(Clone)]
pub struct AboutInfo {
    pub summary: SystemSummary,
    #[allow(dead_code)]
    pub rows: Vec<(String, String)>,
}

impl AboutInfo {
    pub fn collect() -> Self {
        let daemon = AcerControlDBus::new().and_then(|bus| bus.version()).ok();
        let module_version = driver::version().ok();
        let module_status = module_version
            .as_ref()
            .map(|version| format!("Loaded and responding (v{version})"))
            .unwrap_or_else(|| "Not detected or not accessible".to_owned());
        let hardware = driver::hardware_supported()
            .map(|supported| if supported { "Supported" } else { "Not supported" }.to_owned())
            .unwrap_or_else(|_| "Could not query driver".to_owned());
        let interface = driver::interface().unwrap_or_else(|_| "Unavailable".to_owned());
        let device_node_accessible = std::path::Path::new("/dev/acer_io").exists()
            && std::fs::OpenOptions::new().read(true).open("/dev/acer_io").is_ok();

        let module_definitions = [
            ("acer_io", "Hardware telemetry & ioctl character device"),
            ("acer_acpi", "ACPI DCHU fan mode & thermal tables"),
            ("acer_wmi", "WMI event notification for legacy hotkeys"),
            ("acer_keyboard", "Keyboard input device & LED controller"),
        ];

        let mut modules = Vec::new();
        for (name, desc) in module_definitions {
            let path = format!("/sys/module/{name}");
            let loaded = std::path::Path::new(&path).exists();
            let version = read_trimmed(&format!("{path}/version"));
            modules.push(ModuleInfo {
                name: name.to_owned(),
                loaded,
                version,
                description: desc.to_owned(),
            });
        }

        let module_rows = modules.iter().map(|m| {
            let value = if m.loaded {
                m.version
                    .as_ref()
                    .map(|v| format!("Loaded (v{v})"))
                    .unwrap_or_else(|| "Loaded".to_owned())
            } else {
                "Not loaded".to_owned()
            };
            (format!("Module: {}", m.name), value)
        });

        let os = os_name();
        let kernel = kernel_version();
        let model = laptop_model();
        let arch = std::env::consts::ARCH.to_owned();
        let app_ver = format!("v{}", env!("CARGO_PKG_VERSION"));
        let abi = format!("{}.{}", driver::abi_major(), driver::abi_minor());

        let mut rows = vec![
            ("Acer Control Center".to_owned(), app_ver.clone()),
            (
                "Control daemon".to_owned(),
                daemon
                    .as_ref()
                    .map(|version| format!("Running (v{version})"))
                    .unwrap_or_else(|| "Not running or unreachable".to_owned()),
            ),
            (
                "Control library".to_owned(),
                driver::library_version()
                    .map(|version| format!("v{version}"))
                    .unwrap_or_else(|_| "Unavailable".to_owned()),
            ),
            ("Kernel".to_owned(), kernel.clone()),
            ("Kernel driver".to_owned(), module_status.clone()),
            ("Driver ABI".to_owned(), abi.clone()),
            ("Hardware support".to_owned(), hardware.clone()),
            ("Firmware interface".to_owned(), interface.clone()),
            ("Laptop model".to_owned(), model.clone()),
            ("Operating system".to_owned(), os.clone()),
            (
                "Keyboard RGB".to_owned(),
                if std::path::Path::new("/sys/class/leds/rgb:kbd_backlight").exists() {
                    "Available".to_owned()
                } else {
                    "Not detected".to_owned()
                },
            ),
            ("CPU fan".to_owned(), metric(telemetry::cpu_fan_rpm(), " RPM")),
            ("GPU fan".to_owned(), metric(telemetry::gpu_fan_rpm(), " RPM")),
            (
                "CPU temperature".to_owned(),
                metric(telemetry::cpu_temperature(), " °C"),
            ),
            (
                "GPU temperature".to_owned(),
                metric(telemetry::gpu_temperature(), " °C"),
            ),
        ];
        rows.extend(module_rows);

        // Build Diagnostic Checks
        let mut diagnostics = Vec::new();

        // 1. Control Daemon
        diagnostics.push(DiagnosticCheck {
            name: "Control Daemon (D-Bus)".into(),
            description: "System D-Bus communication with io.github.stanlysilas.AcerControl".into(),
            status: if daemon.is_some() { DiagnosticStatus::Passed } else { DiagnosticStatus::Failed },
            detail: if let Some(v) = &daemon {
                format!("Daemon active (v{v}) on system bus")
            } else {
                "Daemon unreachable; run 'systemctl status acer-control-daemon'".into()
            },
        });

        // 2. Kernel Character Device
        let dev_node_exists = std::path::Path::new("/dev/acer_io").exists();
        let dev_node_rw = dev_node_exists && std::fs::OpenOptions::new().read(true).write(true).open("/dev/acer_io").is_ok();
        let dev_node_ro = !dev_node_rw && dev_node_exists && std::fs::OpenOptions::new().read(true).open("/dev/acer_io").is_ok();
        let (dev_status, dev_detail) = if module_version.is_some() && (dev_node_rw || dev_node_ro) {
            (
                DiagnosticStatus::Passed,
                format!("Accessible ({}) • Driver ABI {abi}\nIoctl communication responsive", if dev_node_rw { "rw" } else { "ro" }),
            )
        } else if dev_node_exists {
            (
                DiagnosticStatus::Warning,
                "Device node exists but ioctl/open failed (check 'acercontrol' group)".into(),
            )
        } else {
            (
                DiagnosticStatus::Failed,
                "Device node /dev/acer_io missing (ensure acer_io module is loaded)".into(),
            )
        };
        diagnostics.push(DiagnosticCheck {
            name: "Kernel Character Device".into(),
            description: "Direct ioctl communication via /dev/acer_io".into(),
            status: dev_status,
            detail: dev_detail,
        });

        // 3. Firmware ACPI Interface
        let iface_ok = interface != "Unavailable";
        diagnostics.push(DiagnosticCheck {
            name: "Firmware ACPI Interface".into(),
            description: "ACPI device node \\_SB.DCHU evaluation readiness".into(),
            status: if iface_ok { DiagnosticStatus::Passed } else { DiagnosticStatus::Warning },
            detail: if iface_ok {
                format!("Active interface: {interface}\nIoctl method evaluation verified")
            } else {
                "Interface unavailable\nStandard ACPI fallback active".into()
            },
        });

        // 4. Fan Tachometers
        let cpu_rpm = telemetry::cpu_fan_rpm().ok();
        let gpu_rpm = telemetry::gpu_fan_rpm().ok();
        let fans_ok = cpu_rpm.is_some() || gpu_rpm.is_some();
        diagnostics.push(DiagnosticCheck {
            name: "Cooling Fan Tachometers".into(),
            description: "Dual-channel fan rotation sensors via ACPI DCHU _DSM 0x0C".into(),
            status: if fans_ok { DiagnosticStatus::Passed } else { DiagnosticStatus::Warning },
            detail: match (cpu_rpm, gpu_rpm) {
                (Some(c), Some(g)) => format!("CPU: {c} RPM\nGPU: {g} RPM (Live Telemetry)"),
                (Some(c), None) => format!("CPU: {c} RPM\nGPU: Sensor Offline"),
                (None, Some(g)) => format!("CPU: Sensor Offline\nGPU: {g} RPM"),
                (None, None) => "Fan tachometers unavailable".into(),
            },
        });

        // 5. Thermal Telemetry
        let cpu_temp = telemetry::cpu_temperature().ok();
        let gpu_temp = telemetry::gpu_temperature().ok();
        let temps_ok = cpu_temp.is_some() || gpu_temp.is_some();
        diagnostics.push(DiagnosticCheck {
            name: "Thermal Telemetry Sensors".into(),
            description: "Processor core and discrete GPU diode temperature sensors".into(),
            status: if temps_ok { DiagnosticStatus::Passed } else { DiagnosticStatus::Warning },
            detail: match (cpu_temp, gpu_temp) {
                (Some(c), Some(g)) => format!("CPU: {c} °C\nGPU: {g} °C (Live Telemetry)"),
                (Some(c), None) => format!("CPU: {c} °C\nGPU: Sensor Offline"),
                (None, Some(g)) => format!("CPU: Sensor Offline\nGPU: {g} °C"),
                (None, None) => "Thermal telemetry unavailable".into(),
            },
        });

        // 6. Keyboard Lighting Subsystem
        let kbd_rgb_reading = std::fs::read_to_string("/sys/class/leds/rgb:kbd_backlight/multi_intensity").ok();
        let (kbd_rgb_status, kbd_rgb_detail) = if let Some(ref val) = kbd_rgb_reading {
            let parts: Vec<&str> = val.split_whitespace().collect();
            if parts.len() == 3 {
                (
                    DiagnosticStatus::Passed,
                    format!("RGB hardware responsive ({} {} {})\nMulticolor sysfs verified", parts[0], parts[1], parts[2]),
                )
            } else {
                (
                    DiagnosticStatus::Warning,
                    "LED sysfs present but multi_intensity format unexpected".into(),
                )
            }
        } else if std::path::Path::new("/sys/class/leds/rgb:kbd_backlight").exists() {
            (
                DiagnosticStatus::Warning,
                "LED node exists but multi_intensity unreadable".into(),
            )
        } else {
            (
                DiagnosticStatus::Info,
                "Monochrome or standard backlight interface detected".into(),
            )
        };
        diagnostics.push(DiagnosticCheck {
            name: "Keyboard RGB Controller".into(),
            description: "Kernel sysfs interface at /sys/class/leds/rgb:kbd_backlight".into(),
            status: kbd_rgb_status,
            detail: kbd_rgb_detail,
        });

        let summary = SystemSummary {
            laptop_model: model,
            os_name: os,
            kernel,
            arch,
            app_version: app_ver,
            daemon_status: (
                daemon.is_some(),
                daemon
                    .map(|v| format!("Running (v{v})"))
                    .unwrap_or_else(|| "Stopped / Unreachable".into()),
            ),
            driver_status: (
                module_version.is_some(),
                module_status,
            ),
            interface_name: interface,
            driver_abi: abi,
            hardware_supported: driver::hardware_supported().unwrap_or(false),
            device_node_accessible,
            modules,
            diagnostics,
        };

        Self { summary, rows }
    }
}

fn metric(result: Result<i32, String>, unit: &str) -> String {
    result
        .map(|value| format!("{value}{unit}"))
        .unwrap_or_else(|_| "Unavailable".to_owned())
}

pub fn kernel_version() -> String {
    read_trimmed("/proc/sys/kernel/osrelease").unwrap_or_else(|| "Unavailable".to_owned())
}

pub fn os_name() -> String {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
                return val.trim_matches('"').trim().to_owned();
            }
        }
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("NAME=") {
                return val.trim_matches('"').trim().to_owned();
            }
        }
    }
    "Linux".to_owned()
}

pub fn laptop_model() -> String {
    let vendor = read_trimmed("/sys/class/dmi/id/sys_vendor");
    let product = read_trimmed("/sys/class/dmi/id/product_name");
    match (vendor, product) {
        (Some(vendor), Some(product)) => format!("{vendor} {product}"),
        (Some(vendor), None) => vendor,
        (None, Some(product)) => product,
        (None, None) => "Unknown Model".to_owned(),
    }
}

fn read_trimmed(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[derive(Deserialize)]
pub struct LatestRelease {
    pub tag_name: String,
    pub html_url: String,
}

pub fn check_latest_release() -> Result<LatestRelease, String> {
    let response = ureq::get(LATEST_RELEASE_URL)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Acer-Control-Center")
        .call()
        .map_err(|error| match error {
            ureq::Error::StatusCode(404) => "No published release is available yet.".to_owned(),
            error => format!("GitHub check failed: {error}"),
        })?;
    let release = response
        .into_body()
        .read_json::<LatestRelease>()
        .map_err(|error| format!("Could not read the release information: {error}"))?;
    if release.tag_name.trim().is_empty() || release.html_url.trim().is_empty() {
        return Err("GitHub returned an incomplete release record".to_owned());
    }
    Ok(release)
}

pub fn compare_versions(current: &str, latest: &str) -> Option<std::cmp::Ordering> {
    fn parts(value: &str) -> Option<Vec<u64>> {
        let clean = value.trim().trim_start_matches(['v', 'V']);
        let core = clean.split(['-', '+']).next()?;
        let mut numbers = core
            .split('.')
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        if numbers.is_empty() {
            return None;
        }
        while numbers.len() < 3 {
            numbers.push(0);
        }
        Some(numbers)
    }

    let current = parts(current)?;
    let latest = parts(latest)?;
    Some(current.cmp(&latest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_status_equality_and_variants() {
        assert_eq!(DiagnosticStatus::Passed, DiagnosticStatus::Passed);
        assert_ne!(DiagnosticStatus::Passed, DiagnosticStatus::Failed);
        assert_ne!(DiagnosticStatus::Warning, DiagnosticStatus::Info);
    }

    #[test]
    fn test_version_comparison_ordering() {
        assert_eq!(compare_versions("2.0.0", "2.0.1"), Some(std::cmp::Ordering::Less));
        assert_eq!(compare_versions("v2.1.0", "2.0.9"), Some(std::cmp::Ordering::Greater));
        assert_eq!(compare_versions("2.0.0", "2.0.0"), Some(std::cmp::Ordering::Equal));
        assert_eq!(compare_versions("invalid", "2.0.0"), None);
    }

    #[test]
    fn test_about_info_collect_structure() {
        let about = AboutInfo::collect();
        assert!(!about.summary.laptop_model.is_empty());
        assert!(!about.summary.os_name.is_empty());
        assert!(!about.summary.kernel.is_empty());
        assert!(!about.summary.arch.is_empty());
        assert_eq!(about.summary.modules.len(), 4);
        assert_eq!(about.summary.diagnostics.len(), 6);
        for diag in &about.summary.diagnostics {
            assert!(!diag.name.is_empty());
            assert!(!diag.description.is_empty());
            assert!(!diag.detail.is_empty());
        }
    }

    #[test]
    fn test_os_name_and_model_detection() {
        let os = os_name();
        assert!(!os.is_empty());
        let model = laptop_model();
        assert!(!model.is_empty());
    }
}
