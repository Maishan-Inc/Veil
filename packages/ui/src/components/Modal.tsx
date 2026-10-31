import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

export type ModalSize = "sm" | "md" | "lg";

export interface ModalProps {
  /** Whether the modal is rendered; nothing is mounted when false. */
  open: boolean;
  /** Localized title; rendered in the header. */
  title: ReactNode;
  /** Called on Escape, backdrop click, or close button. */
  onClose: () => void;
  /** Modal body. */
  children?: ReactNode;
  /** Optional right-aligned footer; usually action buttons. */
  footer?: ReactNode;
  /** Width preset. Defaults to `md`. */
  size?: ModalSize;
  /** When true, clicking the backdrop does NOT close. Default false. */
  dismissOnBackdrop?: boolean;
  /** When true, Escape does NOT close. Default false. */
  dismissOnEscape?: boolean;
  /** Optional aria-describedby id; useful when the body is a single descriptive block. */
  describedBy?: string;
}

/**
 * Accessible modal dialog primitive. Renders into a portal at the end of `document.body`.
 *
 * Behavior:
 * - Locks page scroll while open.
 * - Closes on Escape (unless `dismissOnEscape={false}`).
 * - Closes on backdrop click (unless `dismissOnBackdrop={false}`).
 * - Focus is moved to the panel on open.
 * - `role="dialog"`, `aria-modal="true"`, labelled by the title.
 *
 * Visual style and tokens live in `packages/ui/src/styles/components.css`. Consumers should
 * compose their content with the existing primitives (`TextField`, `Select`, `Button`, …).
 */
export function Modal({
  open,
  title,
  onClose,
  children,
  footer,
  size = "md",
  dismissOnBackdrop = true,
  dismissOnEscape = true,
  describedBy,
}: ModalProps): ReactNode {
  const titleId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const [rendered, setRendered] = useState(open);

  useEffect(() => {
    if (open) {
      setRendered(true);
      return;
    }
    if (!rendered) return;
    if (typeof window === "undefined") {
      setRendered(false);
      return;
    }
    const id = window.setTimeout(() => setRendered(false), 180);
    return () => window.clearTimeout(id);
  }, [open, rendered]);

  // Lock page scroll while open and focus the panel on open.
  useEffect(() => {
    if (!open || typeof document === "undefined") return;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const previousFocus = document.activeElement as HTMLElement | null;
    // Focus the panel itself; consumers can put autofocus on their own inputs.
    panelRef.current?.focus();
    return () => {
      document.body.style.overflow = previousOverflow;
      previousFocus?.focus?.();
    };
  }, [open]);

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (dismissOnEscape && event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    },
    [dismissOnEscape, onClose],
  );

  const handleBackdropClick = useCallback(
    (event: MouseEvent<HTMLDivElement>) => {
      // Only close when the actual backdrop was clicked, not a child.
      if (dismissOnBackdrop && event.target === event.currentTarget) {
        onClose();
      }
    },
    [dismissOnBackdrop, onClose],
  );

  if (!rendered || typeof document === "undefined") return null;

  return createPortal(
    <div
      className="veil-modal__backdrop"
      data-veil-modal=""
      data-state={open ? "open" : "closing"}
      onClick={handleBackdropClick}
      onKeyDown={handleKeyDown}
      role="presentation"
    >
      <div
        ref={panelRef}
        className="veil-modal__panel"
        data-size={size}
        data-state={open ? "open" : "closing"}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={describedBy}
        tabIndex={-1}
      >
        <header className="veil-modal__header">
          <h2 id={titleId} className="veil-modal__title">
            {title}
          </h2>
          <button type="button" className="veil-modal__close" aria-label="Close" onClick={onClose}>
            ×
          </button>
        </header>
        <div className="veil-modal__body">{children}</div>
        {footer ? <footer className="veil-modal__footer">{footer}</footer> : null}
      </div>
    </div>,
    document.body,
  );
}
