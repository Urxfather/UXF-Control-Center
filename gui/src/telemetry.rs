//! Unified In-Process Hardware Telemetry Collector for Acer Control Center
//!
//! Metrics Source and Units Documentation:
//! - CPU Frequency (MHz): Read from `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq`.
//!   Kernel reports in kHz; converted to instantaneous MHz.
//! - CPU Utilization (%): Calculated from `/proc/stat` counter deltas between 1-second polling intervals.
//! - CPU Temperature (°C): Read from Clevo DCHU EC via `libacercontrol::telemetry::cpu_temperature()`.
//! - GPU Telemetry: Read via in-process NVML (`libnvidia-ml.so.1`) ONLY when PCI device is active.
//!   If `/sys/bus/pci/devices/0000:01:00.0/power/runtime_status` is `suspended`, the GPU is reported as
//!   `Sleeping` to prevent unnecessary dGPU wake-ups.
//! - RAM (MB / %): Calculated from `/proc/meminfo` (`MemTotal`, `MemAvailable`).
//! - Storage (GB / %): Calculated via POSIX `statvfs` on `/home` (or `/`).
//! - Fan Speeds (RPM): Read from Clevo DCHU ACPI tachometer buffers via `libacercontrol`.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::libacercontrol::telemetry as libtelemetry;

