import { useMemo, type ReactNode } from "react";
import type {
  ActivityLogEntry,
  SystemHealthReport,
  SystemInformationReport,
  UpdateCheckResult,
} from "../../types/health";
import type { TelemetrySnapshot } from "../../types/system";
import type { SystemCenterTabId } from "../../types/navigation";
import {
  formatDurationSeconds,
  formatRelativeTime,
} from "../../hooks/useSystemCenter";
import {
  CpuChipIcon,
  GpuCardIcon,
  RamStickIcon,
  StatusAlertIcon,
  StatusCircleCheckIcon,
  StorageDriveIcon,
} from "./SystemCenterIcons";

interface OverviewTabProps {
  appVersion: string;
  uptimeSeconds: number | null;
  telemetry: TelemetrySnapshot | null;
  healthReport: SystemHealthReport | null;
  healthCached: boolean;
  healthRefreshing: boolean;
  updateResult: UpdateCheckResult | null;
  updatesChecking: boolean;
  systemInfo: SystemInformationReport | null;
  activity: ActivityLogEntry[];
  onNavigateTab: (tab: SystemCenterTabId) => void;
}

function StatusDot({ tone }: { tone: "green" | "blue" | "amber" | "red" | "muted" }) {
  const cls =
    tone === "green"
      ? "bg-[#9be000]"
      : tone === "blue"
        ? "bg-[#60a5fa]"
        : tone === "amber"
          ? "bg-amber-400"
          : tone === "red"
            ? "bg-red-500"
            : "bg-slate-500";
  return <span className={`h-2 w-2 shrink-0 rounded-full ${cls}`} />;
}

function ServicePill({
  label,
  statusText,
  tone,
}: {
  label: string;
  statusText: string;
  tone: "green" | "blue" | "amber" | "red" | "muted";
}) {
  const textCls =
    tone === "green"
      ? "text-[#9be000]"
      : tone === "blue"
        ? "text-[#7dd3fc]"
        : tone === "amber"
          ? "text-amber-300"
          : tone === "red"
            ? "text-red-400"
            : "text-slate-400";

  return (
    <div
      title={`${label}: ${statusText}`}
      className="flex min-w-0 items-center justify-between gap-2 rounded-xl border border-[#202632] bg-[#171b24] px-2.5 py-2.5"
    >
      <div className="flex min-w-0 items-center gap-1.5">
        <StatusDot tone={tone} />
        <span className="truncate text-[11.5px] font-medium text-slate-200 xl:text-[12.5px]">
          {label}
        </span>
      </div>
      <span className={`shrink-0 whitespace-nowrap text-[11.5px] font-semibold xl:text-[12px] ${textCls}`}>
        {statusText}
      </span>
    </div>
  );
}

