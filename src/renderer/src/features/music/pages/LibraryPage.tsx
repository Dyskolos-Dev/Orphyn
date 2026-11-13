import { ArrowUpDown } from 'lucide-react'
import { useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Button, EmptyState, Grid, MenuButton, Spinner, type MenuItem } from '@renderer/design'
import { flattenPages, type BaseItemDto } from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { GridSkeleton } from '../components/Cards'
import { useEndReached } from '../components/hooks'

export interface SortOption {
  id: string
  label: string
  sortBy: string[]
  /** Order used when this sort is picked. */
  defaultOrder: 'Ascending' | 'Descending'
}

/** Sort state persisted per page. */
export function useSortPref(key: string, options: readonly SortOption[]) {
  const storageKey = `orphyn.music.sort.${key}`
  const [state, setState] = useState<{ id: string; order: 'Ascending' | 'Descending' }>(() => {
    try {
      const raw = JSON.parse(localStorage.getItem(storageKey) ?? 'null') as {
        id?: string
        order?: string
      } | null
      const found = options.find((o) => o.id === raw?.id)
      if (found) {
        return { id: found.id, order: raw?.order === 'Descending' ? 'Descending' : 'Ascending' }
      }
    } catch {
      /* ignore bad data */
    }
    const first = options[0] as SortOption
    return { id: first.id, order: first.defaultOrder }
  })
  const update = (next: { id: string; order: 'Ascending' | 'Descending' }) => {
    setState(next)
    try {
      localStorage.setItem(storageKey, JSON.stringify(next))
    } catch {
      /* best effort */
    }
  }
  const current = options.find((o) => o.id === state.id) ?? (options[0] as SortOption)
  return {
    sortBy: current.sortBy,
    sortOrder: state.order,
    items: [
      ...options.map((o): MenuItem => ({
        label: o.label,
        checked: o.id === current.id,
        onSelect: () => update({ id: o.id, order: o.defaultOrder }),
      })),
      { type: 'separator' },
      ...(['Ascending', 'Descending'] as const).map((order): MenuItem => ({
        id: order,
        label: order === 'Ascending' ? '↑' : '↓',
        checked: state.order === order,
        onSelect: () => update({ id: current.id, order }),
      })),
    ] as MenuItem[],
  }
}

interface InfiniteQueryLike {
  data: Parameters<typeof flattenPages>[0]
  isLoading: boolean
  isError: boolean
  hasNextPage: boolean
  isFetchingNextPage: boolean
  fetchNextPage(): unknown
  refetch(): unknown
}

interface LibraryPageProps {
  title: string
  query: InfiniteQueryLike
  kind: 'square' | 'round'
  render(item: BaseItemDto): ReactNode
  emptyIcon: ReactNode
  emptyTitle: string
  emptyMessage: string
  sortItems?: readonly MenuItem[]
  count?: number
}

/** Paginated grid page (albums, artists, playlists): loads the next page as the end comes near. */
export function LibraryPage({
  title,
  query,
  kind,
  render,
  emptyIcon,
  emptyTitle,
  emptyMessage,
  sortItems,
}: LibraryPageProps) {
  const { t } = useTranslation('music')
  const items = flattenPages(query.data)
  const total = query.data?.pages[0]?.total
  const sentinel = useEndReached(
    () => !query.isFetchingNextPage && void query.fetchNextPage(),
    query.hasNextPage,
  )

  return (
    <ShellPage
      title={title}
      subtitle={total != null ? t('library.count', { count: total }) : undefined}
      actions={
        sortItems && (
          <MenuButton label={t('library.sort')} icon={<ArrowUpDown />} items={sortItems} />
        )
      }
      gutter
    >
      {query.isLoading ? (
        <GridSkeleton kind={kind} />
      ) : query.isError ? (
        <EmptyState
          title={t('library.error')}
          action={<Button onClick={() => void query.refetch()}>{t('library.retry')}</Button>}
        />
      ) : items.length === 0 ? (
        <EmptyState icon={emptyIcon} title={emptyTitle} message={emptyMessage} />
      ) : (
        <>
          <Grid kind={kind} aria-label={title} className="jrm-grid">
            {items.map((item) => (
              <div key={item.Id}>{render(item)}</div>
            ))}
          </Grid>
          <div ref={sentinel} className="jrm-sentinel" aria-hidden="true">
            {query.isFetchingNextPage && <Spinner />}
          </div>
        </>
      )}
    </ShellPage>
  )
}
