import { AnimatePresence, motion } from 'framer-motion'
import { X } from 'lucide-react'
import { useEffect, useRef, useSyncExternalStore, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import { springs } from '../motion'
import './overlays.css'

export interface ToastOptions {
  title: ReactNode
  description?: ReactNode
  /** Leading icon (lucide). */
  icon?: ReactNode
  /** One inline action, e.g. Undo. */
  action?: { label: string; onClick(): void }
  /** Text button replacing the close icon (e.g. Later). */
  dismissLabel?: string
  /** ms before it hides; `Infinity` keeps it until dismissed. Default 4000. */
  duration?: number
  /** `error` is announced assertively. */
  tone?: 'default' | 'error'
}

export interface ToastRecord extends ToastOptions {
  id: number
}

type Listener = () => void
let toasts: ToastRecord[] = []
let nextId = 1
const listeners = new Set<Listener>()
const emit = () => listeners.forEach((l) => l())

/** Shows a toast (HUD at the bottom of the window). Works from anywhere, React or not. */
export function toast(options: ToastOptions): number {
  const id = nextId++
  // Keep the last 3, like macOS keeps its HUD stack short.
  toasts = [...toasts, { ...options, id }].slice(-3)
  emit()
  return id
}

export function dismissToast(id: number): void {
  toasts = toasts.filter((t) => t.id !== id)
  emit()
}

const toastStore = {
  subscribe(l: Listener) {
    listeners.add(l)
    return () => {
      listeners.delete(l)
    }
  },
  get: () => toasts,
}

/** `const toast = useToast(); toast({ title: 'Added to Library' })`. */
export function useToast(): typeof toast {
  return toast
}

/** Renders the toasts. Mounted once by the app shell. */
export function ToastViewport() {
  const list = useSyncExternalStore(toastStore.subscribe, toastStore.get, toastStore.get)
  if (typeof document === 'undefined') return null
  return createPortal(
    <div className="jr-toast-viewport">
      <div role="status" aria-live="polite" className="jr-toast-stack">
        <AnimatePresence initial={false}>
          {list
            .filter((t) => t.tone !== 'error')
            .map((t) => (
              <ToastItem key={t.id} toast={t} />
            ))}
        </AnimatePresence>
      </div>
      <div role="alert" aria-live="assertive" className="jr-toast-stack">
        <AnimatePresence initial={false}>
          {list
            .filter((t) => t.tone === 'error')
            .map((t) => (
              <ToastItem key={t.id} toast={t} />
            ))}
        </AnimatePresence>
      </div>
    </div>,
    document.body,
  )
}

function ToastItem({ toast: t }: { toast: ToastRecord }) {
  const { t: tr } = useTranslation('shell')
  const paused = useRef(false)
  const remaining = useRef(t.duration ?? 4000)

  useEffect(() => {
    if (!Number.isFinite(remaining.current)) return
    let last = Date.now()
    const timer = setInterval(() => {
      const now = Date.now()
      if (!paused.current) remaining.current -= now - last
      last = now
      if (remaining.current <= 0) dismissToast(t.id)
    }, 100)
    return () => clearInterval(timer)
  }, [t.id])

  return (
    <motion.div
      layout
      className="jr-toast jr-material-thick"
      data-tone={t.tone ?? 'default'}
      initial={{ opacity: 0, y: 16, scale: 0.96 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, scale: 0.96, transition: { duration: 0.15 } }}
      transition={springs.bouncy}
      onPointerEnter={() => (paused.current = true)}
      onPointerLeave={() => (paused.current = false)}
      onFocus={() => (paused.current = true)}
      onBlur={() => (paused.current = false)}
    >
      {t.icon && (
        <span className="jr-toast__icon" aria-hidden="true">
          {t.icon}
        </span>
      )}
      <span className="jr-toast__text">
        <span className="jr-toast__title">{t.title}</span>
        {t.description && <span className="jr-toast__description">{t.description}</span>}
      </span>
      {t.action && (
        <button
          type="button"
          className="jr-toast__action"
          onClick={() => {
            t.action?.onClick()
            dismissToast(t.id)
          }}
        >
          {t.action.label}
        </button>
      )}
      {t.dismissLabel ? (
        <button type="button" className="jr-toast__action" onClick={() => dismissToast(t.id)}>
          {t.dismissLabel}
        </button>
      ) : (
        <button
          type="button"
          className="jr-toast__close"
          aria-label={tr('design.dismiss')}
          onClick={() => dismissToast(t.id)}
        >
          <X size={14} strokeWidth={2.5} aria-hidden="true" />
        </button>
      )}
    </motion.div>
  )
}
