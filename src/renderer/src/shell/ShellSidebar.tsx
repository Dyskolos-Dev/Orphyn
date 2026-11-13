import {
  CirclePlay,
  Clapperboard,
  Disc3,
  House,
  ListMusic,
  MicVocal,
  Music,
  RotateCw,
  Settings,
  Tv,
} from 'lucide-react'
import { useState, type ReactNode, type Ref } from 'react'
import { useTranslation } from 'react-i18next'
import { useLocation, useNavigate, useSearchParams } from 'react-router'
import {
  formatShortcut,
  IconButton,
  SearchField,
  Sidebar,
  SidebarItem,
  SidebarSection,
} from '@renderer/design'
import { useShellExtensions, type ShellSidebarEntry, type ShellSidebarSection } from './registry'

/**
 * Built-in links. Movies / TV Shows use the library aliases `movies` / `series` of the
 * `/video/library/:libraryId` route (the video feature resolves them to the server's libraries).
 */
export const BUILTIN_SIDEBAR_ITEMS: readonly (Omit<ShellSidebarEntry, 'label'> & {
  labelKey: string
})[] = [
  {
    id: 'watch.home',
    section: 'watch',
    labelKey: 'nav.home',
    icon: <CirclePlay />,
    to: '/video',
    order: 0,
  },
  {
    id: 'watch.movies',
    section: 'watch',
    labelKey: 'nav.movies',
    icon: <Clapperboard />,
    to: '/video/library/movies',
    order: 10,
  },
  {
    id: 'watch.series',
    section: 'watch',
    labelKey: 'nav.series',
    icon: <Tv />,
    to: '/video/library/series',
    order: 20,
  },
  {
    id: 'music.home',
    section: 'music',
    labelKey: 'nav.home',
    icon: <House />,
    to: '/music',
    order: 0,
  },
  {
    id: 'music.albums',
    section: 'music',
    labelKey: 'nav.albums',
    icon: <Disc3 />,
    to: '/music/albums',
    order: 10,
  },
  {
    id: 'music.artists',
    section: 'music',
    labelKey: 'nav.artists',
    icon: <MicVocal />,
    to: '/music/artists',
    order: 20,
  },
  {
    id: 'music.playlists',
    section: 'music',
    labelKey: 'nav.playlists',
    icon: <ListMusic />,
    to: '/music/playlists',
    order: 30,
  },
  {
    id: 'music.songs',
    section: 'music',
    labelKey: 'nav.songs',
    icon: <Music />,
    to: '/music/songs',
    order: 40,
  },
]

export interface ShellSidebarProps {
  searchRef?: Ref<HTMLInputElement>
  /** Element the server-switcher slot portals into. */
  serverSwitcherRef: Ref<HTMLDivElement>
  onRefresh?: () => void
  refreshing?: boolean
}

interface Entry {
  id: string
  label: ReactNode
  icon?: ReactNode
  to: string
  order: number
}

/** The sidebar of the app shell. */
export function ShellSidebar({
  searchRef,
  serverSwitcherRef,
  onRefresh,
  refreshing,
}: ShellSidebarProps) {
  const { t } = useTranslation('shell')
  const extensions = useShellExtensions()
  const location = useLocation()
  const navigate = useNavigate()
  const [params] = useSearchParams()

  // The field mirrors ?q= while on /search; typing navigates there.
  const onSearchPage = location.pathname === '/search'
  const urlQuery = onSearchPage ? (params.get('q') ?? '') : ''
  const [draft, setDraft] = useState(urlQuery)
  const [syncedQuery, setSyncedQuery] = useState(urlQuery)
  if (urlQuery !== syncedQuery) {
    setSyncedQuery(urlQuery)
    setDraft(urlQuery)
  }
  const search = (q: string) => {
    setDraft(q)
    setSyncedQuery(q)
    void navigate(`/search?q=${encodeURIComponent(q)}`, { replace: onSearchPage })
  }

  const entries = (section: ShellSidebarSection): Entry[] => {
    const builtins: Entry[] = BUILTIN_SIDEBAR_ITEMS.filter((i) => i.section === section).map(
      (i) => ({ id: i.id, label: t(i.labelKey), icon: i.icon, to: i.to, order: i.order ?? 0 }),
    )
    const added: Entry[] = extensions.flatMap((e) =>
      (e.sidebarItems ?? [])
        .filter((i) => i.section === section)
        .map((i) => ({ ...i, id: `${e.id}.${i.id}`, order: i.order ?? 100 })),
    )
    return [...builtins, ...added].sort((a, b) => a.order - b.order)
  }

  const renderItems = (list: Entry[]) =>
    list.map((i) => <SidebarItem key={i.id} icon={i.icon} label={i.label} to={i.to} />)

  const more = entries('more')
  const switchers = extensions.filter((e) => e.serverSwitcher)

  return (
    <Sidebar
      aria-label={t('nav.label')}
      header={
        <SearchField
          ref={searchRef}
          aria-label={t('search.label')}
          placeholder={t('search.placeholder')}
          value={draft}
          onChange={search}
          onSubmit={search}
        />
      }
      footer={
        <>
          <div className="jr-shell__footer-row">
            <div ref={serverSwitcherRef} className="jr-shell__server-switcher">
              {switchers.map((e) => {
                const Switcher = e.serverSwitcher!
                return <Switcher key={e.id} />
              })}
            </div>
            {onRefresh && (
              <IconButton
                className="jr-shell__refresh"
                data-spinning={refreshing || undefined}
                icon={<RotateCw />}
                label={t('refresh.label')}
                title={`${t('refresh.label')} (${formatShortcut('R')})`}
                size="small"
                onClick={onRefresh}
              />
            )}
          </div>
          <ul className="jr-sidebar-section__list">
            <SidebarItem icon={<Settings />} label={t('nav.settings')} to="/settings" />
          </ul>
        </>
      }
    >
      <SidebarSection title={t('nav.watch')} collapsible>
        {renderItems(entries('watch'))}
      </SidebarSection>
      <SidebarSection title={t('nav.music')} collapsible>
        {renderItems(entries('music'))}
      </SidebarSection>
      {more.length > 0 && (
        <SidebarSection title={t('nav.more')} collapsible>
          {renderItems(more)}
        </SidebarSection>
      )}
    </Sidebar>
  )
}
