import { Clock, Disc3, Film, Music, Search, SearchX, TriangleAlert, Tv, User } from 'lucide-react'
import { useEffect, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useSearchParams } from 'react-router'
import {
  Artwork,
  Button,
  EmptyState,
  LandscapeCard,
  List,
  ListRow,
  PosterCard,
  RoundCard,
  Shelf,
  Skeleton,
  SquareCard,
  Text,
  TrackList,
  TrackRow,
  initials,
} from '@renderer/design'
import { imageUrl, useSearch, type BaseItemDto } from '@renderer/jellyfin'
import { musicPlayer } from '@renderer/features/music'
import { ShellPage } from '@renderer/shell'
import { groupResults, routeFor, type ResultKind, type ResultSection } from './grouping'
import { addRecentSearch, clearRecentSearches, useRecentSearches } from './recents'
import './search.css'

const DEBOUNCE_MS = 250

function useDebounced<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), delay)
    return () => clearTimeout(id)
  }, [value, delay])
  return debounced
}

const ticksToSeconds = (ticks?: number | null) => (ticks ? ticks / 10_000_000 : undefined)

const KIND_ICON: Record<ResultKind, ReactNode> = {
  movie: <Film />,
  series: <Tv />,
  episode: <Tv />,
  artist: <User />,
  album: <Disc3 />,
  song: <Music />,
}

/** `/search?q=`: debounced search across the active server, grouped like Apple Music / TV. */
export function SearchPage() {
  const { t } = useTranslation('search')
  const [params, setParams] = useSearchParams()
  const q = params.get('q') ?? ''
  const term = useDebounced(q.trim(), DEBOUNCE_MS)
  const query = useSearch(term)
  const recents = useRecentSearches()

  // The query follows the URL straight away; a stale debounced term never shows as current.
  const settled = term === q.trim()
  const ready = term.length >= 2 && settled

  return (
    <ShellPage
      title={t('title')}
      subtitle={q.trim().length >= 2 ? t('subtitle', { query: q.trim() }) : undefined}
    >
      {/* Remember the search once the user opens something from its results. */}
      <div
        className="jr-search"
        onClickCapture={(e) => {
          if (ready && (e.target as HTMLElement).closest('a,button')) addRecentSearch(term)
        }}
      >
        {!q.trim() ? (
          <Landing recents={recents} onPick={(s) => setParams({ q: s }, { replace: true })} />
        ) : q.trim().length < 2 ? (
          <EmptyState size="inline" icon={<Search />} title={t('tooShort')} />
        ) : !ready || query.isPending ? (
          <Loading />
        ) : query.isError ? (
          <EmptyState
            icon={<TriangleAlert />}
            title={t('error.title')}
            message={t('error.message')}
            action={
              <Button variant="tinted" onClick={() => void query.refetch()}>
                {t('error.retry')}
              </Button>
            }
          />
        ) : (
          <Results term={term} data={query.data} />
        )}
      </div>
    </ShellPage>
  )
}

function Landing({ recents, onPick }: { recents: string[]; onPick(term: string): void }) {
  const { t } = useTranslation('search')
  if (recents.length === 0) {
    return <EmptyState icon={<Search />} title={t('empty.title')} message={t('empty.message')} />
  }
  return (
    <div className="jr-search__recent">
      <List
        variant="inset"
        header={
          <span className="jr-search__recent-header">
            {t('recent.title')}
            <Button variant="plain" size="small" onClick={clearRecentSearches}>
              {t('recent.clear')}
            </Button>
          </span>
        }
      >
        {recents.map((s) => (
          <ListRow key={s} leading={<Clock />} title={s} onClick={() => onPick(s)} />
        ))}
      </List>
    </div>
  )
}

function Loading() {
  return (
    <div className="jr-search__loading" aria-busy="true">
      <Skeleton aspectRatio="3 / 1" radius="var(--radius-xl)" />
      <Skeleton shape="text" lines={3} />
    </div>
  )
}

function Results({ term, data }: { term: string; data: Parameters<typeof groupResults>[0] }) {
  const { t } = useTranslation('search')
  const grouped = groupResults(data, term)

  if (grouped.total === 0) {
    return (
      <EmptyState
        icon={<SearchX />}
        title={t('noResults.title', { query: term })}
        message={t('noResults.message')}
      />
    )
  }

  return (
    <>
      {grouped.top && (
        <section className="jr-search__top" aria-label={t('topResult')}>
          <Text variant="title-2" as="h2">
            {t('topResult')}
          </Text>
          <TopCard kind={grouped.top.kind} item={grouped.top.item} />
        </section>
      )}
      {grouped.sections.map((section) => (
        <SectionView
          key={section.id}
          section={section}
          onPlaySong={(item) => {
            if (item.Id) void musicPlayer.playItem(item.Id)
          }}
        />
      ))}
    </>
  )
}

