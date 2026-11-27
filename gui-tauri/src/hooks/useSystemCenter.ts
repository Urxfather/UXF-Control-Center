import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ActivityLogEntry,
  CachedHealthEnvelope,
  CachedUpdatesEnvelope,
  SystemCenterOverviewSnapshot,
  SystemHealthReport,
  SystemInformationReport,
  UpdateCheckResult,
} from "../types/health";
import type { SystemIdentity, TelemetrySnapshot } from "../types/system";
import type { SystemCenterTabId } from "../types/navigation";
import { isVersionGreater, purgeUpdateCache } from "./useUpdateChecker";

export function formatDurationSeconds(totalSeconds: number | null | undefined): string {
  if (totalSeconds === null || totalSeconds === undefined || !Number.isFinite(totalSeconds)) {
    return "Unavailable";
  }
  const s = Math.max(0, Math.floor(totalSeconds));
  const days = Math.floor(s / 86400);
  const hours = Math.floor((s % 86400) / 3600);
  const mins = Math.floor((s % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h ${mins}m`;
  if (hours > 0) return `${hours}h ${mins}m`;
  return `${mins}m`;
}

export function formatUtcTimestamp(ms: number | null | undefined): string {
  if (!ms || !Number.isFinite(ms)) return "Not checked yet";
  const d = new Date(ms);
  const yyyy = d.getUTCFullYear();
  const mm = String(d.getUTCMonth() + 1).padStart(2, "0");
  const dd = String(d.getUTCDate()).padStart(2, "0");
  const hh = String(d.getUTCHours()).padStart(2, "0");
  const mi = String(d.getUTCMinutes()).padStart(2, "0");
  const ss = String(d.getUTCSeconds()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd} ${hh}:${mi}:${ss} UTC`;
}

export function formatClockUtc(ms: number | null | undefined): string {
  if (!ms || !Number.isFinite(ms)) return "--:--:--";
  const d = new Date(ms);
  const hh = String(d.getUTCHours()).padStart(2, "0");
  const mi = String(d.getUTCMinutes()).padStart(2, "0");
  const ss = String(d.getUTCSeconds()).padStart(2, "0");
  return `${hh}:${mi}:${ss} UTC`;
}

export function formatRelativeTime(ms: number | null | undefined): string {
  if (!ms || !Number.isFinite(ms)) return "just now";
  const diffSec = Math.max(0, Math.floor((Date.now() - ms) / 1000));
  if (diffSec < 15) return "just now";
  if (diffSec < 60) return `${diffSec}s ago`;
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  return formatClockUtc(ms);
}

export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    try {
      const textarea = document.createElement("textarea");
      textarea.value = text;
      textarea.style.position = "fixed";
      textarea.style.opacity = "0";
      document.body.appendChild(textarea);
      textarea.focus();
      textarea.select();
      const ok = document.execCommand("copy");
      document.body.removeChild(textarea);
      return ok;
    } catch {
      return false;
    }
  }
}

