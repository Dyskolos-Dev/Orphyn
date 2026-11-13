import { describe, expect, it } from 'vitest'
import { STALE, jfKeys } from './keys'

describe('jfKeys', () => {
  it('scopes every key by account id first', () => {
    const keys = [
      jfKeys.views('A'),
      jfKeys.items('A', { parentId: 'p' }),
      jfKeys.list('A', 'albums', {}),
      jfKeys.item('A', '1'),
      jfKeys.resume('A', 'video'),
      jfKeys.nextUp('A'),
      jfKeys.latest('A', 'p'),
      jfKeys.seasons('A', 's'),
      jfKeys.episodes('A', 's'),
      jfKeys.similar('A', '1'),
      jfKeys.artistAlbums('A', '1'),
      jfKeys.albumTracks('A', '1'),
      jfKeys.playlistItems('A', '1'),
      jfKeys.instantMix('A', '1'),
      jfKeys.lyrics('A', '1'),
      jfKeys.search('A', 'x'),
    ]
    for (const k of keys) expect(k.slice(0, 2)).toEqual(['jf', 'A'])
    expect(new Set(keys.map((k) => JSON.stringify(k))).size).toBe(keys.length)
  })

  it('differs between servers and between parameters', () => {
    expect(jfKeys.item('A', '1')).not.toEqual(jfKeys.item('B', '1'))
    expect(jfKeys.items('A', { parentId: 'x' })).not.toEqual(jfKeys.items('A', { parentId: 'y' }))
    expect(jfKeys.episodes('A', 's', '1')).not.toEqual(jfKeys.episodes('A', 's', '2'))
    expect(jfKeys.resume('A', 'audio')).not.toEqual(jfKeys.resume('A', 'video'))
  })

  it('keeps resume fresher than the catalog', () => {
    expect(STALE.progress).toBeLessThan(STALE.catalog)
    expect(STALE.views).toBeGreaterThanOrEqual(STALE.catalog)
  })
})
