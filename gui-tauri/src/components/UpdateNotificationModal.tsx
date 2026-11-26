import { useState } from "react";
import { CloseIcon, InfoIcon } from "./NavIcons";
import type { UpdateCheckResult } from "../types/health";

interface UpdateNotificationModalProps {
  updateInfo: UpdateCheckResult;
  onDismiss: (dontShowAgain: boolean) => void;
}

export function UpdateNotificationModal({
  updateInfo,
  onDismiss,
}: UpdateNotificationModalProps) {
  const [copied, setCopied] = useState(false);

  const handleCopyCommand = () => {
    if (updateInfo.download_url) {
      const cmd = `curl -LO "${updateInfo.download_url}" && sudo dnf install "./${updateInfo.download_url.split('/').pop()}"`;
      void navigator.clipboard.writeText(cmd);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="update-dialog-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-xs select-none"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-lg rounded-xl border border-[#00d8ff]/40 bg-[#0d1117] p-5 shadow-2xl transition-all"
      >
        {/* Header */}
        <div className="flex items-start justify-between border-b border-[#21262d] pb-3.5">
          <div className="flex items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-lg border border-[#00d8ff]/40 bg-[#00d8ff]/10 text-[#00d8ff]">
              <InfoIcon className="h-5 w-5" />
            </span>
            <div>
              <h2
                id="update-dialog-title"
                className="text-base font-semibold tracking-wide text-white"
              >
                UXF-Control-Center Update Available
              </h2>
              <p className="text-xs text-[#8b949e]">
                A new official stable release is ready for installation
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={() => onDismiss(false)}
            aria-label="Dismiss notification"
            className="flex h-7 w-7 items-center justify-center rounded-md text-slate-400 transition-colors hover:bg-[#161b22] hover:text-white focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff]"
          >
            <CloseIcon className="h-4 w-4" />
          </button>
        </div>

        {/* Version Information */}
        <div className="mt-4 space-y-3 text-xs">
          <div className="flex items-center justify-between rounded-lg border border-[#21262d] bg-[#090d13] px-3.5 py-2.5">
            <div className="flex flex-col">
              <span className="text-[#8b949e]">Installed Version</span>
              <span className="font-mono text-slate-200">
                v{updateInfo.current_version.replace(/^v/, '')}
              </span>
            </div>
            <div className="flex flex-col text-right">
              <span className="text-[#8b949e]">Latest Release</span>
              <span className="font-mono font-semibold text-[#00d8ff]">
                v{(updateInfo.latest_version || '').replace(/^v/, '')}
              </span>
            </div>
          </div>

          {/* Release Notes */}
          {updateInfo.release_notes && (
            <div className="rounded-lg border border-[#21262d] bg-[#090d13] p-3 text-[11.5px] leading-relaxed text-slate-300">
              <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-[#8b949e]">
                Release Highlights
              </div>
              <p className="line-clamp-4 text-slate-300 whitespace-pre-line">
                {updateInfo.release_notes}
              </p>
            </div>
          )}

          {/* Installation Instructions */}
          <div className="rounded-lg border border-[#238636]/40 bg-[#238636]/10 p-3 text-slate-200">
            <div className="flex items-center justify-between">
              <span className="font-medium text-white">Recommended Installation:</span>
              {updateInfo.download_url && (
                <button
                  type="button"
                  onClick={handleCopyCommand}
                  className="rounded border border-[#30363d] bg-[#161b22] px-2 py-0.5 text-[10px] text-slate-300 hover:text-white"
                >
                  {copied ? "Copied!" : "Copy Command"}
                </button>
              )}
            </div>

            {updateInfo.has_dnf_repo ? (
              <p className="mt-1 font-mono text-[11px] text-[#9be000]">
                sudo dnf upgrade uxf-control-center
              </p>
            ) : updateInfo.download_url ? (
              <div className="mt-2 space-y-1.5">
                <p className="text-[11px] text-slate-300">
                  Download the official RPM package from GitHub Releases and install with DNF:
                </p>
                <div className="overflow-x-auto rounded border border-[#30363d] bg-black/60 p-2 font-mono text-[10.5px] text-[#9be000] select-text">
                  curl -LO "{updateInfo.download_url}" && sudo dnf install "./{updateInfo.download_url.split('/').pop()}"
                </div>
              </div>
            ) : (
              <p className="mt-1 text-[11px] text-slate-300">
                Please visit GitHub Releases to download the latest RPM asset.
              </p>
            )}
          </div>
        </div>

        {/* Actions */}
        <div className="mt-5 flex items-center justify-between border-t border-[#21262d] pt-3.5">
          <button
            type="button"
            onClick={() => onDismiss(true)}
            className="text-[11px] text-[#8b949e] hover:text-slate-200"
          >
            Don't show again for this version
          </button>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => onDismiss(false)}
              className="rounded border border-[#30363d] bg-[#161b22] px-3 py-1.5 text-xs text-slate-300 transition-colors hover:border-slate-500 hover:text-white"
            >
              Later
            </button>

            {updateInfo.download_url && (
              <a
                href={updateInfo.download_url}
                target="_blank"
                rel="noreferrer"
                className="flex items-center gap-1 rounded bg-[#238636] px-3.5 py-1.5 text-xs font-semibold text-white shadow transition-colors hover:bg-[#2ea043]"
              >
                Download RPM
              </a>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
