import { describe, expect, it } from 'vitest'
import type { BaseItemDto } from '@renderer/jellyfin'
import { artistKey, groupArtists, groupIdsOf, splitArtistNames } from './artistGroups'

const a = (Id: string, Name: string): BaseItemDto => ({ Id, Name }) as BaseItemDto

describe('artistKey', () => {
  it('ignores case, accents, spacing and a leading "The"', () => {
    expect(artistKey('  Beyoncé  ')).toBe('beyonce')
    expect(artistKey('BEYONCE')).toBe('beyonce')
    expect(artistKey('The   Weeknd')).toBe('weeknd')
    expect(artistKey('the weeknd')).toBe(artistKey('Weeknd'))
    expect(artistKey('Theory of a Deadman')).toBe('theory of a deadman')
  })
})

describe('splitArtistNames', () => {
  it('splits on feat., ft., featuring, &, comma, semicolon, x and with', () => {
    expect(splitArtistNames('A feat. B')).toEqual(['A', 'B'])
    expect(splitArtistNames('A ft B')).toEqual(['A', 'B'])
    expect(splitArtistNames('A (featuring B & C)')).toEqual(['A', 'B', 'C'])
    expect(splitArtistNames('A, B; C')).toEqual(['A', 'B', 'C'])
    expect(splitArtistNames('A x B')).toEqual(['A', 'B'])
    expect(splitArtistNames('A with B')).toEqual(['A', 'B'])
  })
  it('leaves a single name alone, even when it contains a letter x', () => {
    expect(splitArtistNames('Xavier Rudd')).toEqual(['Xavier Rudd'])
    expect(splitArtistNames('Daft Punk')).toEqual(['Daft Punk'])
  })
})

describe('groupArtists', () => {
  it('merges spelling variants and credits that start with a known artist', () => {
    const roots = [
      a('1', 'Daft Punk'),
      a('2', 'daft punk '),
      a('3', 'Daft Punk feat. Pharrell'),
      a('4', 'Beyoncé'),
      a('5', 'Simon & Garfunkel'),
    ]
    const groups = groupArtists(roots, [...roots, a('6', 'Pharrell'), a('7', 'Daft Punk & Julian')])
    expect(groups).toHaveLength(3)
    const daft = groups.find((g) => g.item.Name === 'Daft Punk')
    expect(daft?.ids.sort()).toEqual(['1', '2', '3', '7'])
    expect(groupIdsOf(groups, '7').sort()).toEqual(['1', '2', '3', '7'])
    expect(groups.some((g) => g.item.Name === 'Simon & Garfunkel')).toBe(true)
  })
  it('leaves guest-only artists out and unknown ids alone', () => {
    const groups = groupArtists([a('1', 'A')], [a('1', 'A'), a('9', 'Guest')])
    expect(groups.map((g) => g.ids)).toEqual([['1']])
    expect(groupIdsOf(groups, '9')).toEqual(['9'])
  })
  it('roots a feat. credit on its head even when the head only exists as a guest', () => {
    const groups = groupArtists([a('1', 'Solo feat. Friend')], [a('2', 'Solo')])
    expect(groups).toHaveLength(1)
    expect(groups[0]?.ids.sort()).toEqual(['1', '2'])
  })
})
