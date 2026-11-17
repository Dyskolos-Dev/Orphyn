import clsx from 'clsx'
import { motion, useReducedMotion } from 'framer-motion'
import { CornerDownLeft, Disc3, Film, ListMusic, Music, Search, Tv, User } from 'lucide-react'
import {
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import { Artwork, Spinner, getPlatform, initials, springs } from '@renderer/design'
import { musicPlayer } from '@renderer/features/music'
import { groupResults } from '@renderer/features/search'
import {
  flattenPages,
  imageUrl,
  usePlaylists,
  useSearch,
  type BaseItemDto,
} from '@renderer/jellyfin'
import { paletteRegistry } from './commands'
import { fuzzyMatch, highlightRuns } from './fuzzy'
import {
  buildModel,
  openRoute,
  playAction,
  type LibraryEntry,
  type LibraryKind,
  type LibrarySection,
  type PaletteEntry,
} from './model'
import { initialNavState, navReducer } from './navReducer'
import { readRecentCommands, recordRecentCommand } from './recents'
import { commandIcon, searchCommands, type Lang } from './registry'
import { closePalette } from './store'
import { usePaletteContext } from './usePaletteContext'
import './palette.css'

const DEBOUNCE_MS = 180

const KIND_ICON: Record<LibraryKind, ReactNode> = {
  movie: <Film />,
  series: <Tv />,
  episode: <Tv />,
  artist: <User />,
  album: <Disc3 />,
  song: <Music />,
  playlist: <ListMusic />,
}

const RATIO: Record<LibraryKind, string> = {
  movie: '2/3',
  series: '2/3',
  episode: '16/9',
  artist: '1/1',
  album: '1/1',
  song: '1/1',
  playlist: '1/1',
}

function useDebounced<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), delay)
    return () => clearTimeout(id)
  }, [value, delay])
  return debounced
}

function itemSubtitle(kind: LibraryKind, item: BaseItemDto): string | undefined {
  switch (kind) {
    case 'movie':
    case 'series':
      return item.ProductionYear?.toString()
    case 'episode':
      return item.SeriesName ?? undefined
    case 'album':
      return [item.AlbumArtist, item.ProductionYear].filter(Boolean).join(' · ') || undefined
    case 'song':
      return (
        [item.AlbumArtist ?? item.Artists?.[0], item.Album].filter(Boolean).join(' · ') || undefined
      )
    default:
      return undefined
  }
}

/** Text with the matched letters emphasised. */
function Highlighted({ text, indices }: { text: string; indices: readonly number[] }) {
  return (
    <>
      {highlightRuns(text, indices).map((run, i) =>
        run.match ? (
          <mark key={i} className="jr-palette__mark">
            {run.text}
          </mark>
        ) : (
          <span key={i}>{run.text}</span>
        ),
      )}
    </>
  )
}

