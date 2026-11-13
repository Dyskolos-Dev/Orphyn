import { Clapperboard } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { EmptyState, Shelf, Skeleton } from '@renderer/design'
import {
  flattenPages,
  useItems,
  useLatest,
  useNextUp,
  useResumeItems,
  useUserViews,
  type BaseItemDto,
} from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { HeroCarousel } from './HeroCarousel'
import { ItemLandscape, ItemPoster } from './cards'

const VIDEO_VIEWS = new Set(['movies', 'tvshows'])

function ShelfSkeleton({ landscape = false }: { landscape?: boolean }) {
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

function LibraryLatest({ library }: { library: BaseItemDto }) {
  const { t } = useTranslation('video')
  const latest = useLatest(library.Id, 20)
  if (!latest.data?.length) return null
  return (
    <Shelf
      title={t('home.recentlyAdded', { library: library.Name })}
      kind="poster"
      seeAllTo={`/video/library/${library.Id}`}
    >
      {latest.data.map((item) => (
        <ItemPoster key={item.Id} item={item} />
      ))}
    </Shelf>
  )
}

const hasBackdrop = (i: BaseItemDto) =>
  Boolean(i.BackdropImageTags?.length || i.ParentBackdropImageTags?.length)

/** /video: "Watch Now". */
export function VideoHome() {
  const { t } = useTranslation('video')
  const views = useUserViews()
  const libraries = (views.data ?? []).filter((v) => VIDEO_VIEWS.has(v.CollectionType ?? ''))
  const enabled = libraries.length > 0
  const resume = useResumeItems('video', 20)
  const nextUp = useNextUp(undefined, 20)
  const recent = useItems({
    includeItemTypes: ['Movie', 'Series'],
    sortBy: ['DateCreated'],
    sortOrder: 'Descending',
    pageSize: 24,
    enabled,
  })
  const favourites = useItems({
    includeItemTypes: ['Movie', 'Series'],
    isFavorite: true,
    sortBy: ['SortName'],
    pageSize: 30,
    enabled,
  })

  if (views.isLoading) {
    return (
      <ShellPage title={t('home.title')} showBack={false}>
        <ShelfSkeleton landscape />
        <ShelfSkeleton />
      </ShellPage>
    )
  }
  if (views.isError || !enabled) {
    return (
      <ShellPage title={t('home.title')} showBack={false}>
        <EmptyState
          icon={<Clapperboard />}
          title={views.isError ? t('home.loadError') : t('home.noLibrary.title')}
          message={views.isError ? undefined : t('home.noLibrary.message')}
        />
      </ShellPage>
    )
  }

  const featured = flattenPages(recent.data).filter(hasBackdrop).slice(0, 8)
  const resumeItems = resume.data ?? []
  const resumeIds = new Set(resumeItems.map((i) => i.Id))
  const nextUpItems = (nextUp.data ?? []).filter((i) => !resumeIds.has(i.Id))
  const favouriteItems = flattenPages(favourites.data)
  const heroLoading = recent.isLoading

  return (
    <ShellPage
      title={t('home.title')}
      header={featured.length > 0 ? 'hero' : 'large'}
      showBack={false}
    >
      {featured.length > 0 ? (
        <HeroCarousel items={featured} />
      ) : heroLoading ? (
        <Skeleton aspectRatio="21 / 9" radius="0" />
      ) : null}
      {resumeItems.length > 0 ? (
        <Shelf title={t('home.continueWatching')} kind="landscape">
          {resumeItems.map((item) => (
            <ItemLandscape key={item.Id} item={item} />
          ))}
        </Shelf>
      ) : resume.isLoading ? (
        <ShelfSkeleton landscape />
      ) : null}
      {nextUpItems.length > 0 && (
        <Shelf title={t('home.nextUp')} kind="landscape">
          {nextUpItems.map((item) => (
            <ItemLandscape key={item.Id} item={item} showProgress={false} />
          ))}
        </Shelf>
      )}
      {libraries.map((library) => (
        <LibraryLatest key={library.Id} library={library} />
      ))}
      {favouriteItems.length > 0 && (
        <Shelf title={t('home.favourites')} kind="poster">
          {favouriteItems.map((item) => (
            <ItemPoster key={item.Id} item={item} />
          ))}
        </Shelf>
      )}
    </ShellPage>
  )
}
