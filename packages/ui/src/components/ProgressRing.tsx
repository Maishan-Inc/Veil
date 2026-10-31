import type { ReactNode } from "react";
import { cx } from "../utils";

export type ProgressRingTone = "accent" | "success" | "warning" | "error";

export interface ProgressRingProps {
  /**
   * 0..100. Omit (or pass `undefined`) for indeterminate: a fixed-length arc sweeps instead.
   * A known value is always preferred — an indeterminate ring cannot tell "slow" from "stuck".
   */
  value?: number | undefined;
  /** Outer diameter in px. Default 48. */
  size?: number;
  /** Stroke width in px. Default 4. */
  thickness?: number;
  tone?: ProgressRingTone;
  /** Accessible name, e.g. "Downloading 47%". The only thing announced. */
  label?: string | undefined;
  /**
   * Halo that breathes behind the ring, for work that runs long enough that a static ring
   * reads as frozen. Independent of `value`: a determinate ring can still pulse.
   */
  pulse?: boolean;
  /** Rendered in the middle. Decorative — describe the state in `label`, not here. */
  children?: ReactNode;
  className?: string | undefined;
}

/**
 * Circular counterpart to [`ProgressBar`]. Determinate or indeterminate, with an optional
 * pulsing halo.
 *
 * The arc is an SVG circle whose `stroke-dashoffset` carries the value, so a determinate ring
 * animates through the CSS transition on that property rather than re-rendering per byte.
 *
 * Visual style and tokens live in `packages/ui/src/styles/components.css`.
 */
export function ProgressRing({
  value,
  size = 48,
  thickness = 4,
  tone = "accent",
  label,
  pulse = false,
  children,
  className,
}: ProgressRingProps): ReactNode {
  const indeterminate = value === undefined;
  const clamped = indeterminate ? undefined : Math.max(0, Math.min(100, value));
  const radius = (size - thickness) / 2;
  const circumference = 2 * Math.PI * radius;
  // Indeterminate draws a quarter-turn arc and lets CSS spin it; determinate offsets the
  // full-circle dash by the remaining fraction.
  const dashArray = indeterminate
    ? `${circumference * 0.25} ${circumference}`
    : `${circumference} ${circumference}`;
  const dashOffset = indeterminate ? 0 : circumference * (1 - (clamped ?? 0) / 100);

  return (
    <div
      className={cx("veil-ring", className)}
      data-tone={tone}
      data-pulse={pulse || undefined}
      style={{ width: size, height: size }}
    >
      <svg
        className="veil-ring__svg"
        width={size}
        height={size}
        viewBox={`0 0 ${size} ${size}`}
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        {...(clamped === undefined ? {} : { "aria-valuenow": Math.round(clamped) })}
      >
        <circle
          className="veil-ring__track"
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          strokeWidth={thickness}
        />
        <circle
          className="veil-ring__arc"
          data-indeterminate={indeterminate || undefined}
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          strokeWidth={thickness}
          strokeLinecap="round"
          strokeDasharray={dashArray}
          strokeDashoffset={dashOffset}
        />
      </svg>
      {children ? <span className="veil-ring__center">{children}</span> : null}
    </div>
  );
}
