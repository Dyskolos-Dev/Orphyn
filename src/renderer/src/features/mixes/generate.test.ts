import { describe, expect, it } from 'vitest'
import {
  MAX_PER_ARTIST,
  MAX_TRACKS,
  albumKey,
  artistKey,
  blendPools,
  clusterByGenre,
  diversify,
  generateMixes,
  onRepeatTracks,
  orderPleasantly,
  rediscoverTracks,
  seededRng,
  topGenres,
} from './generate'
import type { MixSource, Track } from './types'

const DAY = 24 * 60 * 60 * 1000
const NOW = Date.parse('2026-11-17T12:00:00Z')

let seq = 0
function track(o: {
  artist: string
  album?: string
  genres?: string[]
  plays?: number
  daysAgo?: number
  fav?: boolean
  name?: string
}): Track {
  seq += 1
  return {
    Id: `t${seq}`,
    Name: o.name ?? `Song ${seq}`,
    Type: 'Audio',
    Album: o.album ?? `${o.artist} album`,
    AlbumId: `${o.album ?? o.artist}-album`,
    Artists: [o.artist],
    ArtistItems: [{ Id: o.artist, Name: o.artist }],
    Genres: o.genres ?? [],
    UserData: {
      PlayCount: o.plays ?? 0,
      IsFavorite: o.fav ?? false,
      LastPlayedDate:
        o.daysAgo === undefined ? undefined : new Date(NOW - o.daysAgo * DAY).toISOString(),
    },
  } as Track
}

const many = (n: number, f: (i: number) => Track): Track[] =>
  Array.from({ length: n }, (_, i) => f(i))
const count = <T>(xs: T[], key: (x: T) => string): Map<string, number> => {
  const m = new Map<string, number>()
  for (const x of xs) m.set(key(x), (m.get(key(x)) ?? 0) + 1)
  return m
}

describe('diversify', () => {
  it('keeps at most 3 tracks per artist', () => {
    const tracks = many(10, () => track({ artist: 'A' }))
    expect(diversify(tracks)).toHaveLength(MAX_PER_ARTIST)
  })

  it('drops duplicate ids and the same song listed twice', () => {
    const a = track({ artist: 'A', name: 'Same' })
    const b = track({ artist: 'A', name: 'same' })
    expect(diversify([a, a, b])).toEqual([a])
  })

  it('stops at the limit and honours exclusions', () => {
    const tracks = many(30, (i) => track({ artist: `ar${i}` }))
    expect(diversify(tracks, { limit: 5 })).toHaveLength(5)
    const out = diversify(tracks, { exclude: new Set([tracks[0]?.Id ?? '']) })
    expect(out.find((t) => t.Id === tracks[0]?.Id)).toBeUndefined()
  })
})

describe('orderPleasantly', () => {
  it('never puts two tracks of one album back to back when it can be avoided', () => {
    const tracks = many(40, (i) => track({ artist: `ar${i % 10}`, album: `al${i % 8}` }))
    for (const seed of ['a', 'b', 'c', 'd']) {
      const out = orderPleasantly(tracks, seededRng(seed))
      expect(out).toHaveLength(tracks.length)
      expect(new Set(out.map((t) => t.Id)).size).toBe(tracks.length)
      const adjacent = out.filter((t, i) => i > 0 && albumKey(t) === albumKey(out[i - 1] as Track))
      expect(adjacent).toHaveLength(0)
    }
  })

  it('is deterministic for a given seed and differs between seeds', () => {
    const tracks = many(20, (i) => track({ artist: `ar${i}` }))
    const ids = (s: string) => orderPleasantly(tracks, seededRng(s)).map((t) => t.Id)
    expect(ids('x')).toEqual(ids('x'))
    expect(ids('x')).not.toEqual(ids('y'))
  })
})

describe('blendPools', () => {
  it('takes from the pools in proportion to their weight', () => {
    const a = many(30, (i) => track({ artist: `a${i}` }))
    const b = many(30, (i) => track({ artist: `b${i}` }))
    const out = blendPools([
      { tracks: a, weight: 3 },
      { tracks: b, weight: 1 },
    ]).slice(0, 20)
    expect(out.filter((t) => a.includes(t))).toHaveLength(15)
  })
})

