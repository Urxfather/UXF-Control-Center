import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SystemHealthReport, SystemInformationReport } from "../../types/health";
import { copyToClipboard } from "../../hooks/useSystemCenter";
import { CopyIcon } from "./SystemCenterIcons";

interface AboutTabProps {
  appVersion: string;
  healthReport: SystemHealthReport | null;
  systemInfo: SystemInformationReport | null;
}

const REPO_URL = "https://github.com/Urxfather/UXF-Control-Center";
const ISSUES_URL = "https://github.com/Urxfather/UXF-Control-Center/issues";

export function AboutTab({ appVersion, healthReport, systemInfo }: AboutTabProps) {
  const [copied, setCopied] = useState(false);

  const cleanVersion = appVersion.replace(/^v/i, "");

  const handleOpenExternal = (url: string) => {
    void invoke("open_external_url", { url }).catch((err) => {
      console.warn("open_external_url failed:", err);
      window.open(url, "_blank", "noopener,noreferrer");
    });
  };

  const handleCopyVersion = async () => {
    const lines = [
      `UXF Control Center v${cleanVersion}`,
      `Architecture: Tauri v2 + React 19 + Rust`,
      `Daemon Version: ${healthReport?.daemon_version ?? "Unavailable"}`,
      `ACPI Backend: ${healthReport?.acpi_interface ?? "Unavailable"}`,
      `OS: ${systemInfo?.osKernel.distribution ?? "Linux"}`,
      `Kernel: ${systemInfo?.osKernel.kernelRelease ?? healthReport?.kernel_version ?? "Unknown"}`,
      `Repository: ${REPO_URL}`,
      `License: GPL-3.0-or-later`,
    ].join("\n");
    const ok = await copyToClipboard(lines);
    if (ok) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="uxf-tab-fade space-y-5">
      {/* Top Identity Banner Card */}
      <section className="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-[#1e232d] bg-[#12151c] p-5 sm:flex-nowrap sm:p-6">
        <div className="flex min-w-0 items-center gap-4">
          <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl border border-[#28303e] bg-[#181c25] shadow-inner sm:h-16 sm:w-16">
            <span className="text-[18px] font-bold tracking-tight text-[#9be000] sm:text-[19px]">
              UXF
            </span>
          </div>

          <div className="min-w-0">
            <h2 className="text-[22px] font-bold tracking-tight text-white sm:text-[24px]">
              UXF Control Center
            </h2>
            <p className="mt-0.5 font-mono text-[12.5px] font-medium text-[#7dd3fc] sm:text-[13px]">
              Version v{cleanVersion} · Tauri + React + Rust
            </p>
            <p className="mt-1 text-[13px] text-[#94a3b8]">
              Linux hardware telemetry, cooling control, RGB lighting, and ACPI diagnostics utility.
            </p>
          </div>
        </div>

        <button
          type="button"
          onClick={() => void handleCopyVersion()}
          className="inline-flex shrink-0 cursor-pointer items-center gap-2 whitespace-nowrap rounded-full border border-[#2a3140] bg-[#1d222c] px-4 py-2 text-[12.5px] font-medium text-slate-200 transition-colors hover:border-[#3b465b] hover:bg-[#252b38] hover:text-white focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff]"
        >
          <CopyIcon className="h-4 w-4 text-slate-300" />
          <span>{copied ? "Copied!" : "Copy Version Info"}</span>
        </button>
      </section>

      {/* Bottom 2-Column Details */}
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-2">
        {/* Project & Developer */}
        <section className="min-w-0 rounded-2xl border border-[#1e232d] bg-[#12151c] p-6">
          <h3 className="mb-4 text-[18px] font-semibold tracking-tight text-white">
            Project & Developer
          </h3>

          <div className="divide-y divide-[#1d222c] text-[13px]">
            <div className="flex items-baseline justify-between gap-4 py-3 first:pt-1">
              <span className="text-[#94a3b8]">Maintainer</span>
              <span className="font-medium text-slate-100">Urxfather (UXF⚡)</span>
            </div>

            <div className="flex items-baseline justify-between gap-3 py-3">
              <span className="shrink-0 text-[#94a3b8]">Repository</span>
              <button
                type="button"
                onClick={() => handleOpenExternal(REPO_URL)}
                className="min-w-0 cursor-pointer truncate text-right font-mono text-[11.5px] text-[#7dd3fc] transition-colors hover:text-[#bae6fd] hover:underline focus:outline-none"
                title={REPO_URL}
              >
                github.com/Urxfather/UXF-Control-Center
              </button>
            </div>

            <div className="flex items-baseline justify-between gap-3 py-3 last:pb-1">
              <span className="shrink-0 text-[#94a3b8]">Issue Tracker</span>
              <button
                type="button"
                onClick={() => handleOpenExternal(ISSUES_URL)}
                className="min-w-0 cursor-pointer truncate text-right font-mono text-[11.5px] text-[#7dd3fc] transition-colors hover:text-[#bae6fd] hover:underline focus:outline-none"
                title={ISSUES_URL}
              >
                /Urxfather/UXF-Control-Center/issues
              </button>
            </div>
          </div>
        </section>

        {/* License & Acknowledgements */}
        <section className="rounded-2xl border border-[#1e232d] bg-[#12151c] p-6">
          <h3 className="mb-4 text-[18px] font-semibold tracking-tight text-white">
            License & Acknowledgements
          </h3>

          <p className="text-[13.5px] font-medium text-slate-100">
            Licensed under the{" "}
            <span className="font-mono text-[#9be000]">GPL-3.0-or-later</span>{" "}
            (GNU General Public License v3.0 or later).
          </p>

          <p className="mt-3 text-[13px] leading-relaxed text-[#94a3b8]">
            Built with Tauri v2, React, TypeScript, and Rust. Derived upstream from{" "}
            <span className="font-mono text-slate-300">acer-control-center</span> by
            Stanly Silas & contributors under GPL-3.0-or-later. Uses Lucide iconography,
            Inter & JetBrains Mono typography, and Linux kernel{" "}
            <span className="font-mono text-slate-300">acer_acpi</span> /{" "}
            <span className="font-mono text-slate-300">acer_io</span> subsystems.
          </p>
        </section>
      </div>
    </div>
  );
}
