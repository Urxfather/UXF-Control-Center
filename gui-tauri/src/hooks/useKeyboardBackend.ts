import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { KeyboardEffectMode, KeyboardState } from "../types/system";

const KEYBOARD_EVENT = "keyboard-state-changed";
const COLOR_THROTTLE_MS = 60;
const GRACE_MS = 240;

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

interface ColorSlot {
  lastSentAt: number;
  timerId: number | null;
  pending: [number, number, number] | null;
  optimisticUntil: number;
}

export function useKeyboardBackend() {
  const [keyboard, setKeyboard] = useState<KeyboardState | null>(null);
  const [keyboardError, setKeyboardError] = useState<string | null>(null);
  const [draftColor, setDraftColor] = useState<[number, number, number] | null>(null);
  const [draftStep, setDraftStep] = useState<number | null>(null);

  const colorSlotRef = useRef<ColorSlot>({
    lastSentAt: 0,
    timerId: null,
    pending: null,
    optimisticUntil: 0,
  });

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const init = async () => {
      try {
        const snap = await invoke<KeyboardState>("get_keyboard_state");
        if (active) {
          setKeyboard(snap);
          setKeyboardError(snap.error);
        }
      } catch (err) {
        if (active) {
          setKeyboardError(err instanceof Error ? err.message : String(err));
        }
      }

      try {
        unlisten = await listen<KeyboardState>(KEYBOARD_EVENT, (event) => {
          if (!active || !event.payload) {
            return;
          }
          setKeyboard(event.payload);
          if (performance.now() >= colorSlotRef.current.optimisticUntil) {
            setDraftColor(null);
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
      if (colorSlotRef.current.timerId !== null) {
        window.clearTimeout(colorSlotRef.current.timerId);
      }
    };
  }, []);

  const setColor = useCallback(
    (rgb: [number, number, number], commit = true) => {
      const clamped: [number, number, number] = [
        Math.max(0, Math.min(255, Math.round(rgb[0]))),
        Math.max(0, Math.min(255, Math.round(rgb[1]))),
        Math.max(0, Math.min(255, Math.round(rgb[2]))),
      ];
      const slot = colorSlotRef.current;
      const now = performance.now();
      slot.optimisticUntil = now + GRACE_MS;
      setDraftColor(clamped);

      const send = async (target: [number, number, number]) => {
        slot.lastSentAt = performance.now();
        try {
          const next = await invoke<KeyboardState>("set_keyboard_color", {
            r: target[0],
            g: target[1],
            b: target[2],
          });
          setKeyboard(next);
          setKeyboardError(next.error);
          if (commit || slot.pending === null) {
            slot.optimisticUntil = 0;
            setDraftColor(null);
          }
        } catch (err) {
          setKeyboardError(err instanceof Error ? err.message : String(err));
          slot.optimisticUntil = 0;
          setDraftColor(null);
        }
      };

      const elapsed = now - slot.lastSentAt;
      if (commit || elapsed >= COLOR_THROTTLE_MS) {
        if (slot.timerId !== null) {
          window.clearTimeout(slot.timerId);
          slot.timerId = null;
        }
        slot.pending = null;
        void send(clamped);
      } else {
        slot.pending = clamped;
        if (slot.timerId === null) {
          const remaining = Math.max(0, COLOR_THROTTLE_MS - elapsed);
          slot.timerId = window.setTimeout(() => {
            slot.timerId = null;
            const queued = slot.pending;
            slot.pending = null;
            if (queued) {
              void send(queued);
            }
          }, remaining);
        }
      }
    },
    []
  );

  const setBrightnessStep = useCallback(async (step: number) => {
    const clamped = Math.max(0, Math.min(4, Math.round(step)));
    setDraftStep(clamped);
    try {
      const next = await invoke<KeyboardState>("set_keyboard_brightness_step", {
        step: clamped,
      });
      setKeyboard(next);
      setKeyboardError(next.error);
    } catch (err) {
      setKeyboardError(err instanceof Error ? err.message : String(err));
    } finally {
      setDraftStep(null);
    }
  }, []);

  const setEffect = useCallback(async (effect: KeyboardEffectMode) => {
    try {
      const next = await invoke<KeyboardState>("set_keyboard_effect", {
        effect,
      });
      setKeyboard(next);
      setKeyboardError(next.error);
    } catch (err) {
      setKeyboardError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  const setSleepTimer = useCallback(async (enabled: boolean, seconds: number) => {
    try {
      const next = await invoke<KeyboardState>("set_keyboard_sleep_timer", {
        enabled,
        seconds,
      });
      setKeyboard(next);
      setKeyboardError(next.error);
    } catch (err) {
      setKeyboardError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  const activeColor = draftColor ?? keyboard?.color ?? [255, 0, 0];
  const activeStep = draftStep ?? keyboard?.brightnessStep ?? 4;

  return {
    keyboard,
    keyboardError,
    activeColor,
    activeColorHex: rgbToHex(activeColor),
    activeStep,
    setColor,
    setBrightnessStep,
    setEffect,
    setSleepTimer,
  };
}
