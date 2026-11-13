import { ArrowUpDown, Clapperboard, ListFilter } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router'
import {
  EmptyState,
  Grid,
  MenuButton,
  SegmentedControl,
  Skeleton,
  Spinner,
  type MenuItem,
} from '@renderer/design'
import { flattenPages, useItems, useUserViews, type BaseItemDto } from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { ItemPoster } from './cards'

type SortKey = 'name' | 'dateAdded' | 'releaseDate' | 'rating'
type PlayedFilter = 'all' | 'unplayed' | 'played'

const SORTS: Record<SortKey, { by: string[]; order: 'Ascending' | 'Descending'; label: string }> = {
  name: { by: ['SortName'], order: 'Ascending', label: 'library.sortName' },
  dateAdded: {
    by: ['DateCreated', 'SortName'],
    order: 'Descending',
    label: 'library.sortDateAdded',
  },
  releaseDate: {
    by: ['PremiereDate', 'ProductionYear', 'SortName'],
    order: 'Descending',
    label: 'library.sortReleaseDate',
  },
  rating: { by: ['CommunityRating', 'SortName'], order: 'Descending', label: 'library.sortRating' },
}

const LETTERS = ['#', ...'ABCDEFGHIJKLMNOPQRSTUVWXYZ']

/** First letter of the sort name (accents stripped); anything else is "#". */
export function letterOf(item: BaseItemDto): string {
  const first = (item.SortName ?? item.Name ?? '').normalize('NFD').replace(/[̀-ͯ]/g, '')[0]
  const up = first?.toUpperCase() ?? '#'
  return /[A-Z]/.test(up) ? up : '#'
}

/** Index of the first loaded item whose letter is >= `letter` (items sorted A-Z), or -1. */
export function firstIndexForLetter(items: readonly BaseItemDto[], letter: string): number {
  if (letter === '#') return items.length > 0 ? 0 : -1
  return items.findIndex((i) => {
    const l = letterOf(i)
    return l !== '#' && l >= letter
  })
}

