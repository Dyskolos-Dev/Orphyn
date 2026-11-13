import { describe, expect, it } from 'vitest'
import type { BaseItemDto, PlaybackTracks } from '@renderer/jellyfin'
import {
  badgeList,
  formatClock,
  formatRuntime,
  nextEpisode,
  pickSeriesEpisode,
  primaryAction,
  progressOf,
  sortEpisodes,
  streamBadges,
} from './format'
import {
  audioSwitchNeedsReload,
  pickInitialAudio,
  pickInitialSubtitle,
  sameLanguage,
  subtitleSwitchNeedsReload,
} from './tracks'
import { UP_NEXT_WINDOW_SECONDS, upNextState } from './upNext'

const MIN = 600_000_000

describe('format', () => {
  it('formats runtimes and clocks', () => {
    expect(formatRuntime(155 * MIN)).toBe('2 h 35 min')
    expect(formatRuntime(120 * MIN)).toBe('2 h')
    expect(formatRuntime(45 * MIN)).toBe('45 min')
    expect(formatRuntime(undefined)).toBe('')
    expect(formatClock(62)).toBe('1:02')
    expect(formatClock(3723)).toBe('1:02:03')
    expect(formatClock(NaN)).toBe('0:00')
  })

  it('builds the primary action for movies', () => {
    expect(primaryAction({ Type: 'Movie' })).toEqual({ kind: 'play' })
    expect(
      primaryAction({ Type: 'Movie', UserData: { PlaybackPositionTicks: 42 * MIN } as never }),
    ).toEqual({ kind: 'resume', positionTicks: 42 * MIN, minutes: 42 })
  })

  it('builds the primary action for a series from its episode', () => {
    const ep: BaseItemDto = { Id: 'e3', ParentIndexNumber: 2, IndexNumber: 3 }
    expect(primaryAction({ Type: 'Series' }, ep)).toEqual({
      kind: 'episode',
      episodeId: 'e3',
      season: 2,
      episode: 3,
      positionTicks: 0,
    })
    expect(primaryAction({ Type: 'Series' })).toEqual({ kind: 'play' })
  })

  it('computes progress', () => {
    expect(
      progressOf({
        RunTimeTicks: 100 * MIN,
        UserData: { PlaybackPositionTicks: 25 * MIN } as never,
      }),
    ).toBeCloseTo(0.25)
    expect(progressOf({ RunTimeTicks: 100 * MIN })).toBeUndefined()
  })

  it('orders episodes by season then number, specials last', () => {
    const eps: BaseItemDto[] = [
      { Id: 'sp', ParentIndexNumber: 0, IndexNumber: 1 },
      { Id: 's2e1', ParentIndexNumber: 2, IndexNumber: 1 },
      { Id: 's1e2', ParentIndexNumber: 1, IndexNumber: 2 },
      { Id: 's1e1', ParentIndexNumber: 1, IndexNumber: 1 },
    ]
    expect(sortEpisodes(eps).map((e) => e.Id)).toEqual(['s1e1', 's1e2', 's2e1', 'sp'])
    expect(nextEpisode(eps, 's1e2')?.Id).toBe('s2e1')
    expect(nextEpisode(eps, 'sp')).toBeUndefined()
  })

  it('picks the episode a series plays', () => {
    const eps: BaseItemDto[] = [
      { Id: 'a', ParentIndexNumber: 1, IndexNumber: 1, UserData: { Played: true } as never },
      { Id: 'b', ParentIndexNumber: 1, IndexNumber: 2 },
      {
        Id: 'c',
        ParentIndexNumber: 1,
        IndexNumber: 3,
        UserData: { PlaybackPositionTicks: 5 * MIN } as never,
      },
    ]
    expect(pickSeriesEpisode(eps)?.Id).toBe('c')
    expect(pickSeriesEpisode(eps.slice(0, 2))?.Id).toBe('b')
    expect(pickSeriesEpisode(eps.slice(0, 2), { Id: 'a' })?.Id).toBe('a')
  })

  it('derives badges from streams', () => {
    const item: BaseItemDto = {
      MediaSources: [
        {
          MediaStreams: [
            { Type: 'Video', Width: 3840, Height: 2160, VideoRangeType: 'DOVIWithHDR10' },
            { Type: 'Audio', Channels: 6 },
          ],
        },
      ],
    }
    expect(badgeList(streamBadges(item))).toEqual(['4K', 'Dolby Vision', '5.1'])
    expect(badgeList(streamBadges({}))).toEqual([])
  })
})