export function PalettePanel() {
  const { t, i18n } = useTranslation('palette')
  const lang: Lang = i18n.language?.toLowerCase().startsWith('fr') ? 'fr' : 'en'
  const navigate = useNavigate()
  const reduced = useReducedMotion()
  const ctx = usePaletteContext()
  const [nav, dispatch] = useReducer(navReducer, initialNavState)
  const [recentIds] = useState(readRecentCommands)
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  // Remember what had focus to give it back on close (the panel is unmounted by then).
  useEffect(() => {
    const before = document.activeElement as HTMLElement | null
    inputRef.current?.focus()
    return () => {
      if (before?.isConnected) before.focus({ preventScroll: true })
    }
  }, [])

  const term = useDebounced(nav.query.trim(), DEBOUNCE_MS)
  const settled = term === nav.query.trim()
  const searching = term.length >= 2
  const search = useSearch(term)
  const playlists = usePlaylists({ searchTerm: term, pageSize: 4, enabled: searching })

  const library = useMemo<LibrarySection[]>(() => {
    if (!searching || !settled) return []
    const sections: LibrarySection[] = groupResults(search.data, term).sections.map((s) => ({
      id: s.id,
      kind: s.kind,
      items: s.items,
    }))
    const lists = flattenPages(playlists.data)
    if (lists.length) sections.push({ id: 'playlists', kind: 'playlist', items: lists })
    return sections
  }, [searching, settled, search.data, playlists.data, term])

  const translate = useMemo(
    () => (lng: Lang, key: string) =>
      i18n.getFixedT(lng, 'palette')(key, { defaultValue: '' }) as string,
    [i18n],
  )
  const commands = useMemo(() => paletteRegistry.list(ctx), [ctx])
  const matches = useMemo(
    () => searchCommands(commands, ctx, nav.query, { lang, translate, recentIds }),
    [commands, ctx, nav.query, lang, translate, recentIds],
  )
  const model = useMemo(
    () =>
      buildModel({
        query: nav.query,
        commands: matches,
        recentIds,
        library,
        highlight: (name) => fuzzyMatch(term, name)?.indices ?? [],
      }),
    [nav.query, matches, recentIds, library, term],
  )

  useEffect(() => dispatch({ type: 'clamp', count: model.flat.length }), [model.flat.length])

  const activeEntry: PaletteEntry | undefined = model.flat[nav.index]
  const optionId = (key: string) => `jr-palette-opt-${key}`

  useEffect(() => {
    if (nav.source !== 'keyboard' || !activeEntry) return
    document.getElementById(optionId(activeEntry.key))?.scrollIntoView?.({ block: 'nearest' })
  }, [activeEntry, nav.source, nav.index])

  const run = (entry: PaletteEntry, secondary: boolean) => {
    if (entry.type === 'command') {
      const cmd = entry.match.command
      recordRecentCommand(cmd.id)
      closePalette()
      void cmd.run(ctx)
      return
    }
    const play = secondary ? playAction(entry.kind, entry.item) : null
    closePalette()
    if (play?.type === 'video') navigate(play.route)
    else if (play?.type === 'audio') void musicPlayer.playItem(play.id)
    else navigate(openRoute(entry.kind, entry.item))
  }

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.nativeEvent.isComposing) return
    const count = model.flat.length
    const mod = getPlatform() === 'darwin' ? e.metaKey : e.ctrlKey
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        dispatch({ type: 'move', delta: 1, count })
        break
      case 'ArrowUp':
        e.preventDefault()
        dispatch({ type: 'move', delta: -1, count })
        break
      case 'Tab':
        e.preventDefault()
        dispatch({
          type: 'cycleGroup',
          direction: e.shiftKey ? -1 : 1,
          groupStarts: model.groupStarts,
        })
        break
      case 'Enter':
        e.preventDefault()
        if (activeEntry) run(activeEntry, mod)
        break
      case 'Escape':
        e.preventDefault()
        e.stopPropagation()
        closePalette()
        break
    }
  }

  const loading = searching && (!settled || search.isFetching) && library.length === 0
  const empty = model.flat.length === 0 && !loading
  const canPlay = activeEntry?.type === 'item' && playAction(activeEntry.kind, activeEntry.item)
  const modKey = getPlatform() === 'darwin' ? '⌘' : 'Ctrl'

  const spring = reduced ? { duration: 0.12 } : springs.snappy
  return (
    <div className="jr-palette-layer" onKeyDown={onKeyDown}>
      <motion.div
        className="jr-palette-backdrop"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.18 }}
        onMouseDown={closePalette}
        aria-hidden="true"
      />
      <motion.div
        className="jr-palette jr-material-thick"
        role="dialog"
        aria-modal="true"
        aria-label={t('label')}
        initial={reduced ? { opacity: 0 } : { opacity: 0, scale: 0.95, y: -14 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={reduced ? { opacity: 0 } : { opacity: 0, scale: 0.97, y: -8 }}
        transition={spring}
      >
        <div className="jr-palette__search">
          {loading ? (
            <Spinner size="small" label={t('searching')} />
          ) : (
            <Search aria-hidden="true" />
          )}
          <input
            ref={inputRef}
            className="jr-palette__input"
            type="text"
            role="combobox"
            aria-expanded="true"
            aria-controls="jr-palette-list"
            aria-activedescendant={activeEntry ? optionId(activeEntry.key) : undefined}
            aria-label={t('label')}
            placeholder={t('placeholder')}
            value={nav.query}
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            onChange={(e) => dispatch({ type: 'setQuery', query: e.target.value })}
          />
        </div>

        <div className="jr-palette__list" id="jr-palette-list" role="listbox" ref={listRef}>
          {model.sections.map((section) => {
            const [kind, name] = section.id.split(':') as [string, string | undefined]
            const title =
              kind === 'library'
                ? t(`library.${libraryKind(name ?? '')}`)
                : t(`groups.${kind === 'group' ? name : kind}`)
            return (
              <div key={section.id} role="group" aria-label={title}>
                <div className="jr-palette__header" aria-hidden="true">
                  {title}
                </div>
                {section.entries.map((entry) => {
                  const index = model.flat.indexOf(entry)
                  const active = index === nav.index
                  return (
                    <div
                      key={entry.key}
                      id={optionId(entry.key)}
                      role="option"
                      aria-selected={active}
                      className={clsx('jr-palette__row', active && 'jr-palette__row--active')}
                      onMouseMove={() => dispatch({ type: 'hover', index })}
                      onClick={(e) => run(entry, e.metaKey || e.ctrlKey)}
                    >
                      {entry.type === 'command' ? (
                        <CommandRow entry={entry} ctx={ctx} active={active} />
                      ) : (
                        <ItemRow entry={entry} kindLabel={t(`kinds.${entry.kind}`)} />
                      )}
                    </div>
                  )
                })}
              </div>
            )
          })}
          {empty && (
            <div className="jr-palette__empty">
              <strong>{t('noResults', { query: nav.query.trim() })}</strong>
              <span>{t('noResultsHint')}</span>
            </div>
          )}
        </div>

        <div className="jr-palette__footer" aria-hidden="true">
          <Hint
            keys={<CornerDownLeft />}
            label={activeEntry?.type === 'command' ? t('hints.run') : t('hints.open')}
          />
          {canPlay && <Hint keys={`${modKey}↵`} label={t('hints.play')} />}
          {model.sections.length > 1 && <Hint keys="Tab" label={t('hints.group')} />}
          <Hint keys="Esc" label={t('hints.close')} />
        </div>
      </motion.div>
    </div>
  )
}

