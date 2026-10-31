import { forwardRef, useId, type InputHTMLAttributes, type ReactNode } from "react";
import { cx } from "../utils";

export interface TextFieldProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  /** Small explanatory line under the input; `undefined` renders nothing. */
  hint?: string | undefined;
}

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
  { label, hint, id, className, ...props },
  ref,
): ReactNode {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  return (
    <div className="veil-field">
      {label ? (
        <label className="veil-field__label" htmlFor={inputId}>
          {label}
        </label>
      ) : null}
      <input ref={ref} id={inputId} className={cx("veil-input", className)} {...props} />
      {hint ? <span className="veil-field__hint">{hint}</span> : null}
    </div>
  );
});