export function VideoLibrary() {
  const { t } = useTranslation('video')
  const { libraryId = '' } = useParams()
  const views = useUserViews()
  const [sort, setSort] = useState<SortKey>('name')
  const [played, setPlayed] = useState<PlayedFilter>('all')
  const [genre, setGenre] = useState<string | undefined>()
  const [pendingLetter, setPendingLetter] = useState<string | null>(null)

  const alias = libraryId === 'movies' || libraryId === 'series' ? libraryId : null
  const view = alias ? undefined : views.data?.find((v) => v.Id === libraryId)
  const includeItemTypes =
    alias === 'movies' || view?.CollectionType === 'movies'
      ? ['Movie']
      : alias === 'series' || view?.CollectionType === 'tvshows'
        ? ['Series']
        : ['Movie', 'Series']
  const title = alias
    ? t(`library.${alias}`)
    : (view?.Name ?? t(includeItemTypes[0] === 'Series' ? 'library.series' : 'library.movies'))
  const ready = alias !== null || !views.isLoading

  const spec = SORTS[sort]
  const query = useItems({
    parentId: alias ? undefined : libraryId,
    includeItemTypes,
    sortBy: spec.by,
    sortOrder: spec.order,
    filters: played === 'all' ? undefined : [played === 'played' ? 'IsPlayed' : 'IsUnplayed'],
    genres: genre ? [genre] : undefined,
    pageSize: 60,
    enabled: ready,
  })
  const items = useMemo(() => flattenPages(query.data), [query.data])
  const total = query.data?.pages[0]?.total ?? items.length

  // The data layer has no genre list yet: offer the genres of what is loaded (+ the selected one).
  const genres = useMemo(() => {
    const set = new Set<string>(genre ? [genre] : [])
    items.forEach((i) => i.Genres?.forEach((g) => set.add(g)))
    return [...set].sort((a, b) => a.localeCompare(b))
  }, [items, genre])

  const { hasNextPage, isFetchingNextPage, fetchNextPage } = query
  const loadMore = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) void fetchNextPage()
  }, [hasNextPage, isFetchingNextPage, fetchNextPage])

  // Infinite scroll: load the next page when the sentinel comes into view.
  const sentinel = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const el = sentinel.current
    if (!el || typeof IntersectionObserver === 'undefined') return
    const io = new IntersectionObserver((e) => e[0]?.isIntersecting && loadMore(), {
      rootMargin: '600px',
    })
    io.observe(el)
    return () => io.disconnect()
  }, [loadMore, items.length])

  // A-Z rail: scroll to the letter, loading further pages until it is reached.
  useEffect(() => {
    if (!pendingLetter) return
    const i = firstIndexForLetter(items, pendingLetter)
    const lastLetter = items.length > 0 ? letterOf(items[items.length - 1] as BaseItemDto) : '#'
    if (i >= 0 && (pendingLetter === '#' || lastLetter >= pendingLetter || !hasNextPage)) {
      document
        .querySelector(`[data-az-index="${i}"]`)
        ?.scrollIntoView({ block: 'start', behavior: 'smooth' })
      setPendingLetter(null)
    } else if (hasNextPage) {
      loadMore()
    } else {
      setPendingLetter(null)
    }
  }, [pendingLetter, items, hasNextPage, loadMore])

  const sortItems: MenuItem[] = (Object.keys(SORTS) as SortKey[]).map((key) => ({
    label: t(SORTS[key].label),
    checked: sort === key,
    onSelect: () => setSort(key),
  }))
  const genreItems: MenuItem[] = [
    { label: t('library.allGenres'), checked: !genre, onSelect: () => setGenre(undefined) },
    ...genres.map((g) => ({ label: g, checked: genre === g, onSelect: () => setGenre(g) })),
  ]

  return (
    <ShellPage
      title={title}
      subtitle={query.data ? t('library.count', { count: total }) : undefined}
      gutter
      headerAccessory={
        <SegmentedControl
          aria-label={t('library.filter')}
          value={played}
          onChange={(v) => setPlayed(v as PlayedFilter)}
          options={[
            { value: 'all', label: t('library.filterAll') },
            { value: 'unplayed', label: t('library.filterUnplayed') },
            { value: 'played', label: t('library.filterPlayed') },
          ]}
        />
      }
      actions={
        <>
          <MenuButton label={t('library.sort')} icon={<ArrowUpDown />} items={sortItems} />
          <MenuButton label={t('library.genre')} icon={<ListFilter />} items={genreItems} />
        </>
      }
    >
      {query.isLoading || !ready ? (
        <Grid kind="poster">
          {Array.from({ length: 18 }, (_, i) => (
            <Skeleton key={i} aspectRatio="2 / 3" radius="var(--radius-poster)" />
          ))}
        </Grid>
      ) : items.length === 0 ? (
        <EmptyState
          icon={<Clapperboard />}
          title={t('library.empty.title')}
          message={t('library.empty.message')}
        />
      ) : (
        <div className="jr-video-library">
          <Grid kind="poster" aria-label={title}>
            {items.map((item, i) => (
              <div key={item.Id} data-az-index={i} className="jr-video-library__cell">
                <ItemPoster item={item} />
              </div>
            ))}
          </Grid>
          <div ref={sentinel} className="jr-video-library__sentinel" aria-hidden="true" />
          {isFetchingNextPage && (
            <div className="jr-video-library__loading">
              <Spinner />
            </div>
          )}
          {sort === 'name' && (
            <nav className="jr-video-az" aria-label={t('library.jumpTo')}>
              {LETTERS.map((l) => (
                <button key={l} type="button" onClick={() => setPendingLetter(l)}>
                  {l}
                </button>
              ))}
            </nav>
          )}
        </div>
      )}
    </ShellPage>
  )
}