function libraryKind(sectionId: string): LibraryKind {
  switch (sectionId) {
    case 'movies':
      return 'movie'
    case 'series':
      return 'series'
    case 'episodes':
      return 'episode'
    case 'artists':
      return 'artist'
    case 'albums':
      return 'album'
    case 'songs':
      return 'song'
    default:
      return 'playlist'
  }
}

function Hint({ keys, label }: { keys: ReactNode; label: string }) {
  return (
    <span className="jr-palette__hint">
      <kbd>{keys}</kbd>
      {label}
    </span>
  )
}

function CommandRow({
  entry,
  ctx,
  active,
}: {
  entry: Extract<PaletteEntry, { type: 'command' }>
  ctx: Parameters<typeof commandIcon>[1]
  active: boolean
}) {
  const { command, title, indices } = entry.match
  const Icon = commandIcon(command, ctx)
  return (
    <>
      <span className="jr-palette__icon">
        <Icon aria-hidden="true" />
      </span>
      <span className="jr-palette__title">
        <Highlighted text={title} indices={indices} />
      </span>
      {command.shortcut && !active && <kbd className="jr-palette__kbd">{command.shortcut}</kbd>}
      {active && <CornerDownLeft className="jr-palette__enter" aria-hidden="true" />}
    </>
  )
}

function ItemRow({ entry, kindLabel }: { entry: LibraryEntry; kindLabel: string }) {
  const { item, kind } = entry
  const subtitle = itemSubtitle(kind, item)
  const round = kind === 'artist'
  return (
    <>
      <span
        className={clsx('jr-palette__thumb', `jr-palette__thumb--${RATIO[kind].replace('/', 'x')}`)}
      >
        <Artwork
          src={imageUrl(item, 'Primary', { maxWidth: 120 })}
          fill
          shape={round ? 'circle' : 'rounded'}
          fallback={round ? initials(item.Name ?? '') : KIND_ICON[kind]}
        />
      </span>
      <span className="jr-palette__text">
        <span className="jr-palette__title">
          <Highlighted text={item.Name ?? ''} indices={entry.indices} />
        </span>
        {subtitle && <span className="jr-palette__subtitle">{subtitle}</span>}
      </span>
      <span className="jr-palette__kind">{kindLabel}</span>
    </>
  )
}
