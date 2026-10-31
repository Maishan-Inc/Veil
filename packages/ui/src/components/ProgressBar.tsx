import type { ReactNode } from "react";
import { cx } from "../utils";

export type ProgressBarTone = "accent" | "success" | "warning" | "error";

export interface ProgressBarProps {
  /** 0..100. Omit (or pass `undefined`) for an indeterminate animation. */
  value?: number | undefined;
  tone?: ProgressBarTone;
  /** Visual + ARIA label (e.g. "Downloading… 47%"). */
  label?: string | undefined;
  /** Hide the label visually (still announced for screen readers). */
  hideLabel?: boolean;
  className?: string | undefined;
}

/**
 * Determinate or indeterminate progress indicator. Always renders a single accessible
 * `progressbar` element; the surrounding label is purely visual.
 */
export function ProgressBar({
  value,
  tone = "accent",
  label,
  hideLabel = false,
  className,
}: ProgressBarProps): ReactNode {
  const indeterminate = value === undefined;
  const clamped = indeterminate ? undefined : Math.max(0, Math.min(100, value));
  return (
    <div className={cx("veil-progress", className)} data-tone={tone}>
      {label && !hideLabel ? <span className="veil-progress__label">{label}</span> : null}
      <div
        className="veil-progress__track"
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={clamped}
      >
        <div
          className="veil-progress__bar"
          data-indeterminate={indeterminate}
          style={indeterminate ? undefined : { transform: `scaleX(${(clamped ?? 0) / 100})` }}
        />
      </div>
    </div>
  );
}
