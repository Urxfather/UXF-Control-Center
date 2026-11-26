import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { UpdateCheckResult } from "../types/health";

const DISMISSED_VERSION_KEY = "uxf_dismissed_update_version";
const LAST_CHECK_TIME_KEY = "uxf_last_update_check_time";
const CACHED_RESULT_KEY = "uxf_cached_update_result";

// Minimum interval between automated background network checks: 4 hours
const CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000;

export interface ParsedSemver {
  numbers: number[];
  release: number;
}

export function parseSemver(v: string): ParsedSemver {
  const clean = v.trim().replace(/^[vV]/, "");
  const [verPart, relPart] = clean.split("-");
  const numbers = (verPart || "").split(".").map((n) => parseInt(n, 10) || 0);
  let release = 1;
  if (relPart) {
    const digits = relPart.replace(/\D.*$/, "");
    if (digits) {
      release = parseInt(digits, 10) || 1;
    }
  }
  return { numbers, release };
}

export function isVersionGreater(local: string, remote: string): boolean {
  const l = parseSemver(local);
  const r = parseSemver(remote);
  const maxLen = Math.max(l.numbers.length, r.numbers.length);
  for (let i = 0; i < maxLen; i++) {
    const ln = l.numbers[i] ?? 0;
    const rn = r.numbers[i] ?? 0;
    if (rn > ln) return true;
    if (rn < ln) return false;
  }
  return r.release > l.release;
}

export function purgeUpdateCache() {
  try {
    localStorage.removeItem(CACHED_RESULT_KEY);
    localStorage.removeItem(LAST_CHECK_TIME_KEY);
  } catch {
    // ignore in environments without localStorage
  }
}

export function useUpdateChecker(currentVersion: string) {
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResult | null>(null);
  const [showPopup, setShowPopup] = useState(false);

  useEffect(() => {
    let active = true;

    const performCheck = async (force: boolean) => {
      try {
        // 1. Resolve actual local version, never accepting "1.0.0" or empty
        let activeVersion = currentVersion?.trim();
        if (!activeVersion || activeVersion === "1.0.0") {
          try {
            activeVersion = await invoke<string>("get_app_version");
          } catch {
            activeVersion = "2.0.2-3";
          }
        }
        if (!activeVersion || activeVersion === "1.0.0") {
          activeVersion = "2.0.2-3";
        }

        const now = Date.now();
        const lastCheckStr = localStorage.getItem(LAST_CHECK_TIME_KEY);
        const lastCheck = lastCheckStr ? parseInt(lastCheckStr, 10) : 0;

        // 2. Inspect existing cache
        const cachedStr = localStorage.getItem(CACHED_RESULT_KEY);
        if (cachedStr) {
          try {
            const cached: UpdateCheckResult = JSON.parse(cachedStr);

            // Stale cache invalidation conditions:
            // a) Cached current_version is "1.0.0" or uninitialized
            // b) Cached current_version does not match activeVersion (app was upgraded)
            // c) Cached latest_version is not strictly newer than activeVersion
            const isDummyVersion = !cached.current_version || cached.current_version === "1.0.0";
            const isVersionMismatch = cached.current_version !== activeVersion;
            const isNotNewer = cached.latest_version
              ? !isVersionGreater(activeVersion, cached.latest_version)
              : true;

            if (isDummyVersion || isVersionMismatch || isNotNewer) {
              // Invalidate stale cache immediately
              purgeUpdateCache();
              if (active) {
                setShowPopup(false);
                setUpdateInfo(null);
              }
            } else if (!force && now - lastCheck < CHECK_INTERVAL_MS) {
              // Valid cached result indicating genuine newer version
              if (active && cached.update_available && cached.latest_version) {
                const dismissed = localStorage.getItem(DISMISSED_VERSION_KEY);
                if (dismissed !== cached.latest_version) {
                  setUpdateInfo(cached);
                  setShowPopup(true);
                }
              }
              return;
            }
          } catch {
            purgeUpdateCache();
          }
        }

        // 3. Perform live query through Tauri backend
        const result = await invoke<UpdateCheckResult>("check_updates", {
          currentVersion: activeVersion,
        });

        if (!active) return;

        // Safety assertion: ensure client-side semver verification matches
        if (result.latest_version) {
          const strictlyNewer = isVersionGreater(activeVersion, result.latest_version);
          if (!strictlyNewer) {
            result.update_available = false;
          }
        } else {
          result.update_available = false;
        }

        localStorage.setItem(LAST_CHECK_TIME_KEY, now.toString());
        localStorage.setItem(CACHED_RESULT_KEY, JSON.stringify(result));
        setUpdateInfo(result);

        if (result.update_available && result.latest_version) {
          const dismissed = localStorage.getItem(DISMISSED_VERSION_KEY);
          if (dismissed !== result.latest_version) {
            setShowPopup(true);
          } else {
            setShowPopup(false);
          }
        } else {
          setShowPopup(false);
        }
      } catch (err) {
        console.warn("Background update check failed:", err);
      }
    };

    // Initial check on app startup (non-blocking)
    const startupTimer = setTimeout(() => {
      void performCheck(false);
    }, 2500);

    // Periodic check every 4 hours while app stays running
    const intervalTimer = setInterval(() => {
      void performCheck(false);
    }, CHECK_INTERVAL_MS);

    return () => {
      active = false;
      clearTimeout(startupTimer);
      clearInterval(intervalTimer);
    };
  }, [currentVersion]);

  const dismissPopup = (dontShowAgain: boolean) => {
    setShowPopup(false);
    if (dontShowAgain && updateInfo?.latest_version) {
      localStorage.setItem(DISMISSED_VERSION_KEY, updateInfo.latest_version);
    }
  };

  return {
    updateInfo,
    showPopup,
    dismissPopup,
  };
}
