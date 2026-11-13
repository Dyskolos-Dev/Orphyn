import type { BaseItemDto } from '@jellyfin/sdk/lib/generated-client/models'

export type { BaseItemDto }

export type ImageKind = 'Primary' | 'Backdrop' | 'Logo' | 'Thumb'

export type ListKind = 'albums' | 'artists' | 'albumArtists' | 'playlists' | 'songs'

/** Parameters shared by every paginated list hook. */
export interface ListParams {
  /** Restrict to a library / folder. */
  parentId?: string
  searchTerm?: string
  sortBy?: string[]
  sortOrder?: 'Ascending' | 'Descending'
  /** Only favourites. */
  isFavorite?: boolean
  genres?: string[]
  /** Page size (default 50). */
  pageSize?: number
  enabled?: boolean
}

/** Parameters of the generic `useItems` query (subset of GET /Items worth exposing). */
export interface ItemsParams extends ListParams {
  includeItemTypes?: string[]
  /** Default true. */
  recursive?: boolean
  filters?: string[]
  artistIds?: string[]
  albumArtistIds?: string[]
  years?: number[]
  fields?: string[]
  mediaTypes?: string[]
}

export interface ItemsPage {
  items: BaseItemDto[]
  total: number
  startIndex: number
}

export interface TimedLyricLine {
  text: string
  /** Start in milliseconds; null for unsynced lyrics. */
  startMs: number | null
}

export interface Lyrics {
  /** True when every line has a start time (karaoke-style display possible). */
  synced: boolean
  lines: TimedLyricLine[]
}

export interface SearchResults {
  movies: BaseItemDto[]
  series: BaseItemDto[]
  episodes: BaseItemDto[]
  albums: BaseItemDto[]
  artists: BaseItemDto[]
  tracks: BaseItemDto[]
}
