import { useTranslation } from 'react-i18next'
import { Shelf, Skeleton } from '@renderer/design'
import { RowFrame, countOf, rowTitle, type HomeRow } from '@renderer/features/home'
import {
  flattenPages,
  useItems,
  useLatest,
  useNextUp,
  useRandomItems,
  useRecentlyReleased,
  useResumeItems,
  useSimilar,
  type BaseItemDto,
} from '@renderer/jellyfin'
import { ItemLandscape, ItemPoster } from './cards'

const VIDEO_TYPES = ['Movie', 'Series']

export function ShelfSkeleton({ landscape = false }: { landscape?: boolean }) {
  return (
    <Shelf title={<Skeleton shape="text" />} kind={landscape ? 'landscape' : 'poster'}>
      {Array.from({ length: 8 }, (_, i) => (
        <Skeleton
          key={i}
          aspectRatio={landscape ? '16 / 9' : '2 / 3'}
          radius="var(--radius-poster)"
        />
      ))}
    </Shelf>
  )
}

function PosterShelf({
  row,
  items,
  title,
  seeAllTo,
}: {
  row: HomeRow
  items: BaseItemDto[]
  title: string
  seeAllTo?: string
}) {
  if (items.length === 0) return null
  return (
    <RowFrame size={row.size}>
      <Shelf title={title} kind="poster" seeAllTo={seeAllTo}>
        {items.map((item) => (
          <ItemPoster key={item.Id} item={item} />
        ))}
      </Shelf>
    </RowFrame>
  )
}

type RowProps = { row: HomeRow; libraries: { id: string; name: string }[] }

function useTitle(row: HomeRow, label?: string): string {
  const { t } = useTranslation('home')
  return rowTitle(t, 'video', label ? { ...row, label } : row)
}

function ContinueRow({ row }: RowProps) {
  const resume = useResumeItems('video', countOf(row, 'video'))
  const title = useTitle(row)
  const items = resume.data ?? []
  if (items.length === 0) return resume.isLoading ? <ShelfSkeleton landscape /> : null
  return (
    <RowFrame size={row.size}>
      <Shelf title={title} kind="landscape">
        {items.map((item) => (
          <ItemLandscape key={item.Id} item={item} />
        ))}
      </Shelf>
    </RowFrame>
  )
}

function NextUpRow({ row }: RowProps) {
  const count = countOf(row, 'video')
  const resume = useResumeItems('video', 20)
  const nextUp = useNextUp(undefined, count)
  const title = useTitle(row)
  const resumeIds = new Set((resume.data ?? []).map((i) => i.Id))
  const items = (nextUp.data ?? []).filter((i) => !resumeIds.has(i.Id))
  if (items.length === 0) return null
  return (
    <RowFrame size={row.size}>
      <Shelf title={title} kind="landscape">
        {items.map((item) => (
          <ItemLandscape key={item.Id} item={item} showProgress={false} />
        ))}
      </Shelf>
    </RowFrame>
  )
}

function LatestRow({ row, libraries }: RowProps) {
  const library = libraries.find((l) => l.id === row.param)
  const latest = useLatest(library?.id, countOf(row, 'video'))
  const title = useTitle(row, library?.name)
  if (!library) return null
  return (
    <PosterShelf
      row={row}
      items={latest.data ?? []}
      title={title}
      seeAllTo={`/video/library/${library.id}`}
    />
  )
}

function FavouritesRow({ row }: RowProps) {
  const q = useItems({
    includeItemTypes: VIDEO_TYPES,
    isFavorite: true,
    sortBy: ['SortName'],
    pageSize: countOf(row, 'video'),
  })
  return <PosterShelf row={row} items={flattenPages(q.data)} title={useTitle(row)} />
}

function GenreRow({ row }: RowProps) {
  const q = useItems({
    includeItemTypes: VIDEO_TYPES,
    genres: row.param ? [row.param] : undefined,
    sortBy: ['DateCreated'],
    sortOrder: 'Descending',
    pageSize: countOf(row, 'video'),
    enabled: !!row.param,
  })
  return <PosterShelf row={row} items={flattenPages(q.data)} title={useTitle(row)} />
}

function CollectionRow({ row }: RowProps) {
  const q = useItems({
    parentId: row.param,
    sortBy: ['PremiereDate', 'SortName'],
    pageSize: countOf(row, 'video'),
    enabled: !!row.param,
  })
  return (
    <PosterShelf
      row={row}
      items={flattenPages(q.data)}
      title={useTitle(row)}
      seeAllTo={row.param ? `/video/item/${row.param}` : undefined}
    />
  )
}

/** Similar titles to the last thing watched. */
function BecauseRow({ row }: RowProps) {
  const { t } = useTranslation('home')
  const played = useItems({
    includeItemTypes: ['Movie', 'Episode'],
    filters: ['IsPlayed'],
    sortBy: ['DatePlayed'],
    sortOrder: 'Descending',
    pageSize: 1,
  })
  const last = flattenPages(played.data)[0]
  // An episode stands for its series.
  const seedId = last?.Type === 'Episode' ? last.SeriesId : last?.Id
  const seedName = last?.Type === 'Episode' ? last.SeriesName : last?.Name
  const similar = useSimilar(seedId ?? undefined, countOf(row, 'video'))
  return (
    <PosterShelf
      row={row}
      items={similar.data ?? []}
      title={t('rows.video.because.title', { name: seedName ?? '' })}
    />
  )
}

function ReleasedRow({ row }: RowProps) {
  const q = useRecentlyReleased(countOf(row, 'video'))
  return <PosterShelf row={row} items={flattenPages(q.data)} title={useTitle(row)} />
}

function RandomRow({ row }: RowProps) {
  const q = useRandomItems({ includeItemTypes: VIDEO_TYPES, pageSize: countOf(row, 'video') })
  return <PosterShelf row={row} items={flattenPages(q.data)} title={useTitle(row)} />
}

/** A shelf of the video home, by row type (the hero is handled by the page). */
export function VideoRow(props: RowProps) {
  switch (props.row.type) {
    case 'continue':
      return <ContinueRow {...props} />
    case 'nextUp':
      return <NextUpRow {...props} />
    case 'latest':
      return <LatestRow {...props} />
    case 'favourites':
      return <FavouritesRow {...props} />
    case 'genre':
      return <GenreRow {...props} />
    case 'collection':
      return <CollectionRow {...props} />
    case 'because':
      return <BecauseRow {...props} />
    case 'released':
      return <ReleasedRow {...props} />
    case 'random':
      return <RandomRow {...props} />
    default:
      return null
  }
}
