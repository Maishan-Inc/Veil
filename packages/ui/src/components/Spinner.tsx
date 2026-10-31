import type { ReactNode } from "react";

export interface SpinnerProps {
  /** Diameter in pixels. */
  size?: number;
  label?: string;
}

export function Spinner({ size = 16, label }: SpinnerProps): ReactNode {
  return (
    <span
      className="veil-spinner"
      role="status"
      aria-label={label ?? "Loading"}
      style={{ width: size, height: size }}
    />
  );
}
