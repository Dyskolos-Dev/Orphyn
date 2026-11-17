/**
 * Mix generation rules. Everything here is pure (no network, no React): the data comes from a
 * `MixSource` (see api.ts) so the rules can be tested on plain tracks.
 */
import type { Mix, MixKind, MixSource, Rng, Track } from './types'

export const MIN_TRACKS = 25
export const MAX_TRACKS = 50
/** A mix smaller than this is not worth showing (small libraries). */
export const MIN_SHOWN = 10
export const MAX_PER_ARTIST = 3
const DAY_MS = 24 * 60 * 60 * 1000

// ---------------------------------------------------------------- track helpers

export const artistKey = (t: Track): string =>
  (t.ArtistItems?.[0]?.Id ?? t.AlbumArtists?.[0]?.Id ?? t.Artists?.[0] ?? t.AlbumArtist ?? '')
    .toString()
    .toLowerCase()

export const albumKey = (t: Track): string => t.AlbumId ?? t.Album ?? ''

const titleKey = (t: Track): string => `${(t.Name ?? '').trim().toLowerCase()}|${artistKey(t)}`

export const playCount = (t: Track): number => t.UserData?.PlayCount ?? 0

const lastPlayed = (t: Track): number => {
  const d = t.UserData?.LastPlayedDate
  const ms = d ? Date.parse(d) : NaN
  return Number.isFinite(ms) ? ms : 0
}

/** Deterministic generator (mulberry32) so a mix of a given day does not change on every launch. */
export function seededRng(seed: string): Rng {
  let h = 1779033703 ^ seed.length
  for (let i = 0; i < seed.length; i++) {
    h = Math.imul(h ^ seed.charCodeAt(i), 3432918353)
    h = (h << 13) | (h >>> 19)
  }
  let a = h >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

export function shuffled<T>(items: readonly T[], rng: Rng): T[] {
  const out = [...items]
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1))
    ;[out[i], out[j]] = [out[j] as T, out[i] as T]
  }
  return out
}

// ---------------------------------------------------------------- diversity & order

export interface DiversifyOptions {
  limit?: number
  maxPerArtist?: number
  /** Ids (and "title|artist" pairs) that must not appear, e.g. what a radio queued already. */
  exclude?: ReadonlySet<string>
}

/**
 * Keeps the first occurrence of every track (by id, and by title + artist so the same song on a
 * single and on an album is not listed twice) and at most `maxPerArtist` tracks per artist.
 */
export function diversify(tracks: readonly Track[], opts: DiversifyOptions = {}): Track[] {
  const limit = opts.limit ?? MAX_TRACKS
  const max = opts.maxPerArtist ?? MAX_PER_ARTIST
  const ids = new Set<string>()
  const titles = new Set<string>()
  const perArtist = new Map<string, number>()
  const out: Track[] = []
  for (const t of tracks) {
    if (out.length >= limit) break
    const id = t.Id
    if (!id || ids.has(id) || opts.exclude?.has(id)) continue
    const title = titleKey(t)
    if (titles.has(title)) continue
    const artist = artistKey(t)
    const n = perArtist.get(artist) ?? 0
    if (artist && n >= max) continue
    ids.add(id)
    titles.add(title)
    perArtist.set(artist, n + 1)
    out.push(t)
  }
  return out
}

/**
 * Shuffles, then walks the result picking at each step the first track that shares neither album
 * nor artist with the previous one (falling back to "not the same album", then to anything).
 */
export function orderPleasantly(tracks: readonly Track[], rng: Rng): Track[] {
  const rest = shuffled(tracks, rng)
  const out: Track[] = []
  while (rest.length > 0) {
    const prev = out[out.length - 1]
    let at = 0
    if (prev) {
      const album = albumKey(prev)
      const artist = artistKey(prev)
      const differs = (t: Track, strict: boolean) =>
        (!album || albumKey(t) !== album) && (!strict || !artist || artistKey(t) !== artist)
      at = rest.findIndex((t) => differs(t, true))
      if (at < 0) at = rest.findIndex((t) => differs(t, false))
      if (at < 0) at = 0
    }
    out.push(rest.splice(at, 1)[0] as Track)
  }
  return out
}

export interface Pool {
  tracks: readonly Track[]
  /** Relative share of the mix this pool fills. */
  weight: number
}