function HardwareSummaryRow({
  icon,
  iconTone,
  title,
  detail,
  metricText,
  fillPct,
  accentTone = "lime",
}: {
  icon: ReactNode;
  iconTone: string;
  title: string;
  detail: string;
  metricText: string;
  fillPct: number;
  accentTone?: "lime" | "blue";
}) {
  const textCls = accentTone === "blue" ? "text-[#60a5fa]" : "text-[#9be000]";
  const barCls = accentTone === "blue" ? "bg-[#60a5fa]" : "bg-[#9be000]";
  const clampedPct = Math.max(0, Math.min(100, fillPct));

  return (
    <div className="flex flex-col gap-1.5 py-2.5 first:pt-1 last:pb-0.5">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <span
            className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-[#171c25] ${iconTone}`}
          >
            {icon}
          </span>
          <span className="text-[14px] font-semibold text-white">{title}</span>
        </div>

        <div className="flex w-32 shrink-0 items-center gap-2.5 sm:w-36">
          <span
            className={`w-16 text-right font-mono text-[12px] font-semibold tabular-nums ${textCls}`}
          >
            {metricText}
          </span>
          <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-[#1e2430]">
            <div
              className={`h-full rounded-full transition-all duration-300 ${barCls}`}
              style={{ width: `${clampedPct}%` }}
            />
          </div>
        </div>
      </div>

      <div
        title={detail}
        className="truncate pl-11 font-mono text-[11.5px] text-[#8b949e]"
      >
        {detail}
      </div>
    </div>
  );
}

export function OverviewTab({
  appVersion,
  uptimeSeconds,
  telemetry,
  healthReport,
  healthCached,
  healthRefreshing,
  updateResult,
  updatesChecking,
  systemInfo,
  activity,
  onNavigateTab,
}: OverviewTabProps) {
  const overallView = useMemo(() => {
    if (!healthReport && healthRefreshing) {
      return {
        label: "Scanning...",
        tone: "muted" as const,
        iconBg: "bg-[#1a1f28] border-[#29303e] text-slate-300",
        textCls: "text-slate-200",
        sub: "Evaluating subsystems asynchronously...",
      };
    }
    if (!healthReport) {
      return {
        label: "Pending",
        tone: "muted" as const,
        iconBg: "bg-[#1a1f28] border-[#29303e] text-slate-300",
        textCls: "text-slate-200",
        sub: `Uptime: ${formatDurationSeconds(uptimeSeconds)}`,
      };
    }

    const up = formatDurationSeconds(uptimeSeconds);
    const cacheTag = healthCached ? "cached" : "live";
    const summarySub = `${healthReport.healthy_count}/${healthReport.total_count} healthy · ${up !== "Unavailable" ? `up ${up}` : "uptime N/A"} · ${cacheTag}`;

    if (healthReport.overall_status === "passed") {
      return {
        label: "Healthy",
        tone: "green" as const,
        iconBg: "bg-[#172312] border-[#2b421c] text-[#9be000]",
        textCls: "text-[#9be000]",
        sub: summarySub,
      };
    }
    if (healthReport.overall_status === "failed") {
      return {
        label: "Attention Needed",
        tone: "red" as const,
        iconBg: "bg-[#2a1518] border-[#4b2126] text-red-400",
        textCls: "text-red-400",
        sub: summarySub,
      };
    }
    return {
      label: "Degraded",
      tone: "amber" as const,
      iconBg: "bg-[#282112] border-[#493b1a] text-amber-400",
      textCls: "text-amber-300",
      sub: summarySub,
    };
  }, [healthReport, healthRefreshing, healthCached, uptimeSeconds]);

  const cpuPct = telemetry?.ready ? Math.round(telemetry.cpu.utilizationPct) : null;
  const cpuTemp = telemetry?.ready && telemetry.cpu.tempC !== null ? `${telemetry.cpu.tempC} °C` : null;
  const cpuModel = systemInfo?.processorGraphics.cpuModel ?? "Detecting CPU...";

  let gpuName = systemInfo?.processorGraphics.discreteGpu ?? "Detecting GPU...";
  let gpuMetricText = "--";
  let gpuFillPct = 0;
  if (telemetry?.ready) {
    const g = telemetry.gpu;
    if (g.state === "active") {
      gpuName = `${gpuName} · ${g.tempC} °C`;
      gpuMetricText = g.vramTotalMb > 0 ? `${g.vramUsedMb} MB` : `${g.utilizationPct}%`;
      gpuFillPct =
        g.vramTotalMb > 0
          ? Math.min(100, Math.round((g.vramUsedMb / g.vramTotalMb) * 100))
          : g.utilizationPct;
    } else if (g.state === "partial") {
      gpuName = `${gpuName} · ${g.tempC} °C`;
      gpuMetricText = "Temp Only";
      gpuFillPct = Math.min(100, g.tempC);
    } else if (g.state === "sleeping") {
      gpuMetricText = "Sleeping";
      gpuFillPct = 0;
    } else {
      gpuMetricText = "Unavailable";
      gpuFillPct = 0;
    }
  }

  const memPct =
    telemetry?.ready && telemetry.memory.ramTotalMb > 0
      ? Math.round(telemetry.memory.ramPercent)
      : null;
  const memDetail =
    telemetry?.ready && telemetry.memory.ramTotalMb > 0
      ? `${(telemetry.memory.ramUsedMb / 1024).toFixed(1)} / ${(telemetry.memory.ramTotalMb / 1024).toFixed(1)} GB`
      : systemInfo?.memoryStorage.ramTotalDisplay ?? "Unavailable";

  const diskPct =
    telemetry?.ready && telemetry.memory.diskTotalGb > 0
      ? Math.round(telemetry.memory.diskPercent)
      : null;
  const diskDetail =
    telemetry?.ready && telemetry.memory.diskTotalGb > 0
      ? `${telemetry.memory.diskUsedGb} / ${telemetry.memory.diskTotalGb} GB (${systemInfo?.memoryStorage.rootFilesystem.split(" ")[0] ?? "Root"})`
      : systemInfo?.memoryStorage.rootFilesystem ?? "Unavailable";

  const laptopLabel =
    systemInfo?.hardware.productModel && systemInfo.hardware.productModel !== "Unavailable"
      ? `${systemInfo.hardware.manufacturer !== "Unavailable" ? `${systemInfo.hardware.manufacturer} ` : ""}${systemInfo.hardware.productModel}`
      : healthReport?.hardware_model || "Linux System";

  const subById = (id: string) =>
    healthReport?.subsystems.find((s) => s.id === id);

  const acpiSub = subById("acpi_backend");
  const dkmsSub = subById("dkms");
  const daemonSub = subById("daemon_dbus");

  const toneForSub = (
    status: string | undefined
  ): "green" | "blue" | "amber" | "red" | "muted" => {
    if (status === "passed") return "green";
    if (status === "warning") return "amber";
    if (status === "failed") return "red";
    if (status === "info") return "blue";
    return "muted";
  };

  const updatePillState = useMemo(() => {
    if (updatesChecking && !updateResult) {
      return { text: "Checking...", tone: "muted" as const };
    }
    if (!updateResult) {
      return { text: `v${appVersion}`, tone: "blue" as const };
    }
    if (updateResult.error) {
      return { text: "Offline", tone: "amber" as const };
    }
    if (updateResult.update_available && updateResult.latest_version) {
      return { text: `v${updateResult.latest_version}`, tone: "blue" as const };
    }
    return {
      text: `v${updateResult.current_version}`,
      tone: "blue" as const,
    };
  }, [updatesChecking, updateResult, appVersion]);

  const displayedActivity = activity.slice(0, 4);

  return (
    <div className="uxf-tab-fade grid min-w-0 grid-cols-1 gap-4 lg:grid-cols-[minmax(0,0.90fr)_minmax(0,1.10fr)]">
      {/* Left Column */}
      <div className="flex min-w-0 flex-col gap-4">
        {/* Overall System Status Card */}
        <section className="flex items-center gap-4 rounded-2xl border border-[#1e232d] bg-[#12151c] px-5 py-4">
          <div
            className={`flex h-12 w-12 shrink-0 items-center justify-center rounded-full border ${overallView.iconBg}`}
          >
            {overallView.tone === "green" || overallView.tone === "muted" ? (
              <StatusCircleCheckIcon className="h-6 w-6" />
            ) : (
              <StatusAlertIcon className="h-6 w-6" />
            )}
          </div>
          <div className="min-w-0 flex-1">
            <div className="text-[13px] font-medium text-[#94a3b8]">
              Overall System Status
            </div>
            <div
              className={`mt-0.5 text-[28px] leading-tight font-bold tracking-tight ${overallView.textCls}`}
            >
              {overallView.label}
            </div>
            <div className="mt-1 truncate font-mono text-[12px] text-[#8b949e]">
              {overallView.sub}
            </div>
          </div>
        </section>

        {/* Hardware Summary Card */}
        <section className="flex flex-1 flex-col rounded-2xl border border-[#1e232d] bg-[#12151c] p-5">
          <div className="mb-2.5 flex items-baseline justify-between gap-3">
            <h2 className="text-[17px] font-semibold tracking-tight text-white whitespace-nowrap">
              Hardware Summary
            </h2>
            <span className="truncate font-mono text-[12px] text-[#8b949e]">
              {laptopLabel}
            </span>
          </div>

          <div className="flex flex-1 flex-col justify-between divide-y divide-[#1d222c]">
            <HardwareSummaryRow
              icon={<CpuChipIcon className="h-[18px] w-[18px]" />}
              iconTone="text-[#cbd5e1]"
              title="CPU"
              detail={`${cpuModel}${cpuTemp ? ` · ${cpuTemp}` : ""}`}
              metricText={cpuPct !== null ? `${cpuPct}%` : "--"}
              fillPct={Math.max(cpuPct ? 4 : 0, cpuPct ?? 0)}
            />
            <HardwareSummaryRow
              icon={<GpuCardIcon className="h-[18px] w-[18px]" />}
              iconTone="text-[#9be000]"
              title="GPU"
              detail={gpuName}
              metricText={gpuMetricText}
              fillPct={Math.max(gpuFillPct > 0 ? 5 : 0, gpuFillPct)}
            />
            <HardwareSummaryRow
              icon={<RamStickIcon className="h-[18px] w-[18px]" />}
              iconTone="text-[#9be000]"
              title="Memory"
              detail={memDetail}
              metricText={memPct !== null ? `${memPct}%` : "--"}
              fillPct={memPct ?? 0}
            />
            <HardwareSummaryRow
              icon={<StorageDriveIcon className="h-[18px] w-[18px]" />}
              iconTone="text-[#60a5fa]"
              title="Storage"
              detail={diskDetail}
              metricText={diskPct !== null ? `${diskPct}%` : "--"}
              fillPct={diskPct ?? 0}
              accentTone="blue"
            />
          </div>
        </section>
      </div>

      {/* Right Column */}
      <div className="flex min-w-0 flex-col gap-4">
        {/* Services & Drivers Card */}
        <section className="min-w-0 rounded-2xl border border-[#1e232d] bg-[#12151c] p-4 sm:p-5">
          <div className="mb-3.5 flex items-center justify-between gap-2">
            <h2 className="whitespace-nowrap text-[16.5px] font-semibold tracking-tight text-white">
              Services & Drivers
            </h2>
            <button
              type="button"
              onClick={() => onNavigateTab("health")}
              className="shrink-0 cursor-pointer whitespace-nowrap text-[12px] font-medium text-[#7dd3fc] transition-colors hover:text-[#bae6fd] focus:outline-none focus-visible:underline"
            >
              View Diagnostics
            </button>
          </div>

          <div className="grid min-w-0 grid-cols-1 gap-2 sm:grid-cols-2">
            <ServicePill
              label="Acer ACPI"
              statusText={acpiSub?.status_label ?? "Checking..."}
              tone={toneForSub(acpiSub?.status)}
            />
            <ServicePill
              label="DKMS Module"
              statusText={dkmsSub?.status_label ?? "Checking..."}
              tone={toneForSub(dkmsSub?.status)}
            />
            <ServicePill
              label="D-Bus Daemon"
              statusText={daemonSub?.status_label ?? "Checking..."}
              tone={toneForSub(daemonSub?.status)}
            />
            <ServicePill
              label="UXF Update"
              statusText={updatePillState.text}
              tone={updatePillState.tone}
            />
          </div>
        </section>

        {/* Recent Activity Card */}
        <section className="flex min-w-0 flex-1 flex-col rounded-2xl border border-[#1e232d] bg-[#12151c] p-4 sm:p-5">
          <div className="mb-3 flex items-baseline justify-between gap-2">
            <h2 className="text-[17px] font-semibold tracking-tight text-white">
              Recent Activity
            </h2>
            <span className="text-[12px] text-[#8b949e]">Session Log</span>
          </div>

          <div className="flex flex-1 flex-col divide-y divide-[#1d222c]">
            {displayedActivity.length === 0 ? (
              <div className="flex flex-1 items-center justify-center py-6 text-[13px] text-[#8b949e]">
                Recording system events...
              </div>
            ) : (
              displayedActivity.map((item) => {
                const dotTone =
                  item.tone === "healthy"
                    ? "green"
                    : item.tone === "info"
                      ? "blue"
                      : item.tone === "warning"
                        ? "amber"
                        : "red";
                const timeText = item.timeLabel ?? formatRelativeTime(item.timestampMs);
                return (
                  <div
                    key={item.id}
                    className="flex items-start gap-3 py-2.5 first:pt-1 last:pb-0.5"
                  >
                    <div className="mt-1.5">
                      <StatusDot tone={dotTone} />
                    </div>
                    <div className="min-w-0 flex-1">
                      <p className="text-[13.5px] leading-snug text-slate-200">
                        {item.message}
                      </p>
                      <p className="mt-1 font-mono text-[11.5px] text-[#7c8594]">
                        {timeText} · {item.source}
                      </p>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