export function useSystemCenter(
  appVersion: string,
  activeTab: SystemCenterTabId
) {
  const [identity, setIdentity] = useState<SystemIdentity | null>(null);
  const [telemetry, setTelemetry] = useState<TelemetrySnapshot | null>(null);
  const [uptimeSeconds, setUptimeSeconds] = useState<number | null>(null);

  const [healthReport, setHealthReport] = useState<SystemHealthReport | null>(null);
  const [healthCached, setHealthCached] = useState(true);
  const [healthRefreshing, setHealthRefreshing] = useState(false);
  const [healthCheckedAtMs, setHealthCheckedAtMs] = useState<number | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);

  const [updateResult, setUpdateResult] = useState<UpdateCheckResult | null>(null);
  const [updatesCached, setUpdatesCached] = useState(true);
  const [updatesChecking, setUpdatesChecking] = useState(false);
  const [updatesCheckedAtMs, setUpdatesCheckedAtMs] = useState<number | null>(null);

  const [systemInfo, setSystemInfo] = useState<SystemInformationReport | null>(null);
  const [systemInfoLoading, setSystemInfoLoading] = useState(false);

  const [activity, setActivity] = useState<ActivityLogEntry[]>([]);

  const healthInFlightRef = useRef(false);
  const updatesInFlightRef = useRef(false);
  const sysInfoInFlightRef = useRef(false);

  const applyHealthEnvelope = useCallback((env: CachedHealthEnvelope) => {
    if (env.report) {
      setHealthReport(env.report);
      setHealthError(null);
    }
    setHealthCached(env.cached);
    setHealthRefreshing(env.refreshing);
    if (env.checkedAtMs) {
      setHealthCheckedAtMs(env.checkedAtMs);
    }
  }, []);

  const refreshHealth = useCallback(async (force = true) => {
    if (healthInFlightRef.current) return;
    healthInFlightRef.current = true;
    setHealthRefreshing(true);
    try {
      const env = await invoke<CachedHealthEnvelope>("refresh_system_health", {
        force,
      });
      applyHealthEnvelope(env);
      const ov = await invoke<SystemCenterOverviewSnapshot>(
        "get_system_center_overview"
      ).catch(() => null);
      if (ov) {
        setActivity(ov.activity);
        if (ov.uptimeSeconds !== null) setUptimeSeconds(ov.uptimeSeconds);
      }
    } catch (err) {
      setHealthError(err instanceof Error ? err.message : String(err));
    } finally {
      healthInFlightRef.current = false;
      setHealthRefreshing(false);
    }
  }, [applyHealthEnvelope]);

  const checkUpdates = useCallback(
    async (force = false) => {
      if (updatesInFlightRef.current) return;
      updatesInFlightRef.current = true;
      setUpdatesChecking(true);
      try {
        const res = await invoke<UpdateCheckResult>("check_updates", {
          currentVersion: appVersion,
          force,
        });

        // Apply same client-side SemVer protection as background hook
        if (res.latest_version) {
          if (!isVersionGreater(res.current_version, res.latest_version)) {
            res.update_available = false;
          }
        } else {
          res.update_available = false;
        }

        if (!res.update_available) {
          purgeUpdateCache();
        }

        setUpdateResult(res);
        setUpdatesCached(!force);
        setUpdatesCheckedAtMs(res.checked_at_ms);

        const ov = await invoke<SystemCenterOverviewSnapshot>(
          "get_system_center_overview"
        ).catch(() => null);
        if (ov) {
          setActivity(ov.activity);
        }
      } catch (err) {
        console.warn("Update check failed:", err);
      } finally {
        updatesInFlightRef.current = false;
        setUpdatesChecking(false);
      }
    },
    [appVersion]
  );

  const loadSystemInfo = useCallback(async (force = false) => {
    if (sysInfoInFlightRef.current) return;
    sysInfoInFlightRef.current = true;
    setSystemInfoLoading(true);
    try {
      const report = await invoke<SystemInformationReport>("get_system_information", {
        force,
      });
      setSystemInfo(report);
    } catch (err) {
      console.warn("System info load failed:", err);
    } finally {
      sysInfoInFlightRef.current = false;
      setSystemInfoLoading(false);
    }
  }, []);

  // Instant initial load from Rust caches (< 2ms), then non-blocking async fills
  useEffect(() => {
    let active = true;
    let unlistenHealth: UnlistenFn | undefined;
    let unlistenUpdates: UnlistenFn | undefined;

    const bootstrap = async () => {
      const [overview, iden, snap] = await Promise.all([
        invoke<SystemCenterOverviewSnapshot>("get_system_center_overview").catch(() => null),
        invoke<SystemIdentity>("get_system_identity").catch(() => null),
        invoke<TelemetrySnapshot>("get_telemetry_snapshot").catch(() => null),
      ]);

      if (!active) return;

      if (iden) setIdentity(iden);
      if (snap) setTelemetry(snap);

      if (overview) {
        if (overview.uptimeSeconds !== null) {
          setUptimeSeconds(overview.uptimeSeconds);
        }
        setActivity(overview.activity);
        applyHealthEnvelope(overview.health);

        if (overview.updates.result) {
          setUpdateResult(overview.updates.result);
          setUpdatesCached(overview.updates.cached);
          setUpdatesCheckedAtMs(overview.updates.checkedAtMs);
        }

        // Non-blocking background fills ONLY if cache empty (never stalls render!)
        if (!overview.health.report && !overview.health.refreshing) {
          void refreshHealth(false);
        }
        if (!overview.updates.result && !overview.updates.checking) {
          void checkUpdates(false);
        }
      } else {
        void refreshHealth(false);
        void checkUpdates(false);
      }

      void loadSystemInfo(false);

      try {
        unlistenHealth = await listen<CachedHealthEnvelope>(
          "system-health-updated",
          (ev) => {
            if (active && ev.payload) {
              applyHealthEnvelope(ev.payload);
            }
          }
        );
        unlistenUpdates = await listen<CachedUpdatesEnvelope>(
          "system-updates-updated",
          (ev) => {
            if (active && ev.payload?.result) {
              setUpdateResult(ev.payload.result);
              setUpdatesCached(ev.payload.cached);
              setUpdatesCheckedAtMs(ev.payload.checkedAtMs);
            }
          }
        );
      } catch {
        // ignore in unit test environments
      }
    };

    void bootstrap();

    return () => {
      active = false;
      unlistenHealth?.();
      unlistenUpdates?.();
    };
  }, [applyHealthEnvelope, checkUpdates, loadSystemInfo, refreshHealth]);

  // Fast memory-only telemetry updates while viewing Overview (zero extra D-Bus calls!)
  useEffect(() => {
    if (activeTab !== "overview" && activeTab !== "system-info") {
      return;
    }
    let active = true;
    const poll = async () => {
      const [snap, ov] = await Promise.all([
        invoke<TelemetrySnapshot>("get_telemetry_snapshot").catch(() => null),
        invoke<SystemCenterOverviewSnapshot>("get_system_center_overview").catch(() => null),
      ]);
      if (!active) return;
      if (snap) setTelemetry(snap);
      if (ov) {
        if (ov.uptimeSeconds !== null) setUptimeSeconds(ov.uptimeSeconds);
        setActivity(ov.activity);
      }
    };
    const timer = window.setInterval(() => {
      void poll();
    }, 1500);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [activeTab]);

  return {
    identity,
    telemetry,
    uptimeSeconds,
    healthReport,
    healthCached,
    healthRefreshing,
    healthCheckedAtMs,
    healthError,
    refreshHealth,
    updateResult,
    updatesCached,
    updatesChecking,
    updatesCheckedAtMs,
    checkUpdates,
    systemInfo,
    systemInfoLoading,
    loadSystemInfo,
    activity,
  };
}
