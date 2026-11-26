import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import type { PreferencesState } from "../types/preferences";

export function usePreferences() {
  const [preferences, setPreferences] = useState<PreferencesState | null>(null);
  const [pendingIds, setPendingIds] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<PreferencesState>("get_preferences_state");
      setPreferences(next);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | null = null;

    const init = async () => {
      try {
        unlisten = await listen<PreferencesState>(
          "preferences-updated",
          (event) => {
            if (active && event.payload) {
              setPreferences(event.payload);
              setError(null);
            }
          }
        );
      } catch (err) {
        console.warn("Preferences event listener unavailable:", err);
      }

      if (active) {
        await refresh();
      }
    };

    const onWindowFocus = () => {
      if (active) {
        void refresh();
      }
    };

    void init();
    window.addEventListener("focus", onWindowFocus);

    return () => {
      active = false;
      window.removeEventListener("focus", onWindowFocus);
      if (unlisten) {
        unlisten();
      }
    };
  }, [refresh]);

  const togglePreference = useCallback(async (id: string) => {
    setPendingIds((prev) => ({ ...prev, [id]: true }));
    setError(null);
    try {
      const next = await invoke<PreferencesState>("toggle_preference", { id });
      setPreferences(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      // Resync with real OS state in case the underlying setting remained unchanged
      void refresh();
    } finally {
      setPendingIds((prev) => {
        const updated = { ...prev };
        delete updated[id];
        return updated;
      });
    }
  }, [refresh]);

  return {
    preferences,
    pendingIds,
    error,
    refresh,
    togglePreference,
  };
}
