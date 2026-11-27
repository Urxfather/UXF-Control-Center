import type { CSSProperties } from "react";
import type { KeyboardState } from "../types/system";

export const FADE_DURATION_MS = 900;
export const RAINBOW_CYCLE_MS = 5000;
export const BREATH_CYCLE_MS = 2000;

export interface AnimatedKeyboardFrame {
  rgb: [number, number, number];
  effectiveRaw: number;
  levelFactor: number;
  isLit: boolean;
  animating: boolean;
}

/** Pure full-saturation RGB spectrum cycling matching daemon/src/animation.rs::rainbow_rgb. */
export function rainbowRgb(elapsedMs: number): [number, number, number] {
  const modMs = ((elapsedMs % RAINBOW_CYCLE_MS) + RAINBOW_CYCLE_MS) % RAINBOW_CYCLE_MS;
  const phase = modMs / RAINBOW_CYCLE_MS;
  const scaled = phase * 6;
  const sector = Math.floor(scaled) % 6;
  const fraction = Math.max(0, Math.min(255, Math.round((scaled - Math.floor(scaled)) * 255)));

  switch (sector) {
    case 0:
      return [255, fraction, 0];
    case 1:
      return [255 - fraction, 255, 0];
    case 2:
      return [0, 255, fraction];
    case 3:
      return [0, 255 - fraction, 255];
    case 4:
      return [fraction, 0, 255];
    default:
      return [255, 0, 255 - fraction];
  }
}

/** Cosine breathing modulation matching daemon/src/animation.rs::breathing_scale. */
export function breathingScale(elapsedMs: number): number {
  const modMs = ((elapsedMs % BREATH_CYCLE_MS) + BREATH_CYCLE_MS) % BREATH_CYCLE_MS;
  const phase = modMs / BREATH_CYCLE_MS;
  const cosine = 0.5 + 0.5 * Math.cos(2 * Math.PI * phase);
  return 0.06 + 0.94 * cosine;
}

/** Shared ease-in-out cosine fade curve matching daemon/src/animation.rs::smooth_fade_value. */
export function smoothFadeValue(
  from: number,
  to: number,
  elapsedMs: number,
  durationMs: number
): number {
  if (durationMs <= 0 || elapsedMs >= durationMs) {
    return to;
  }
  const t = Math.max(0, Math.min(1, elapsedMs / durationMs));
  const eased = 0.5 - 0.5 * Math.cos(Math.PI * t);
  const val = from + (to - from) * eased;
  return Math.max(0, Math.min(255, Math.round(val)));
}

/**
 * Evaluates the live visual keyboard state phase-locked to daemon CLOCK_MONOTONIC.
 */
export function evaluateKeyboardFrame(
  keyboard: KeyboardState | null,
  draftColor: [number, number, number] | null,
  draftBrightnessRaw: number | null,
  approxMonoNowMs: number
): AnimatedKeyboardFrame {
  const maxBrightness = Math.max(1, keyboard?.maxBrightness ?? 255);
  const baseRgb: [number, number, number] = draftColor ?? keyboard?.color ?? [255, 0, 0];
  const effect = keyboard?.effect ?? "static";

  let effectiveRaw = 0;
  let isFading = false;

  if (draftBrightnessRaw !== null) {
    effectiveRaw = Math.max(0, Math.min(maxBrightness, draftBrightnessRaw));
  } else if (
    keyboard &&
    keyboard.fadeDurationMs > 0 &&
    approxMonoNowMs < keyboard.fadeStartMonoMs + keyboard.fadeDurationMs
  ) {
    const elapsedFade = Math.max(0, approxMonoNowMs - keyboard.fadeStartMonoMs);
    effectiveRaw = smoothFadeValue(
      keyboard.fadeFrom,
      keyboard.fadeTo,
      elapsedFade,
      keyboard.fadeDurationMs
    );
    isFading = true;
  } else if (keyboard) {
    effectiveRaw = keyboard.idleDimmed ? 0 : keyboard.brightnessRaw;
  }

  let levelFactor = Math.max(0, Math.min(1, effectiveRaw / maxBrightness));
  let rgb: [number, number, number] = baseRgb;
  let animating = isFading;

  if (levelFactor > 0) {
    const animElapsed = Math.max(0, approxMonoNowMs - (keyboard?.animEpochMs ?? 0));
    if (effect === "rainbow") {
      rgb = rainbowRgb(animElapsed);
      animating = true;
    } else if (effect === "breathing") {
      rgb = baseRgb;
      levelFactor = Math.max(0, Math.min(1, levelFactor * breathingScale(animElapsed)));
      animating = true;
    }
  }

  return {
    rgb,
    effectiveRaw,
    levelFactor,
    isLit: effectiveRaw > 0.5 && levelFactor > 0.002,
    animating,
  };
}

