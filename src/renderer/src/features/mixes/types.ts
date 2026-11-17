import type { BaseItemDto } from '@renderer/jellyfin'

export type Track = BaseItemDto

/** Anything that returns numbers in [0, 1): injectable so tests are deterministic. */
export type Rng = () => number

export type MixKind = 'forYou' | 'discovery' | 'onRepeat' | 'rediscover' | 'genre'

export interface Mix {
  /** Stable id, used in the URL of the mix page. */
  id: string
  kind: MixKind
  /** 1-3 for the "For You" mixes. */
  n?: number
  /** Genre of a genre mix. */
  genre?: string
  /** A few artists heard in the mix, for its subtitle. */
  artists: string[]
  /** Hue (0-360) of the generated cover. */
  hue: number
  generatedAt: number
  tracks: Track[]
}

/** What the generator reads from the server (api.ts implements it, tests fake it). */
export interface MixSource {
  /** The user's played tracks (most played first) and loved tracks. */
  signals(): Promise<{ played: Track[]; favourites: Track[] }>
  /** Tracks close to a song (InstantMix). */
  similarTo(trackId: string, limit: number): Promise<Track[]>
  /** Songs of the artists most similar to the given ones, and the artists' own songs. */
  discoveryPool(artistIds: readonly string[], limit: number): Promise<Track[]>
  /** Random songs of a genre. */
  genreTracks(genre: string, limit: number): Promise<Track[]>
}
