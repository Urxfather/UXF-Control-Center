import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { KeyboardEffectMode, KeyboardState } from "../types/system";

const KEYBOARD_EVENT = "keyboard-state-changed";

export function rgbToHex([r, g, b]: readonly [number, number, number]): string {
  return (
    "#" +
    [r, g, b]
      .map((c) => Math.max(0, Math.min(255, Math.round(c))).toString(16).padStart(2, "0"))
      .join("")
  );
}

export function hexToRgb(hex: string): [number, number, number] {
  const clean = hex.replace("#", "").trim();
  if (clean.length !== 6) {
    return [147, 51, 234];
  }
  const r = Number.parseInt(clean.slice(0, 2), 16);
  const g = Number.parseInt(clean.slice(2, 4), 16);
  const b = Number.parseInt(clean.slice(4, 6), 16);
  if ([r, g, b].some(Number.isNaN)) {
    return [147, 51, 234];
  }
  return [r, g, b];
}

interface SingleFlightQueue<T> {
  inFlight: boolean;
  pending: { value: T; commit: boolean } | null;
}

function makeQueue<T>(): SingleFlightQueue<T> {
  return { inFlight: false, pending: null };
}

export function useKeyboardBackend() {
  const [keyboard, setKeyboard] = useState<KeyboardState | null>(null);
  const [keyboardError, setKeyboardError] = useState<string | null>(null);
  const [draftColor, setDraftColor] = useState<[number, number, number] | null>(null);
  const [draftBrightnessRaw, setDraftBrightnessRaw] = useState<number | null>(null);
  const [monoOffsetMs, setMonoOffsetMs] = useState<number>(0);

  const colorQueueRef = useRef(makeQueue<[number, number, number]>());
  const brightnessQueueRef = useRef(makeQueue<number>());
  const maxBrightnessRef = useRef<number>(255);

  const acceptServerState = useCallback((next: KeyboardState) => {
    if (next.monoNowMs > 0) {
      setMonoOffsetMs(next.monoNowMs - performance.now());
    }
    if (next.maxBrightness > 0) {
      maxBrightnessRef.current = next.maxBrightness;
    }
    setKeyboard(next);
    setKeyboardError(next.error);

    if (!colorQueueRef.current.inFlight && !colorQueueRef.current.pending) {
      setDraftColor(null);
    }
    if (!brightnessQueueRef.current.inFlight && !brightnessQueueRef.current.pending) {
      setDraftBrightnessRaw(null);
    }
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const init = async () => {
      try {
        const snap = await invoke<KeyboardState>("get_keyboard_state");
        if (active) {
          acceptServerState(snap);
        }
      } catch (err) {
        if (active) {
          setKeyboardError(err instanceof Error ? err.message : String(err));
        }
      }

      try {
        unlisten = await listen<KeyboardState>(KEYBOARD_EVENT, (event) => {
          if (active && event.payload) {
            acceptServerState(event.payload);
          }
        });
      } catch {
        // Ignore outside Tauri runtime (unit/SSR environments).
      }
    };

    void init();

    return () => {
      active = false;
      unlisten?.();
    };
  }, [acceptServerState]);

  const setColor = useCallback(
    (rgb: [number, number, number], commit = true) => {
      const clamped: [number, number, number] = [
        Math.max(0, Math.min(255, Math.round(rgb[0]))),
        Math.max(0, Math.min(255, Math.round(rgb[1]))),
        Math.max(0, Math.min(255, Math.round(rgb[2]))),
      ];
      setDraftColor(clamped);

      const queue = colorQueueRef.current;
      const runNext = async (target: [number, number, number], isCommit: boolean) => {
        queue.inFlight = true;
        try {
          const next = await invoke<KeyboardState>("set_keyboard_color", {
            r: target[0],
            g: target[1],
            b: target[2],
            commit: isCommit,
          });
          acceptServerState(next);
        } catch (err) {
          setKeyboardError(err instanceof Error ? err.message : String(err));
        } finally {
          queue.inFlight = false;
          const queued = queue.pending;
          queue.pending = null;
          if (queued) {
            void runNext(queued.value, queued.commit);
          } else {
            setDraftColor(null);
          }
        }
      };

      if (queue.inFlight) {
        queue.pending = {
          value: clamped,
          commit: Boolean(queue.pending?.commit) || commit,
        };
      } else {
        void runNext(clamped, commit);
      }
    },
    [acceptServerState]
  );

  const setBrightnessRaw = useCallback(
    (raw: number, commit = true) => {
      const maxB = Math.max(1, maxBrightnessRef.current);
      const clamped = Math.max(0, Math.min(maxB, Math.round(raw)));
      setDraftBrightnessRaw(clamped);

      const queue = brightnessQueueRef.current;
      const runNext = async (target: number, isCommit: boolean) => {
        queue.inFlight = true;
        try {
          const next = await invoke<KeyboardState>("set_keyboard_brightness", {
            raw: target,
            commit: isCommit,
          });
          acceptServerState(next);
        } catch (err) {
          setKeyboardError(err instanceof Error ? err.message : String(err));
        } finally {
          queue.inFlight = false;
          const queued = queue.pending;
          queue.pending = null;
          if (queued) {
            void runNext(queued.value, queued.commit);
          } else {
            setDraftBrightnessRaw(null);
          }
        }
      };

      if (queue.inFlight) {
        queue.pending = {
          value: clamped,
          commit: Boolean(queue.pending?.commit) || commit,
        };
      } else {
        void runNext(clamped, commit);
      }
    },
    [acceptServerState]
  );

  const toggleBacklight = useCallback(async () => {
    try {
      const next = await invoke<KeyboardState>("toggle_keyboard_backlight");
      acceptServerState(next);
    } catch (err) {
      setKeyboardError(err instanceof Error ? err.message : String(err));
    }
  }, [acceptServerState]);

  const setBrightnessStep = useCallback(
    async (step: number) => {
      const clampedStep = Math.max(0, Math.min(4, Math.round(step)));
      const map = [0, 64, 128, 192, 255];
      setBrightnessRaw(map[clampedStep] ?? 255, true);
    },
    [setBrightnessRaw]
  );

  const setEffect = useCallback(
    async (effect: KeyboardEffectMode) => {
      try {
        const next = await invoke<KeyboardState>("set_keyboard_effect", {
          effect,
        });
        acceptServerState(next);
      } catch (err) {
        setKeyboardError(err instanceof Error ? err.message : String(err));
      }
    },
    [acceptServerState]
  );

  const setSleepTimer = useCallback(
    async (enabled: boolean, seconds: number) => {
      try {
        const next = await invoke<KeyboardState>("set_keyboard_sleep_timer", {
          enabled,
          seconds,
        });
        acceptServerState(next);
      } catch (err) {
        setKeyboardError(err instanceof Error ? err.message : String(err));
      }
    },
    [acceptServerState]
  );

  const maxBrightness = Math.max(1, keyboard?.maxBrightness ?? 255);
  const activeColor = draftColor ?? keyboard?.color ?? [255, 0, 0];
  const activeBrightnessRaw =
    draftBrightnessRaw ?? (keyboard?.idleDimmed ? 0 : (keyboard?.brightnessRaw ?? 0));
  const activeBrightnessTarget =
    draftBrightnessRaw ?? keyboard?.brightnessTarget ?? activeBrightnessRaw;
  const activeStep = keyboard?.brightnessStep ?? 0;

  return {
    keyboard,
    keyboardError,
    monoOffsetMs,
    draftColor,
    draftBrightnessRaw,
    activeColor,
    activeColorHex: rgbToHex(activeColor),
    activeBrightnessRaw,
    activeBrightnessTarget,
    maxBrightness,
    activeStep,
    setColor,
    setBrightnessRaw,
    toggleBacklight,
    setBrightnessStep,
    setEffect,
    setSleepTimer,
  };
}
