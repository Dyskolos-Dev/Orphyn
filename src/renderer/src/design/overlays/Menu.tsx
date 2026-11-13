import clsx from 'clsx'
import { Check } from 'lucide-react'
import {
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type ReactElement,
  type ReactNode,
} from 'react'
import { IconButton, type IconButtonProps } from '../controls/IconButton'
import { Popover, type PopoverAnchor, type PopoverPlacement } from './Popover'
import './overlays.css'

export interface MenuActionItem {
  type?: 'item'
  /** Stable key; defaults to the label when it is a string. */
  id?: string
  label: ReactNode
  icon?: ReactNode
  /** Shortcut hint shown on the right, e.g. "⌘L". Display only. */
  shortcut?: string
  /** Shows a check mark and makes the item a menuitemcheckbox. */
  checked?: boolean
  disabled?: boolean
  destructive?: boolean
  onSelect(): void
}
export interface MenuSeparatorItem {
  type: 'separator'
  id?: string
}
export interface MenuHeaderItem {
  type: 'header'
  id?: string
  label: ReactNode
}
export type MenuItem = MenuActionItem | MenuSeparatorItem | MenuHeaderItem

export interface MenuProps {
  open: boolean
  onClose(): void
  anchor: PopoverAnchor
  items: readonly MenuItem[]
  placement?: PopoverPlacement
  'aria-label'?: string
}

const isAction = (i: MenuItem): i is MenuActionItem => i.type === undefined || i.type === 'item'

/**
 * Apple-style menu (role="menu") on a popover. Arrow Up / Down, Home / End move, Enter / Space
 * select, Escape closes, typing a letter jumps to the next matching item.
 */
export function Menu({
  open,
  onClose,
  anchor,
  items,
  placement,
  'aria-label': ariaLabel,
}: MenuProps) {
  const listRef = useRef<HTMLDivElement>(null)

  const enabledItems = () =>
    Array.from(
      listRef.current?.querySelectorAll<HTMLElement>(
        '[data-menu-item]:not([aria-disabled="true"])',
      ) ?? [],
    )

  const onKeyDown = (e: KeyboardEvent) => {
    const list = enabledItems()
    if (list.length === 0) return
    const i = list.indexOf(document.activeElement as HTMLElement)
    let next: HTMLElement | undefined
    if (e.key === 'ArrowDown') next = list[(i + 1) % list.length]
    else if (e.key === 'ArrowUp') next = list[(i - 1 + list.length) % list.length]
    else if (e.key === 'Home') next = list[0]
    else if (e.key === 'End') next = list[list.length - 1]
    else if (e.key === 'Tab') {
      e.preventDefault()
      onClose()
      return
    } else if (e.key.length === 1 && /\S/.test(e.key)) {
      const ch = e.key.toLowerCase()
      const ordered = [...list.slice(i + 1), ...list.slice(0, i + 1)]
      next = ordered.find((el) => el.textContent?.trim().toLowerCase().startsWith(ch))
    } else return
    e.preventDefault()
    next?.focus()
  }

  const select = (item: MenuActionItem) => {
    if (item.disabled) return
    onClose()
    item.onSelect()
  }

  return (
    <Popover
      open={open}
      onClose={onClose}
      anchor={anchor}
      placement={placement}
      role="presentation"
      autoFocus={false}
    >
      <div
        ref={(el) => {
          listRef.current = el
          // Focus the first item on open so arrow keys work immediately.
          if (el && !el.contains(document.activeElement)) {
            el.querySelector<HTMLElement>('[data-menu-item]:not([aria-disabled="true"])')?.focus()
          }
        }}
        role="menu"
        aria-label={ariaLabel}
        className="jr-menu"
        onKeyDown={onKeyDown}
        onContextMenu={(e) => e.preventDefault()}
      >
        {items.map((item, idx) => {
          const key =
            item.id ?? (('label' in item && typeof item.label === 'string' && item.label) || idx)
          if (item.type === 'separator') {
            return <div key={key} role="separator" className="jr-menu__separator" />
          }
          if (item.type === 'header') {
            return (
              <div key={key} role="presentation" className="jr-menu__header">
                {item.label}
              </div>
            )
          }
          if (!isAction(item)) return null
          const hasCheck = items.some((i) => isAction(i) && i.checked !== undefined)
          return (
            <div
              key={key}
              data-menu-item=""
              role={item.checked !== undefined ? 'menuitemcheckbox' : 'menuitem'}
              aria-checked={item.checked}
              aria-disabled={item.disabled || undefined}
              tabIndex={-1}
              className={clsx(
                'jr-menu__item',
                item.destructive && 'jr-menu__item--destructive',
                item.disabled && 'jr-menu__item--disabled',
              )}
              onClick={() => select(item)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault()
                  select(item)
                }
              }}
              onPointerMove={(e) => {
                if (!item.disabled) (e.currentTarget as HTMLElement).focus({ preventScroll: true })
              }}
            >
              {hasCheck && (
                <span className="jr-menu__check" aria-hidden="true">
                  {item.checked && <Check size={14} strokeWidth={2.6} />}
                </span>
              )}
              {item.icon && (
                <span className="jr-menu__icon" aria-hidden="true">
                  {item.icon}
                </span>
              )}
              <span className="jr-menu__label">{item.label}</span>
              {item.shortcut && <span className="jr-menu__shortcut">{item.shortcut}</span>}
            </div>
          )
        })}
      </div>
    </Popover>
  )
}