#[derive(Debug, Clone, PartialEq)]
pub enum GpuTelemetry {
    Active {
        core_clock_mhz: u32,
        temp_c: u32,
        utilization_pct: u32,
        vram_used_mb: u64,
        vram_total_mb: u64,
    },
    Sleeping,
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuTelemetry {
    /// Instantaneous average core frequency across all logical CPUs (MHz)
    pub avg_freq_mhz: u32,
    /// Instantaneous maximum core frequency among all logical CPUs (MHz)
    pub max_freq_mhz: u32,
    /// Overall CPU load percentage (0.0 to 100.0)
    pub utilization_pct: f32,
    /// CPU Temperature in degrees Celsius
    pub temp_c: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemMemory {
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub ram_percent: f32,
    pub disk_used_gb: u64,
    pub disk_total_gb: u64,
    pub disk_percent: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FanTelemetry {
    pub cpu_fan_rpm: Option<i32>,
    pub gpu_fan_rpm: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TelemetrySnapshot {
    pub cpu: CpuTelemetry,
    pub gpu: GpuTelemetry,
    pub memory: SystemMemory,
    pub fans: FanTelemetry,
    pub last_updated: Instant,
}

impl Default for TelemetrySnapshot {
    fn default() -> Self {
        Self {
            cpu: CpuTelemetry {
                avg_freq_mhz: 0,
                max_freq_mhz: 0,
                utilization_pct: 0.0,
                temp_c: None,
            },
            gpu: GpuTelemetry::Unavailable("Initializing...".into()),
            memory: SystemMemory {
                ram_used_mb: 0,
                ram_total_mb: 0,
                ram_percent: 0.0,
                disk_used_gb: 0,
                disk_total_gb: 0,
                disk_percent: 0.0,
            },
            fans: FanTelemetry {
                cpu_fan_rpm: None,
                gpu_fan_rpm: None,
            },
            last_updated: Instant::now(),
        }
    }
}

// Low-level NVML C signatures for zero-fork in-process telemetry
type NvmlReturn = libc::c_int;
type NvmlDevice = *mut libc::c_void;

#[repr(C)]
struct NvmlUtilization {
    gpu: libc::c_uint,
    memory: libc::c_uint,
}

#[repr(C)]
struct NvmlMemory {
    total: libc::c_ulonglong,
    free: libc::c_ulonglong,
    used: libc::c_ulonglong,
}

struct NvmlLibrary {
    lib_handle: *mut libc::c_void,
    get_handle_fn: unsafe extern "C" fn(libc::c_uint, *mut NvmlDevice) -> NvmlReturn,
    get_clock_fn: unsafe extern "C" fn(NvmlDevice, libc::c_uint, *mut libc::c_uint) -> NvmlReturn,
    get_temp_fn: unsafe extern "C" fn(NvmlDevice, libc::c_uint, *mut libc::c_uint) -> NvmlReturn,
    get_util_fn: unsafe extern "C" fn(NvmlDevice, *mut NvmlUtilization) -> NvmlReturn,
    get_mem_fn: unsafe extern "C" fn(NvmlDevice, *mut NvmlMemory) -> NvmlReturn,
    shutdown_fn: unsafe extern "C" fn() -> NvmlReturn,
    initialized: bool,
}

impl NvmlLibrary {
    fn load() -> Result<Self, String> {
        let soname = b"libnvidia-ml.so.1\0";
        let lib_handle = unsafe { libc::dlopen(soname.as_ptr() as *const libc::c_char, libc::RTLD_LAZY) };
        if lib_handle.is_null() {
            return Err("libnvidia-ml.so.1 could not be loaded".into());
        }

        macro_rules! sym {
            ($name:expr) => {{
                let s = unsafe { libc::dlsym(lib_handle, $name.as_ptr() as *const libc::c_char) };
                if s.is_null() {
                    unsafe { libc::dlclose(lib_handle) };
                    return Err(format!("Symbol {} not found in NVML", String::from_utf8_lossy($name)));
                }
                unsafe { std::mem::transmute(s) }
            }};
        }

        let init_fn: unsafe extern "C" fn() -> NvmlReturn = sym!(b"nvmlInit_v2\0");
        let get_handle_fn = sym!(b"nvmlDeviceGetHandleByIndex_v2\0");
        let get_clock_fn = sym!(b"nvmlDeviceGetClockInfo\0");
        let get_temp_fn = sym!(b"nvmlDeviceGetTemperature\0");
        let get_util_fn = sym!(b"nvmlDeviceGetUtilizationRates\0");
        let get_mem_fn = sym!(b"nvmlDeviceGetMemoryInfo\0");
        let shutdown_fn = sym!(b"nvmlShutdown\0");

        let ret = unsafe { (init_fn)() };
        if ret != 0 {
            unsafe { libc::dlclose(lib_handle) };
            return Err(format!("nvmlInit_v2 failed with error code {}", ret));
        }

        Ok(Self {
            lib_handle,
            get_handle_fn,
            get_clock_fn,
            get_temp_fn,
            get_util_fn,
            get_mem_fn,
            shutdown_fn,
            initialized: true,
        })
    }

    fn query(&self) -> GpuTelemetry {
        if !self.initialized {
            return GpuTelemetry::Unavailable("NVML not initialized".into());
        }

        let mut device: NvmlDevice = std::ptr::null_mut();
        let ret = unsafe { (self.get_handle_fn)(0, &mut device) };
        if ret != 0 || device.is_null() {
            return GpuTelemetry::Unavailable(format!("Failed to acquire device handle (code {})", ret));
        }

        let mut clock_mhz: libc::c_uint = 0;
        let _ = unsafe { (self.get_clock_fn)(device, 0, &mut clock_mhz) };

        let mut temp_c: libc::c_uint = 0;
        let _ = unsafe { (self.get_temp_fn)(device, 0, &mut temp_c) };

        let mut util = NvmlUtilization { gpu: 0, memory: 0 };
        let _ = unsafe { (self.get_util_fn)(device, &mut util) };

        let mut mem = NvmlMemory { total: 0, free: 0, used: 0 };
        let _ = unsafe { (self.get_mem_fn)(device, &mut mem) };

        GpuTelemetry::Active {
            core_clock_mhz: clock_mhz as u32,
            temp_c: temp_c as u32,
            utilization_pct: util.gpu as u32,
            vram_used_mb: (mem.used / (1024 * 1024)) as u64,
            vram_total_mb: (mem.total / (1024 * 1024)) as u64,
        }
    }
}

impl Drop for NvmlLibrary {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                (self.shutdown_fn)();
                libc::dlclose(self.lib_handle);
            }
            self.initialized = false;
        }
    }
}

unsafe impl Send for NvmlLibrary {}
unsafe impl Sync for NvmlLibrary {}

pub struct TelemetryCollector {
    snapshot: Arc<RwLock<TelemetrySnapshot>>,
    running: Arc<AtomicBool>,
    worker_handle: Option<JoinHandle<()>>,
}

impl TelemetryCollector {
    pub fn new() -> Self {
        let snapshot = Arc::new(RwLock::new(TelemetrySnapshot::default()));
        let running = Arc::new(AtomicBool::new(true));

        let snapshot_clone = Arc::clone(&snapshot);
        let running_clone = Arc::clone(&running);

        let worker_handle = thread::Builder::new()
            .name("acer-telemetry".into())
            .spawn(move || {
                let nvml = NvmlLibrary::load().ok();
                let mut prev_idle = 0u64;
                let mut prev_total = 0u64;

                while running_clone.load(Ordering::Relaxed) {
                    let (utilization_pct, cur_idle, cur_total) = Self::sample_cpu_load(prev_idle, prev_total);
                    prev_idle = cur_idle;
                    prev_total = cur_total;

                    let (avg_freq_mhz, max_freq_mhz) = Self::sample_cpu_frequencies();
                    let cpu_temp = libtelemetry::cpu_temperature().ok();
                    let gpu_temp = libtelemetry::gpu_temperature().ok();
                    let cpu_rpm = libtelemetry::cpu_fan_rpm().ok();
                    let gpu_rpm = libtelemetry::gpu_fan_rpm().ok();

                    let gpu_telemetry = Self::sample_gpu_telemetry(&nvml, gpu_temp);
                    let memory = Self::sample_memory();

                    let new_snapshot = TelemetrySnapshot {
                        cpu: CpuTelemetry {
                            avg_freq_mhz,
                            max_freq_mhz,
                            utilization_pct,
                            temp_c: cpu_temp,
                        },
                        gpu: gpu_telemetry,
                        memory,
                        fans: FanTelemetry {
                            cpu_fan_rpm: cpu_rpm,
                            gpu_fan_rpm: gpu_rpm,
                        },
                        last_updated: Instant::now(),
                    };

                    if let Ok(mut lock) = snapshot_clone.write() {
                        *lock = new_snapshot;
                    }

                    thread::sleep(Duration::from_millis(1000));
                }
            })
            .ok();

        Self {
            snapshot,
            running,
            worker_handle,
        }
    }

