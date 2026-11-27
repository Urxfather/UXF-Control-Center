use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::acercontrol::AcerControlLib;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemIdentity {
    pub cpu_model: String,
    pub gpu_model: String,
    pub cpu_max_ghz: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuTelemetry {
    pub avg_freq_mhz: u32,
    pub max_freq_mhz: u32,
    pub utilization_pct: f32,
    pub temp_c: Option<i32>,
    pub temp_source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum GpuTelemetry {
    #[serde(rename_all = "camelCase")]
    Active {
        core_clock_mhz: u32,
        max_clock_mhz: u32,
        temp_c: u32,
        utilization_pct: u32,
        vram_used_mb: u64,
        vram_total_mb: u64,
    },
    #[serde(rename_all = "camelCase")]
    Partial {
        temp_c: i32,
        reason: String,
    },
    Sleeping,
    #[serde(rename_all = "camelCase")]
    Unavailable { reason: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMemory {
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub ram_percent: f32,
    pub disk_used_gb: u64,
    pub disk_total_gb: u64,
    pub disk_percent: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FanTelemetry {
    pub cpu_fan_rpm: Option<i32>,
    pub gpu_fan_rpm: Option<i32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySnapshot {
    pub cpu: CpuTelemetry,
    pub gpu: GpuTelemetry,
    pub memory: SystemMemory,
    pub fans: FanTelemetry,
    pub sampled_at_ms: u64,
    pub ready: bool,
}

impl Default for TelemetrySnapshot {
    fn default() -> Self {
        Self {
            cpu: CpuTelemetry {
                avg_freq_mhz: 0,
                max_freq_mhz: 0,
                utilization_pct: 0.0,
                temp_c: None,
                temp_source: "unavailable".into(),
            },
            gpu: GpuTelemetry::Unavailable {
                reason: "Initializing telemetry...".into(),
            },
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
                error: None,
            },
            sampled_at_ms: now_ms(),
            ready: false,
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

type NvmlReturn = libc::c_int;
type NvmlDevice = *mut c_void;

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
    lib_handle: *mut c_void,
    get_handle_fn: unsafe extern "C" fn(libc::c_uint, *mut NvmlDevice) -> NvmlReturn,
    get_clock_fn: unsafe extern "C" fn(NvmlDevice, libc::c_uint, *mut libc::c_uint) -> NvmlReturn,
    get_max_clock_fn:
        Option<unsafe extern "C" fn(NvmlDevice, libc::c_uint, *mut libc::c_uint) -> NvmlReturn>,
    get_temp_fn: unsafe extern "C" fn(NvmlDevice, libc::c_uint, *mut libc::c_uint) -> NvmlReturn,
    get_util_fn: unsafe extern "C" fn(NvmlDevice, *mut NvmlUtilization) -> NvmlReturn,
    get_mem_fn: unsafe extern "C" fn(NvmlDevice, *mut NvmlMemory) -> NvmlReturn,
    get_name_fn: Option<unsafe extern "C" fn(NvmlDevice, *mut libc::c_char, libc::c_uint) -> NvmlReturn>,
    shutdown_fn: unsafe extern "C" fn() -> NvmlReturn,
    initialized: bool,
}

unsafe impl Send for NvmlLibrary {}
unsafe impl Sync for NvmlLibrary {}

impl NvmlLibrary {
    fn load() -> Result<Self, String> {
        let soname = b"libnvidia-ml.so.1\0";
        let lib_handle =
            unsafe { libc::dlopen(soname.as_ptr() as *const libc::c_char, libc::RTLD_LAZY) };
        if lib_handle.is_null() {
            return Err("libnvidia-ml.so.1 unavailable".into());
        }

        macro_rules! sym {
            ($name:expr) => {{
                let ptr = unsafe { libc::dlsym(lib_handle, $name.as_ptr() as *const libc::c_char) };
                if ptr.is_null() {
                    unsafe { libc::dlclose(lib_handle) };
                    return Err(format!(
                        "Symbol {} missing in libnvidia-ml.so.1",
                        String::from_utf8_lossy($name).trim_end_matches('\0')
                    ));
                }
                unsafe { std::mem::transmute(ptr) }
            }};
        }

        let init_fn: unsafe extern "C" fn() -> NvmlReturn = sym!(b"nvmlInit_v2\0");
        let get_handle_fn = sym!(b"nvmlDeviceGetHandleByIndex_v2\0");
        let get_clock_fn = sym!(b"nvmlDeviceGetClockInfo\0");
        let get_temp_fn = sym!(b"nvmlDeviceGetTemperature\0");
        let get_util_fn = sym!(b"nvmlDeviceGetUtilizationRates\0");
        let get_mem_fn = sym!(b"nvmlDeviceGetMemoryInfo\0");
        let shutdown_fn = sym!(b"nvmlShutdown\0");

        let max_clock_ptr = unsafe {
            libc::dlsym(
                lib_handle,
                b"nvmlDeviceGetMaxClockInfo\0".as_ptr() as *const libc::c_char,
            )
        };
        let get_max_clock_fn = if max_clock_ptr.is_null() {
            None
        } else {
            Some(unsafe { std::mem::transmute(max_clock_ptr) })
        };

        let name_ptr = unsafe {
            libc::dlsym(
                lib_handle,
                b"nvmlDeviceGetName\0".as_ptr() as *const libc::c_char,
            )
        };
        let get_name_fn = if name_ptr.is_null() {
            None
        } else {
            Some(unsafe { std::mem::transmute(name_ptr) })
        };

        let ret = unsafe { init_fn() };
        if ret != 0 {
            unsafe { libc::dlclose(lib_handle) };
            return Err(format!("nvmlInit_v2 returned code {ret}"));
        }

        Ok(Self {
            lib_handle,
            get_handle_fn,
            get_clock_fn,
            get_max_clock_fn,
            get_temp_fn,
            get_util_fn,
            get_mem_fn,
            get_name_fn,
            shutdown_fn,
            initialized: true,
        })
    }

    fn device_name_and_vram(&self) -> Option<String> {
        let get_name = self.get_name_fn?;
        let mut device: NvmlDevice = std::ptr::null_mut();
        if unsafe { (self.get_handle_fn)(0, &mut device) } != 0 || device.is_null() {
            return None;
        }
        let mut buf = [0u8; 96];
        if unsafe { (get_name)(device, buf.as_mut_ptr() as *mut libc::c_char, buf.len() as libc::c_uint) } != 0 {
            return None;
        }
        let raw_name = String::from_utf8_lossy(&buf)
            .trim_end_matches('\0')
            .trim()
            .to_string();
        if raw_name.is_empty() {
            return None;
        }
        let mut mem = NvmlMemory {
            total: 0,
            free: 0,
            used: 0,
        };
        if unsafe { (self.get_mem_fn)(device, &mut mem) } == 0 && mem.total > 0 {
            let gb = ((mem.total as f64) / (1024.0 * 1024.0 * 1024.0)).round() as u64;
            if gb > 0 {
                return Some(format!("{raw_name} {gb}GB"));
            }
        }
        Some(raw_name)
    }

    fn query(&self) -> Result<GpuTelemetry, String> {
        if !self.initialized {
            return Err("NVML not initialized".into());
        }

        let mut device: NvmlDevice = std::ptr::null_mut();
        let ret = unsafe { (self.get_handle_fn)(0, &mut device) };
        if ret != 0 || device.is_null() {
            return Err(format!("NVML device handle failed (code {ret})"));
        }

        let mut clock_mhz: libc::c_uint = 0;
        let _ = unsafe { (self.get_clock_fn)(device, 0, &mut clock_mhz) };

        let mut max_clock_mhz: libc::c_uint = 2100;
        if let Some(max_fn) = self.get_max_clock_fn {
            let mut reported_max: libc::c_uint = 0;
            if unsafe { max_fn(device, 0, &mut reported_max) } == 0 && reported_max > 0 {
                max_clock_mhz = reported_max;
            }
        }

        let mut temp_c: libc::c_uint = 0;
        let _ = unsafe { (self.get_temp_fn)(device, 0, &mut temp_c) };

        let mut util = NvmlUtilization { gpu: 0, memory: 0 };
        let _ = unsafe { (self.get_util_fn)(device, &mut util) };

        let mut mem = NvmlMemory {
            total: 0,
            free: 0,
            used: 0,
        };
        let _ = unsafe { (self.get_mem_fn)(device, &mut mem) };

        Ok(GpuTelemetry::Active {
            core_clock_mhz: clock_mhz as u32,
            max_clock_mhz: max_clock_mhz as u32,
            temp_c: temp_c as u32,
            utilization_pct: util.gpu as u32,
            vram_used_mb: (mem.used / (1024 * 1024)) as u64,
            vram_total_mb: (mem.total / (1024 * 1024)) as u64,
        })
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

pub struct TelemetryCollector {
    identity: Arc<RwLock<SystemIdentity>>,
    snapshot: Arc<RwLock<TelemetrySnapshot>>,
    shutdown: Arc<(Mutex<bool>, Condvar)>,
    worker_handle: Option<JoinHandle<()>>,
}

impl TelemetryCollector {
    pub fn new() -> Self {
        let initial_identity = detect_cpu_identity_fast();
        let identity = Arc::new(RwLock::new(initial_identity));
        let snapshot = Arc::new(RwLock::new(TelemetrySnapshot::default()));
        let shutdown = Arc::new((Mutex::new(false), Condvar::new()));

        let identity_clone = Arc::clone(&identity);
        let snapshot_clone = Arc::clone(&snapshot);
        let shutdown_clone = Arc::clone(&shutdown);

        let worker_handle = thread::Builder::new()
            .name("uxf-telemetry".into())
            .spawn(move || {
                let dgpu_runtime_path = find_nvidia_runtime_status_path();
                let full_identity = detect_system_identity(dgpu_runtime_path.as_deref());
                if let Ok(mut guard) = identity_clone.write() {
                    *guard = full_identity;
                }

                let acer_lib = AcerControlLib::load();
                let mut nvml: Option<NvmlLibrary> = None;

                let (_, mut prev_idle, mut prev_total) = sample_cpu_load(0, 0);
                // Short warmup slice so the very first UI snapshot already carries real CPU load %.
                {
                    let (lock, cvar) = &*shutdown_clone;
                    if let Ok(stopped) = lock.lock() {
                        if !*stopped {
                            let _ = cvar.wait_timeout(stopped, Duration::from_millis(120));
                        }
                    }
                }

                loop {
                    {
                        let (lock, _) = &*shutdown_clone;
                        if lock.lock().map(|g| *g).unwrap_or(true) {
                            break;
                        }
                    }

                    let (utilization_pct, cur_idle, cur_total) =
                        sample_cpu_load(prev_idle, prev_total);
                    prev_idle = cur_idle;
                    prev_total = cur_total;

                    let (avg_freq_mhz, max_freq_mhz) = sample_cpu_frequencies();

                    let (ec_cpu_temp, ec_gpu_temp, cpu_rpm, gpu_rpm, acer_err) = match &acer_lib {
                        Ok(lib) => match lib.open_context() {
                            Ok(ctx) => {
                                let c_temp = ctx.cpu_temperature();
                                let g_temp = ctx.gpu_temperature();
                                let c_rpm = ctx.cpu_fan_rpm();
                                let g_rpm = ctx.gpu_fan_rpm();
                                let err = c_rpm
                                    .as_ref()
                                    .err()
                                    .or_else(|| g_rpm.as_ref().err())
                                    .cloned();
                                (c_temp.ok(), g_temp.ok(), c_rpm.ok(), g_rpm.ok(), err)
                            }
                            Err(err) => (None, None, None, None, Some(err)),
                        },
                        Err(err) => (None, None, None, None, Some(err.clone())),
                    };

                    let (cpu_temp_c, temp_source) = match ec_cpu_temp {
                        Some(t) => (Some(t), "ec".to_string()),
                        None => match sample_coretemp_celsius() {
                            Some(t) => (Some(t), "coretemp".to_string()),
                            None => (None, "unavailable".to_string()),
                        },
                    };

                    let gpu = sample_gpu_telemetry(
                        dgpu_runtime_path.as_deref(),
                        &mut nvml,
                        ec_gpu_temp,
                    );
                    let memory = sample_memory_and_storage();

                    let next = TelemetrySnapshot {
                        cpu: CpuTelemetry {
                            avg_freq_mhz,
                            max_freq_mhz,
                            utilization_pct,
                            temp_c: cpu_temp_c,
                            temp_source,
                        },
                        gpu,
                        memory,
                        fans: FanTelemetry {
                            cpu_fan_rpm: cpu_rpm,
                            gpu_fan_rpm: gpu_rpm,
                            error: acer_err,
                        },
                        sampled_at_ms: now_ms(),
                        ready: true,
                    };

                    if let Ok(mut guard) = snapshot_clone.write() {
                        *guard = next;
                    }

                    let (lock, cvar) = &*shutdown_clone;
                    if let Ok(stopped) = lock.lock() {
                        if *stopped {
                            break;
                        }
                        let (after, _) = cvar
                            .wait_timeout(stopped, Duration::from_millis(1000))
                            .unwrap_or_else(|e| e.into_inner());
                        if *after {
                            break;
                        }
                    }
                }
            })
            .ok();

        Self {
            identity,
            snapshot,
            shutdown,
            worker_handle,
        }
    }

    pub fn identity(&self) -> SystemIdentity {
        self.identity
            .read()
            .map(|g| g.clone())
            .unwrap_or_else(|_| detect_cpu_identity_fast())
    }

    pub fn identity_handle(&self) -> Arc<RwLock<SystemIdentity>> {
        Arc::clone(&self.identity)
    }

    pub fn snapshot(&self) -> TelemetrySnapshot {
        self.snapshot.read().map(|s| s.clone()).unwrap_or_default()
    }
}

impl Drop for TelemetryCollector {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.shutdown;
        if let Ok(mut stopped) = lock.lock() {
            *stopped = true;
            cvar.notify_all();
        }
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

fn find_nvidia_runtime_status_path() -> Option<PathBuf> {
    let default_path = PathBuf::from("/sys/bus/pci/devices/0000:01:00.0/power/runtime_status");
    if let Ok(entries) = fs::read_dir("/sys/bus/pci/devices") {
        for entry in entries.flatten() {
            let dir = entry.path();
            let vendor = fs::read_to_string(dir.join("vendor")).unwrap_or_default();
            let class = fs::read_to_string(dir.join("class")).unwrap_or_default();
            if vendor.trim().eq_ignore_ascii_case("0x10de") && class.trim().starts_with("0x03") {
                let candidate = dir.join("power/runtime_status");
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    if default_path.exists() {
        Some(default_path)
    } else {
        None
    }
}

fn is_dgpu_suspended(runtime_status_path: Option<&Path>) -> bool {
    let Some(path) = runtime_status_path else {
        return false;
    };
    fs::read_to_string(path)
        .map(|s| s.trim() == "suspended")
        .unwrap_or(false)
}

fn detect_cpu_identity_fast() -> SystemIdentity {
    let cpu_model = fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "Unknown CPU".into());

    let cpu_max_ghz = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|khz| ((khz + 999_999) / 1_000_000).clamp(5, 8) as u32)
        .unwrap_or(5);

    SystemIdentity {
        cpu_model,
        gpu_model: "Detecting GPU...".into(),
        cpu_max_ghz,
    }
}

fn detect_system_identity(runtime_status_path: Option<&Path>) -> SystemIdentity {
    let fast = detect_cpu_identity_fast();
    let cpu_model = fast.cpu_model;
    let cpu_max_ghz = fast.cpu_max_ghz;

    let mut gpu_model = None;
    if !is_dgpu_suspended(runtime_status_path) {
        if let Ok(nvml) = NvmlLibrary::load() {
            gpu_model = nvml.device_name_and_vram();
        }
        if gpu_model.is_none() {
            gpu_model = Command::new("nvidia-smi")
                .args(["--query-gpu=name", "--format=csv,noheader"])
                .output()
                .ok()
                .filter(|out| out.status.success())
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }
    }

    if gpu_model.is_none() {
        gpu_model = Command::new("lspci")
            .output()
            .ok()
            .and_then(|out| {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    if (line.contains("3D controller") || line.contains("VGA compatible controller"))
                        && (line.contains("NVIDIA")
                            || line.contains("AMD")
                            || line.contains("Advanced Micro Devices"))
                    {
                        if let Some(name) = line.split(':').nth(2) {
                            let clean = name.trim().to_string();
                            if !clean.is_empty() {
                                return Some(clean);
                            }
                        }
                    }
                }
                None
            });
    }

    SystemIdentity {
        cpu_model,
        gpu_model: gpu_model.unwrap_or_else(|| "Discrete GPU Unavailable".into()),
        cpu_max_ghz,
    }
}

fn sample_gpu_telemetry(
    runtime_status_path: Option<&Path>,
    nvml: &mut Option<NvmlLibrary>,
    fallback_temp: Option<i32>,
) -> GpuTelemetry {
    if is_dgpu_suspended(runtime_status_path) {
        return GpuTelemetry::Sleeping;
    }

    if nvml.is_none() {
        *nvml = NvmlLibrary::load().ok();
    }

    if let Some(lib) = nvml.as_ref() {
        match lib.query() {
            Ok(active) => return active,
            Err(err) => {
                if let Some(temp) = fallback_temp {
                    return GpuTelemetry::Partial {
                        temp_c: temp,
                        reason: err,
                    };
                }
                return GpuTelemetry::Unavailable { reason: err };
            }
        }
    }

    if let Some(temp) = fallback_temp {
        GpuTelemetry::Partial {
            temp_c: temp,
            reason: "NVML library unavailable (EC temperature only)".into(),
        }
    } else {
        GpuTelemetry::Unavailable {
            reason: "NVIDIA NVML unavailable".into(),
        }
    }
}

fn sample_cpu_frequencies() -> (u32, u32) {
    let mut freqs_khz = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/devices/system/cpu") {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if name.starts_with("cpu") && name[3..].chars().all(|c| c.is_ascii_digit()) {
                let path = entry.path().join("cpufreq/scaling_cur_freq");
                if let Ok(raw) = fs::read_to_string(path) {
                    if let Ok(khz) = raw.trim().parse::<u64>() {
                        freqs_khz.push(khz);
                    }
                }
            }
        }
    }

    if freqs_khz.is_empty() {
        return (0, 0);
    }

    let max_mhz = (freqs_khz.iter().copied().max().unwrap_or(0) / 1000) as u32;
    let avg_mhz =
        (freqs_khz.iter().sum::<u64>() / (freqs_khz.len() as u64 * 1000)) as u32;
    (avg_mhz, max_mhz)
}

fn sample_cpu_load(prev_idle: u64, prev_total: u64) -> (f32, u64, u64) {
    let Ok(stat) = fs::read_to_string("/proc/stat") else {
        return (0.0, prev_idle, prev_total);
    };
    let Some(first) = stat.lines().next().filter(|l| l.starts_with("cpu ")) else {
        return (0.0, prev_idle, prev_total);
    };
    let values: Vec<u64> = first
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse::<u64>().ok())
        .collect();
    if values.len() < 4 {
        return (0.0, prev_idle, prev_total);
    }
    let idle = values[3] + values.get(4).copied().unwrap_or(0);
    let total: u64 = values.iter().sum();

    if prev_total == 0 || total <= prev_total {
        return (0.0, idle, total);
    }

    let total_delta = (total - prev_total) as f32;
    let idle_delta = idle.saturating_sub(prev_idle) as f32;
    let util = ((total_delta - idle_delta) / total_delta * 100.0).clamp(0.0, 100.0);
    (util, idle, total)
}

fn sample_coretemp_celsius() -> Option<i32> {
    let entries = fs::read_dir("/sys/class/hwmon").ok()?;
    for entry in entries.flatten() {
        let dir = entry.path();
        let Ok(name) = fs::read_to_string(dir.join("name")) else {
            continue;
        };
        if name.trim() == "coretemp" {
            if let Ok(raw) = fs::read_to_string(dir.join("temp1_input")) {
                if let Ok(milli_c) = raw.trim().parse::<i32>() {
                    return Some(milli_c / 1000);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_and_freq_sampling_works_unprivileged() {
        let mem = sample_memory_and_storage();
        assert!(mem.ram_total_mb > 0);
        assert!(mem.disk_total_gb > 0);

        let (avg_mhz, max_mhz) = sample_cpu_frequencies();
        assert!(avg_mhz > 0);
        assert!(max_mhz >= avg_mhz);
    }

    #[test]
    fn telemetry_collector_collects_live_snapshot() {
        let collector = TelemetryCollector::new();
        let identity = collector.identity();
        assert!(!identity.cpu_model.is_empty());
        assert!(!identity.gpu_model.is_empty());
        assert!(identity.cpu_max_ghz >= 5);

        let mut snapshot = collector.snapshot();
        for _ in 0..20 {
            if snapshot.ready {
                break;
            }
            thread::sleep(Duration::from_millis(50));
            snapshot = collector.snapshot();
        }
        assert!(snapshot.ready, "Collector should populate initial live snapshot");
        assert!(snapshot.cpu.avg_freq_mhz > 0);
        assert!(snapshot.cpu.temp_c.is_some());
        assert!(snapshot.memory.ram_total_mb > 0);
        assert!(snapshot.memory.disk_total_gb > 0);
        assert!(snapshot.fans.cpu_fan_rpm.is_some());
        assert!(snapshot.fans.gpu_fan_rpm.is_some());
        assert!(matches!(
            snapshot.gpu,
            GpuTelemetry::Active { .. } | GpuTelemetry::Sleeping
        ));
    }
}

fn sample_memory_and_storage() -> SystemMemory {
    let mut total_kb = 0u64;
    let mut avail_kb = 0u64;

    if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("MemAvailable:") {
                avail_kb = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            }
        }
    }

    let ram_total_mb = total_kb / 1024;
    let ram_used_mb = total_kb.saturating_sub(avail_kb) / 1024;
    let ram_percent = if ram_total_mb > 0 {
        (ram_used_mb as f32 / ram_total_mb as f32) * 100.0
    } else {
        0.0
    };

    let mut disk_used_gb = 0u64;
    let mut disk_total_gb = 0u64;
    let mut disk_percent = 0.0f32;

    unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        let root_c = b"/\0";
        if libc::statvfs(root_c.as_ptr() as *const libc::c_char, &mut stat) == 0 {
            let block_size = stat.f_frsize as u64;
            let total_bytes = (stat.f_blocks as u64).saturating_mul(block_size);
            let avail_bytes = (stat.f_bavail as u64).saturating_mul(block_size);
            let used_bytes = total_bytes.saturating_sub(avail_bytes);

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