/** Takes from the pools in proportion to their weights, keeping each pool's own order. */
export function blendPools(pools: readonly Pool[]): Track[] {
  const queues = pools.filter((p) => p.weight > 0).map((p) => ({ ...p, taken: 0 }))
  const out: Track[] = []
  for (;;) {
    let best: (typeof queues)[number] | undefined
    for (const q of queues) {
      if (q.taken >= q.tracks.length) continue
      if (!best || q.taken / q.weight < best.taken / best.weight) best = q
    }
    if (!best) return out
    out.push(best.tracks[best.taken] as Track)
    best.taken++
  }
}

/**
 * One mix: the pools are blended by weight, deduplicated, capped per artist, cut at `MAX_TRACKS`
 * and put in a pleasant order. `null` when too few tracks are left to be worth a mix.
 */
export function assembleTracks(
  pools: readonly Pool[],
  rng: Rng,
  minShown = MIN_SHOWN,
): Track[] | null {
  const picked = diversify(blendPools(pools), { limit: MAX_TRACKS })
  if (picked.length < minShown) return null
  return orderPleasantly(picked, rng)
}

// ---------------------------------------------------------------- listening signals

export interface GenreWeight {
  genre: string
  weight: number
}

/** Genres by how much they are listened to (every play counts, a track never played counts once). */
export function topGenres(tracks: readonly Track[], count: number): GenreWeight[] {
  const weights = new Map<string, number>()
  for (const t of tracks) {
    for (const g of t.Genres ?? []) {
      if (!g) continue
      weights.set(g, (weights.get(g) ?? 0) + playCount(t) + 1)
    }
  }
  return [...weights.entries()]
    .map(([genre, weight]) => ({ genre, weight }))
    .sort((a, b) => b.weight - a.weight || a.genre.localeCompare(b.genre))
    .slice(0, count)
}

/** Most listened artists (ids), by play count. */
export function topArtistIds(tracks: readonly Track[], count: number): string[] {
  const weights = new Map<string, number>()
  for (const t of tracks) {
    const id = t.ArtistItems?.[0]?.Id ?? t.AlbumArtists?.[0]?.Id
    if (id) weights.set(id, (weights.get(id) ?? 0) + playCount(t) + 1)
  }
  return [...weights.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, count)
    .map(([id]) => id)
}

/**
 * Splits played tracks into one cluster per top genre (a track goes to its best ranked genre), the
 * most played tracks first.
 */
export function clusterByGenre(
  played: readonly Track[],
  count: number,
): { genre: string; tracks: Track[] }[] {
  const genres = topGenres(played, count).map((g) => g.genre)
  const clusters = genres.map((genre) => ({ genre, tracks: [] as Track[] }))
  for (const t of played) {
    const at = genres.findIndex((g) => t.Genres?.includes(g))
    if (at >= 0) clusters[at]?.tracks.push(t)
  }
  for (const c of clusters) c.tracks.sort((a, b) => playCount(b) - playCount(a))
  return clusters.filter((c) => c.tracks.length > 0)
}

/** Picks `n` tracks at random, favouring the most played ones. */
export function sampleByPlays(tracks: readonly Track[], n: number, rng: Rng): Track[] {
  const rest = tracks.map((t) => ({ t, w: playCount(t) + 1 }))
  const out: Track[] = []
  while (out.length < n && rest.length > 0) {
    const total = rest.reduce((s, x) => s + x.w, 0)
    let r = rng() * total
    let at = rest.length - 1
    for (let i = 0; i < rest.length; i++) {
      r -= (rest[i] as { w: number }).w
      if (r < 0) {
        at = i
        break
      }
    }
    out.push((rest.splice(at, 1)[0] as { t: Track }).t)
  }
  return out
}

/** Played in the last 30 days, the most played first. */
export function onRepeatTracks(played: readonly Track[], now: number): Track[] {
  return played
    .filter((t) => playCount(t) > 0 && now - lastPlayed(t) <= 30 * DAY_MS && lastPlayed(t) > 0)
    .sort((a, b) => playCount(b) - playCount(a) || lastPlayed(b) - lastPlayed(a))
}

/** Loved tracks (hearts, and songs played a lot) not played for 3 months or more. */
export function rediscoverTracks(
  favourites: readonly Track[],
  played: readonly Track[],
  now: number,
): Track[] {
  const stale = (t: Track) => lastPlayed(t) === 0 || now - lastPlayed(t) >= 90 * DAY_MS
  const hearts = favourites.filter(stale)
  const heard = played.filter((t) => playCount(t) >= 3 && stale(t))
  const byOldest = (a: Track, b: Track) => lastPlayed(a) - lastPlayed(b)
  return [...hearts.sort(byOldest), ...heard.sort(byOldest)]
}

