import type { BaseItemDto } from '@renderer/jellyfin'

const TICKS_PER_SECOND = 10_000_000
const TICKS_PER_MINUTE = TICKS_PER_SECOND * 60

/** "2 h 10 min", "45 min", "" when unknown. */
export function formatRuntime(ticks: number | null | undefined): string {
  if (!ticks || ticks <= 0) return ''
  const total = Math.max(1, Math.round(ticks / TICKS_PER_MINUTE))
  const h = Math.floor(total / 60)
  const m = total % 60
  if (h === 0) return `${m} min`
  return m === 0 ? `${h} h` : `${h} h ${m} min`
}

/** "1:02:03" / "42:07" from seconds. */
export function formatClock(seconds: number): string {
  const s = Math.max(0, Math.floor(Number.isFinite(seconds) ? seconds : 0))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = String(s % 60).padStart(2, '0')
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
}

export type PrimaryAction =
  | { kind: 'play' }
  /** Resume a movie / episode at `minutes`. */
  | { kind: 'resume'; positionTicks: number; minutes: number }
  /** A series: play (or resume) a specific episode. */
  | { kind: 'episode'; episodeId: string; season?: number; episode?: number; positionTicks: number }

/** Minutes shown in "Resume at 42 min" (rounded, at least 1). */
export function resumeMinutes(positionTicks: number): number {
  return Math.max(1, Math.round(positionTicks / TICKS_PER_MINUTE))
}

/**
 * What the big primary button of a detail page does. For a series pass the episode to play
 * (see `pickSeriesEpisode`).
 */
export function primaryAction(item: BaseItemDto, episode?: BaseItemDto | null): PrimaryAction {
  if (item.Type === 'Series' || item.Type === 'Season') {
    if (!episode?.Id) return { kind: 'play' }
    return {
      kind: 'episode',
      episodeId: episode.Id,
      season: episode.ParentIndexNumber ?? undefined,
      episode: episode.IndexNumber ?? undefined,
      positionTicks: episode.UserData?.PlaybackPositionTicks ?? 0,
    }
  }
  const pos = item.UserData?.PlaybackPositionTicks ?? 0
  return pos > 0
    ? { kind: 'resume', positionTicks: pos, minutes: resumeMinutes(pos) }
    : { kind: 'play' }
}

/** 0..1 watched progress of an item, or undefined when not started. */
export function progressOf(item: BaseItemDto): number | undefined {
  const pos = item.UserData?.PlaybackPositionTicks ?? 0
  const total = item.RunTimeTicks ?? 0
  if (pos > 0 && total > 0) return Math.min(1, pos / total)
  const pct = item.UserData?.PlayedPercentage
  return pct ? Math.min(1, pct / 100) : undefined
}

/** Minutes left, for "24 min left" captions; 0 when unknown. */
export function remainingMinutes(item: BaseItemDto): number {
  const total = item.RunTimeTicks ?? 0
  const pos = item.UserData?.PlaybackPositionTicks ?? 0
  return total > 0 ? Math.max(1, Math.round((total - pos) / TICKS_PER_MINUTE)) : 0
}

/** Episodes ordered by season then episode number; specials (season 0) go last. */
export function sortEpisodes(episodes: readonly BaseItemDto[]): BaseItemDto[] {
  const season = (e: BaseItemDto) => {
    const s = e.ParentIndexNumber ?? 0
    return s === 0 ? Number.MAX_SAFE_INTEGER : s
  }
  const num = (e: BaseItemDto) => e.IndexNumber ?? Number.MAX_SAFE_INTEGER
  return [...episodes].sort(
    (a, b) =>
      season(a) - season(b) ||
      num(a) - num(b) ||
      (a.SortName ?? a.Name ?? '').localeCompare(b.SortName ?? b.Name ?? ''),
  )
}

/** The episode that follows `currentId` in watch order, if any. */
export function nextEpisode(
  episodes: readonly BaseItemDto[],
  currentId: string,
): BaseItemDto | undefined {
  const sorted = sortEpisodes(episodes)
  const i = sorted.findIndex((e) => e.Id === currentId)
  return i >= 0 ? sorted[i + 1] : undefined
}

/** The episode a series' primary button plays: in progress, else Next Up, else first unplayed, else first. */
export function pickSeriesEpisode(
  episodes: readonly BaseItemDto[],
  nextUp?: BaseItemDto | null,
): BaseItemDto | undefined {
  const sorted = sortEpisodes(episodes)
  const inProgress = sorted.find((e) => (e.UserData?.PlaybackPositionTicks ?? 0) > 0)
  if (inProgress) return inProgress
  if (nextUp?.Id) return sorted.find((e) => e.Id === nextUp.Id) ?? nextUp
  return sorted.find((e) => !e.UserData?.Played) ?? sorted[0]
}

export interface StreamBadges {
  resolution?: '4K' | 'HD'
  hdr?: 'HDR' | 'Dolby Vision'
  audio?: 'Atmos' | '7.1' | '5.1'
}

/** 4K / HDR / 5.1 style badges from the first media source's streams. */
export function streamBadges(item: BaseItemDto): StreamBadges {
  const streams = item.MediaSources?.[0]?.MediaStreams ?? item.MediaStreams ?? []
  const video = streams.find((s) => s.Type === 'Video')
  const audio = streams.filter((s) => s.Type === 'Audio')
  const badges: StreamBadges = {}
  if (video) {
    if ((video.Width ?? 0) >= 3800 || (video.Height ?? 0) >= 2100) badges.resolution = '4K'
    else if ((video.Width ?? 0) >= 1280) badges.resolution = 'HD'
    const range = `${video.VideoRangeType ?? ''} ${video.VideoRange ?? ''}`
    if (/dovi|dolby/i.test(range)) badges.hdr = 'Dolby Vision'
    else if (/hdr/i.test(range)) badges.hdr = 'HDR'
  }
  const best = audio.reduce((m, s) => Math.max(m, s.Channels ?? 0), 0)
  if (audio.some((s) => /atmos/i.test(`${s.Profile ?? ''} ${s.DisplayTitle ?? ''}`)))
    badges.audio = 'Atmos'
  else if (best >= 8) badges.audio = '7.1'
  else if (best >= 6) badges.audio = '5.1'
  return badges
}

/** Badge strings in display order (HD is not worth a badge). */
export const badgeList = (b: StreamBadges): string[] =>
  [b.resolution === '4K' ? '4K' : undefined, b.hdr, b.audio].filter((x): x is string => !!x)

/** Year, genres, runtime, rating parts of the metadata line; empty ones dropped. */
export function metaParts(item: BaseItemDto): string[] {
  return [
    item.ProductionYear ? String(item.ProductionYear) : undefined,
    item.Genres?.slice(0, 2).join(', ') || undefined,
    item.Type === 'Series' ? undefined : formatRuntime(item.RunTimeTicks) || undefined,
    item.OfficialRating ?? undefined,
  ].filter((x): x is string => !!x)
}
