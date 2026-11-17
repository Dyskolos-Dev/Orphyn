/** Pure aggregation of a user's played items into a year in review. No I/O here. */
import type { BaseItemDto } from '@renderer/jellyfin/types'

const TICKS_PER_MINUTE = 600_000_000

export interface RankedEntry {
  key: string
  name: string
  plays: number
  /** Representative item, used for its artwork. */
  item: BaseItemDto
  /** Secondary line: artist of a song or album, year of a film. */
  subtitle?: string
}

export interface GenreEntry {
  name: string
  plays: number
  /** Share of all genre plays, 0..1. */
  share: number
}

export interface YearStats {
  year: number
  music: {
    plays: number
    minutes: number
    distinctSongs: number
    artists: RankedEntry[]
    songs: RankedEntry[]
    albums: RankedEntry[]
    genres: GenreEntry[]
  }
  video: {
    movies: RankedEntry[]
    series: RankedEntry[]
    moviePlays: number
    episodePlays: number
    minutes: number
  }
  facts: {
    totalMinutes: number
    /** 0-based month with the most plays, null when there is no data. */
    busiestMonth: number | null
    /** Share of music plays going to the top artist, 0..1. */
    topArtistShare: number
    /** Average minutes per day over the year (up to today for the current year). */
    minutesPerDay: number
  }
  isEmpty: boolean
}

export interface AggregateOptions {
  /** Exact play counts per item id from the Playback Reporting plugin; otherwise approximated. */
  pluginPlays?: ReadonlyMap<string, number>
  /** Injected for tests. */
  now?: Date
  limit?: number
}

const yearOf = (iso: string | null | undefined): number | null => {
  if (!iso) return null
  const t = new Date(iso)
  return Number.isNaN(t.getTime()) ? null : t.getFullYear()
}

/** Plays of an item within `year`: exact from the plugin, else PlayCount when last played that year. */
export function playsInYear(
  item: BaseItemDto,
  year: number,
  pluginPlays?: ReadonlyMap<string, number>,
): number {
  if (pluginPlays) return (item.Id && pluginPlays.get(item.Id)) || 0
  const ud = item.UserData
  if (!ud || yearOf(ud.LastPlayedDate) !== year) return 0
  return Math.max(1, ud.PlayCount ?? 0)
}

/** Years (descending) in which something was last played, always including the current one. */
export function availableYears(items: readonly BaseItemDto[], now = new Date()): number[] {
  const years = new Set<number>([now.getFullYear()])
  for (const it of items) {
    const y = yearOf(it.UserData?.LastPlayedDate)
    if (y !== null) years.add(y)
  }
  return [...years].sort((a, b) => b - a)
}

function bump(map: Map<string, RankedEntry>, entry: Omit<RankedEntry, 'plays'>, plays: number) {
  const cur = map.get(entry.key)
  if (cur) cur.plays += plays
  else map.set(entry.key, { ...entry, plays })
}

const top = (map: Map<string, RankedEntry>, limit: number): RankedEntry[] =>
  [...map.values()]
    .sort((a, b) => b.plays - a.plays || a.name.localeCompare(b.name))
    .slice(0, limit)

const minutesOf = (item: BaseItemDto, plays: number) =>
  ((item.RunTimeTicks ?? 0) / TICKS_PER_MINUTE) * plays

const artistOf = (item: BaseItemDto): { id: string; name: string } | null => {
  const a = item.ArtistItems?.[0] ?? item.AlbumArtists?.[0]
  if (a?.Name) return { id: a.Id ?? a.Name, name: a.Name }
  const name = item.AlbumArtist ?? item.Artists?.[0]
  return name ? { id: name, name } : null
}

