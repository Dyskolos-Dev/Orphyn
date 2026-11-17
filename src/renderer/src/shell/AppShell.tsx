import clsx from 'clsx'
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Outlet, useLocation, useNavigate } from 'react-router'
import { ThemeProvider, ToastViewport, useWindowState } from '@renderer/design'
import { ShellAmbient } from './ShellAmbient'
import { applyWindowStateToDocument } from '@renderer/design/platform'
import { ShellContext, useImmersiveState, type ShellContextValue } from './context'
import { useShellExtensions, type ShellExtension } from './registry'
import { ShellSidebar } from './ShellSidebar'
import { UpdateNotice } from './UpdateNotice'
import { useLibraryRefresh, useLibraryRefreshTriggers } from './useLibraryRefresh'
import './shell.css'

/**
 * Application layout (route element of every page): theme, translucent sidebar, scrolling content
 * with the routed page, now-playing bar, toasts. Owner: shell agent.
 */
export function AppShell() {
  const extensions = useShellExtensions()
  // Feature providers wrap the whole layout, in registration order (first = outermost).
  const providers = extensions.filter(
    (e): e is ShellExtension & { provider: NonNullable<ShellExtension['provider']> } =>
      e.provider != null,
  )
  let content: ReactNode = <ShellLayout />
  for (const e of [...providers].reverse()) {
    const Provider = e.provider
    content = <Provider key={e.id}>{content}</Provider>
  }
  return <ThemeProvider>{content}</ThemeProvider>
}

function ShellLayout() {
  const { t } = useTranslation('shell')
  const extensions = useShellExtensions()
  const windowState = useWindowState()
  const { pathname } = useLocation()
  const scrollRef = useRef<HTMLDivElement>(null)
  const searchRef = useRef<HTMLInputElement>(null)
  const [nowPlayingTarget, setNowPlayingTarget] = useState<HTMLElement | null>(null)
  const [serverSwitcherTarget, setServerSwitcherTarget] = useState<HTMLDivElement | null>(null)
  const [immersive, setImmersiveCount] = useImmersiveState()

  const { platform, isFullscreen, isMaximized, isFocused, material } = windowState
  const { refresh, fetching } = useLibraryRefresh()
  useLibraryRefreshTriggers(platform, refresh)
  const navigate = useNavigate()
  useEffect(() => window.orphyn?.app?.onOpenSettings(() => void navigate('/settings')), [navigate])
  useEffect(() => {
    applyWindowStateToDocument(platform, { isFullscreen, isMaximized, isFocused, material })
  }, [platform, isFullscreen, isMaximized, isFocused, material])

  // A new page starts at the top.
  useEffect(() => {
    scrollRef.current?.scrollTo?.({ top: 0 })
  }, [pathname])

  // Cmd+F (macOS) / Ctrl+F elsewhere focuses the sidebar search.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = platform === 'darwin' ? e.metaKey : e.ctrlKey
      if (mod && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'f') {
        e.preventDefault()
        searchRef.current?.focus()
        searchRef.current?.select()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [platform])

  const ctx = useMemo<ShellContextValue>(
    () => ({ scrollRef, nowPlayingTarget, serverSwitcherTarget, setImmersiveCount }),
    [nowPlayingTarget, serverSwitcherTarget, setImmersiveCount],
  )

  return (
    <ShellContext.Provider value={ctx}>
      <div className={clsx('jr-shell', immersive && 'jr-shell--immersive')}>
        <ShellAmbient scrollRef={scrollRef} fallback={nowPlayingTarget} hidden={immersive} />
        <div className="jr-shell__sidebar" hidden={immersive}>
          <ShellSidebar
            searchRef={searchRef}
            serverSwitcherRef={setServerSwitcherTarget}
            onRefresh={refresh}
            refreshing={fetching}
          />
        </div>
        <div className="jr-shell__main">
          <main ref={scrollRef} className="jr-shell__scroll">
            <Outlet />
          </main>
          <section
            ref={setNowPlayingTarget}
            className="jr-shell__now-playing jr-material-thick"
            aria-label={t('slots.nowPlaying')}
            hidden={immersive}
          >
            {extensions.map((e) => {
              const NowPlaying = e.nowPlaying
              return NowPlaying ? <NowPlaying key={e.id} /> : null
            })}
          </section>
        </div>
        {extensions.map((e) => {
          const Background = e.background
          return Background ? <Background key={e.id} /> : null
        })}
        <ToastViewport />
        <UpdateNotice />
      </div>
    </ShellContext.Provider>
  )
}
