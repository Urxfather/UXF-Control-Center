import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { UpdateCheckResult } from "../../types/health";
import { copyToClipboard, formatUtcTimestamp } from "../../hooks/useSystemCenter";
import { CopyIcon, RefreshIcon } from "./SystemCenterIcons";

interface UpdatesTabProps {
  appVersion: string;
  updateResult: UpdateCheckResult | null;
  cached: boolean;
  checking: boolean;
  checkedAtMs: number | null;
  onCheckAgain: () => void;
}

function parseReleaseBullets(notes: string | null | undefined): string[] {
  if (!notes) return [];
  const bullets: string[] = [];
  for (const rawLine of notes.split("\n")) {
    const trimmed = rawLine.trim();
    if (!trimmed) continue;
    if (trimmed.startsWith("#") || trimmed.startsWith("```") || trimmed.startsWith("sudo ")) {
      continue;
    }
    if (trimmed.startsWith("-") || trimmed.startsWith("*")) {
      const cleaned = trimmed
        .replace(/^[-*]\s+/, "")
        .replace(/\*\*/g, "")
        .replace(/`/g, "")
        .trim();
      if (cleaned.length > 6 && !cleaned.startsWith("File:") && !cleaned.startsWith("Target OS:") && !cleaned.startsWith("SHA-256:")) {
        bullets.push(cleaned);
      }
    }
  }
  return bullets.slice(0, 6);
}

function formatBytesMb(bytes: number | null | undefined): string | null {
  if (!bytes || !Number.isFinite(bytes) || bytes <= 0) return null;
  const mb = bytes / (1024 * 1024);
  return `${mb.toFixed(1)} MB`;
}

export function UpdatesTab({
  appVersion,
  updateResult,
  cached,
  checking,
  checkedAtMs,
  onCheckAgain,
}: UpdatesTabProps) {
  const [copiedCmd, setCopiedCmd] = useState(false);

  const installedDisplay = (updateResult?.current_version || appVersion).replace(/^v/i, "");
  const latestDisplay = updateResult?.latest_version?.replace(/^v/i, "") ?? null;
  const updateAvailable = Boolean(updateResult?.update_available && latestDisplay);
  const hasError = Boolean(updateResult?.error);
  const sizeLabel = formatBytesMb(updateResult?.asset_size_bytes);

  const parsedBullets = useMemo(
    () => parseReleaseBullets(updateResult?.release_notes),
    [updateResult?.release_notes]
  );

  const handleOpenUrl = (url: string) => {
    void invoke("open_external_url", { url }).catch((err) => {
      console.warn("open_external_url failed:", err);
      window.open(url, "_blank", "noopener,noreferrer");
    });
  };

  const handleCopyCommand = async () => {
    if (!updateResult?.update_command) return;
    const ok = await copyToClipboard(updateResult.update_command);
    if (ok) {
      setCopiedCmd(true);
      window.setTimeout(() => setCopiedCmd(false), 2000);
    }
  };

  const statusBadge = checking && !updateResult
    ? { label: "Checking", bg: "bg-[#192130]", text: "text-[#7dd3fc]", dot: "bg-[#7dd3fc]" }
    : hasError
      ? { label: "Check Failed", bg: "bg-[#2a2012]", text: "text-amber-300", dot: "bg-amber-400" }
      : updateAvailable
        ? { label: "Update Available", bg: "bg-[#152436]", text: "text-[#7dd3fc]", dot: "bg-[#7dd3fc]" }
        : { label: "Up to Date", bg: "bg-[#162514]", text: "text-[#9be000]", dot: "bg-[#9be000]" };

  const heroTitleVersion = updateAvailable && latestDisplay ? latestDisplay : installedDisplay;
  const heroSubtitle = hasError
    ? updateResult?.error ?? "Unable to contact GitHub Releases. Showing local package version."
    : updateAvailable && latestDisplay
      ? `Stable release v${latestDisplay} is published on GitHub Releases for Fedora Linux x86_64.`
      : `UXF-Control-Center v${installedDisplay} matches the latest stable release on GitHub.`;

  const historyList = updateResult?.history?.length
    ? updateResult.history
    : [
        {
          version: installedDisplay,
          name: `UXF-Control-Center v${installedDisplay}`,
          published_at: null,
          is_installed: true,
          notes_summary: "Current installed build",
          html_url: "https://github.com/Urxfather/UXF-Control-Center/releases",
        },
      ];

  return (
    <div className="uxf-tab-fade space-y-5">
      {/* Top Release Hero Card */}
      <section className="rounded-2xl border border-[#1e232d] bg-[#12151c] p-6">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex flex-wrap items-center gap-3">
            <span
              className={`inline-flex items-center gap-2 rounded-full px-3 py-1 text-[12px] font-semibold ${statusBadge.bg} ${statusBadge.text}`}
            >
              <span className={`h-2 w-2 rounded-full ${statusBadge.dot}`} />
              {statusBadge.label}
            </span>

            <span className="font-mono text-[12.5px] text-[#94a3b8]">
              Installed: v{installedDisplay}
              {latestDisplay ? `  →  Latest: v${latestDisplay}` : ""}
            </span>
          </div>

          <span className="font-mono text-[11.5px] text-[#6b7280]">
            Last checked: {formatUtcTimestamp(checkedAtMs ?? updateResult?.checked_at_ms)} ·{" "}
            {checking ? "Checking..." : cached ? "Cached" : "Fresh"}
          </span>
        </div>

        <h2 className="mt-4 text-[26px] font-bold tracking-tight text-white">
          UXF Control Center v{heroTitleVersion}
        </h2>

        <p className="mt-1.5 max-w-3xl text-[13.5px] leading-relaxed text-[#94a3b8]">
          {heroSubtitle}
        </p>

        <div className="mt-5 flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={onCheckAgain}
            disabled={checking}
            className="inline-flex cursor-pointer items-center gap-2 rounded-full border border-[#2a3140] bg-[#1d222c] px-5 py-2 text-[13px] font-medium text-slate-200 transition-colors hover:border-[#3a4457] hover:bg-[#252b38] hover:text-white disabled:cursor-wait disabled:opacity-65 focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff]"
          >
            {checking && <RefreshIcon className="h-3.5 w-3.5 animate-spin" />}
            <span>{checking ? "Checking..." : "Check Again"}</span>
          </button>

          {updateAvailable && updateResult?.download_url ? (
            <button
              type="button"
              onClick={() => handleOpenUrl(updateResult.download_url!)}
              className="inline-flex cursor-pointer items-center gap-2 rounded-full bg-[#93c5fd] px-5 py-2 text-[13px] font-semibold text-[#081018] shadow-sm transition-colors hover:bg-[#bfdbfe] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#00d8ff]"
            >
              Download Update v{latestDisplay}
            </button>
          ) : (
            <button
              type="button"
              onClick={() =>
                handleOpenUrl("https://github.com/Urxfather/UXF-Control-Center/releases")
              }
              className="inline-flex cursor-pointer items-center gap-2 rounded-full border border-[#263246] bg-[#142030] px-4 py-2 text-[13px] font-medium text-[#7dd3fc] transition-colors hover:bg-[#1a293d] focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff]"
            >
              View GitHub Releases
            </button>
          )}
        </div>
      </section>

      {/* Bottom 2-Column Split */}
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-[1.28fr_0.92fr]">
        {/* Release Notes Card */}
        <section className="flex flex-col rounded-2xl border border-[#1e232d] bg-[#12151c] p-6">
          <div className="mb-4 flex items-baseline justify-between gap-2">
            <h3 className="text-[18px] font-semibold tracking-tight text-white">
              Release Notes
            </h3>
            <span className="font-mono text-[12px] text-[#7dd3fc]">
              v{heroTitleVersion}
              {sizeLabel ? ` · ${sizeLabel}` : ""}
            </span>
          </div>

          {parsedBullets.length > 0 ? (
            <ul className="space-y-3 text-[13.5px] leading-relaxed text-[#cbd5e1]">
              {parsedBullets.map((line, i) => (
                <li key={i} className="flex items-start gap-3">
                  <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-[#9be000]" />
                  <span>{line}</span>
                </li>
              ))}
            </ul>
          ) : updateResult?.release_notes ? (
            <p className="text-[13px] leading-relaxed text-[#94a3b8] whitespace-pre-line">
              {updateResult.release_notes}
            </p>
          ) : (
            <p className="text-[13px] text-[#8b949e]">
              {hasError
                ? "Release notes are unavailable while offline. Click Check Again when connected."
                : "No release notes available for this build."}
            </p>
          )}

          {updateAvailable && updateResult?.update_command && (
            <div className="mt-5 rounded-xl border border-[#202632] bg-[#0d1016] p-3.5">
              <div className="mb-2 flex items-center justify-between gap-2">
                <span className="text-[12px] font-medium text-[#94a3b8]">
                  DNF Installation Command
                </span>
                <button
                  type="button"
                  onClick={() => void handleCopyCommand()}
                  className="inline-flex cursor-pointer items-center gap-1.5 rounded-md border border-[#2a3140] bg-[#181c25] px-2.5 py-1 text-[11px] font-medium text-slate-200 transition-colors hover:text-white"
                >
                  <CopyIcon className="h-3.5 w-3.5" />
                  <span>{copiedCmd ? "Copied!" : "Copy"}</span>
                </button>
              </div>
              <pre className="overflow-x-auto font-mono text-[11.5px] text-[#9be000] select-text">
                {updateResult.update_command}
              </pre>
            </div>
          )}
        </section>

        {/* Update History Card */}
        <section className="flex flex-col rounded-2xl border border-[#1e232d] bg-[#12151c] p-6">
          <div className="mb-4 flex items-baseline justify-between gap-2">
            <h3 className="text-[18px] font-semibold tracking-tight text-white">
              Update History
            </h3>
            <span className="font-mono text-[12px] text-[#8b949e]">
              {historyList.length} {historyList.length === 1 ? "entry" : "entries"}
            </span>
          </div>

          <div className="flex flex-col divide-y divide-[#1d222c]">
            {historyList.map((entry) => (
              <div
                key={entry.version}
                className="flex items-start justify-between gap-4 py-3.5 first:pt-1 last:pb-1"
              >
                <div className="min-w-0 flex-1">
                  <div className="font-mono text-[13.5px] font-semibold text-white">
                    v{entry.version}
                  </div>
                  <div className="mt-0.5 truncate text-[12.5px] text-[#8b949e]">
                    {entry.notes_summary ?? entry.name}
                  </div>
                </div>
                <span
                  className={`shrink-0 font-mono text-[12px] ${
                    entry.is_installed ? "font-semibold text-[#9be000]" : "text-[#8b949e]"
                  }`}
                >
                  {entry.published_at ?? (entry.is_installed ? "Installed" : "--")}
                </span>
              </div>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
