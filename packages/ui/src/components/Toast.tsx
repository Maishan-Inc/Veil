import type { ReactNode } from "react";
import { cx } from "../utils";

export type ToastTone = "info" | "success" | "warning" | "error";

export interface ToastProps {
  tone?: ToastTone;
  title?: ReactNode;
  children?: ReactNode;
  /** Leading slot, typically a status icon. Decorative. */
  icon?: ReactNode;
  /** Trailing action slot (typically a Button). */
  action?: ReactNode;
  /** When provided, a close affordance is rendered. */
  onDismiss?: (() => void) | undefined;
  /** Localized accessible name for the close affordance. Required when `onDismiss` is set. */
  dismissLabel?: string | undefined;
  /** `"entering"` plays the slide-in, `"leaving"` the slide-out. Default `"entering"`. */
  state?: "entering" | "leaving";
  className?: string | undefined;
}

/**
 * Transient notification card. Unlike [`Banner`], which is an inline strip that belongs to the
 * section it describes, a toast is stacked by a host outside the page and disappears on its own.
 *
 * This primitive is presentation only — it owns no timer and no queue, so a host decides where
 * toasts sit, how long they last, and how many are visible. `role="status"` with a polite live
 * region: an alert would interrupt a screen reader mid-sentence for something non-blocking.
 */
export function Toast({
  tone = "info",
  title,
  children,
  icon,
  action,
  onDismiss,
  dismissLabel,
  state = "entering",
  className,
}: ToastProps): ReactNode {
  return (
    <div
      className={cx("veil-toast", className)}
      data-tone={tone}
      data-state={state}
      role="status"
      aria-live="polite"
    >
      {icon ? (
        <span className="veil-toast__icon" aria-hidden="true">
          {icon}
        </span>
      ) : null}
      <div className="veil-toast__body">
        {title ? <div className="veil-toast__title">{title}</div> : null}
        {children ? <div className="veil-toast__text">{children}</div> : null}
      </div>
      {action ? <div className="veil-toast__action">{action}</div> : null}
      {onDismiss ? (
        <button
          type="button"
          className="veil-toast__close"
          aria-label={dismissLabel}
          onClick={onDismiss}
        >
          ×
        </button>
      ) : null}
    </div>
  );
}