/** Never or rarely played (at most one play). */
export const rarelyPlayed = (tracks: readonly Track[]): Track[] =>
  tracks.filter((t) => playCount(t) <= 1)

// ---------------------------------------------------------------- the mixes

const HUES: Record<MixKind, number> = {
  forYou: 330,
  discovery: 265,
  onRepeat: 8,
  rediscover: 38,
  genre: 160,
}

const hash = (s: string): number => {
  let h = 0
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0
  return Math.abs(h)
}

const hueOf = (kind: MixKind, key: string): number =>
  kind === 'genre' ? hash(key) % 360 : (HUES[kind] + (kind === 'forYou' ? hash(key) % 50 : 0)) % 360

const FOR_YOU = 3
const GENRE_MIXES = 4

const dayKey = (now: number): string => new Date(now).toISOString().slice(0, 10)

function make(
  kind: MixKind,
  tracks: Track[] | null,
  now: number,
  extra: Partial<Pick<Mix, 'n' | 'genre' | 'artists'>> = {},
): Mix | null {
  if (!tracks) return null
  const key = extra.genre ?? String(extra.n ?? '')
  const id = key ? `${kind}-${kind === 'genre' ? hash(key).toString(36) : key}` : kind
  const artists: string[] = []
  for (const t of tracks) {
    const a = t.ArtistItems?.[0]?.Name ?? t.Artists?.[0] ?? t.AlbumArtist
    if (a && !artists.includes(a)) artists.push(a)
    if (artists.length === 3) break
  }
  return { id, kind, hue: hueOf(kind, key), generatedAt: now, ...extra, artists, tracks }
}

/**
 * The daily mixes of a user. Order: For You 1-3, Discovery, On Repeat, Rediscover, genre mixes.
 * A mix the library cannot fill is left out.
 */
export async function generateMixes(
  source: MixSource,
  now: number = Date.now(),
  rngFor: (id: string) => Rng = (id) => seededRng(`${dayKey(now)}:${id}`),
): Promise<Mix[]> {
  const { played, favourites } = await source.signals()
  const mixes: Mix[] = []
  const add = (m: Mix | null) => m && mixes.push(m)

  // For You: one mix per cluster of favourite genres, plus similar tracks they play less.
  const clusters = clusterByGenre(played, FOR_YOU)
  for (const [i, cluster] of clusters.entries()) {
    const n = i + 1
    const rng = rngFor(`forYou-${n}`)
    const seeds = sampleByPlays(cluster.tracks, 32, rng)
    const seed = seeds[0] ?? cluster.tracks[0]
    const similar = seed?.Id ? await source.similarTo(seed.Id, 40).catch(() => []) : []
    const fresh = await source.genreTracks(cluster.genre, 40).catch(() => [])
    add(
      make(
        'forYou',
        assembleTracks(
          [
            { tracks: seeds, weight: 3 },
            { tracks: shuffled([...similar, ...rarelyPlayed(fresh)], rng), weight: 2 },
          ],
          rng,
        ),
        now,
        { n },
      ),
    )
  }

  // Discovery: unplayed tracks from artists close to the favourites.
  {
    const rng = rngFor('discovery')
    const artistIds = topArtistIds([...favourites, ...played], 6)
    const pool = await source.discoveryPool(artistIds, 80).catch(() => [])
    add(
      make(
        'discovery',
        assembleTracks([{ tracks: shuffled(rarelyPlayed(pool), rng), weight: 1 }], rng),
        now,
      ),
    )
  }

  {
    const rng = rngFor('onRepeat')
    add(
      make(
        'onRepeat',
        assembleTracks([{ tracks: onRepeatTracks(played, now), weight: 1 }], rng),
        now,
      ),
    )
  }

  {
    const rng = rngFor('rediscover')
    const pool = rediscoverTracks(favourites, played, now)
    add(make('rediscover', assembleTracks([{ tracks: shuffled(pool, rng), weight: 1 }], rng), now))
  }

  // Genre mixes: top genres, played tracks and a handful of others of the genre.
  for (const { genre } of topGenres(played, GENRE_MIXES)) {
    const rng = rngFor(`genre-${genre}`)
    const own = played.filter((t) => t.Genres?.includes(genre))
    const others = await source.genreTracks(genre, 60).catch(() => [])
    add(
      make(
        'genre',
        assembleTracks(
          [
            { tracks: sampleByPlays(own, 30, rng), weight: 1 },
            { tracks: shuffled(others, rng), weight: 1 },
          ],
          rng,
        ),
        now,
        { genre },
      ),
    )
  }
  return mixes
}