    pub fn snapshot(&self) -> TelemetrySnapshot {
        self.snapshot.read().map(|s| s.clone()).unwrap_or_default()
    }

    fn sample_gpu_telemetry(nvml: &Option<NvmlLibrary>, fallback_temp: Option<i32>) -> GpuTelemetry {
        // Find NVIDIA PCI device runtime_status to avoid waking the GPU when sleeping
        let pci_path = Path::new("/sys/bus/pci/devices/0000:01:00.0/power/runtime_status");
        if pci_path.exists() {
            if let Ok(status) = fs::read_to_string(pci_path) {
                if status.trim() == "suspended" {
                    return GpuTelemetry::Sleeping;
                }
            }
        }

        if let Some(nvml_lib) = nvml {
            nvml_lib.query()
        } else if let Some(temp) = fallback_temp {
            GpuTelemetry::Active {
                core_clock_mhz: 0,
                temp_c: temp as u32,
                utilization_pct: 0,
                vram_used_mb: 0,
                vram_total_mb: 0,
            }
        } else {
            GpuTelemetry::Unavailable("NVIDIA NVML unavailable".into())
        }
    }

    fn sample_cpu_frequencies() -> (u32, u32) {
        let mut freqs_khz = Vec::new();

        // Query sysfs instantaneous frequency per core
        if let Ok(entries) = fs::read_dir("/sys/devices/system/cpu") {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if name.starts_with("cpu") && name[3..].chars().all(|c| c.is_ascii_digit()) {
                    let freq_file = entry.path().join("cpufreq/scaling_cur_freq");
                    if let Ok(val) = fs::read_to_string(&freq_file) {
                        if let Ok(khz) = val.trim().parse::<u32>() {
                            freqs_khz.push(khz);
                        }
                    }
                }
            }
        }

