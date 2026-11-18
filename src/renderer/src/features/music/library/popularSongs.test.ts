import { describe, expect, it } from 'vitest'
import type { BaseItemDto } from '@renderer/jellyfin'
import { normalizeTitle, pickPopularSongs } from './popularSongs'

const song = (id: string, name: string, plays = 0): BaseItemDto =>
  ({ Id: id, Name: name, UserData: { PlayCount: plays } }) as BaseItemDto
const ids = (l: BaseItemDto[]) => l.map((s) => s.Id)

describe('normalizeTitle', () => {
  it('drops remaster, live, version and feature qualifiers', () => {
    const base = normalizeTitle('Starlight')
    expect(normalizeTitle('Starlight (Remastered 2011)')).toBe(base)
    expect(normalizeTitle('Starlight - Remastered')).toBe(base)
    expect(normalizeTitle('Starlight - Live at Wembley')).toBe(base)
    expect(normalizeTitle('Starlight [Radio Edit]')).toBe(base)
    expect(normalizeTitle('Starlight (feat. Someone)')).toBe(base)
    expect(normalizeTitle('Starlight feat. Someone')).toBe(base)
  })
  it('ignores case, accents and punctuation', () => {
    expect(normalizeTitle('Déjà Vu!')).toBe(normalizeTitle('deja vu'))
    expect(normalizeTitle('Don’t Stop')).toBe(normalizeTitle("don't stop"))
  })
  it('keeps meaningful brackets and dashes', () => {
    expect(normalizeTitle('Song (Part 2)')).not.toBe(normalizeTitle('Song'))
    expect(normalizeTitle('Song - Interlude')).not.toBe(normalizeTitle('Song'))
  })
})

describe('pickPopularSongs', () => {
  const songs = [
    song('a', 'Hysteria', 5),
    song('b', 'Starlight (Remastered)', 1),
    song('c', 'Uprising', 50),
    song('d', 'Hysteria - Live', 9),
    song('e', 'Unlisted', 20),
  ]
  it('follows the online ranking, then fills by play count', () => {
    const r = pickPopularSongs(songs, ['Starlight', 'Hysteria', 'Not In Library'], 10)
    expect(ids(r)).toEqual(['b', 'd', 'c', 'e'])
  })
  it('lists a title once, keeping the most played version', () => {
    const r = pickPopularSongs(songs, ['Hysteria'], 10)
    expect(ids(r).filter((i) => i === 'a' || i === 'd')).toEqual(['d'])
  })
  it('falls back to play count alone without a ranking, and respects the limit', () => {
    expect(ids(pickPopularSongs(songs, [], 2))).toEqual(['c', 'e'])
  })
})