describe('listening signals', () => {
  it('ranks genres by plays', () => {
    const tracks = [
      track({ artist: 'a', genres: ['Rock'], plays: 10 }),
      track({ artist: 'b', genres: ['Jazz'], plays: 2 }),
      track({ artist: 'c', genres: ['Rock', 'Jazz'], plays: 1 }),
    ]
    expect(topGenres(tracks, 2).map((g) => g.genre)).toEqual(['Rock', 'Jazz'])
    expect(clusterByGenre(tracks, 2).map((c) => c.genre)).toEqual(['Rock', 'Jazz'])
  })

  it('On Repeat keeps the last 30 days, most played first', () => {
    const recentLow = track({ artist: 'a', plays: 2, daysAgo: 3 })
    const recentHigh = track({ artist: 'b', plays: 9, daysAgo: 20 })
    const old = track({ artist: 'c', plays: 50, daysAgo: 45 })
    expect(onRepeatTracks([recentLow, old, recentHigh], NOW)).toEqual([recentHigh, recentLow])
  })

  it('Rediscover keeps loved tracks not played for 3 months', () => {
    const lovedOld = track({ artist: 'a', fav: true, plays: 5, daysAgo: 120 })
    const lovedNever = track({ artist: 'b', fav: true })
    const lovedRecent = track({ artist: 'c', fav: true, daysAgo: 10 })
    const out = rediscoverTracks([lovedOld, lovedNever, lovedRecent], [], NOW)
    expect(out).toContain(lovedOld)
    expect(out).toContain(lovedNever)
    expect(out).not.toContain(lovedRecent)
  })
})

describe('generateMixes', () => {
  const genres = ['Rock', 'Jazz', 'Pop', 'Folk']
  const played = many(200, (i) =>
    track({
      artist: `artist${i % 25}`,
      album: `album${i % 40}`,
      genres: [genres[i % 4] as string],
      plays: 1 + (i % 7),
      daysAgo: (i * 7) % 150,
    }),
  )
  // Built once: a real server returns the same ids every time.
  const favourites = many(60, (i) => track({ artist: `fav${i % 15}`, fav: true, daysAgo: 100 + i }))
  const makeFresh = (prefix: string) =>
    many(80, (i) =>
      track({ artist: `${prefix}${i % 20}`, album: `${prefix}al${i}`, genres: ['Rock'] }),
    )
  const pools = { sim: makeFresh('sim'), disc: makeFresh('disc'), gen: makeFresh('gen') }
  const source: MixSource = {
    signals: async () => ({ played, favourites }),
    similarTo: async () => pools.sim,
    discoveryPool: async () => pools.disc,
    genreTracks: async () => pools.gen,
  }

  it('builds For You, Discovery, On Repeat, Rediscover and genre mixes', async () => {
    const mixes = await generateMixes(source, NOW)
    const kinds = mixes.map((m) => m.kind)
    expect(kinds.filter((k) => k === 'forYou')).toHaveLength(3)
    for (const k of ['discovery', 'onRepeat', 'rediscover', 'genre'] as const)
      expect(kinds).toContain(k)
    expect(new Set(mixes.map((m) => m.id)).size).toBe(mixes.length)
  })

  it('respects size, duplicates, and the per artist cap in every mix', async () => {
    for (const mix of await generateMixes(source, NOW)) {
      expect(mix.tracks.length).toBeGreaterThanOrEqual(10)
      expect(mix.tracks.length).toBeLessThanOrEqual(MAX_TRACKS)
      expect(new Set(mix.tracks.map((t) => t.Id)).size).toBe(mix.tracks.length)
      for (const n of count(mix.tracks, artistKey).values())
        expect(n).toBeLessThanOrEqual(MAX_PER_ARTIST)
    }
  })

  it('Discovery only holds tracks played at most once', async () => {
    const discovery = (await generateMixes(source, NOW)).find((m) => m.kind === 'discovery')
    expect(discovery?.tracks.every((t) => (t.UserData?.PlayCount ?? 0) <= 1)).toBe(true)
  })

  it('is stable for a given day', async () => {
    const ids = async () => (await generateMixes(source, NOW)).map((m) => m.tracks.map((t) => t.Id))
    expect(await ids()).toEqual(await ids())
  })

  it('leaves a mix out when the library cannot fill it, and survives failing fetches', async () => {
    const offline = async (): Promise<Track[]> => Promise.reject(new Error('offline'))
    const poor: MixSource = {
      signals: async () => ({
        played: [track({ artist: 'a', genres: ['Rock'], plays: 1 })],
        favourites: [],
      }),
      similarTo: offline,
      discoveryPool: offline,
      genreTracks: offline,
    }
    expect(await generateMixes(poor, NOW)).toEqual([])
  })
})