        if freqs_khz.is_empty() {
            return (0, 0);
        }

        let max_mhz = freqs_khz.iter().copied().max().unwrap_or(0) / 1000;
        let avg_mhz = (freqs_khz.iter().map(|&k| k as u64).sum::<u64>() / (freqs_khz.len() as u64 * 1000)) as u32;

        (avg_mhz, max_mhz)
    }

    fn sample_cpu_load(prev_idle: u64, prev_total: u64) -> (f32, u64, u64) {
        let stat_content = match fs::read_to_string("/proc/stat") {
            Ok(c) => c,
            Err(_) => return (0.0, prev_idle, prev_total),
        };

        let first_line = match stat_content.lines().next() {
            Some(l) if l.starts_with("cpu ") => l,
            _ => return (0.0, prev_idle, prev_total),
        };

        let values: Vec<u64> = first_line
            .split_whitespace()
            .skip(1)
            .filter_map(|s| s.parse::<u64>().ok())
            .collect();

        if values.len() < 4 {
            return (0.0, prev_idle, prev_total);
        }

        let idle = values[3] + values.get(4).copied().unwrap_or(0); // idle + iowait
        let total: u64 = values.iter().sum();

        if prev_total == 0 || total <= prev_total {
            return (0.0, idle, total);
        }

        let total_delta = (total - prev_total) as f32;
        let idle_delta = (idle - prev_idle) as f32;

        let utilization = ((total_delta - idle_delta) / total_delta * 100.0).clamp(0.0, 100.0);
        (utilization, idle, total)
    }

    fn sample_memory() -> SystemMemory {
        let mut total_kb = 0u64;
        let mut avail_kb = 0u64;

        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    total_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                } else if line.starts_with("MemAvailable:") {
                    avail_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                }
            }
        }

        let ram_total_mb = total_kb / 1024;
        let ram_used_mb = (total_kb.saturating_sub(avail_kb)) / 1024;
        let ram_percent = if ram_total_mb > 0 {
            (ram_used_mb as f32 / ram_total_mb as f32) * 100.0
        } else {
            0.0
        };

        // Sample root partition storage using statvfs
        let mut disk_used_gb = 0u64;
        let mut disk_total_gb = 0u64;
        let mut disk_percent = 0.0f32;

        unsafe {
            let mut stat: libc::statvfs = std::mem::zeroed();
            let path = b"/\0";
            if libc::statvfs(path.as_ptr() as *const libc::c_char, &mut stat) == 0 {
                let block_size = stat.f_frsize as u64;
                let total_bytes = stat.f_blocks as u64 * block_size;
                let free_bytes = stat.f_bavail as u64 * block_size;
                let used_bytes = total_bytes.saturating_sub(free_bytes);

                disk_total_gb = total_bytes / (1024 * 1024 * 1024);
                disk_used_gb = used_bytes / (1024 * 1024 * 1024);
                if disk_total_gb > 0 {
                    disk_percent = (disk_used_gb as f32 / disk_total_gb as f32) * 100.0;
                }
            }
        }

        SystemMemory {
            ram_used_mb,
            ram_total_mb,
            ram_percent,
            disk_used_gb,
            disk_total_gb,
            disk_percent,
        }
    }
}

impl Drop for TelemetryCollector {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.worker_handle.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_collector_samples_cleanly() {
        let collector = TelemetryCollector::new();
        // Allow the background thread to sample once
        thread::sleep(Duration::from_millis(1200));
        let snapshot = collector.snapshot();

        println!("Sampled Snapshot: {:?}", snapshot);
        assert!(snapshot.memory.ram_total_mb > 0, "RAM total should be non-zero");
        assert!(snapshot.memory.disk_total_gb > 0, "Disk total should be non-zero");
        assert!(matches!(snapshot.gpu, GpuTelemetry::Active { .. } | GpuTelemetry::Sleeping | GpuTelemetry::Unavailable(_)));
        // Dropping collector will cleanly signal running=false and join thread
        drop(collector);
    }
}