const isLeap = (y: number) => (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0

export function aggregateYear(
  items: readonly BaseItemDto[],
  year: number,
  opts: AggregateOptions = {},
): YearStats {
  const limit = opts.limit ?? 5
  const now = opts.now ?? new Date()
  const artists = new Map<string, RankedEntry>()
  const songs = new Map<string, RankedEntry>()
  const albums = new Map<string, RankedEntry>()
  const genres = new Map<string, number>()
  const movies = new Map<string, RankedEntry>()
  const series = new Map<string, RankedEntry>()
  const months = new Array<number>(12).fill(0)
  let musicPlays = 0
  let musicMinutes = 0
  let moviePlays = 0
  let episodePlays = 0
  let videoMinutes = 0

  for (const item of items) {
    const plays = playsInYear(item, year, opts.pluginPlays)
    if (plays <= 0 || !item.Id) continue
    const when = item.UserData?.LastPlayedDate
    if (when) {
      const d = new Date(when)
      if (d.getFullYear() === year) months[d.getMonth()] = (months[d.getMonth()] ?? 0) + plays
    }

    if (item.Type === 'Audio') {
      musicPlays += plays
      musicMinutes += minutesOf(item, plays)
      const artist = artistOf(item)
      bump(songs, { key: item.Id, name: item.Name ?? '', item, subtitle: artist?.name }, plays)
      if (artist) bump(artists, { key: artist.id, name: artist.name, item }, plays)
      if (item.AlbumId && item.Album) {
        bump(
          albums,
          { key: item.AlbumId, name: item.Album, item, subtitle: item.AlbumArtist ?? undefined },
          plays,
        )
      }
      for (const g of item.Genres ?? []) genres.set(g, (genres.get(g) ?? 0) + plays)
    } else if (item.Type === 'Movie') {
      moviePlays += plays
      videoMinutes += minutesOf(item, plays)
      bump(
        movies,
        {
          key: item.Id,
          name: item.Name ?? '',
          item,
          subtitle: item.ProductionYear ? String(item.ProductionYear) : undefined,
        },
        plays,
      )
    } else if (item.Type === 'Episode') {
      episodePlays += plays
      videoMinutes += minutesOf(item, plays)
      const key = item.SeriesId ?? item.SeriesName
      if (key) bump(series, { key, name: item.SeriesName ?? item.Name ?? '', item }, plays)
    }
  }

  const genreTotal = [...genres.values()].reduce((a, b) => a + b, 0)
  const genreList: GenreEntry[] = [...genres.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, limit)
    .map(([name, p]) => ({ name, plays: p, share: genreTotal ? p / genreTotal : 0 }))

  const topArtists = top(artists, limit)
  const totalMinutes = Math.round(musicMinutes + videoMinutes)
  const maxMonth = Math.max(...months)
  const dayCount =
    year === now.getFullYear()
      ? Math.max(1, Math.ceil((now.getTime() - new Date(year, 0, 1).getTime()) / 86_400_000))
      : isLeap(year)
        ? 366
        : 365

  return {
    year,
    music: {
      plays: musicPlays,
      minutes: Math.round(musicMinutes),
      distinctSongs: songs.size,
      artists: topArtists,
      songs: top(songs, limit),
      albums: top(albums, limit),
      genres: genreList,
    },
    video: {
      movies: top(movies, limit),
      series: top(series, limit),
      moviePlays,
      episodePlays,
      minutes: Math.round(videoMinutes),
    },
    facts: {
      totalMinutes,
      busiestMonth: maxMonth > 0 ? months.indexOf(maxMonth) : null,
      topArtistShare: musicPlays && topArtists[0] ? topArtists[0].plays / musicPlays : 0,
      minutesPerDay: totalMinutes / dayCount,
    },
    isEmpty: musicPlays + moviePlays + episodePlays === 0,
  }
}

/** Counts plugin rows per item id; rows without a recognisable id are ignored. */
export function countPluginRows(rows: unknown): Map<string, number> | null {
  if (!Array.isArray(rows)) return null
  const counts = new Map<string, number>()
  for (const row of rows) {
    if (!row || typeof row !== 'object') continue
    const r = row as Record<string, unknown>
    const id = r.item_id ?? r.ItemId ?? r.itemId
    if (typeof id === 'string' && id) counts.set(id, (counts.get(id) ?? 0) + 1)
  }
  return counts.size > 0 ? counts : null
}
