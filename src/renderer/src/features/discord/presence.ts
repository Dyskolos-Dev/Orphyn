import { imageUrl, type BaseItemDto } from '@renderer/jellyfin'
import type { DiscordPresence } from '@shared/ipc/discord'

const TICKS_PER_SECOND = 10_000_000

/** Minimal music player state the presence needs. */
export interface MusicSnapshot {
  track: BaseItemDto | undefined
  status: 'idle' | 'loading' | 'playing' | 'paused' | 'error'
  position: number
  duration: number
}

/** Minimal video controller snapshot the presence needs. */
export interface VideoSnapshot {
  itemId: string | null
  status: 'idle' | 'loading' | 'playing' | 'paused' | 'ended' | 'error'
  positionSeconds: number
  durationSeconds: number
}

export function musicPresence(s: MusicSnapshot): DiscordPresence | null {
  const t = s.track
  if (!t?.Id || (s.status !== 'playing' && s.status !== 'paused' && s.status !== 'loading')) {
    return null
  }
  const duration = s.duration > 0 ? s.duration : (t.RunTimeTicks ?? 0) / TICKS_PER_SECOND
  return {
    kind: 'music',
    paused: s.status === 'paused',
    title: t.Name ?? '',
    artist: t.Artists?.length ? t.Artists.join(', ') : (t.AlbumArtist ?? undefined),
    album: t.Album ?? undefined,
    positionSec: s.position,
    durationSec: duration,
    // Bare URL: the Jellyfin image endpoint needs no key and must never carry one.
    artworkUrl: imageUrl(t, 'Primary', { maxWidth: 512, maxHeight: 512 }),
  }
}

export function videoPresence(
  s: VideoSnapshot,
  item: BaseItemDto | undefined,
): DiscordPresence | null {
  if (
    !s.itemId ||
    !item ||
    item.Id !== s.itemId ||
    (s.status !== 'playing' && s.status !== 'paused' && s.status !== 'loading')
  ) {
    return null
  }
  const isEpisode = item.Type === 'Episode' && !!item.SeriesName
  const duration =
    s.durationSeconds > 0 ? s.durationSeconds : (item.RunTimeTicks ?? 0) / TICKS_PER_SECOND
  return {
    kind: 'video',
    paused: s.status === 'paused',
    title: item.Name ?? '',
    series: isEpisode ? (item.SeriesName ?? undefined) : undefined,
    season: isEpisode ? (item.ParentIndexNumber ?? undefined) : undefined,
    episode: isEpisode ? (item.IndexNumber ?? undefined) : undefined,
    year: item.ProductionYear ?? undefined,
    positionSec: s.positionSeconds,
    durationSec: duration,
    // Series poster for an episode, the movie poster otherwise.
    artworkUrl: imageUrl(
      isEpisode && item.SeriesId
        ? ({
            Id: item.SeriesId,
            Type: 'Series',
            ImageTags: item.SeriesPrimaryImageTag ? { Primary: item.SeriesPrimaryImageTag } : {},
          } as BaseItemDto)
        : item,
      'Primary',
      { maxWidth: 512, maxHeight: 512 },
    ),
  }
}

/** What was last sent, to detect real changes. */
export interface Sent {
  presence: DiscordPresence | null
  at: number
}

/** A progress tick that drifts from where the last update predicted is a seek. */
const SEEK_DRIFT_SEC = 3

/**
 * Whether `next` must be sent: the item, play/pause state or metadata changed, or the position
 * jumped away from the one extrapolated from the last update (a seek). Plain progress ticks are
 * not updates: Discord animates the progress bar by itself.
 */
export function shouldSend(prev: Sent | null, next: DiscordPresence | null, now: number): boolean {
  const before = prev?.presence ?? null
  if (!before || !next) return before !== next
  if (
    before.kind !== next.kind ||
    before.title !== next.title ||
    before.paused !== next.paused ||
    before.artworkUrl !== next.artworkUrl
  ) {
    return true
  }
  if (before.durationSec !== next.durationSec && next.durationSec > 0) return true
  const expected = before.paused
    ? before.positionSec
    : before.positionSec + (now - (prev?.at ?? now)) / 1000
  return Math.abs(next.positionSec - expected) > SEEK_DRIFT_SEC
}
