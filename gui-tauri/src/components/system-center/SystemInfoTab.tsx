import { useState } from "react";
import type { SystemInformationReport } from "../../types/health";
import { copyToClipboard } from "../../hooks/useSystemCenter";
import {
  CopyIcon,
  CpuChipIcon,
  MonitorLaptopIcon,
  StorageDriveIcon,
  TerminalPromptIcon,
} from "./SystemCenterIcons";

interface SystemInfoTabProps {
  info: SystemInformationReport | null;
  loading: boolean;
  appVersion: string;
}

function InfoRow({
  label,
  value,
  valueClassName = "text-slate-100",
}: {
  label: string;
  value: string;
  valueClassName?: string;
}) {
  return (
    <div className="flex items-baseline justify-between gap-3 py-2.5 first:pt-1 last:pb-0.5">
      <span className="shrink-0 text-[12.5px] text-[#94a3b8]">{label}</span>
      <span
        className={`text-right font-mono text-[12px] ${valueClassName}`}
      >
        {value}
      </span>
    </div>
  );
}

export function SystemInfoTab({ info, loading, appVersion }: SystemInfoTabProps) {
  const [copied, setCopied] = useState(false);

  const formatReportText = () => {
    if (!info) return "";
    const pg = info.processorGraphics;
    const coresLabel =
      pg.physicalCores && pg.logicalThreads
        ? `${pg.physicalCores} Cores / ${pg.logicalThreads} Threads${
            pg.maxClockGhz ? ` (${pg.maxClockGhz.toFixed(2)} GHz max)` : ""
          }`
        : "Unavailable";

    return [
      `UXF-Control-Center System Report (v${appVersion})`,
      `==================================================`,
      `Manufacturer:     ${info.hardware.manufacturer}`,
      `Product Model:    ${info.hardware.productModel}`,
      `BIOS Version:     ${info.hardware.biosVersion}${
        info.hardware.biosDate ? ` (${info.hardware.biosDate})` : ""
      }`,
      `Boot Mode:        ${info.hardware.bootMode} (Secure Boot: ${info.hardware.secureBoot})`,
      `Distribution:     ${info.osKernel.distribution}`,
      `Kernel Release:   ${info.osKernel.kernelRelease}`,
      `Display Server:   ${info.osKernel.displayServer}`,
      `Hostname:         ${info.osKernel.hostname}`,
      `CPU:              ${pg.cpuModel}`,
      `CPU Topology:     ${coresLabel}`,
      `Discrete GPU:     ${pg.discreteGpu}${
        pg.discreteGpuDriver ? ` [${pg.discreteGpuDriver}]` : ""
      }`,
      `Integrated GPU:   ${pg.integratedGpu}`,
      `Installed RAM:    ${info.memoryStorage.ramTotalDisplay}`,
      `Swap:             ${info.memoryStorage.swapDisplay}`,
      `Primary Storage:  ${info.memoryStorage.primaryDrive}`,
      `Root Filesystem:  ${info.memoryStorage.rootFilesystem}`,
    ].join("\n");
  };

  const handleCopyReport = async () => {
    const text = formatReportText();
    if (!text) return;
    const ok = await copyToClipboard(text);
    if (ok) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2200);
    }
  };

  const pg = info?.processorGraphics;
  const coresDisplay =
    pg?.physicalCores && pg?.logicalThreads
      ? `${pg.physicalCores} Cores / ${pg.logicalThreads} Threads${
          pg.maxClockGhz ? ` · ${pg.maxClockGhz.toFixed(1)} GHz` : ""
        }`
      : pg?.logicalThreads
        ? `${pg.logicalThreads} Threads`
        : "Unavailable";

  const dgpuDisplay = pg
    ? `${pg.discreteGpu}${pg.discreteGpuDriver ? ` (${pg.discreteGpuDriver})` : ""}`
    : "Detecting...";

  const bootDisplay = info
    ? `${info.hardware.bootMode} (Secure Boot ${info.hardware.secureBoot})`
    : "Detecting...";

  const bootColor =
    info?.hardware.secureBoot === "Enabled" || info?.hardware.bootMode === "UEFI"
      ? "text-[#9be000]"
      : "text-slate-100";

  return (
    <div className="uxf-tab-fade space-y-4">
      {/* Header Row */}
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="text-[22px] font-semibold tracking-tight text-white">
            System Information
          </h2>
          <p className="mt-0.5 text-[12.5px] text-[#8b949e]">
            Hardware specifications, firmware, and Linux kernel environment
          </p>
        </div>

        <button
          type="button"
          onClick={() => void handleCopyReport()}
          disabled={!info}
          className="inline-flex cursor-pointer items-center gap-2 rounded-full border border-[#2a3140] bg-[#1a1f29] px-4 py-1.5 text-[12.5px] font-medium text-slate-200 transition-colors hover:border-[#3b465b] hover:bg-[#232936] hover:text-white disabled:opacity-50 focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff]"
        >
          <CopyIcon className="h-4 w-4 text-slate-300" />
          <span>{copied ? "Copied Report!" : "Copy System Report"}</span>
        </button>
      </div>

      {/* 2x2 Bento Grid */}
      {!info && loading ? (
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          {Array.from({ length: 4 }, (_, idx) => (
            <div
              key={idx}
              className="h-52 animate-pulse rounded-2xl border border-[#1e232d] bg-[#12151c] p-5"
            />
          ))}
        </div>
      ) : info ? (
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          {/* Card 1: Hardware Model & BIOS */}
          <section className="rounded-2xl border border-[#1e232d] bg-[#12151c] p-5">
            <div className="mb-3 flex items-center gap-2.5">
              <MonitorLaptopIcon className="h-4.5 w-4.5 text-[#7dd3fc]" />
              <h3 className="text-[16px] font-semibold text-white">
                Hardware Model & BIOS
              </h3>
            </div>
            <div className="divide-y divide-[#1d222c]">
              <InfoRow label="Manufacturer" value={info.hardware.manufacturer} />
              <InfoRow label="Product Model" value={info.hardware.productModel} />
              <InfoRow
                label="BIOS Version"
                value={
                  info.hardware.biosDate
                    ? `${info.hardware.biosVersion} (${info.hardware.biosDate})`
                    : info.hardware.biosVersion
                }
              />
              <InfoRow
                label="Boot Mode"
                value={bootDisplay}
                valueClassName={bootColor}
              />
            </div>
          </section>

          {/* Card 2: Operating System & Kernel */}
          <section className="rounded-2xl border border-[#1e232d] bg-[#12151c] p-5">
            <div className="mb-3 flex items-center gap-2.5">
              <TerminalPromptIcon className="h-4.5 w-4.5 text-[#38bdf8]" />
              <h3 className="text-[16px] font-semibold text-white">
                Operating System & Kernel
              </h3>
            </div>
            <div className="divide-y divide-[#1d222c]">
              <InfoRow label="Distribution" value={info.osKernel.distribution} />
              <InfoRow label="Kernel Release" value={info.osKernel.kernelRelease} />
              <InfoRow label="Display Server" value={info.osKernel.displayServer} />
              <InfoRow label="Hostname" value={info.osKernel.hostname} />
            </div>
          </section>

          {/* Card 3: Processor & Graphics */}
          <section className="rounded-2xl border border-[#1e232d] bg-[#12151c] p-5">
            <div className="mb-3 flex items-center gap-2.5">
              <CpuChipIcon className="h-4.5 w-4.5 text-[#9be000]" />
              <h3 className="text-[16px] font-semibold text-white">
                Processor & Graphics
              </h3>
            </div>
            <div className="divide-y divide-[#1d222c]">
              <InfoRow label="CPU" value={info.processorGraphics.cpuModel} />
              <InfoRow label="Cores / Max Clock" value={coresDisplay} />
              <InfoRow
                label="Discrete GPU"
                value={dgpuDisplay}
                valueClassName={
                  dgpuDisplay.includes("Unavailable")
                    ? "text-[#8b949e]"
                    : "text-slate-100"
                }
              />
              <InfoRow
                label="Integrated GPU"
                value={info.processorGraphics.integratedGpu}
                valueClassName={
                  info.processorGraphics.integratedGpu === "Unavailable"
                    ? "text-[#8b949e]"
                    : "text-slate-100"
                }
              />
            </div>
          </section>

          {/* Card 4: Memory & Storage */}
          <section className="rounded-2xl border border-[#1e232d] bg-[#12151c] p-5">
            <div className="mb-3 flex items-center gap-2.5">
              <StorageDriveIcon className="h-4.5 w-4.5 text-[#9be000]" />
              <h3 className="text-[16px] font-semibold text-white">
                Memory & Storage
              </h3>
            </div>
            <div className="divide-y divide-[#1d222c]">
              <InfoRow
                label="Installed RAM"
                value={info.memoryStorage.ramTotalDisplay}
              />
              <InfoRow label="Swap" value={info.memoryStorage.swapDisplay} />
              <InfoRow
                label="Primary Drive"
                value={info.memoryStorage.primaryDrive}
                valueClassName={
                  info.memoryStorage.primaryDrive === "Unavailable"
                    ? "text-[#8b949e]"
                    : "text-slate-100"
                }
              />
              <InfoRow
                label="Root Filesystem"
                value={info.memoryStorage.rootFilesystem}
              />
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
}
