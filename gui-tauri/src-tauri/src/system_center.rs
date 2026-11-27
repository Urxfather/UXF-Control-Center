use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

use crate::health::{
    check_app_update, check_system_health, detect_installed_version, HealthStatus,
    SystemHealthReport, UpdateCheckResult,
};
use crate::system_info::{
    collect_system_information, now_ms, read_uptime_seconds, run_command_with_timeout,
    SystemInformationReport,
};
use crate::telemetry::SystemIdentity;

pub const HEALTH_UPDATED_EVENT: &str = "system-health-updated";
pub const UPDATES_UPDATED_EVENT: &str = "system-updates-updated";

const MAX_ACTIVITY_ENTRIES: usize = 16;
const HEALTH_CACHE_TTL_MS: u64 = 45_000;
const UPDATES_CACHE_TTL_MS: u64 = 10 * 60 * 1000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityLogEntry {
    pub id: String,
    pub message: String,
    pub source: String,
    pub tone: String, // "healthy" | "info" | "warning" | "error"
    pub timestamp_ms: u64,
    pub time_label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedHealthEnvelope {
    pub report: Option<SystemHealthReport>,
    pub cached: bool,
    pub refreshing: bool,
    pub checked_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedUpdatesEnvelope {
    pub result: Option<UpdateCheckResult>,
    pub cached: bool,
    pub checking: bool,
    pub checked_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemCenterOverviewSnapshot {
    pub app_version: String,
    pub uptime_seconds: Option<u64>,
    pub health: CachedHealthEnvelope,
    pub updates: CachedUpdatesEnvelope,
    pub activity: Vec<ActivityLogEntry>,
}

pub struct SystemCenterManager {
    installed_version: String,
    health_cache: RwLock<Option<SystemHealthReport>>,
    health_sync: (Mutex<bool>, Condvar),
    updates_cache: RwLock<Option<UpdateCheckResult>>,
    updates_sync: (Mutex<bool>, Condvar),
    sysinfo_cache: RwLock<Option<SystemInformationReport>>,
    activity: RwLock<VecDeque<ActivityLogEntry>>,
    app_handle: RwLock<Option<AppHandle>>,
}

impl SystemCenterManager {
    pub fn new() -> Arc<Self> {
        let installed_version = detect_installed_version();
        let mut initial_activity = VecDeque::with_capacity(MAX_ACTIVITY_ENTRIES);

        // Seed real system service enter timestamp from systemd if active
        if let Some(out) = run_command_with_timeout(
            "systemctl",
            &[
                "show",
                "acer-control-daemon.service",
                "-p",
                "ActiveEnterTimestamp",
                "--value",
            ],
            Duration::from_millis(600),
        ) {
            if out.status.success() {
                let ts = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !ts.is_empty() && ts != "n/a" {
                    initial_activity.push_back(ActivityLogEntry {
                        id: "systemd-daemon-start".into(),
                        message: "Control daemon service started and registered on D-Bus".into(),
                        source: "acer-control-daemon".into(),
                        tone: "healthy".into(),
                        timestamp_ms: now_ms(),
                        time_label: Some(ts),
                    });
                }
            }
        }

        initial_activity.push_back(ActivityLogEntry {
            id: "app-launch".into(),
            message: format!("UXF Control Center v{installed_version} session initialized"),
            source: "uxf-control-center".into(),
            tone: "info".into(),
            timestamp_ms: now_ms(),
            time_label: None,
        });

        Arc::new(Self {
            installed_version,
            health_cache: RwLock::new(None),
            health_sync: (Mutex::new(false), Condvar::new()),
            updates_cache: RwLock::new(None),
            updates_sync: (Mutex::new(false), Condvar::new()),
            sysinfo_cache: RwLock::new(None),
            activity: RwLock::new(initial_activity),
            app_handle: RwLock::new(None),
        })
    }

    pub fn installed_version(&self) -> String {
        self.installed_version.clone()
    }

    pub fn bind_app(&self, handle: AppHandle) {
        if let Ok(mut guard) = self.app_handle.write() {
            *guard = Some(handle);
        }
    }

    pub fn start_warmup(self: &Arc<Self>, identity_ref: Arc<std::sync::RwLock<SystemIdentity>>) {
        let mgr = Arc::clone(self);
        let _ = thread::Builder::new()
            .name("uxf-sc-warmup".into())
            .spawn(move || {
                // Allow primary dashboard & telemetry collector to win first frame
                thread::sleep(Duration::from_millis(900));
                if let Ok(identity) = identity_ref.read().map(|g| g.clone()) {
                    mgr.refresh_system_info(&identity);
                }
                let _ = mgr.get_or_refresh_health(true);
            });
    }

    pub fn record_activity(
        &self,
        id_prefix: &str,
        message: impl Into<String>,
        source: impl Into<String>,
        tone: impl Into<String>,
    ) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let entry = ActivityLogEntry {
            id: format!("{id_prefix}-{ts}"),
            message: message.into(),
            source: source.into(),
            tone: tone.into(),
            timestamp_ms: ts,
            time_label: None,
        };
        if let Ok(mut guard) = self.activity.write() {
            guard.push_front(entry);
            while guard.len() > MAX_ACTIVITY_ENTRIES {
                guard.pop_back();
            }
        }
    }

    pub fn get_activity_list(&self) -> Vec<ActivityLogEntry> {
        self.activity
            .read()
            .map(|a| a.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn refresh_system_info(&self, identity: &SystemIdentity) -> SystemInformationReport {
        let info = collect_system_information(identity);
        if let Ok(mut guard) = self.sysinfo_cache.write() {
            *guard = Some(info.clone());
        }
        info
    }

    pub fn get_system_information(
        &self,
        identity: &SystemIdentity,
        force: bool,
    ) -> SystemInformationReport {
        if !force {
            if let Ok(guard) = self.sysinfo_cache.read() {
                if let Some(cached) = guard.as_ref() {
                    // Refresh fast dynamic fields (<0.5ms) while returning immediately
                    let mut updated = cached.clone();
                    updated.os_kernel.uptime_seconds = read_uptime_seconds();
                    return updated;
                }
            }
        }
        self.refresh_system_info(identity)
    }

    pub fn peek_health(&self) -> CachedHealthEnvelope {
        let report = self.health_cache.read().ok().and_then(|g| g.clone());
        let refreshing = self.health_sync.0.lock().map(|g| *g).unwrap_or(false);
        let checked_at_ms = report.as_ref().map(|r| r.checked_at_ms);
        CachedHealthEnvelope {
            report,
            cached: true,
            refreshing,
            checked_at_ms,
        }
    }

    pub fn peek_updates(&self) -> CachedUpdatesEnvelope {
        let result = self.updates_cache.read().ok().and_then(|g| g.clone());
        let checking = self.updates_sync.0.lock().map(|g| *g).unwrap_or(false);
        let checked_at_ms = result.as_ref().map(|r| r.checked_at_ms);
        CachedUpdatesEnvelope {
            result,
            cached: true,
            checking,
            checked_at_ms,
        }
    }

    pub fn get_overview_snapshot(&self) -> SystemCenterOverviewSnapshot {
        SystemCenterOverviewSnapshot {
            app_version: self.installed_version.clone(),
            uptime_seconds: read_uptime_seconds(),
            health: self.peek_health(),
            updates: self.peek_updates(),
            activity: self.get_activity_list(),
        }
    }

    /// Returns cached health report immediately if fresh, or deduplicates concurrent
    /// diagnostic runs via Mutex + Condvar so only one scan runs at a time.
    pub fn get_or_refresh_health(&self, force: bool) -> CachedHealthEnvelope {
        if !force {
            if let Ok(guard) = self.health_cache.read() {
                if let Some(report) = guard.as_ref() {
                    if now_ms().saturating_sub(report.checked_at_ms) < HEALTH_CACHE_TTL_MS {
                        return CachedHealthEnvelope {
                            report: Some(report.clone()),
                            cached: true,
                            refreshing: false,
                            checked_at_ms: Some(report.checked_at_ms),
                        };
                    }
                }
            }
        }

        let (lock, cvar) = &self.health_sync;
        let mut in_flight = match lock.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        if *in_flight {
            // Another thread is already running the diagnostic scan; wait for it to publish
            while *in_flight {
                in_flight = match cvar.wait(in_flight) {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
            }
            if let Ok(guard) = self.health_cache.read() {
                if let Some(report) = guard.as_ref() {
                    return CachedHealthEnvelope {
                        report: Some(report.clone()),
                        cached: true,
                        refreshing: false,
                        checked_at_ms: Some(report.checked_at_ms),
                    };
                }
            }
        }

        *in_flight = true;
        drop(in_flight);

        let report = check_system_health();

        if let Ok(mut guard) = self.health_cache.write() {
            *guard = Some(report.clone());
        }

        let tone = match report.overall_status {
            HealthStatus::Passed => "healthy",
            HealthStatus::Warning | HealthStatus::Info => "warning",
            HealthStatus::Failed => "error",
        };
        self.record_activity(
            "health",
            format!(
                "System diagnostics checked: {}/{} subsystems healthy",
                report.healthy_count, report.total_count
            ),
            "health-probe",
            tone,
        );

        let env = CachedHealthEnvelope {
            report: Some(report.clone()),
            cached: false,
            refreshing: false,
            checked_at_ms: Some(report.checked_at_ms),
        };

        if let Ok(Some(handle)) = self.app_handle.read().map(|g| g.clone()) {
            let _ = handle.emit(HEALTH_UPDATED_EVENT, env.clone());
        }

        if let Ok(mut in_flight) = lock.lock() {
            *in_flight = false;
            cvar.notify_all();
        }

        env
    }

    pub fn check_updates_dedup(&self, current_version: &str, force: bool) -> UpdateCheckResult {
        if !force {
            if let Ok(guard) = self.updates_cache.read() {
                if let Some(cached) = guard.as_ref() {
                    if now_ms().saturating_sub(cached.checked_at_ms) < UPDATES_CACHE_TTL_MS {
                        return cached.clone();
                    }
                }
            }
        }

        let (lock, cvar) = &self.updates_sync;
        let mut in_flight = match lock.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        if *in_flight {
            while *in_flight {
                in_flight = match cvar.wait(in_flight) {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
            }
            if let Ok(guard) = self.updates_cache.read() {
                if let Some(cached) = guard.as_ref() {
                    return cached.clone();
                }
            }
        }

        *in_flight = true;
        drop(in_flight);

        let result = check_app_update(current_version);

        if let Ok(mut guard) = self.updates_cache.write() {
            *guard = Some(result.clone());
        }

        let (msg, tone) = if let Some(ref err) = result.error {
            (format!("Update check failed: {err}"), "warning")
        } else if result.update_available {
            (
                format!(
                    "Release v{} available on GitHub Releases",
                    result.latest_version.as_deref().unwrap_or("unknown")
                ),
                "info",
            )
        } else if let Some(ref latest) = result.latest_version {
            (
                format!("Release v{latest} verified current from GitHub Releases"),
                "healthy",
            )
        } else {
            ("GitHub release check completed".into(), "info")
        };

        self.record_activity("updates", msg, "uxf-updater", tone);

        if let Ok(Some(handle)) = self.app_handle.read().map(|g| g.clone()) {
            let env = CachedUpdatesEnvelope {
                result: Some(result.clone()),
                cached: false,
                checking: false,
                checked_at_ms: Some(result.checked_at_ms),
            };
            let _ = handle.emit(UPDATES_UPDATED_EVENT, env);
        }

        if let Ok(mut in_flight) = lock.lock() {
            *in_flight = false;
            cvar.notify_all();
        }

        result
    }
}
