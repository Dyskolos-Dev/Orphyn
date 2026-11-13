import { Check, Clapperboard, Heart, Play, RotateCcw, ChevronsUpDown } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Navigate, useNavigate, useParams, useSearchParams } from 'react-router'
import {
  Button,
  EmptyState,
  HeroBanner,
  IconButton,
  LandscapeCard,
  MenuButton,
  RoundCard,
  SegmentedControl,
  Shelf,
  Skeleton,
  Text,
  type MenuItem,
} from '@renderer/design'
import {
  imageUrl,
  useEpisodes,
  useItem,
  useMarkPlayed,
  useSeasons,
  useSimilar,
  useToggleFavorite,
  type BaseItemDto,
} from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { ItemPoster, itemPath, landscapeImage, playPath } from './cards'
import {
  badgeList,
  formatRuntime,
  metaParts,
  pickSeriesEpisode,
  primaryAction,
  progressOf,
  remainingMinutes,
  sortEpisodes,
  streamBadges,
} from './format'

function Badges({ item }: { item: BaseItemDto }) {
  const list = badgeList(streamBadges(item))
  return (
    <>
      {list.map((b) => (
        <span key={b} className="jr-video-badge">
          {b}
        </span>
      ))}
    </>
  )
}

function EpisodeShelf({
  seriesId,
  initialSeasonId,
  highlightId,
}: {
  seriesId: string
  initialSeasonId?: string
  highlightId?: string
}) {
  const { t } = useTranslation('video')
  const navigate = useNavigate()
  const seasons = useSeasons(seriesId)
  const episodes = useEpisodes(seriesId)
  const [chosen, setChosen] = useState<string | undefined>(initialSeasonId)
  const list = useMemo(() => seasons.data ?? [], [seasons.data])
  const seasonId = chosen ?? list.find((s) => (s.IndexNumber ?? 0) > 0)?.Id ?? list[0]?.Id
  useEffect(() => {
    if (initialSeasonId) setChosen(initialSeasonId)
  }, [initialSeasonId])

  const seasonLabel = (s: BaseItemDto) =>
    s.IndexNumber === 0 || s.IndexNumber == null
      ? (s.Name ?? t('item.specials'))
      : t('item.season', { number: s.IndexNumber })
  const options = list.map((s) => ({ value: s.Id ?? '', label: seasonLabel(s) }))
  const shown = sortEpisodes((episodes.data ?? []).filter((e) => e.SeasonId === seasonId))
  const pickerItems: MenuItem[] = options.map((o) => ({
    label: o.label,
    checked: o.value === seasonId,
    onSelect: () => setChosen(o.value),
  }))

  if (seasons.isLoading || episodes.isLoading) {
    return (
      <Shelf title={t('item.episodes')} kind="landscape">
        {Array.from({ length: 4 }, (_, i) => (
          <Skeleton key={i} aspectRatio="16 / 9" radius="var(--radius-poster)" />
        ))}
      </Shelf>
    )
  }
  if (list.length === 0) return null
  return (
    <section className="jr-video-section" aria-label={t('item.episodes')}>
      <div className="jr-video-seasons">
        {options.length <= 5 ? (
          <SegmentedControl
            aria-label={t('item.seasonPicker')}
            value={seasonId ?? ''}
            onChange={setChosen}
            options={options}
          />
        ) : (
          <>
            <Text variant="title-2">{options.find((o) => o.value === seasonId)?.label}</Text>
            <MenuButton
              label={t('item.seasonPicker')}
              icon={<ChevronsUpDown />}
              items={pickerItems}
              variant="gray"
            />
          </>
        )}
      </div>
      <Shelf kind="landscape" aria-label={t('item.episodes')}>
        {shown.map((e) => {
          const left = remainingMinutes(e)
          const progress = progressOf(e)
          return (
            <LandscapeCard
              key={e.Id}
              title={`${e.IndexNumber != null ? `${e.IndexNumber}. ` : ''}${e.Name ?? ''}`}
              subtitle={formatRuntime(e.RunTimeTicks)}
              description={e.Overview}
              image={landscapeImage(e)}
              progress={progress}
              caption={
                e.UserData?.Played
                  ? t('item.watched')
                  : progress && left > 0
                    ? t('item.minutesLeft', { minutes: left })
                    : undefined
              }
              selected={e.Id === highlightId}
              to={itemPath(e)}
              onPlay={() =>
                e.Id && navigate(playPath(e.Id, e.UserData?.PlaybackPositionTicks ?? 0))
              }
              fallback={<Clapperboard />}
            />
          )
        })}
      </Shelf>
    </section>
  )
}

function CastShelf({ item }: { item: BaseItemDto }) {
  const { t } = useTranslation('video')
  const people = useMemo(() => {
    const seen = new Set<string>()
    return (item.People ?? []).filter((p) => {
      const key = `${p.Id ?? p.Name}`
      if (!p.Name || seen.has(key)) return false
      seen.add(key)
      return true
    })
  }, [item.People])
  if (people.length === 0) return null
  return (
    <Shelf title={t('item.castCrew')} kind="round">
      {people.slice(0, 30).map((p) => (
        <RoundCard
          key={`${p.Id}-${p.Role}-${p.Type}`}
          title={p.Name ?? ''}
          subtitle={p.Role || p.Type || undefined}
          image={
            p.Id && p.PrimaryImageTag
              ? imageUrl({ Id: p.Id, ImageTags: { Primary: p.PrimaryImageTag } }, 'Primary', {
                  maxWidth: 300,
                })
              : undefined
          }
        />
      ))}
    </Shelf>
  )
}

