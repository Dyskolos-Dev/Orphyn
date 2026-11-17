import { describe, expect, it } from 'vitest'
import type { BaseItemDto } from '@renderer/jellyfin/types'
import { aggregateYear, availableYears, countPluginRows, playsInYear } from './aggregate'

const MIN = 600_000_000 // ticks in one minute

type UD = BaseItemDto['UserData']
const ud = (plays: number, date: string) => ({ PlayCount: plays, LastPlayedDate: date }) as UD

function song(
  id: string,
  artist: string,
  album: string,
  plays: number,
  date: string,
  genres = ['Rock'],
): BaseItemDto {
  return {
    Id: id,
    Name: `Song ${id}`,
    Type: 'Audio',
    RunTimeTicks: 3 * MIN,
    Genres: genres,
    AlbumId: `al-${album}`,
    Album: album,
    AlbumArtist: artist,
    ArtistItems: [{ Id: `ar-${artist}`, Name: artist }],
    UserData: ud(plays, date),
  }
}
const episode = (id: string, series: string, plays: number, date: string): BaseItemDto => ({
  Id: id,
  Name: id,
  Type: 'Episode',
  SeriesId: `s-${series}`,
  SeriesName: series,
  RunTimeTicks: 40 * MIN,
  UserData: ud(plays, date),
})
const movie = (id: string, plays: number, date: string): BaseItemDto => ({
  Id: id,
  Name: id,
  Type: 'Movie',
  ProductionYear: 2020,
  RunTimeTicks: 100 * MIN,
  UserData: ud(plays, date),
})

const NOW = new Date(2026, 5, 15)
const items: BaseItemDto[] = [
  song('a', 'Muse', 'Drones', 10, '2026-03-02T10:00:00'),
  song('b', 'Muse', 'Drones', 4, '2026-03-10T10:00:00', ['Rock', 'Alt']),
  song('c', 'Daft Punk', 'Discovery', 6, '2026-07-01T10:00:00', ['Electro']),
  song('old', 'Muse', 'Origin', 50, '2025-01-01T10:00:00'),
  episode('e1', 'Show', 2, '2026-03-05T10:00:00'),
  episode('e2', 'Show', 1, '2026-03-06T10:00:00'),
  movie('m1', 1, '2026-04-01T10:00:00'),
]

describe('aggregateYear', () => {
  const s = aggregateYear(items, 2026, { now: NOW })

  it('ignores items last played in other years', () => {
    expect(s.music.plays).toBe(20)
    expect(s.music.songs.map((x) => x.key)).not.toContain('old')
  })
  it('ranks artists, songs and albums by plays', () => {
    expect(s.music.artists[0]).toMatchObject({ name: 'Muse', plays: 14 })
    expect(s.music.songs[0]?.key).toBe('a')
    expect(s.music.albums[0]).toMatchObject({ name: 'Drones', plays: 14 })
  })
  it('estimates minutes and weights genres by plays', () => {
    expect(s.music.minutes).toBe(60)
    expect(s.music.genres[0]).toMatchObject({ name: 'Rock', plays: 14 })
    expect(s.music.genres.reduce((a, g) => a + g.share, 0)).toBeCloseTo(1)
  })
  it('groups episodes by series and counts them', () => {
    expect(s.video.series).toHaveLength(1)
    expect(s.video.series[0]).toMatchObject({ name: 'Show', plays: 3 })
    expect(s.video.episodePlays).toBe(3)
    expect(s.video.moviePlays).toBe(1)
    expect(s.video.minutes).toBe(220)
  })
  it('derives fun facts', () => {
    expect(s.facts.totalMinutes).toBe(280)
    expect(s.facts.busiestMonth).toBe(2)
    expect(s.facts.topArtistShare).toBeCloseTo(0.7)
    expect(s.facts.minutesPerDay).toBeGreaterThan(0)
  })
  it('reports an empty year', () => {
    const e = aggregateYear(items, 2019, { now: NOW })
    expect(e.isEmpty).toBe(true)
    expect(e.facts.busiestMonth).toBeNull()
    expect(e.facts.topArtistShare).toBe(0)
  })
  it('limits the lists', () => {
    expect(aggregateYear(items, 2026, { now: NOW, limit: 1 }).music.songs).toHaveLength(1)
  })
})

describe('plugin counts', () => {
  it('uses exact plays when provided', () => {
    const plugin = new Map([['c', 9]])
    expect(playsInYear(items[2] as BaseItemDto, 2026, plugin)).toBe(9)
    expect(playsInYear(items[0] as BaseItemDto, 2026, plugin)).toBe(0)
    expect(aggregateYear(items, 2026, { pluginPlays: plugin, now: NOW }).music.plays).toBe(9)
  })
  it('counts rows per item id and rejects unknown shapes', () => {
    expect(countPluginRows([{ item_id: 'x' }, { ItemId: 'x' }, { item_id: 'y' }])).toEqual(
      new Map([
        ['x', 2],
        ['y', 1],
      ]),
    )
    expect(countPluginRows({ nope: true })).toBeNull()
    expect(countPluginRows([{ foo: 1 }])).toBeNull()
  })
})

describe('availableYears', () => {
  it('lists years descending and always includes the current one', () => {
    expect(availableYears(items, NOW)).toEqual([2026, 2025])
    expect(availableYears([], NOW)).toEqual([2026])
  })
})
