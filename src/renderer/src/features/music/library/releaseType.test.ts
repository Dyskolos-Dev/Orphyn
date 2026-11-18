import { describe, expect, it } from 'vitest'
import type { BaseItemDto } from '@renderer/jellyfin'
import { buildDiscography, classifyRelease } from './releaseType'

const album = (over: Partial<BaseItemDto>): BaseItemDto =>
  ({ Id: 'x', Name: 'Album', ChildCount: 12, ...over }) as BaseItemDto

describe('classifyRelease', () => {
  it('calls a full-length release an album', () => {
    expect(classifyRelease(album({}))).toBe('album')
    expect(classifyRelease(album({ ChildCount: undefined }))).toBe('album')
  })
  it('uses the track count for EPs and singles', () => {
    expect(classifyRelease(album({ ChildCount: 1 }))).toBe('ep')
    expect(classifyRelease(album({ ChildCount: 6 }))).toBe('ep')
    expect(classifyRelease(album({ ChildCount: 7 }))).toBe('album')
  })
  it('reads the title', () => {
    expect(classifyRelease(album({ Name: 'Bloom - EP' }))).toBe('ep')
    expect(classifyRelease(album({ Name: 'Hello (Single)' }))).toBe('ep')
    expect(classifyRelease(album({ Name: 'Greatest Hits' }))).toBe('compilation')
    expect(classifyRelease(album({ Name: 'Best of 2000-2010', ChildCount: 3 }))).toBe('compilation')
  })
  it('treats Various Artists as a compilation', () => {
    expect(classifyRelease(album({ AlbumArtist: 'Various Artists' }))).toBe('compilation')
  })
})

describe('buildDiscography', () => {
  const mine = [{ Id: 'a1' }, { Id: 'a2' }]
  it('splits by type, newest first, and merged ids count as the artist', () => {
    const d = buildDiscography(
      [
        album({ Id: '1', ProductionYear: 2001, AlbumArtists: [{ Id: 'a1' }] }),
        album({ Id: '2', ProductionYear: 2010, AlbumArtists: [{ Id: 'a2' }] }),
        album({ Id: '3', ProductionYear: 2005, ChildCount: 3, AlbumArtists: [{ Id: 'a1' }] }),
        album({ Id: '4', Name: 'Hits', AlbumArtists: mine }),
        album({ Id: '5', AlbumArtists: [{ Id: 'other' }] }),
        album({ Id: '6', ProductionYear: 1999, AlbumArtists: [] }),
      ],
      ['a1', 'a2'],
    )
    expect(d.albums.map((a) => a.Id)).toEqual(['2', '1'])
    expect(d.eps.map((a) => a.Id)).toEqual(['3'])
    expect(d.compilations.map((a) => a.Id)).toEqual(['4'])
    expect(d.appearsOn.map((a) => a.Id)).toEqual(['6', '5'])
  })
})
