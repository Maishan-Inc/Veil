import type { ReactNode } from "react";
import { cx } from "../utils";

export type BannerTone = "info" | "warning" | "error" | "success";

export interface BannerProps {
  tone?: BannerTone;
  title?: ReactNode;
  children?: ReactNode;
  /** Right-aligned action slot (typically a Button). */
  action?: ReactNode;
  className?: string;
}

/**
 * Inline notification strip — non-blocking, no dismiss. Used at the top of a section to
 * surface conditional state (e.g. "no engine installed → open Browser").
 */
export function Banner({
  tone = "info",
  title,
  children,
  action,
  className,
}: BannerProps): ReactNode {
  return (
    <div className={cx("veil-banner", className)} data-tone={tone} role="status">
      <div className="veil-banner__body">
        {title ? <div className="veil-banner__title">{title}</div> : null}
        {children ? <div className="veil-banner__text">{children}</div> : null}
      </div>
      {action ? <div className="veil-banner__action">{action}</div> : null}
    </div>
  );
}
