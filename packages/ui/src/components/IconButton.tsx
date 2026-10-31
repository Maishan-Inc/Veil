import type { ButtonHTMLAttributes, ReactNode } from "react";
import type { ButtonSize } from "./Button";
import { cx } from "../utils";

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** Accessible label (also used as the tooltip). Required for icon-only buttons. */
  label: string;
  size?: ButtonSize;
  children: ReactNode;
}

export function IconButton({
  label,
  size = "md",
  type = "button",
  className,
  children,
  ...props
}: IconButtonProps): ReactNode {
  return (
    <button
      type={type}
      aria-label={label}
      title={label}
      className={cx("veil-icon-button", className)}
      data-size={size}
      {...props}
    >
      {children}
    </button>
  );
}
