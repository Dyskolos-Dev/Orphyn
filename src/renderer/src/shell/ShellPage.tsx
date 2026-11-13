import clsx from 'clsx'
import { ChevronLeft } from 'lucide-react'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { useLocation, useNavigate } from 'react-router'
import { IconButton, Toolbar } from '@renderer/design'
import { useShellContext } from './context'
import { useShellExtensions } from './registry'

export interface ShellPageProps {
  /** Page title: the large title, the collapsed toolbar title and the window title. */
  title: string
  /** Line under the large title (counts, server name...). */
  subtitle?: ReactNode
  /** Right side of the large-title row (sort / filter controls). */
  headerAccessory?: ReactNode
  /** Toolbar buttons on the right (before global extension items). */
  actions?: ReactNode
  /**
   * `large` (default): Apple large title that collapses into the toolbar on scroll.
   * `hero`: no large title; the toolbar is transparent over the first child (a HeroBanner)
   * and turns into a material once it scrolled away.
   * `inline`: the title is always in the toolbar (detail pages), unless the content has its own
   *   large title marked `data-page-title`: the toolbar then shows it once that one scrolled away.
   */
  header?: 'large' | 'hero' | 'inline'
  /** Show a back button in the toolbar (default: when there is history to go back to). */
  showBack?: boolean
  /** Horizontal page gutter around the content (grids, lists). Shelves bring their own. */
  gutter?: boolean
  children?: ReactNode
  className?: string
}

/**
 * Page frame used by every feature page: sticky window toolbar (draggable), large title that
 * collapses on scroll, content. Must be rendered inside AppShell's outlet.
 */
export function ShellPage({
  title,
  subtitle,
  headerAccessory,
  actions,
  header = 'large',
  showBack,
  gutter = false,
  children,
  className,
}: ShellPageProps) {
  const { t } = useTranslation('shell')
  const shell = useShellContext()
  const navigate = useNavigate()
  const location = useLocation()
  const extensions = useShellExtensions()
  const sentinel = useRef<HTMLDivElement>(null)
  const content = useRef<HTMLDivElement>(null)
  const [collapsed, setCollapsed] = useState(header === 'inline')

  useEffect(() => {
    document.title = title ? `${title} – Lumen` : 'Lumen'
  }, [title])

  useEffect(() => {
    const scroller = shell?.scrollRef.current
    if (header === 'inline' && !scroller) {
      setCollapsed(true)
      return
    }
    if (!scroller) return
    let frame = 0
    const update = () => {
      frame = 0
      const toolbar = parseFloat(getComputedStyle(scroller).getPropertyValue('--toolbar-height'))
      const limit = scroller.getBoundingClientRect().top + (Number.isFinite(toolbar) ? toolbar : 52)
      // Large title: collapse once its bottom passes under the toolbar. Hero: once the first
      // child (the HeroBanner) has scrolled under the toolbar.
      if (header === 'inline') {
        const own = content.current?.querySelector('[data-page-title]')
        setCollapsed(own ? own.getBoundingClientRect().bottom <= limit : true)
        return
      }
      const mark =
        header === 'hero' ? content.current?.firstElementChild : (sentinel.current ?? undefined)
      if (!mark) return
      const edge =
        header === 'hero' ? mark.getBoundingClientRect().bottom : mark.getBoundingClientRect().top
      setCollapsed(edge <= limit)
    }
    const onScroll = () => {
      if (!frame) frame = requestAnimationFrame(update)
    }
    update()
    scroller.addEventListener('scroll', onScroll, { passive: true })
    // Detail pages load their header after the first render: re-evaluate when the content resizes.
    const observer =
      header === 'inline' && content.current && typeof ResizeObserver !== 'undefined'
        ? new ResizeObserver(onScroll)
        : null
    if (observer && content.current) observer.observe(content.current)
    return () => {
      observer?.disconnect()
      scroller.removeEventListener('scroll', onScroll)
      if (frame) cancelAnimationFrame(frame)
    }
  }, [shell, header])

  const canGoBack = showBack ?? location.key !== 'default'
  const extensionItems = extensions.filter((e) => e.toolbarItems)

  return (
    <div className={clsx('jr-page', `jr-page--${header}`, className)}>
      <div className="jr-page__toolbar">
        <Toolbar
          title={title}
          showTitle={collapsed}
          appearance={collapsed ? 'material' : 'transparent'}
          leading={
            canGoBack && (
              <IconButton
                icon={<ChevronLeft strokeWidth={2.4} />}
                label={t('nav.back')}
                onClick={() => void navigate(-1)}
              />
            )
          }
          trailing={
            (actions || extensionItems.length > 0) && (
              <>
                {actions}
                {extensionItems.map((e) => {
                  const Items = e.toolbarItems!
                  return <Items key={e.id} />
                })}
              </>
            )
          }
        />
      </div>
      {header === 'large' && (
        <header className="jr-page__header">
          <div className="jr-page__titles">
            <h1 className="jr-page__title">{title}</h1>
            {subtitle && <p className="jr-page__subtitle">{subtitle}</p>}
          </div>
          {headerAccessory && <div className="jr-page__accessory">{headerAccessory}</div>}
          <div ref={sentinel} className="jr-page__sentinel" aria-hidden="true" />
        </header>
      )}
      {header === 'inline' && <h1 className="jr-visually-hidden">{title}</h1>}
      <div ref={content} className={clsx('jr-page__content', gutter && 'jr-page__content--gutter')}>
        {children}
      </div>
    </div>
  )
}
