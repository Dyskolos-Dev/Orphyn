import { Clapperboard } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { EmptyState, Skeleton } from '@renderer/design'
import { countOf, useHomeEditor } from '@renderer/features/home'
import { flattenPages, useItems, useUserViews, type BaseItemDto } from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { HeroCarousel } from './HeroCarousel'
import { ShelfSkeleton, VideoRow } from './homeRows'

const VIDEO_VIEWS = new Set(['movies', 'tvshows'])

const hasBackdrop = (i: BaseItemDto) =>
  Boolean(i.BackdropImageTags?.length || i.ParentBackdropImageTags?.length)

/** /video: "Watch Now", arranged by the user (see features/home). */
export function VideoHome() {
  const { t } = useTranslation('video')
  const views = useUserViews()
  const libraries = (views.data ?? [])
    .filter((v) => VIDEO_VIEWS.has(v.CollectionType ?? ''))
    .flatMap((v) => (v.Id ? [{ id: v.Id, name: v.Name ?? '' }] : []))
  const enabled = libraries.length > 0
  const editor = useHomeEditor('video', libraries)

  const visible = editor.layout.rows.filter((r) => !r.hidden)
  const heroRow = visible.find((r) => r.type === 'hero')
  const recent = useItems({
    includeItemTypes: ['Movie', 'Series'],
    sortBy: ['DateCreated'],
    sortOrder: 'Descending',
    pageSize: 24,
    enabled: enabled && !!heroRow,
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

  const featured = heroRow
    ? flattenPages(recent.data).filter(hasBackdrop).slice(0, countOf(heroRow, 'video'))
    : []
  // The toolbar floats over the hero only when the hero is the first thing on the page.
  const heroFirst = visible[0]?.type === 'hero' && featured.length > 0

  return (
    <ShellPage
      title={t('home.title')}
      header={heroFirst ? 'hero' : 'large'}
      showBack={false}
    >
      {visible.map((row) =>
        row.type === 'hero' ? (
          featured.length > 0 ? (
            <HeroCarousel key={row.id} items={featured} />
          ) : recent.isLoading ? (
            <Skeleton key={row.id} aspectRatio="21 / 9" radius="0" />
          ) : null
        ) : (
          <VideoRow key={row.id} row={row} libraries={libraries} />
        ),
      )}
    </ShellPage>
  )
}
