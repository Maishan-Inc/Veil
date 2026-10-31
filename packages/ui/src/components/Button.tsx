import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cx } from "../utils";

export type ButtonVariant = "primary" | "secondary" | "subtle" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  children: ReactNode;
}

export function Button({
  variant = "secondary",
  size = "md",
  type = "button",
  className,
  children,
  ...props
}: ButtonProps): ReactNode {
  return (
    <button
      type={type}
      className={cx("veil-button", className)}
      data-variant={variant}
      data-size={size}
      {...props}
    >
      {children}
    </button>
  );
}