function itemSubtitle(kind: ResultKind, item: BaseItemDto): string | undefined {
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
    case 'artist':
      return undefined
  }
}

function TopCard({ kind, item }: { kind: ResultKind; item: BaseItemDto }) {
  const { t } = useTranslation('search')
  const round = kind === 'artist'
  const ratio = kind === 'movie' || kind === 'series' ? '2/3' : kind === 'episode' ? '16/9' : '1/1'
  const subtitle = itemSubtitle(kind, item)
  return (
    <Link to={routeFor(kind, item)} className="jr-search__top-card" draggable={false}>
      <div className={`jr-search__top-art jr-search__top-art--${ratio.replace('/', 'x')}`}>
        <Artwork
          src={imageUrl(item, 'Primary', { maxWidth: 400 })}
          ratio={ratio}
          shape={round ? 'circle' : 'rounded'}
          shadow
          fallback={round ? initials(item.Name ?? '') : KIND_ICON[kind]}
        />
      </div>
      <div className="jr-search__top-text">
        <Text variant="caption" color="secondary">
          {t(`kinds.${kind}`)}
        </Text>
        <Text variant="title-1" as="span">
          {item.Name}
        </Text>
        {subtitle && (
          <Text variant="callout" color="secondary">
            {subtitle}
          </Text>
        )}
      </div>
    </Link>
  )
}

function SectionView({
  section,
  onPlaySong,
}: {
  section: ResultSection
  onPlaySong(item: BaseItemDto): void
}) {
  const { t } = useTranslation('search')
  const title = t(`sections.${section.id}`)

  if (section.id === 'songs') {
    return (
      <section className="jr-search__songs" aria-label={title}>
        <Text variant="title-2" as="h2">
          {title}
        </Text>
        <TrackList aria-label={title}>
          {section.items.map((item) => (
            <TrackRow
              key={item.Id}
              title={item.Name ?? ''}
              artist={item.AlbumArtist ?? item.Artists?.[0]}
              album={item.Album}
              duration={ticksToSeconds(item.RunTimeTicks)}
              artwork={imageUrl(item, 'Primary', { maxWidth: 80 }) ?? null}
              onPlay={() => onPlaySong(item)}
            />
          ))}
        </TrackList>
      </section>
    )
  }

  const kind =
    section.id === 'episodes'
      ? 'landscape'
      : section.id === 'artists'
        ? 'round'
        : section.id === 'albums'
          ? 'square'
          : 'poster'

  return (
    <Shelf title={title} kind={kind}>
      {section.items.map((item) => {
        const to = routeFor(section.kind, item)
        const name = item.Name ?? ''
        switch (section.id) {
          case 'movies':
          case 'series':
            return (
              <PosterCard
                key={item.Id}
                title={name}
                subtitle={itemSubtitle(section.kind, item)}
                image={imageUrl(item, 'Primary', { maxWidth: 300 })}
                fallback={KIND_ICON[section.kind]}
                to={to}
              />
            )
          case 'episodes':
            return (
              <LandscapeCard
                key={item.Id}
                title={name}
                subtitle={[
                  item.ParentIndexNumber != null && item.IndexNumber != null
                    ? t('episodeLabel', {
                        season: item.ParentIndexNumber,
                        episode: item.IndexNumber,
                      })
                    : null,
                  item.SeriesName,
                ]
                  .filter(Boolean)
                  .join(' · ')}
                image={imageUrl(item, 'Thumb', { maxWidth: 480 })}
                fallback={KIND_ICON.episode}
                to={to}
              />
            )
          case 'artists':
            return (
              <RoundCard
                key={item.Id}
                title={name}
                image={imageUrl(item, 'Primary', { maxWidth: 300 })}
                fallback={initials(name)}
                to={to}
              />
            )
          default:
            return (
              <SquareCard
                key={item.Id}
                title={name}
                subtitle={itemSubtitle('album', item)}
                image={imageUrl(item, 'Primary', { maxWidth: 300 })}
                fallback={KIND_ICON.album}
                to={to}
              />
            )
        }
      })}
    </Shelf>
  )
}
