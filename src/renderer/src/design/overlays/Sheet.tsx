import clsx from 'clsx'
import { AnimatePresence, motion } from 'framer-motion'
import { X } from 'lucide-react'
import { useId, useRef, type ReactNode, type RefObject } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import { IconButton } from '../controls/IconButton'
import { springs } from '../motion'
import { useFocusScope } from './focus'
import './overlays.css'

export interface SheetProps {
  open: boolean
  /** Escape, backdrop click or the close button. */
  onClose(): void
  /** Dialog title (also its accessible name). */
  title: ReactNode
  /** Line under the title (also its accessible description). */
  description?: ReactNode
  children?: ReactNode
  /** Action buttons, right-aligned (primary last, as on macOS). */
  footer?: ReactNode
  /** 420 / 560 / 760 px wide. */
  size?: 'small' | 'medium' | 'large'
  /** Allow Escape / backdrop to close (default true). */
  dismissible?: boolean
  /** Hide the close (x) button. */
  hideCloseButton?: boolean
  /** Element focused on open; defaults to the first focusable element. */
  initialFocus?: RefObject<HTMLElement | null>
  className?: string
}

/**
 * Modal sheet (role="dialog", aria-modal). Focus is trapped inside and restored on close; the
 * page behind is dimmed. Use for forms, confirmations and pickers.
 */
export function Sheet(props: SheetProps) {
  return <AnimatePresence>{props.open && <SheetContent {...props} />}</AnimatePresence>
}

/** Alias of Sheet, for readers looking for a "Modal". */
export const Modal = Sheet

function SheetContent({
  onClose,
  title,
  description,
  children,
  footer,
  size = 'medium',
  dismissible = true,
  hideCloseButton = false,
  initialFocus,
  className,
}: SheetProps) {
  const { t } = useTranslation('shell')
  const ref = useRef<HTMLDivElement>(null)
  const titleId = useId()
  const descId = useId()
  useFocusScope(ref, { trap: true, initialFocus })

  return createPortal(
    <div
      className="jr-sheet-layer"
      onKeyDown={(e) => {
        if (e.key === 'Escape' && dismissible) {
          e.stopPropagation()
          onClose()
        }
      }}
    >
      <motion.div
        className="jr-sheet-backdrop"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.2 }}
        onClick={dismissible ? onClose : undefined}
        aria-hidden="true"
      />
      <motion.div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={description ? descId : undefined}
        tabIndex={-1}
        className={clsx('jr-sheet', `jr-sheet--${size}`, className)}
        initial={{ opacity: 0, scale: 0.96, y: 12 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.97, y: 8, transition: { duration: 0.16 } }}
        transition={springs.smooth}
      >
        <header className="jr-sheet__header">
          <div className="jr-sheet__titles">
            <h2 id={titleId} className="jr-sheet__title">
              {title}
            </h2>
            {description && (
              <p id={descId} className="jr-sheet__description">
                {description}
              </p>
            )}
          </div>
          {!hideCloseButton && (
            <IconButton
              variant="gray"
              size="small"
              icon={<X strokeWidth={2.5} />}
              label={t('design.close')}
              onClick={onClose}
              data-skip-autofocus=""
            />
          )}
        </header>
        {children != null && <div className="jr-sheet__body">{children}</div>}
        {footer && <footer className="jr-sheet__footer">{footer}</footer>}
      </motion.div>
    </div>,
    document.body,
  )
}
