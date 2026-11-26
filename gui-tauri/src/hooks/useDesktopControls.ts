import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { DesktopControlsState } from "../types/system";

const DESKTOP_CONTROLS_EVENT = "desktop-controls-changed";
const THROTTLE_MS = 50;
const OPTIMISTIC_GRACE_MS = 220;

interface ThrottleSlot {
  lastSentAt: number;
  timerId: number | null;
  pendingVal: number | null;
  optimisticUntil: number;
}

function makeSlot(): ThrottleSlot {
  return {
    lastSentAt: 0,
    timerId: null,
    pendingVal: null,
    optimisticUntil: 0,
  };
}

export function useDesktopControls() {
  const [controls, setControls] = useState<DesktopControlsState | null>(null);
  const [controlsError, setControlsError] = useState<string | null>(null);

  const [brightnessDraft, setBrightnessDraft] = useState<number | null>(null);
  const [blueLightDraft, setBlueLightDraft] = useState<number | null>(null);
  const [volumeDraft, setVolumeDraft] = useState<number | null>(null);

  const brightnessSlotRef = useRef<ThrottleSlot>(makeSlot());
  const blueLightSlotRef = useRef<ThrottleSlot>(makeSlot());
  const volumeSlotRef = useRef<ThrottleSlot>(makeSlot());

  const applyServerState = useCallback((next: DesktopControlsState) => {
    setControls(next);
    const now = performance.now();
    if (now >= brightnessSlotRef.current.optimisticUntil) {
      setBrightnessDraft(null);
    }
    if (now >= blueLightSlotRef.current.optimisticUntil) {
      setBlueLightDraft(null);
    }
    if (now >= volumeSlotRef.current.optimisticUntil) {
      setVolumeDraft(null);
    }
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const init = async () => {
      try {
        const initial = await invoke<DesktopControlsState>(
          "get_desktop_controls_state"
        );
        if (active) {
          setControls(initial);
          setControlsError(null);
        }
      } catch (err) {
        if (active) {
          setControlsError(err instanceof Error ? err.message : String(err));
        }
      }

      try {
        unlisten = await listen<DesktopControlsState>(
          DESKTOP_CONTROLS_EVENT,
          (event) => {
            if (active && event.payload) {
              applyServerState(event.payload);
            }
          }
        );
      } catch {
        // Ignore outside Tauri runtime (e.g. unit/SSR environments).
      }
    };

    void init();

    return () => {
      active = false;
      unlisten?.();
      for (const slot of [
        brightnessSlotRef.current,
        blueLightSlotRef.current,
        volumeSlotRef.current,
      ]) {
        if (slot.timerId !== null) {
          window.clearTimeout(slot.timerId);
        }
      }
    };
  }, [applyServerState]);

  const dispatchThrottled = useCallback(
    (
      slot: ThrottleSlot,
      value: number,
      commit: boolean,
      setDraft: (v: number | null) => void,
      sender: (v: number) => Promise<DesktopControlsState>
    ) => {
      const now = performance.now();
      slot.optimisticUntil = now + OPTIMISTIC_GRACE_MS;
      setDraft(value);

      const sendValue = async (target: number) => {
        slot.lastSentAt = performance.now();
        try {
          const updated = await sender(target);
          setControls(updated);
          setControlsError(null);
          if (commit || slot.pendingVal === null) {
            slot.optimisticUntil = 0;
            setDraft(null);
          }
        } catch (err) {
          setControlsError(err instanceof Error ? err.message : String(err));
          slot.optimisticUntil = 0;
          setDraft(null);
        }
      };

      const elapsed = now - slot.lastSentAt;
      if (commit || elapsed >= THROTTLE_MS) {
        if (slot.timerId !== null) {
          window.clearTimeout(slot.timerId);
          slot.timerId = null;
        }
        slot.pendingVal = null;
        void sendValue(value);
      } else {
        slot.pendingVal = value;
        if (slot.timerId === null) {
          const remaining = Math.max(0, THROTTLE_MS - elapsed);
          slot.timerId = window.setTimeout(() => {
            slot.timerId = null;
            const queued = slot.pendingVal;
            slot.pendingVal = null;
            if (queued !== null) {
              void sendValue(queued);
            }
          }, remaining);
        }
      }
    },
    []
  );

  const setBrightnessPercent = useCallback(
    (percent: number, commit = false) => {
      const clamped = Math.max(0, Math.min(100, Math.round(percent)));
      dispatchThrottled(
        brightnessSlotRef.current,
        clamped,
        commit,
        setBrightnessDraft,
        (val) =>
          invoke<DesktopControlsState>("set_screen_brightness", {
            percent: val,
          })
      );
    },
    [dispatchThrottled]
  );

  const setBlueLightLevel = useCallback(
    (level: number, commit = false) => {
      const clamped =
        Math.round(Math.max(0, Math.min(10, level)) * 100) / 100;
      dispatchThrottled(
        blueLightSlotRef.current,
        clamped,
        commit,
        setBlueLightDraft,
        (val) =>
          invoke<DesktopControlsState>("set_blue_light_filter", { level: val })
      );
    },
    [dispatchThrottled]
  );

  const setVolumePercent = useCallback(
    (percent: number, commit = false) => {
      const clamped = Math.max(0, Math.min(100, Math.round(percent)));
      dispatchThrottled(
        volumeSlotRef.current,
        clamped,
        commit,
        setVolumeDraft,
        (val) =>
          invoke<DesktopControlsState>("set_system_volume", { percent: val })
      );
    },
    [dispatchThrottled]
  );

  return {
    controls,
    controlsError,
    brightnessPercent:
      brightnessDraft ?? controls?.brightness.percent ?? 0,
    blueLightLevel: blueLightDraft ?? controls?.blueLight.level ?? 0,
    volumePercent: volumeDraft ?? controls?.volume.percent ?? 0,
    setBrightnessPercent,
    setBlueLightLevel,
    setVolumePercent,
  };
}
