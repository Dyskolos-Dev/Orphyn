import clsx from 'clsx'
import { CircleX, Search } from 'lucide-react'
import {
  useId,
  useRef,
  type ComponentPropsWithRef,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
} from 'react'
import { useTranslation } from 'react-i18next'
import './controls.css'

export interface TextFieldProps extends Omit<ComponentPropsWithRef<'input'>, 'size'> {
  /** Visible label above the field. Without it, pass `aria-label`. */
  label?: ReactNode
  /** Help text under the field. */
  hint?: ReactNode
  /** Error message; marks the field invalid. */
  error?: ReactNode
  /** Icon inside the field, on the leading side. */
  leadingIcon?: ReactNode
  /** Content inside the field, on the trailing side (e.g. a clear button). */
  trailing?: ReactNode
  size?: 'small' | 'medium' | 'large'
}

/** Rounded text input with optional label, hint and error. */
export function TextField({
  label,
  hint,
  error,
  leadingIcon,
  trailing,
  size = 'medium',
  className,
  id,
  ...rest
}: TextFieldProps) {
  const autoId = useId()
  const inputId = id ?? autoId
  const hintId = `${inputId}-hint`
  const errorId = `${inputId}-error`
  const describedBy =
    [error ? errorId : null, hint ? hintId : null, rest['aria-describedby']]
      .filter(Boolean)
      .join(' ') || undefined
  return (
    <div className={clsx('jr-field', `jr-field--${size}`, error && 'jr-field--invalid', className)}>
      {label && (
        <label htmlFor={inputId} className="jr-field__label">
          {label}
        </label>
      )}
      <div className="jr-field__box">
        {leadingIcon && (
          <span className="jr-field__leading" aria-hidden="true">
            {leadingIcon}
          </span>
        )}
        <input
          id={inputId}
          className="jr-field__input"
          aria-invalid={error ? true : undefined}
          {...rest}
          aria-describedby={describedBy}
        />
        {trailing && <span className="jr-field__trailing">{trailing}</span>}
      </div>
      {error ? (
        <span id={errorId} className="jr-field__error" role="alert">
          {error}
        </span>
      ) : (
        hint && (
          <span id={hintId} className="jr-field__hint">
            {hint}
          </span>
        )
      )}
    </div>
  )
}

export interface SearchFieldProps extends Omit<
  TextFieldProps,
  'value' | 'onChange' | 'type' | 'leadingIcon' | 'trailing' | 'onSubmit'
> {
  value: string
  onChange(value: string): void
  /** Enter pressed. */
  onSubmit?(value: string): void
  /** Called after the clear button or Escape emptied the field. */
  onClear?(): void
}

/**
 * Search field with magnifier and clear button. Escape clears a non-empty field, else blurs it.
 */
export function SearchField({
  value,
  onChange,
  onSubmit,
  onClear,
  size = 'small',
  placeholder,
  ref,
  onKeyDown,
  ...rest
}: SearchFieldProps) {
  const { t } = useTranslation('shell')
  const inner = useRef<HTMLInputElement | null>(null)
  const setRef = (el: HTMLInputElement | null) => {
    inner.current = el
    if (typeof ref === 'function') ref(el)
    else if (ref) (ref as { current: HTMLInputElement | null }).current = el
  }
  const clear = () => {
    onChange('')
    onClear?.()
    inner.current?.focus()
  }
  const handleKey = (e: KeyboardEvent<HTMLInputElement>) => {
    onKeyDown?.(e)
    if (e.defaultPrevented) return
    if (e.key === 'Enter') onSubmit?.(value)
    else if (e.key === 'Escape') {
      if (value) {
        e.preventDefault()
        clear()
      } else inner.current?.blur()
    }
  }
  return (
    <TextField
      {...rest}
      ref={setRef as Ref<HTMLInputElement>}
      type="search"
      size={size}
      className={clsx('jr-search-field', rest.className)}
      value={value}
      placeholder={placeholder ?? t('search.placeholder')}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={handleKey}
      leadingIcon={<Search size={size === 'large' ? 18 : 15} strokeWidth={2.2} />}
      trailing={
        value ? (
          <button
            type="button"
            className="jr-search-field__clear"
            aria-label={t('design.clear')}
            onClick={clear}
            tabIndex={-1}
          >
            <CircleX size={15} fill="currentColor" stroke="var(--surface-window)" />
          </button>
        ) : null
      }
    />
  )
}