const tracks: PlaybackTracks = {
  audio: [
    { index: 1, language: 'eng', title: 'English', isDefault: true },
    { index: 2, language: 'fre', title: 'Français', isDefault: false },
  ],
  subtitles: [
    {
      index: 3,
      language: 'fre',
      title: 'FR forced',
      isDefault: false,
      isForced: true,
      burnInRequired: false,
    },
    {
      index: 4,
      language: 'fre',
      title: 'FR',
      isDefault: false,
      isForced: false,
      burnInRequired: false,
    },
    {
      index: 5,
      language: 'eng',
      title: 'EN PGS',
      isDefault: false,
      isForced: false,
      burnInRequired: true,
    },
  ],
  defaultAudioIndex: 1,
}

describe('track selection', () => {
  it('matches languages across ISO variants', () => {
    expect(sameLanguage('fr', 'fre')).toBe(true)
    expect(sameLanguage('fra', 'fre')).toBe(true)
    expect(sameLanguage('en', 'fre')).toBe(false)
    expect(sameLanguage(undefined, 'en')).toBe(false)
  })

  it('picks audio by preference then default', () => {
    expect(pickInitialAudio(tracks, { audioLanguage: 'fr' })?.index).toBe(2)
    expect(pickInitialAudio(tracks, { audioLanguage: 'ja' })?.index).toBe(1)
    expect(pickInitialAudio(tracks, {})?.index).toBe(1)
  })

  it('picks subtitles, preferring a non-forced track of the language', () => {
    expect(pickInitialSubtitle(tracks, { subtitleLanguage: 'fr' })?.index).toBe(4)
    expect(pickInitialSubtitle(tracks, {})).toBeUndefined()
    expect(pickInitialSubtitle(tracks, { subtitleLanguage: 'ja' })).toBeUndefined()
  })

  it('only reloads for burned-in subtitles and audio changes', () => {
    expect(subtitleSwitchNeedsReload(tracks, undefined, 4)).toBe(false)
    expect(subtitleSwitchNeedsReload(tracks, 4, 5)).toBe(true)
    expect(subtitleSwitchNeedsReload(tracks, 5, -1)).toBe(true)
    expect(subtitleSwitchNeedsReload(tracks, 5, 5)).toBe(false)
    expect(audioSwitchNeedsReload(1, 2)).toBe(true)
    expect(audioSwitchNeedsReload(1, 1)).toBe(false)
  })
})

describe('Up Next timing', () => {
  const base = { durationSeconds: 3000, hasNext: true, dismissed: false }
  it('appears inside the final window with a countdown', () => {
    expect(upNextState({ ...base, positionSeconds: 2900 }).visible).toBe(false)
    const s = upNextState({ ...base, positionSeconds: 3000 - UP_NEXT_WINDOW_SECONDS + 0.5 })
    expect(s).toEqual({ visible: true, secondsLeft: UP_NEXT_WINDOW_SECONDS })
    expect(upNextState({ ...base, positionSeconds: 2995.2 }).secondsLeft).toBe(5)
  })
  it('stays hidden without a next episode, once dismissed, or at the very end', () => {
    expect(upNextState({ ...base, positionSeconds: 2990, hasNext: false }).visible).toBe(false)
    expect(upNextState({ ...base, positionSeconds: 2990, dismissed: true }).visible).toBe(false)
    expect(upNextState({ ...base, positionSeconds: 3000 }).visible).toBe(false)
  })
})
