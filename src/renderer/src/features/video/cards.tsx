import { Check, Film, Heart, Play, Tv } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import { ContextMenu, LandscapeCard, PosterCard, type MenuItem } from '@renderer/design'
import { imageUrl, useMarkPlayed, useToggleFavorite, type BaseItemDto } from '@renderer/jellyfin'
import { progressOf, remainingMinutes } from './format'

export const itemPath = (item: BaseItemDto): string => `/video/item/${item.Id ?? ''}`

export const playPath = (id: string, startTicks = 0): string =>
  `/video/play/${id}${startTicks > 0 ? `?start=${Math.round(startTicks)}` : ''}`

/** A trailer file of the server in Orphyn's player (no resume, nothing reported). */
export const trailerPath = (id: string): string => `/video/play/${id}?trailer=1`

const typeIcon = (item: BaseItemDto) => (item.Type === 'Series' ? <Tv /> : <Film />)

/** 16:9 artwork: episode still, else thumb, else backdrop, else the poster. */
export function landscapeImage(item: BaseItemDto, maxWidth = 640): string | undefined {
  const opts = { maxWidth, quality: 80 }
  if (item.Type === 'Episode') {
    return imageUrl(item, 'Primary', opts) ?? imageUrl(item, 'Thumb', opts)
  }
  return (
    imageUrl(item, 'Thumb', opts) ??
    imageUrl(item, 'Backdrop', opts) ??
    imageUrl(item, 'Primary', opts)
  )
}

/** Right-click / "more" menu shared by every card: play, watched, favourite. */
export function useItemMenu(item: BaseItemDto): MenuItem[] {
  const { t } = useTranslation('video')
  const navigate = useNavigate()
  const played = useMarkPlayed()
  const favourite = useToggleFavorite()
  const id = item.Id
  if (!id) return []
  const isPlayed = item.UserData?.Played === true
  const isFavourite = item.UserData?.IsFavorite === true
  const items: MenuItem[] = []
  if (item.Type === 'Movie' || item.Type === 'Episode') {
    items.push({
      label: t('item.play'),
      icon: <Play />,
      onSelect: () => navigate(playPath(id, item.UserData?.PlaybackPositionTicks ?? 0)),
    })
    items.push({ type: 'separator' })
  }
  items.push(
    {
      label: isPlayed ? t('item.markUnwatched') : t('item.markWatched'),
      icon: <Check />,
      onSelect: () => played.mutate({ id, value: !isPlayed }),
    },
    {
      label: isFavourite ? t('item.unfavourite') : t('item.favourite'),
      icon: <Heart />,
      onSelect: () => favourite.mutate({ id, value: !isFavourite }),
    },
  )
  return items
}

function CardMenu({ item, children }: { item: BaseItemDto; children: React.ReactNode }) {
  const items = useItemMenu(item)
  return <ContextMenu items={items}>{children}</ContextMenu>
}

/** Movie / series poster with watched progress and an unwatched badge. */
export function ItemPoster({ item }: { item: BaseItemDto }) {
  const navigate = useNavigate()
  const unplayed = item.Type === 'Series' ? (item.UserData?.UnplayedItemCount ?? 0) : 0
  return (
    <CardMenu item={item}>
      <PosterCard
        title={item.Name ?? ''}
        subtitle={item.ProductionYear ? String(item.ProductionYear) : undefined}
        image={imageUrl(item, 'Primary', { maxWidth: 400, quality: 80 })}
        to={itemPath(item)}
        progress={progressOf(item)}
        badge={unplayed > 0 ? unplayed : undefined}
        fallback={typeIcon(item)}
        onPlay={
          item.Type === 'Movie' && item.Id
            ? () => navigate(playPath(item.Id as string, item.UserData?.PlaybackPositionTicks ?? 0))
            : undefined
        }
      />
    </CardMenu>
  )
}

/** Continue Watching / Next Up card (16:9). Episodes show the series name. */
export function ItemLandscape({
  item,
  showProgress = true,
}: {
  item: BaseItemDto
  showProgress?: boolean
}) {
  const { t } = useTranslation('video')
  const navigate = useNavigate()
  const isEpisode = item.Type === 'Episode'
  const left = remainingMinutes(item)
  const episodeLabel =
    isEpisode && item.ParentIndexNumber != null && item.IndexNumber != null
      ? t('item.episodeLabel', { season: item.ParentIndexNumber, episode: item.IndexNumber })
      : undefined
  return (
    <CardMenu item={item}>
      <LandscapeCard
        title={isEpisode ? (item.SeriesName ?? item.Name ?? '') : (item.Name ?? '')}
        subtitle={
          isEpisode ? [episodeLabel, item.Name].filter(Boolean).join(' · ') : item.ProductionYear
        }
        image={landscapeImage(item)}
        to={itemPath(item)}
        progress={showProgress ? progressOf(item) : undefined}
        caption={
          showProgress && left > 0 && progressOf(item)
            ? t('item.minutesLeft', { minutes: left })
            : undefined
        }
        fallback={typeIcon(item)}
        onPlay={() =>
          item.Id && navigate(playPath(item.Id, item.UserData?.PlaybackPositionTicks ?? 0))
        }
      />
    </CardMenu>
  )
}
