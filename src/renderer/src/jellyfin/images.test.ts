import { describe, expect, it } from 'vitest'
import { buildImageUrl } from './images'
import type { BaseItemDto } from './types'

const B = 'http://s:8096'

describe('buildImageUrl', () => {
  it('uses the item image with tag, size and quality', () => {
    const item = { Id: 'm1', Type: 'Movie', ImageTags: { Primary: 't1' } } as BaseItemDto
    expect(buildImageUrl(B, item, 'Primary', { maxWidth: 300, quality: 80 })).toBe(
      `${B}/Items/m1/Images/Primary?maxWidth=300&quality=80&tag=t1`,
    )
  })

  it('defaults quality to 90 and lets the caller override the tag', () => {
    const item = { Id: 'm1', ImageTags: { Primary: 't1' } } as BaseItemDto
    expect(buildImageUrl(B, item, 'Primary', { tag: 'zzz' })).toBe(
      `${B}/Items/m1/Images/Primary?quality=90&tag=zzz`,
    )
  })

  it('episodes fall back to the series poster', () => {
    const ep = {
      Id: 'e1',
      Type: 'Episode',
      SeriesId: 's1',
      SeriesPrimaryImageTag: 'st',
      ImageTags: {},
    } as BaseItemDto
    expect(buildImageUrl(B, ep, 'Primary')).toContain('/Items/s1/Images/Primary?')
  })

  it('tracks fall back to the album cover', () => {
    const track = {
      Id: 'a1',
      Type: 'Audio',
      AlbumId: 'al1',
      AlbumPrimaryImageTag: 'at',
    } as BaseItemDto
    expect(buildImageUrl(B, track, 'Primary', { maxWidth: 64 })).toBe(
      `${B}/Items/al1/Images/Primary?maxWidth=64&quality=90&tag=at`,
    )
  })

  it('backdrops use index 0, falling back to the parent backdrop', () => {
    const own = { Id: 'm1', BackdropImageTags: ['b1'] } as BaseItemDto
    expect(buildImageUrl(B, own, 'Backdrop')).toContain('/Items/m1/Images/Backdrop/0?')
    const ep = {
      Id: 'e1',
      ParentBackdropItemId: 's1',
      ParentBackdropImageTags: ['pb'],
    } as BaseItemDto
    expect(buildImageUrl(B, ep, 'Backdrop')).toContain('/Items/s1/Images/Backdrop/0?')
  })

  it('logo and thumb with parent fallbacks; thumb ends on the primary', () => {
    const ep = {
      Id: 'e1',
      ParentLogoItemId: 's1',
      ParentLogoImageTag: 'lg',
      ParentThumbItemId: 's1',
      ParentThumbImageTag: 'th',
    } as BaseItemDto
    expect(buildImageUrl(B, ep, 'Logo')).toContain('/Items/s1/Images/Logo?')
    expect(buildImageUrl(B, ep, 'Thumb')).toContain('/Items/s1/Images/Thumb?')
    const still = { Id: 'e2', Type: 'Episode', ImageTags: { Primary: 'p' } } as BaseItemDto
    expect(buildImageUrl(B, still, 'Thumb')).toContain('/Items/e2/Images/Primary?')
  })

  it('returns undefined when no image exists', () => {
    expect(buildImageUrl(B, { Id: 'x' } as BaseItemDto, 'Primary')).toBeUndefined()
    expect(buildImageUrl(B, {} as BaseItemDto, 'Primary')).toBeUndefined()
  })

  it('tolerates a trailing slash on the base URL', () => {
    const item = { Id: 'm1', ImageTags: { Primary: 't' } } as BaseItemDto
    expect(buildImageUrl(`${B}/`, item)).toMatch(/^http:\/\/s:8096\/Items\/m1/)
  })
})
