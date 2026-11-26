import { useEffect, useRef, useState } from "react";

function easeOutCubic(t: number): number {
  const inv = 1 - t;
  return 1 - inv * inv * inv;
}

/**
 * Smoothly interpolates numeric telemetry values using requestAnimationFrame (~750 ms)
 * so gauge needles, arcs, and digits step organically rather than snapping.
 * Snaps immediately on the first valid sample (null -> number) and falls back cleanly in SSR.
 */
export function useAnimatedNumber(
  target: number | null,
  durationMs = 750
): number | null {
  const isClient =
    typeof window !== "undefined" && typeof requestAnimationFrame !== "undefined";

  const [display, setDisplay] = useState<number | null>(target);
  const currentRef = useRef<number | null>(target);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    if (!isClient) {
      currentRef.current = target;
      setDisplay(target);
      return;
    }

    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }

    if (target === null || !Number.isFinite(target)) {
      currentRef.current = null;
      setDisplay(null);
      return;
    }

    const from = currentRef.current;
    if (from === null || !Number.isFinite(from) || Math.abs(target - from) < 0.01) {
      currentRef.current = target;
      setDisplay(target);
      return;
    }

    const startTime = performance.now();

    const tick = (now: number) => {
      const progress = Math.min(1, Math.max(0, (now - startTime) / durationMs));
      const eased = easeOutCubic(progress);
      const next = from + (target - from) * eased;
      currentRef.current = next;
      setDisplay(next);

      if (progress < 1) {
        rafRef.current = requestAnimationFrame(tick);
      } else {
        currentRef.current = target;
        setDisplay(target);
        rafRef.current = null;
      }
    };

    rafRef.current = requestAnimationFrame(tick);

    return () => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
    };
  }, [target, durationMs, isClient]);

  return isClient ? display : target;
}