function SimilarShelf({ id }: { id: string }) {
  const { t } = useTranslation('video')
  const similar = useSimilar(id)
  if (!similar.data?.length) return null
  return (
    <Shelf title={t('item.similar')} kind="poster">
      {similar.data.map((s) => (
        <ItemPoster key={s.Id} item={s} />
      ))}
    </Shelf>
  )
}

/** /video/item/:itemId: Detail page for a movie, series or episode. */
export function VideoItem() {
  const { t } = useTranslation('video')
  const { itemId } = useParams()
  const [search] = useSearchParams()
  const navigate = useNavigate()
  const item = useItem(itemId)
  const markPlayed = useMarkPlayed()
  const favourite = useToggleFavorite()
  const data = item.data
  const seriesId = data?.Type === 'Series' ? data.Id : (data?.SeriesId ?? undefined)
  const episodes = useEpisodes(data?.Type === 'Series' ? data.Id : undefined)

  if (item.isLoading) {
    return (
      <ShellPage title="" header="inline">
        <Skeleton aspectRatio="21 / 9" radius="0" />
      </ShellPage>
    )
  }
  if (!data?.Id) {
    return (
      <ShellPage title="" header="inline">
        <EmptyState
          icon={<Clapperboard />}
          title={t('item.notFound.title')}
          message={t('item.notFound.message')}
        />
      </ShellPage>
    )
  }
  if (data.Type === 'Season' && data.SeriesId) {
    return <Navigate to={`${itemPath({ Id: data.SeriesId })}?season=${data.Id}`} replace />
  }

  const id = data.Id
  const isSeries = data.Type === 'Series'
  const targetEpisode = isSeries ? pickSeriesEpisode(episodes.data ?? []) : undefined
  const action = primaryAction(data, targetEpisode)
  const played = data.UserData?.Played === true
  const isFavourite = data.UserData?.IsFavorite === true
  const playId = action.kind === 'episode' ? action.episodeId : id
  const startTicks =
    action.kind === 'resume' || action.kind === 'episode' ? action.positionTicks : 0
  const playable = !isSeries || action.kind === 'episode'

  const label =
    action.kind === 'resume'
      ? t('item.resume', { minutes: action.minutes })
      : action.kind === 'episode' && action.season != null && action.episode != null
        ? t(action.positionTicks > 0 ? 'item.resumeEpisode' : 'item.playEpisode', {
            season: action.season,
            episode: action.episode,
          })
        : t('item.play')
  const trailer = data.RemoteTrailers?.[0]?.Url
  const episodeTag =
    data.Type === 'Episode' && data.ParentIndexNumber != null && data.IndexNumber != null
      ? t('item.episodeLabel', { season: data.ParentIndexNumber, episode: data.IndexNumber })
      : undefined
  const meta = [
    ...metaParts(data),
    isSeries && data.ChildCount ? t('item.seasonsCount', { count: data.ChildCount }) : undefined,
  ].filter(Boolean)

  return (
    <ShellPage title={data.Name ?? ''} header="inline" className="jr-video-detail">
      <HeroBanner
        size="medium"
        backdrop={imageUrl(data, 'Backdrop', { maxWidth: 1920, quality: 80 })}
        logo={imageUrl(data, 'Logo', { maxWidth: 600 })}
        title={data.SeriesName && episodeTag ? data.SeriesName : (data.Name ?? '')}
        eyebrow={episodeTag ? `${episodeTag} · ${data.Name ?? ''}` : undefined}
        meta={
          <>
            {meta.join(' · ')}
            {data.CommunityRating ? ` · ★ ${data.CommunityRating.toFixed(1)}` : ''}{' '}
            <Badges item={data} />
          </>
        }
        description={data.Overview}
        progress={playable ? progressOf(targetEpisode ?? data) : undefined}
        progressCaption={
          playable && progressOf(targetEpisode ?? data)
            ? t('item.minutesLeft', { minutes: remainingMinutes(targetEpisode ?? data) })
            : undefined
        }
        actions={
          <>
            {playable && (
              <Button
                variant="contrast"
                size="large"
                icon={<Play fill="currentColor" />}
                onClick={() => navigate(playPath(playId, startTicks))}
              >
                {label}
              </Button>
            )}
            {playable && startTicks > 0 && (
              <IconButton
                variant="glass"
                size="large"
                icon={<RotateCcw />}
                label={t('item.fromStart')}
                onClick={() => navigate(playPath(playId, 0))}
              />
            )}
            {trailer && (
              <IconButton
                variant="glass"
                size="large"
                icon={<Clapperboard />}
                label={t('item.trailer')}
                onClick={() => window.open(trailer, '_blank', 'noopener,noreferrer')}
              />
            )}
            <IconButton
              variant="glass"
              size="large"
              icon={<Check />}
              label={played ? t('item.markUnwatched') : t('item.markWatched')}
              pressed={played}
              onClick={() => markPlayed.mutate({ id, value: !played })}
            />
            <IconButton
              variant="glass"
              size="large"
              icon={<Heart />}
              label={isFavourite ? t('item.unfavourite') : t('item.favourite')}
              pressed={isFavourite}
              onClick={() => favourite.mutate({ id, value: !isFavourite })}
            />
          </>
        }
      />
      {seriesId && (
        <EpisodeShelf
          seriesId={seriesId}
          initialSeasonId={search.get('season') ?? data.SeasonId ?? undefined}
          highlightId={data.Type === 'Episode' ? id : undefined}
        />
      )}
      {data.Overview && (
        <section className="jr-video-section jr-video-synopsis">
          <Text variant="title-2">{t('item.synopsis')}</Text>
          <Text variant="body" color="secondary" selectable>
            {data.Overview}
          </Text>
        </section>
      )}
      <CastShelf item={data} />
      <SimilarShelf id={seriesId ?? id} />
    </ShellPage>
  )
}
