import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CloseIcon, InfoIcon } from "./NavIcons";
import type { SystemHealthReport, UpdateCheckResult } from "../types/health";

interface AboutModalProps {
  open: boolean;
  appVersion: string;
  onClose: () => void;
}

export function AboutModal({ open, appVersion, onClose }: AboutModalProps) {
  const [activeTab, setActiveTab] = useState<"about" | "health" | "updates">("about");
  const [healthReport, setHealthReport] = useState<SystemHealthReport | null>(null);
  const [healthLoading, setHealthLoading] = useState(false);
  const [updateResult, setUpdateResult] = useState<UpdateCheckResult | null>(null);
  const [updateLoading, setUpdateLoading] = useState(false);

  useEffect(() => {
    if (!open) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, onClose]);

  const runHealthCheck = async () => {
    setHealthLoading(true);
    try {
      const report = await invoke<SystemHealthReport>("get_system_health");
      setHealthReport(report);
    } catch (err) {
      console.error("Health check error:", err);
    } finally {
      setHealthLoading(false);
    }
  };

  const runUpdateCheck = async () => {
    setUpdateLoading(true);
    try {
      const result = await invoke<UpdateCheckResult>("check_updates", {
        currentVersion: appVersion,
      });
      setUpdateResult(result);
    } catch (err) {
      console.error("Update check error:", err);
    } finally {
      setUpdateLoading(false);
    }
  };

  useEffect(() => {
    if (open) {
      if (activeTab === "health" && !healthReport) {
        void runHealthCheck();
      } else if (activeTab === "updates" && !updateResult) {
        void runUpdateCheck();
      }
    }
  }, [open, activeTab]);

  if (!open) {
    return null;
  }

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="about-dialog-title"
      onClick={onClose}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-xs select-none"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-xl rounded-xl border border-[#262b36] bg-[#11141a] p-5 shadow-2xl transition-all"
      >
        {/* Header */}
        <div className="flex items-start justify-between border-b border-[#262b36] pb-3.5">
          <div className="flex items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-lg border border-[#00d8ff]/40 bg-[#00d8ff]/10 text-[#00d8ff]">
              <InfoIcon className="h-5 w-5" />
            </span>
            <div>
              <h2
                id="about-dialog-title"
                className="text-base font-semibold tracking-wide text-white"
              >
                UXF-Control-Center
              </h2>
              <p className="text-xs text-[#9ca3af]">
                Modern Linux Control Center for Acer Gaming Laptops
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={onClose}
            aria-label="Close dialog"
            className="flex h-7 w-7 items-center justify-center rounded-md text-slate-400 transition-colors hover:bg-[#191f2a] hover:text-white focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff]"
          >
            <CloseIcon className="h-4 w-4" />
          </button>
        </div>

        {/* Navigation Tabs */}
        <div className="mt-3.5 flex border-b border-[#1f242d] text-xs">
          <button
            type="button"
            onClick={() => setActiveTab("about")}
            className={`border-b-2 px-3.5 py-1.5 font-medium transition-colors ${
              activeTab === "about"
                ? "border-[#00d8ff] text-white"
                : "border-transparent text-slate-400 hover:text-slate-200"
            }`}
          >
            Overview
          </button>
          <button
            type="button"
            onClick={() => setActiveTab("health")}
            className={`flex items-center gap-1.5 border-b-2 px-3.5 py-1.5 font-medium transition-colors ${
              activeTab === "health"
                ? "border-[#00d8ff] text-white"
                : "border-transparent text-slate-400 hover:text-slate-200"
            }`}
          >
            System Health
            {healthReport?.overall_status === "failed" && (
              <span className="h-2 w-2 rounded-full bg-red-500" />
            )}
            {healthReport?.overall_status === "warning" && (
              <span className="h-2 w-2 rounded-full bg-amber-400" />
            )}
            {healthReport?.overall_status === "passed" && (
              <span className="h-2 w-2 rounded-full bg-[#9be000]" />
            )}
          </button>
          <button
            type="button"
            onClick={() => setActiveTab("updates")}
            className={`flex items-center gap-1.5 border-b-2 px-3.5 py-1.5 font-medium transition-colors ${
              activeTab === "updates"
                ? "border-[#00d8ff] text-white"
                : "border-transparent text-slate-400 hover:text-slate-200"
            }`}
          >
            Updates
            {updateResult?.update_available && (
              <span className="h-2 w-2 rounded-full bg-[#00d8ff] animate-pulse" />
            )}
          </button>
        </div>

        {/* Tab 1: About Overview */}
        {activeTab === "about" && (
          <div className="mt-4 space-y-2.5 text-xs">
            <div className="flex items-center justify-between rounded-lg border border-[#262b36] bg-[#0c0f14] px-3.5 py-2.5">
              <span className="text-[#9ca3af]">Installed Version</span>
              <span className="font-mono font-semibold text-[#9be000]">
                v{appVersion}
              </span>
            </div>
            <div className="flex items-center justify-between rounded-lg border border-[#262b36] bg-[#0c0f14] px-3.5 py-2.5">
              <span className="text-[#9ca3af]">UI Architecture</span>
              <span className="text-slate-200">Tauri v2 + React 19 + Vite</span>
            </div>
            <div className="flex items-center justify-between rounded-lg border border-[#262b36] bg-[#0c0f14] px-3.5 py-2.5">
              <span className="text-[#9ca3af]">Maintainer & Creator</span>
              <span className="font-medium text-slate-100">Urxfather (UXF⚡)</span>
            </div>
            <div className="flex items-center justify-between rounded-lg border border-[#262b36] bg-[#0c0f14] px-3.5 py-2.5">
              <span className="text-[#9ca3af]">License & Attribution</span>
              <span className="text-slate-200">GPL-3.0-or-later</span>
            </div>
            <div className="rounded-lg border border-[#1f242d] bg-[#0c0f14] p-3 text-[11.5px] leading-relaxed text-slate-400">
              Native control center for Acer gaming laptops on Fedora Linux. Provides
              hardware cooling controls, RGB backlight effects, display and audio
              integration, and DKMS kernel driver management.
            </div>
          </div>
        )}

        {/* Tab 2: System Health */}
        {activeTab === "health" && (
          <div className="mt-4 max-h-72 space-y-2 overflow-y-auto pr-1 text-xs">
            {healthLoading ? (
              <div className="flex h-32 items-center justify-center text-slate-400">
                Scanning hardware and driver subsystems...
              </div>
            ) : healthReport ? (
              <>
                <div className="mb-2 flex items-center justify-between rounded-lg border border-[#262b36] bg-[#0c0f14] px-3 py-2">
                  <div>
                    <span className="text-slate-400">Hardware: </span>
                    <span className="font-medium text-white">
                      {healthReport.hardware_model}
                    </span>
                  </div>
                  <span
                    className={`rounded px-2 py-0.5 text-[10px] font-semibold uppercase ${
                      healthReport.overall_status === "passed"
                        ? "bg-[#9be000]/15 text-[#9be000]"
                        : healthReport.overall_status === "warning"
                        ? "bg-amber-400/15 text-amber-300"
                        : "bg-red-500/15 text-red-400"
                    }`}
                  >
                    {healthReport.overall_status}
                  </span>
                </div>

                {healthReport.items.map((item) => (
                  <div
                    key={item.id}
                    className="rounded-lg border border-[#262b36] bg-[#0c0f14] p-2.5"
                  >
                    <div className="flex items-center justify-between">
                      <span className="font-semibold text-slate-200">
                        {item.name}
                      </span>
                      <span
                        className={`text-[10px] font-semibold uppercase ${
                          item.status === "passed"
                            ? "text-[#9be000]"
                            : item.status === "warning"
                            ? "text-amber-400"
                            : item.status === "failed"
                            ? "text-red-400"
                            : "text-sky-400"
                        }`}
                      >
                        {item.status}
                      </span>
                    </div>
                    <p className="mt-1 text-slate-300">{item.summary}</p>
                    <pre className="mt-1.5 font-mono text-[10px] text-slate-400 whitespace-pre-wrap">
                      {item.detail}
                    </pre>
                    {item.repair_action && (
                      <div className="mt-2 rounded border border-[#3b2b1a] bg-[#1a140d] px-2 py-1 text-[10px] font-mono text-amber-200">
                        Suggested repair: {item.repair_action}
                      </div>
                    )}
                  </div>
                ))}
              </>
            ) : (
              <div className="text-center text-slate-400">
                No health report available.
              </div>
            )}
          </div>
        )}

        {/* Tab 3: Updates */}
        {activeTab === "updates" && (
          <div className="mt-4 space-y-3 text-xs">
            {updateLoading ? (
              <div className="flex h-32 items-center justify-center text-slate-400">
                Checking GitHub Releases for updates...
              </div>
            ) : updateResult ? (
              <>
                <div className="rounded-lg border border-[#262b36] bg-[#0c0f14] p-3">
                  <div className="flex items-center justify-between">
                    <span className="text-slate-400">Installed version:</span>
                    <span className="font-mono text-slate-200">
                      v{updateResult.current_version}
                    </span>
                  </div>
                  <div className="mt-1 flex items-center justify-between">
                    <span className="text-slate-400">Latest release:</span>
                    <span className="font-mono font-semibold text-[#00d8ff]">
                      {updateResult.latest_version
                        ? `v${updateResult.latest_version}`
                        : "Checking..."}
                    </span>
                  </div>
                </div>

                {updateResult.update_available ? (
                  <div className="rounded-lg border border-[#00d8ff]/30 bg-[#00d8ff]/10 p-3">
                    <p className="font-semibold text-white">
                      An update is available!
                    </p>
                    <p className="mt-1 text-slate-300">
                      {updateResult.has_dnf_repo
                        ? "To update safely through Fedora package management, run:"
                        : "Download the RPM asset from GitHub Releases and install with DNF:"}
                    </p>
                    <div className="mt-2 overflow-x-auto rounded border border-[#262b36] bg-black/60 p-2 font-mono text-[10.5px] text-[#9be000] select-text cursor-text">
                      {updateResult.update_command}
                    </div>
                  </div>
                ) : (
                  <div className="rounded-lg border border-[#9be000]/30 bg-[#9be000]/10 p-3 text-[#9be000]">
                    You are running the latest version (v{updateResult.current_version}).
                  </div>
                )}

                {updateResult.error && (
                  <div className="rounded border border-red-500/30 bg-red-500/10 p-2 text-xs text-red-300">
                    {updateResult.error}
                  </div>
                )}
              </>
            ) : null}
          </div>
        )}

        {/* Footer Actions */}
        <div className="mt-5 flex items-center justify-between border-t border-[#262b36] pt-3.5">
          <div>
            {activeTab === "health" && (
              <button
                type="button"
                onClick={runHealthCheck}
                disabled={healthLoading}
                className="rounded border border-[#262b36] bg-[#171c25] px-3 py-1.5 text-xs text-slate-200 transition-colors hover:border-[#00d8ff]/50 hover:text-white disabled:opacity-50"
              >
                {healthLoading ? "Scanning..." : "Re-scan Health"}
              </button>
            )}
            {activeTab === "updates" && (
              <button
                type="button"
                onClick={runUpdateCheck}
                disabled={updateLoading}
                className="rounded border border-[#262b36] bg-[#171c25] px-3 py-1.5 text-xs text-slate-200 transition-colors hover:border-[#00d8ff]/50 hover:text-white disabled:opacity-50"
              >
                {updateLoading ? "Checking..." : "Check Again"}
              </button>
            )}
          </div>

          <button
            type="button"
            onClick={onClose}
            className="rounded border border-[#262b36] bg-[#171c25] px-4 py-1.5 text-xs font-medium text-slate-200 transition-colors hover:border-[#00d8ff]/50 hover:text-white focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff]"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
