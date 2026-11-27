import { useState } from "react";
import type { HealthStatus, SystemHealthReport } from "../../types/health";
import { formatClockUtc, formatUtcTimestamp } from "../../hooks/useSystemCenter";
import { RefreshIcon } from "./SystemCenterIcons";

interface HealthTabProps {
  report: SystemHealthReport | null;
  cached: boolean;
  refreshing: boolean;
  checkedAtMs: number | null;
  error: string | null;
  onRefresh: () => void;
}

function statusColors(status: HealthStatus) {
  switch (status) {
    case "passed":
      return {
        dot: "bg-[#9be000]",
        text: "text-[#9be000]",
        label: "Passed",
      };
    case "warning":
      return {
        dot: "bg-amber-400",
        text: "text-amber-300",
        label: "Warning",
      };
    case "failed":
      return {
        dot: "bg-red-500",
        text: "text-red-400",
        label: "Failed",
      };
    case "info":
      return {
        dot: "bg-[#60a5fa]",
        text: "text-[#7dd3fc]",
        label: "Info",
      };
  }
}

export function HealthTab({
  report,
  cached,
  refreshing,
  checkedAtMs,
  error,
  onRefresh,
}: HealthTabProps) {
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const sublineText = report
    ? `Last diagnostic run: ${formatUtcTimestamp(checkedAtMs ?? report.checked_at_ms)} · ${report.healthy_count}/${report.total_count} subsystems healthy`
    : refreshing
      ? "Running asynchronous hardware & subsystem diagnostics..."
      : "No diagnostic scan completed yet";

  return (
    <div className="uxf-tab-fade space-y-4">
      {/* Header Row */}
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <div className="flex items-center gap-2.5">
            <h2 className="text-[22px] font-semibold tracking-tight text-white">
              System Health
            </h2>
            {report && (
              <span
                className={`rounded-full border px-2.5 py-0.5 font-mono text-[10.5px] font-medium ${
                  refreshing
                    ? "border-[#7dd3fc]/30 bg-[#142233] text-[#7dd3fc]"
                    : cached
                      ? "border-[#2a3140] bg-[#161b24] text-[#94a3b8]"
                      : "border-[#9be000]/30 bg-[#182412] text-[#9be000]"
                }`}
              >
                {refreshing ? "Refreshing..." : cached ? "Cached" : "Fresh"}
              </span>
            )}
          </div>
          <p className="mt-1 font-mono text-[12.5px] text-[#8b949e]">
            {sublineText}
          </p>
        </div>

        <button
          type="button"
          onClick={onRefresh}
          disabled={refreshing}
          className="inline-flex cursor-pointer items-center gap-2 rounded-full bg-[#93c5fd] px-4 py-2 text-[13px] font-semibold text-[#081018] shadow-sm transition-colors hover:bg-[#bfdbfe] active:scale-[0.98] disabled:cursor-wait disabled:opacity-70 focus:outline-none focus-visible:ring-2 focus-visible:ring-[#00d8ff]"
        >
          <RefreshIcon className={`h-4 w-4 ${refreshing ? "animate-spin" : ""}`} />
          <span>{refreshing ? "Refreshing..." : "Refresh Diagnostics"}</span>
        </button>
      </div>

      {error && (
        <div className="rounded-xl border border-red-500/35 bg-red-950/25 px-4 py-3 text-[13px] text-red-300">
          Diagnostic probe encountered an error: {error}
        </div>
      )}

      {/* 6 Subsystem Cards (3x2 Grid) */}
      <div className="grid grid-cols-1 gap-3.5 md:grid-cols-2 lg:grid-cols-3">
        {!report
          ? Array.from({ length: 6 }, (_, idx) => (
              <div
                key={idx}
                className="min-h-[108px] animate-pulse rounded-2xl border border-[#1e232d] bg-[#12151c] p-4"
              >
                <div className="flex items-center justify-between">
                  <div className="h-4 w-32 rounded bg-[#1d222e]" />
                  <div className="h-3.5 w-16 rounded bg-[#1d222e]" />
                </div>
                <div className="mt-4 h-3 w-full rounded bg-[#1a1f29]" />
                <div className="mt-2 h-3 w-3/4 rounded bg-[#1a1f29]" />
              </div>
            ))
          : report.subsystems.map((sub) => {
              const colors = statusColors(sub.status);
              return (
                <div
                  key={sub.id}
                  className="flex min-h-[108px] flex-col justify-between rounded-2xl border border-[#1e232d] bg-[#12151c] p-4"
                >
                  <div className="flex items-center justify-between gap-2">
                    <h3 className="text-[13.5px] leading-snug font-semibold text-white">
                      {sub.title}
                    </h3>
                    <span
                      className={`inline-flex shrink-0 items-center gap-1.5 text-[12px] font-semibold ${colors.text}`}
                    >
                      <span className={`h-2 w-2 rounded-full ${colors.dot}`} />
                      {sub.status_label}
                    </span>
                  </div>
                  <p className="mt-2.5 text-[12.5px] leading-relaxed text-[#94a3b8]">
                    {sub.description}
                  </p>
                </div>
              );
            })}
      </div>

      {/* Detailed Diagnostic Results Table */}
      <div className="space-y-2.5 pt-1">
        <div className="flex items-baseline justify-between gap-2">
          <h3 className="text-[18px] font-semibold tracking-tight text-white">
            Diagnostic Results
          </h3>
          <span className="text-[12px] text-[#6b7280]">
            Click any row to inspect verbose output
          </span>
        </div>

        <div className="overflow-hidden rounded-2xl border border-[#1e232d] bg-[#12151c]">
          <div className="overflow-x-auto">
            <table className="w-full border-collapse text-left">
              <thead>
                <tr className="border-b border-[#1e232d] bg-[#161a22] text-[12px] font-medium text-[#8b949e]">
                  <th className="px-5 py-3.5">Diagnostic Check</th>
                  <th className="px-4 py-3.5 w-28">Status</th>
                  <th className="px-4 py-3.5">Details</th>
                  <th className="px-5 py-3.5 text-right w-32">Timestamp</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-[#1b2029] text-[13px]">
                {!report ? (
                  <tr>
                    <td
                      colSpan={4}
                      className="px-5 py-8 text-center text-[#8b949e]"
                    >
                      Running system diagnostics...
                    </td>
                  </tr>
                ) : (
                  report.items.map((item) => {
                    const colors = statusColors(item.status);
                    const isExpanded = expandedId === item.id;
                    return (
                      <tr
                        key={item.id}
                        onClick={() =>
                          setExpandedId((prev) => (prev === item.id ? null : item.id))
                        }
                        className="group cursor-pointer transition-colors hover:bg-[#161a22]/75"
                      >
                        <td className="px-5 py-3.5 align-top font-medium text-slate-200">
                          {item.name}
                        </td>
                        <td className={`px-4 py-3.5 align-top font-semibold ${colors.text}`}>
                          {colors.label}
                        </td>
                        <td className="px-4 py-3.5 align-top font-mono text-[12px] text-[#94a3b8]">
                          <div className="text-slate-300">{item.summary}</div>
                          {isExpanded && (
                            <div className="mt-2 rounded-lg border border-[#222834] bg-[#0b0e14] p-3 text-[11.5px] leading-relaxed text-[#94a3b8] whitespace-pre-wrap select-text">
                              {item.detail}
                            </div>
                          )}
                          {item.repair_action && (
                            <div className="mt-2 inline-block rounded border border-amber-500/30 bg-amber-950/25 px-2.5 py-1 text-[11px] text-amber-200 select-text">
                              Repair: {item.repair_action}
                            </div>
                          )}
                        </td>
                        <td className="px-5 py-3.5 align-top text-right font-mono text-[12px] text-[#8b949e] tabular-nums">
                          {formatClockUtc(item.checked_at_ms)}
                        </td>
                      </tr>
                    );
                  })
                )}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  );
}
