import { describe, expect, it } from 'vitest'
import type { BaseItemDto } from '@renderer/jellyfin'
import { musicPresence, shouldSend, videoPresence } from './presence'

const track = {
  Id: 't1',
  Type: 'Audio',
  Name: 'Song',
  Album: 'Album',
  AlbumArtist: 'Artist',
  RunTimeTicks: 2_000_000_000,
} as BaseItemDto

describe('discord presence mapping', () => {
  it('uses RunTimeTicks when the player has no duration yet', () => {
    const p = musicPresence({ track, status: 'paused', position: 12, duration: 0 })
    expect(p).toMatchObject({ kind: 'music', paused: true, durationSec: 200, positionSec: 12 })
  })

  it('is null when nothing plays', () => {
    expect(musicPresence({ track: undefined, status: 'idle', position: 0, duration: 0 })).toBeNull()
    expect(musicPresence({ track, status: 'idle', position: 0, duration: 0 })).toBeNull()
  })

  it('maps an episode and ignores another item', () => {
    const ep = {
      Id: 'e1',
      Type: 'Episode',
      Name: 'Pilot',
      SeriesName: 'Show',
      SeriesId: 's1',
      ParentIndexNumber: 1,
      IndexNumber: 2,
    } as BaseItemDto
    const snap = {
      itemId: 'e1',
      status: 'playing',
      positionSeconds: 5,
      durationSeconds: 100,
    } as const
    expect(videoPresence(snap, ep)).toMatchObject({
      kind: 'video',
      series: 'Show',
      season: 1,
      episode: 2,
      title: 'Pilot',
    })
    expect(videoPresence({ ...snap, itemId: 'other' }, ep)).toBeNull()
  })
})

describe('shouldSend', () => {
  const base = musicPresence({ track, status: 'playing', position: 10, duration: 200 })!
  const sent = { presence: base, at: 1000 }

  it('ignores plain progress ticks', () => {
    expect(shouldSend(sent, { ...base, positionSec: 11.5 }, 2500)).toBe(false)
  })

  it('sends on seek, pause and track change', () => {
    expect(shouldSend(sent, { ...base, positionSec: 90 }, 2000)).toBe(true)
    expect(shouldSend(sent, { ...base, paused: true }, 2000)).toBe(true)
    expect(shouldSend(sent, { ...base, title: 'Other' }, 2000)).toBe(true)
  })

  it('sends when playback starts and stops', () => {
    expect(shouldSend(null, base, 0)).toBe(true)
    expect(shouldSend(sent, null, 0)).toBe(true)
    expect(shouldSend({ presence: null, at: 0 }, null, 0)).toBe(false)
  })
})