/** Computes exact CSS variable tokens applied to the keyboard page container. */
export function buildKeyboardCssVars(frame: AnimatedKeyboardFrame): Record<string, string> {
  const [r, g, b] = frame.rgb;
  const f = frame.levelFactor;
  const isLit = frame.isLit;

  const glowBorderAlpha = isLit ? 0.22 + f * 0.64 : 0.16;
  const glowTextAlpha = isLit ? 0.42 + f * 0.55 : 0.36;
  const glowHaloAlpha = isLit ? 0.1 + f * 0.46 : 0;
  const glowFillAlpha = isLit ? 0.03 + f * 0.23 : 0;

  return {
    "--uxf-kbd-border": isLit
      ? `rgba(${r}, ${g}, ${b}, ${glowBorderAlpha.toFixed(3)})`
      : "rgba(55, 65, 81, 0.48)",
    "--uxf-kbd-text": isLit
      ? `rgba(${r}, ${g}, ${b}, ${glowTextAlpha.toFixed(3)})`
      : "rgba(107, 114, 128, 0.68)",
    "--uxf-kbd-key-bg": isLit
      ? `radial-gradient(ellipse at center, rgba(${r}, ${g}, ${b}, ${(glowFillAlpha * 0.45).toFixed(
          3
        )}) 0%, rgba(10, 13, 18, 0.96) 80%)`
      : "linear-gradient(180deg, rgba(16, 20, 27, 0.96) 0%, rgba(10, 12, 17, 0.98) 100%)",
    "--uxf-kbd-key-shadow": isLit
      ? `0 0 ${Math.round(4 + f * 7)}px rgba(${r}, ${g}, ${b}, ${(
          glowHaloAlpha * 0.62
        ).toFixed(3)}), inset 0 0 ${Math.round(3 + f * 5)}px rgba(${r}, ${g}, ${b}, ${(
          glowFillAlpha * 0.7
        ).toFixed(3)})`
      : "inset 0 1px 0 rgba(255, 255, 255, 0.03)",
    "--uxf-kbd-wrap-shadow": isLit
      ? `0 0 ${Math.round(14 + f * 16)}px rgba(${r}, ${g}, ${b}, ${(0.07 + f * 0.23).toFixed(
          3
        )}), 0 10px 28px rgba(0, 0, 0, 0.72)`
      : "0 10px 28px rgba(0, 0, 0, 0.65)",
    "--uxf-wheel-bg": `radial-gradient(circle, rgba(${r}, ${g}, ${b}, ${(
      isLit ? 0.07 + f * 0.19 : 0
    ).toFixed(3)}) 0%, rgba(6, 9, 14, 0) 65%)`,
    "--uxf-wheel-ambient-op": isLit ? "1" : "0",
    "--uxf-ray-color": isLit ? `rgb(${r}, ${g}, ${b})` : "#252c39",
    "--uxf-ray-op": isLit ? (0.3 + f * 0.65).toFixed(3) : "0.45",
  };
}

export function applyKeyboardCssVars(
  el: HTMLElement | null,
  frame: AnimatedKeyboardFrame
): CSSProperties {
  const vars = buildKeyboardCssVars(frame);
  if (el) {
    for (const [key, val] of Object.entries(vars)) {
      el.style.setProperty(key, val);
    }
  }
  return vars as CSSProperties;
}
