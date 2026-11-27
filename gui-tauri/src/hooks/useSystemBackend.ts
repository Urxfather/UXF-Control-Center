import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  FanStatus,
  SystemIdentity,
  TelemetrySnapshot,
} from "../types/system";

const DEFAULT_FAN_STATUS: FanStatus = {
  mode: "unavailable",
  daemonConnected: false,
  daemonVersion: null,
  error: null,
};

export interface FanFeedback {
  ok: boolean;
  message: string;
}

export function useSystemBackend() {
  const [identity, setIdentity] = useState<SystemIdentity | null>(null);
  const [telemetry, setTelemetry] = useState<TelemetrySnapshot | null>(null);
  const [fanStatus, setFanStatus] = useState<FanStatus>(DEFAULT_FAN_STATUS);
  const [fanPending, setFanPending] = useState(false);
  const [fanFeedback, setFanFeedback] = useState<FanFeedback | null>(null);
  const [backendError, setBackendError] = useState<string | null>(null);

  const inFlightRef = useRef(false);
  const identityResolvedRef = useRef(false);
  const feedbackTimerRef = useRef<number | null>(null);

  const showFeedback = useCallback((feedback: FanFeedback) => {
    setFanFeedback(feedback);
    if (feedbackTimerRef.current !== null) {
      window.clearTimeout(feedbackTimerRef.current);
    }
    feedbackTimerRef.current = window.setTimeout(() => {
      setFanFeedback(null);
      feedbackTimerRef.current = null;
    }, 3500);
  }, []);

  useEffect(() => {
    let active = true;

    const loadIdentity = async () => {
      try {
        const result = await invoke<SystemIdentity>("get_system_identity");
        if (active) {
          setIdentity(result);
          if (result.gpuModel && result.gpuModel !== "Detecting GPU...") {
            identityResolvedRef.current = true;
          }
        }
      } catch (err) {
        if (active) {
          setBackendError(
            err instanceof Error ? err.message : String(err)
          );
        }
      }
    };

    const pollBackend = async () => {
      if (inFlightRef.current) {
        return;
      }
      inFlightRef.current = true;
      try {
        const [snapshot, fan, refreshedIdentity] = await Promise.all([
          invoke<TelemetrySnapshot>("get_telemetry_snapshot"),
          invoke<FanStatus>("get_fan_status"),
          !identityResolvedRef.current
            ? invoke<SystemIdentity>("get_system_identity").catch(() => null)
            : Promise.resolve(null),
        ]);
        if (active) {
          setTelemetry(snapshot);
          setFanStatus(fan);
          if (refreshedIdentity) {
            setIdentity(refreshedIdentity);
            if (
              refreshedIdentity.gpuModel &&
              refreshedIdentity.gpuModel !== "Detecting GPU..."
            ) {
              identityResolvedRef.current = true;
            }
          }
          setBackendError(null);
        }
      } catch (err) {
        if (active) {
          setBackendError(
            err instanceof Error ? err.message : String(err)
          );
        }
      } finally {
        inFlightRef.current = false;
      }
    };

    void loadIdentity();
    void pollBackend();

    const intervalId = window.setInterval(() => {
      void pollBackend();
    }, 1000);

    return () => {
      active = false;
      window.clearInterval(intervalId);
      if (feedbackTimerRef.current !== null) {
        window.clearTimeout(feedbackTimerRef.current);
      }
    };
  }, []);

  const selectFanMode = useCallback(
    async (mode: "auto" | "max") => {
      setFanPending(true);
      try {
        const confirmed = await invoke<FanStatus>("set_fan_mode", { mode });
        setFanStatus(confirmed);
        showFeedback({
          ok: true,
          message:
            confirmed.mode === "max"
              ? "Maximum fan mode confirmed by daemon"
              : "Auto fan mode confirmed by daemon",
        });
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        showFeedback({
          ok: false,
          message: msg,
        });
        try {
          const current = await invoke<FanStatus>("get_fan_status");
          setFanStatus(current);
        } catch {
          // keep last known status if daemon check also failed
        }
      } finally {
        setFanPending(false);
      }
    },
    [showFeedback]
  );

  return {
    identity,
    telemetry,
    fanStatus,
    fanPending,
    fanFeedback,
    backendError,
    selectFanMode,
  };
}
