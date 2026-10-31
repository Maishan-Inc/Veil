import { useEffect, useRef, useState } from "react";

/**
 * Smoothly tweens a displayed integer toward `value` using requestAnimationFrame.
 * Purely presentational — no business logic. Honors `prefers-reduced-motion` by
 * snapping instantly. Non-finite inputs (e.g. NaN) are passed through unchanged.
 */
export function useCountUp(value: number, durationMs = 500): number {
  const [display, setDisplay] = useState(value);
  const fromRef = useRef(value);
  const frameRef = useRef<number>();

  useEffect(() => {
    if (!Number.isFinite(value)) {
      setDisplay(value);
      fromRef.current = value;
      return;
    }

    const prefersReduced =
      typeof window !== "undefined" &&
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    const from = Number.isFinite(fromRef.current) ? fromRef.current : value;
    if (prefersReduced || from === value || durationMs <= 0) {
      setDisplay(value);
      fromRef.current = value;
      return;
    }

    let start: number | undefined;
    const tick = (now: number): void => {
      if (start === undefined) start = now;
      const t = Math.min(1, (now - start) / durationMs);
      // easeOutCubic — quick then settles, matching the app's decelerate feel.
      const eased = 1 - Math.pow(1 - t, 3);
      const current = Math.round(from + (value - from) * eased);
      setDisplay(current);
      if (t < 1) {
        frameRef.current = requestAnimationFrame(tick);
      } else {
        fromRef.current = value;
      }
    };

    frameRef.current = requestAnimationFrame(tick);
    return () => {
      if (frameRef.current !== undefined) cancelAnimationFrame(frameRef.current);
      // Keep the last displayed value as the next animation's origin.
      fromRef.current = display;
    };
    // `display` intentionally omitted: it changes every frame and would restart the tween.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value, durationMs]);

  return display;
}
