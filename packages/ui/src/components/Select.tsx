import { useId, type ReactNode, type SelectHTMLAttributes } from "react";
import { cx } from "../utils";

export interface SelectOption {
  value: string;
  label: string;
  /**
   * Renders the option unselectable. Used for a value a stored record can still hold but the form
   * no longer offers, so the control shows the record's own value instead of going blank without
   * becoming a way to choose it again.
   */
  disabled?: boolean;
}

export interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  options: SelectOption[];
  placeholder?: string;
  /** Small explanatory line under the control; `undefined` renders nothing. */
  hint?: string | undefined;
}

export function Select({
  label,
  options,
  placeholder,
  hint,
  id,
  className,
  ...props
}: SelectProps): ReactNode {
  const generatedId = useId();
  const selectId = id ?? generatedId;
  return (
    <div className="veil-field">
      {label ? (
        <label className="veil-field__label" htmlFor={selectId}>
          {label}
        </label>
      ) : null}
      <select id={selectId} className={cx("veil-select", className)} {...props}>
        {placeholder ? <option value="">{placeholder}</option> : null}
        {options.map((option) => (
          <option key={option.value} value={option.value} disabled={option.disabled}>
            {option.label}
          </option>
        ))}
      </select>
      {hint ? <span className="veil-field__hint">{hint}</span> : null}
    </div>
  );
}
