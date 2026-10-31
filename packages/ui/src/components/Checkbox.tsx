import { useEffect, useRef, type InputHTMLAttributes, type ReactNode } from "react";
import { cx } from "../utils";

export interface CheckboxProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type"> {
  /** Renders the native indeterminate ("some selected") state. */
  indeterminate?: boolean;
}

export function Checkbox({ indeterminate = false, className, ...props }: CheckboxProps): ReactNode {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) {
      ref.current.indeterminate = indeterminate;
    }
  }, [indeterminate]);
  return <input ref={ref} type="checkbox" className={cx("veil-checkbox", className)} {...props} />;
}
