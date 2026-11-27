import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { DesktopControlsState } from "../types/system";

const DESKTOP_CONTROLS_EVENT = "desktop-controls-changed";

interface ControlFlightSlot {
  inFlight: boolean;
  pendingVal: number | null;
}

function makeSlot(): ControlFlightSlot {
  return {
    inFlight: false,
    pendingVal: null,
  };
}

export function useDesktopControls() {
  const [controls, setControls] = useState<DesktopControlsState | null>(null);
  const [controlsError, setControlsError] = useState<string | null>(null);

  const [brightnessDraft, setBrightnessDraft] = useState<number | null>(null);
  const [blueLightDraft, setBlueLightDraft] = useState<number | null>(null);
  const [volumeDraft, setVolumeDraft] = useState<number | null>(null);

  const brightnessSlotRef = useRef<ControlFlightSlot>(makeSlot());
  const blueLightSlotRef = useRef<ControlFlightSlot>(makeSlot());
  const volumeSlotRef = useRef<ControlFlightSlot>(makeSlot());

  const applyServerState = useCallback((next: DesktopControlsState) => {
    setControls(next);
    if (!brightnessSlotRef.current.inFlight && brightnessSlotRef.current.pendingVal === null) {
      setBrightnessDraft(null);
    }
    if (!blueLightSlotRef.current.inFlight && blueLightSlotRef.current.pendingVal === null) {
      setBlueLightDraft(null);
    }
    if (!volumeSlotRef.current.inFlight && volumeSlotRef.current.pendingVal === null) {
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
          applyServerState(initial);
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
    };
  }, [applyServerState]);

  const dispatchSingleFlight = useCallback(
    (
      slot: ControlFlightSlot,
      value: number,
      setDraft: (v: number | null) => void,
      sender: (v: number) => Promise<DesktopControlsState>
    ) => {
      setDraft(value);

      const sendValue = async (target: number) => {
        slot.inFlight = true;
        try {
          const updated = await sender(target);
          setControls(updated);
          setControlsError(null);
        } catch (err) {
          setControlsError(err instanceof Error ? err.message : String(err));
        } finally {
          slot.inFlight = false;
          const queued = slot.pendingVal;
          slot.pendingVal = null;
          if (queued !== null) {
            void sendValue(queued);
          } else {
            setDraft(null);
          }
        }
      };

      if (slot.inFlight) {
        slot.pendingVal = value;
      } else {
        void sendValue(value);
      }
    },
    []
  );

  const setBrightnessPercent = useCallback(
    (percent: number, _commit = false) => {
      const clamped = Math.max(0, Math.min(100, Math.round(percent)));
      dispatchSingleFlight(
        brightnessSlotRef.current,
        clamped,
        setBrightnessDraft,
        (val) =>
          invoke<DesktopControlsState>("set_screen_brightness", {
            percent: val,
          })
      );
    },
    [dispatchSingleFlight]
  );

  const setBlueLightLevel = useCallback(
    (level: number, _commit = false) => {
      const clamped =
        Math.round(Math.max(0, Math.min(10, level)) * 100) / 100;
      dispatchSingleFlight(
        blueLightSlotRef.current,
        clamped,
        setBlueLightDraft,
        (val) =>
          invoke<DesktopControlsState>("set_blue_light_filter", { level: val })
      );
    },
    [dispatchSingleFlight]
  );

  const setVolumePercent = useCallback(
    (percent: number, _commit = false) => {
      const clamped = Math.max(0, Math.min(100, Math.round(percent)));
      dispatchSingleFlight(
        volumeSlotRef.current,
        clamped,
        setVolumeDraft,
        (val) =>
          invoke<DesktopControlsState>("set_system_volume", { percent: val })
      );
    },
    [dispatchSingleFlight]
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