export interface ContextMenuProps {
  items: readonly MenuItem[]
  /** The element(s) that get the menu. */
  children: ReactNode
  'aria-label'?: string
  /** Disable the menu (children render as-is). */
  disabled?: boolean
}

/**
 * Right-click menu around any content. Also opens with the keyboard (Shift+F10 or the Menu key)
 * on a focused child, anchored to that child.
 */
export function ContextMenu({
  items,
  children,
  disabled,
  'aria-label': ariaLabel,
}: ContextMenuProps) {
  const [at, setAt] = useState<{ x: number; y: number } | null>(null)

  const onContextMenu = (e: MouseEvent) => {
    if (disabled) return
    e.preventDefault()
    // Keyboard-triggered contextmenu events have no pointer position: anchor to the target.
    if (e.clientX === 0 && e.clientY === 0 && e.target instanceof HTMLElement) {
      const r = e.target.getBoundingClientRect()
      setAt({ x: r.left + 8, y: r.bottom })
    } else {
      setAt({ x: e.clientX, y: e.clientY })
    }
  }
  const onKeyDown = (e: KeyboardEvent) => {
    if (disabled) return
    if ((e.shiftKey && e.key === 'F10') || e.key === 'ContextMenu') {
      e.preventDefault()
      const r = (e.target as HTMLElement).getBoundingClientRect()
      setAt({ x: r.left + 8, y: r.bottom })
    }
  }

  return (
    <>
      <div className="jr-context-menu-target" onContextMenu={onContextMenu} onKeyDown={onKeyDown}>
        {children}
      </div>
      <Menu
        open={at != null}
        onClose={() => setAt(null)}
        anchor={at ?? { x: 0, y: 0 }}
        items={items}
        aria-label={ariaLabel}
      />
    </>
  )
}

export interface MenuButtonProps extends Omit<IconButtonProps, 'onClick' | 'icon'> {
  items: readonly MenuItem[]
  /** Defaults to an ellipsis. */
  icon?: ReactElement
  placement?: PopoverPlacement
}

/** IconButton that opens a Menu ("..." buttons). */
export function MenuButton({
  items,
  icon,
  placement = 'bottom-end',
  label,
  ...rest
}: MenuButtonProps) {
  const ref = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <>
      <IconButton
        {...rest}
        ref={ref}
        label={label}
        icon={icon ?? <EllipsisIcon />}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      />
      <Menu
        open={open}
        onClose={() => setOpen(false)}
        anchor={ref}
        items={items}
        placement={placement}
        aria-label={label}
      />
    </>
  )
}

function EllipsisIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <circle cx="5" cy="12" r="2" />
      <circle cx="12" cy="12" r="2" />
      <circle cx="19" cy="12" r="2" />
    </svg>
  )
}
