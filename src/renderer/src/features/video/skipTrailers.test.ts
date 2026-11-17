import { describe, expect, it } from 'vitest'
import { normalizeSegments, segmentAt, type MediaSegment } from '@renderer/jellyfin'
import { isAutoSkippable, skipPill, skipTarget } from './skip'
import { pickTrailerSources, youtubeEmbedUrl, youtubeId } from './trailers'

const T = 10_000_000
const seg = (type: MediaSegment['type'], start: number, end: number): MediaSegment => ({
  type,
  start,
  end,
})

describe('media segments', () => {
  it('normalizes the server answer: seconds, sorted, unknown and empty ranges dropped', () => {
    expect(
      normalizeSegments([
        { Type: 'Outro', StartTicks: 1300 * T, EndTicks: 1400 * T },
        { Type: 'Intro', StartTicks: 30 * T, EndTicks: 90 * T },
        { Type: 'Unknown', StartTicks: 0, EndTicks: 5 * T },
        { Type: 'Recap', StartTicks: 10 * T, EndTicks: 10 * T },
      ]),
    ).toEqual([seg('Intro', 30, 90), seg('Outro', 1300, 1400)])
    expect(normalizeSegments(undefined)).toEqual([])
  })

  it('finds the segment at a time: start included, end excluded, type filtered', () => {
    const list = [seg('Intro', 30, 90), seg('Preview', 95, 100)]
    expect(segmentAt(list, 30, ['Intro'])).toBe(list[0])
    expect(segmentAt(list, 89.9, ['Intro'])).toBe(list[0])
    expect(segmentAt(list, 90, ['Intro'])).toBeUndefined()
    expect(segmentAt(list, 10, ['Intro'])).toBeUndefined()
    expect(segmentAt(list, 96, ['Intro'])).toBeUndefined()
    expect(segmentAt(undefined, 10, ['Intro'])).toBeUndefined()
    expect(segmentAt(list, 89.5, ['Intro'], 1)).toBeUndefined()
  })
})

describe('skip pill visibility', () => {
  const segments = [seg('Recap', 0, 20), seg('Intro', 30, 90), seg('Outro', 1300, 1400)]
  const base = { segments, hasNext: false, upNextVisible: false }

  it('shows nothing without segments or outside them', () => {
    expect(skipPill({ ...base, segments: undefined, positionSeconds: 40 })).toBeNull()
    expect(skipPill({ ...base, segments: [], positionSeconds: 40 })).toBeNull()
    expect(skipPill({ ...base, positionSeconds: 25 })).toBeNull()
  })

  it('offers Skip Recap and Skip Intro during them, gone at their very end', () => {
    expect(skipPill({ ...base, positionSeconds: 5 })?.kind).toBe('recap')
    expect(skipPill({ ...base, positionSeconds: 40 })?.kind).toBe('intro')
    expect(skipPill({ ...base, positionSeconds: 89.5 })).toBeNull()
  })

  it('credits: Skip Credits for a movie, Next Episode when one follows', () => {
    expect(skipPill({ ...base, positionSeconds: 1310 })?.kind).toBe('credits')
    expect(skipPill({ ...base, hasNext: true, positionSeconds: 1310 })?.kind).toBe('nextEpisode')
  })

  it('leaves the next-episode action to the Up Next card once it is on screen', () => {
    expect(
      skipPill({ ...base, hasNext: true, upNextVisible: true, positionSeconds: 1310 }),
    ).toBeNull()
  })

  it('never shows for a trailer, nor for preview / commercial segments', () => {
    expect(skipPill({ ...base, trailer: true, positionSeconds: 40 })).toBeNull()
    expect(skipPill({ ...base, segments: [seg('Preview', 0, 50)], positionSeconds: 10 })).toBeNull()
  })

  it('skips to the segment end, never past the duration; only intros and recaps auto-skip', () => {
    const pill = skipPill({ ...base, positionSeconds: 40 })!
    expect(skipTarget(pill.segment, 1500)).toBe(90)
    expect(skipTarget(seg('Outro', 1300, 1600), 1500)).toBe(1500)
    expect(isAutoSkippable(pill)).toBe(true)
    expect(isAutoSkippable(skipPill({ ...base, positionSeconds: 1310 })!)).toBe(false)
  })
})

describe('trailer sources', () => {
  it('reads YouTube ids from every common link shape', () => {
    expect(youtubeId('https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=3')).toBe('dQw4w9WgXcQ')
    expect(youtubeId('https://youtu.be/dQw4w9WgXcQ?si=x')).toBe('dQw4w9WgXcQ')
    expect(youtubeId('https://www.youtube.com/embed/dQw4w9WgXcQ')).toBe('dQw4w9WgXcQ')
    expect(youtubeId('https://vimeo.com/123')).toBeUndefined()
    expect(youtubeId('not a url')).toBeUndefined()
  })

  it('builds a no-cookie autoplay embed without related videos', () => {
    const url = new URL(youtubeEmbedUrl('dQw4w9WgXcQ'))
    expect(url.origin).toBe('https://www.youtube-nocookie.com')
    expect(url.pathname).toBe('/embed/dQw4w9WgXcQ')
    expect(url.searchParams.get('autoplay')).toBe('1')
    expect(url.searchParams.get('rel')).toBe('0')
  })

  it('orders local files, then YouTube, then other links, and drops duplicates', () => {
    const sources = pickTrailerSources(
      [{ Id: 'loc1', Name: 'Teaser' }, { Name: 'no id' }],
      [
        { Url: 'https://vimeo.com/1', Name: 'Vimeo' },
        { Url: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ', Name: 'Official' },
        { Url: 'https://youtu.be/dQw4w9WgXcQ' },
        { Url: 'javascript:alert(1)' },
        { Url: null },
      ],
    )
    expect(sources.map((s) => s.kind)).toEqual(['local', 'youtube', 'external'])
    expect(sources[0]).toMatchObject({ itemId: 'loc1' })
  })

  it('is empty when the item has no trailer', () => {
    expect(pickTrailerSources(undefined, undefined)).toEqual([])
    expect(pickTrailerSources([], [])).toEqual([])
  })
})
