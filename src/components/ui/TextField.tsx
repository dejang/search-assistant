import * as React from 'react';
import { Field } from '@base-ui/react/field';
import { Icon } from './icons/Icon';

export interface TextFieldProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size'> {
  /** Visible label above the input. Omit when the field is labelled by its row or `aria-label`. */
  label?: React.ReactNode;
  /** Helper text below the input. */
  description?: React.ReactNode;
  /** When present, the field renders in its invalid state. */
  error?: React.ReactNode;
  size?: 'small' | 'regular' | 'large';
}

// Input styles per size (spec §5.3 metrics). `data-size` is mirrored onto the input so the
// size variants can target it directly; the attribute on the root is the API surface (spec §2.2).
const sizeClasses: Record<NonNullable<TextFieldProps['size']>, string> = {
  small: 'h-control-small rounded-[4px] px-[5px] text-pn-small',
  regular: 'h-control-regular rounded-pn-sm px-1.5 text-pn-body',
  large: 'h-control-large rounded-pn-sm px-2 text-pn-body',
};

const inputBaseClasses =
  'block w-full appearance-none bg-field-bg text-label ' +
  // rest state: inset control-border + field shadow
  'shadow-[inset_0_0_0_1px_var(--control-border),var(--shadow-field)] ' +
  'placeholder:text-label-secondary ' +
  // focus: inset accent + focus ring
  'focus:outline-hidden focus:shadow-[inset_0_0_0_1px_var(--accent),var(--focus-ring)] ' +
  // invalid: the inset outline becomes red-text (Base UI sets aria-invalid on the control)
  'aria-invalid:shadow-[inset_0_0_0_1px_var(--red-text),var(--shadow-field)] ' +
  'aria-invalid:focus:shadow-[inset_0_0_0_1px_var(--red-text),var(--focus-ring)] ' +
  // disabled
  'disabled:opacity-(--opacity-disabled)';

/** Pane text field (spec §5.3), built on Base UI Field.
 * While invalid, the root carries data-invalid and the input carries aria-invalid. */
export const TextField = React.forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
  { label, description, error, size = 'regular', className, ...rest },
  ref,
) {
  const invalid = error !== undefined && error !== null;
  const rootClasses = ['flex', 'min-w-0', 'flex-col', 'gap-1', className].filter(Boolean).join(' ');
  return (
    <Field.Root data-size={size} invalid={invalid} className={rootClasses}>
      {label != null && <Field.Label className="text-pn-body text-label">{label}</Field.Label>}
      <Field.Control ref={ref} data-size={size} className={`${inputBaseClasses} ${sizeClasses[size]}`} {...rest} />
      {description != null && (
        <Field.Description className="text-pn-small text-label-secondary">{description}</Field.Description>
      )}
      {invalid && (
        <Field.Error className="flex items-center gap-1 text-pn-small text-red-text" match={true}>
          <Icon name="warning" size={12} className="shrink-0" />
          <span>{error}</span>
        </Field.Error>
      )}
    </Field.Root>
  );
});
