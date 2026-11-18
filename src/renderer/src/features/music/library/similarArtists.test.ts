import { describe, expect, it } from 'vitest'
import type { BaseItemDto } from '@renderer/jellyfin'
import { mergeSimilar } from './similarArtists'

const a = (Id: string, Name: string, Type = 'MusicArtist'): BaseItemDto =>
  ({ Id, Name, Type }) as BaseItemDto

describe('mergeSimilar', () => {
  const library = [a('1', 'Muse'), a('2', 'Placebo'), a('3', 'Radiohead'), a('4', 'Blur')]
  it('puts the server list first, then library artists Deezer relates, without repeats', () => {
    const r = mergeSimilar(
      [a('2', 'Placebo'), a('9', 'Some Film', 'Movie')],
      ['placebo', 'Radiohead', 'Not In Library', 'Muse'],
      library,
      ['1'],
      'Muse',
    )
    expect(r.map((x) => x.Id)).toEqual(['2', '3'])
  })
  it('never lists the artist itself, even under another merged id', () => {
    const r = mergeSimilar([a('5', 'The Muse')], ['Muse'], library, ['1', '5'], 'Muse')
    expect(r).toEqual([])
  })
})
