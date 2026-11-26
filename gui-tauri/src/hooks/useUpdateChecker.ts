import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { UpdateCheckResult } from "../types/health";

const DISMISSED_VERSION_KEY = "uxf_dismissed_update_version";
const LAST_CHECK_TIME_KEY = "uxf_last_update_check_time";
const CACHED_RESULT_KEY = "uxf_cached_update_result";

// Minimum interval between automated background network checks: 4 hours
const CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000;

export function useUpdateChecker(currentVersion: string) {
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResult | null>(null);
  const [showPopup, setShowPopup] = useState(false);

  useEffect(() => {
    let active = true;

    const performCheck = async (force: boolean) => {
      try {
        const now = Date.now();
        const lastCheckStr = localStorage.getItem(LAST_CHECK_TIME_KEY);
        const lastCheck = lastCheckStr ? parseInt(lastCheckStr, 10) : 0;

        // If not forced and last check was recent, use cached result
        if (!force && now - lastCheck < CHECK_INTERVAL_MS) {
          const cachedStr = localStorage.getItem(CACHED_RESULT_KEY);
          if (cachedStr) {
            const cached: UpdateCheckResult = JSON.parse(cachedStr);
            if (active && cached.update_available && cached.latest_version) {
              const dismissed = localStorage.getItem(DISMISSED_VERSION_KEY);
              if (dismissed !== cached.latest_version) {
                setUpdateInfo(cached);
                setShowPopup(true);
              }
            }
            return;
          }
        }

        // Perform actual query through Tauri backend
        const result = await invoke<UpdateCheckResult>("check_updates", {
          currentVersion,
        });

        if (!active) return;

        localStorage.setItem(LAST_CHECK_TIME_KEY, now.toString());
        localStorage.setItem(CACHED_RESULT_KEY, JSON.stringify(result));
        setUpdateInfo(result);

        if (result.update_available && result.latest_version) {
          const dismissed = localStorage.getItem(DISMISSED_VERSION_KEY);
          if (dismissed !== result.latest_version) {
            setShowPopup(true);
          }
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
