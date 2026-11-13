import clsx from 'clsx'
import { ChevronDown } from 'lucide-react'
import { useId, useState, type MouseEvent, type ReactNode } from 'react'
import { NavLink } from 'react-router'
import './navigation.css'

export interface SidebarProps {
  /** Accessible name of the <nav>. */
  'aria-label': string
  /** Pinned above the scrolling sections (search field). Sits under the traffic lights. */
  header?: ReactNode
  /** Pinned at the bottom (server switcher, settings). */
  footer?: ReactNode
  children: ReactNode
  className?: string
}

/**
 * Source list. Translucent: it shows the native window material (macOS
 * vibrancy, Windows Mica) or the sidebar surface where there is none.
 */
export function Sidebar({
  header,
  footer,
  children,
  className,
  'aria-label': label,
}: SidebarProps) {
  return (
    <nav aria-label={label} className={clsx('jr-sidebar', className)}>
      <div className="jr-sidebar__titlebar jr-drag" aria-hidden="true" />
      {header && <div className="jr-sidebar__header">{header}</div>}
      <div className="jr-sidebar__scroll">{children}</div>
      {footer && <div className="jr-sidebar__footer">{footer}</div>}
    </nav>
  )
}

export interface SidebarSectionProps {
  /** Small grey heading (e.g. "Watch", "Music"). Omit for an untitled group. */
  title?: ReactNode
  /** Heading becomes a button that shows / hides the items (state kept in memory). */
  collapsible?: boolean
  defaultCollapsed?: boolean
  children: ReactNode
  className?: string
}

/** Titled group of SidebarItems. */
export function SidebarSection({
  title,
  collapsible = false,
  defaultCollapsed = false,
  children,
  className,
}: SidebarSectionProps) {
  const [collapsed, setCollapsed] = useState(defaultCollapsed)
  const listId = useId()
  const headingId = useId()
  return (
    <section
      className={clsx('jr-sidebar-section', className)}
      aria-labelledby={title ? headingId : undefined}
    >
      {title &&
        (collapsible ? (
          <h2 className="jr-sidebar-section__title" id={headingId}>
            <button
              type="button"
              className="jr-sidebar-section__toggle"
              aria-expanded={!collapsed}
              aria-controls={listId}
              onClick={() => setCollapsed((c) => !c)}
            >
              {title}
              <ChevronDown
                className={clsx(
                  'jr-sidebar-section__chevron',
                  collapsed && 'jr-sidebar-section__chevron--collapsed',
                )}
                aria-hidden="true"
              />
            </button>
          </h2>
        ) : (
          <h2 className="jr-sidebar-section__title" id={headingId}>
            {title}
          </h2>
        ))}
      <ul id={listId} className="jr-sidebar-section__list" hidden={collapsible && collapsed}>
        {children}
      </ul>
    </section>
  )
}

export interface SidebarItemProps {
  /** lucide icon, drawn in the accent colour . */
  icon?: ReactNode
  label: ReactNode
  /** Router path; the item is highlighted while it is the current route. */
  to?: string
  /** Only match `to` exactly (use for "Home" items). Default true. */
  end?: boolean
  onClick?(e: MouseEvent): void
  /** Highlight manually (when not using `to`). */
  active?: boolean
  /** Trailing content (count, status dot, spinner). */
  trailing?: ReactNode
  onContextMenu?(e: MouseEvent): void
  className?: string
}

/** One sidebar row: accent icon + label, rounded highlight when active. Renders an <li>. */
export function SidebarItem({
  icon,
  label,
  to,
  end = true,
  onClick,
  active,
  trailing,
  onContextMenu,
  className,
}: SidebarItemProps) {
  const inner = (
    <>
      {icon && (
        <span className="jr-sidebar-item__icon" aria-hidden="true">
          {icon}
        </span>
      )}
      <span className="jr-sidebar-item__label">{label}</span>
      {trailing && <span className="jr-sidebar-item__trailing">{trailing}</span>}
    </>
  )
  return (
    <li className={clsx('jr-sidebar-item', className)} onContextMenu={onContextMenu}>
      {to ? (
        <NavLink
          to={to}
          end={end}
          onClick={onClick}
          draggable={false}
          className={({ isActive }) =>
            clsx('jr-sidebar-item__link', (active ?? isActive) && 'jr-sidebar-item__link--active')
          }
        >
          {inner}
        </NavLink>
      ) : (
        <button
          type="button"
          onClick={onClick}
          aria-current={active ? 'page' : undefined}
          className={clsx('jr-sidebar-item__link', active && 'jr-sidebar-item__link--active')}
        >
          {inner}
        </button>
      )}
    </li>
  )
}
