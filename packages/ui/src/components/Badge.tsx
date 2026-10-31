import type { ReactNode } from "react";

export type BadgeTone = "neutral" | "success" | "warning" | "error" | "info" | "accent";

export interface BadgeProps {
  tone?: BadgeTone;
  dot?: boolean;
  /** Softly pulses the badge to signal an in-progress/transient state (e.g. "launching"). */
  pulse?: boolean;
  children: ReactNode;
}

export function Badge({
  tone = "neutral",
  dot = false,
  pulse = false,
  children,
}: BadgeProps): ReactNode {
  return (
    <span className="veil-badge" data-tone={tone} data-pulse={pulse ? "true" : undefined}>
      {dot ? <span className="veil-badge__dot" /> : null}
      {children}
    </span>
  );
}
