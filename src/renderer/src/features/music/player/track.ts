import { imageUrl, type BaseItemDto } from '@renderer/jellyfin'

const TICKS_PER_SECOND = 10_000_000

export const trackTitle = (t: BaseItemDto): string => t.Name ?? ''

export const trackArtist = (t: BaseItemDto): string =>
  t.Artists?.length ? t.Artists.join(', ') : (t.AlbumArtist ?? '')

export const trackDuration = (t: BaseItemDto): number =>
  t.RunTimeTicks ? t.RunTimeTicks / TICKS_PER_SECOND : 0

export const trackArt = (t: BaseItemDto | undefined, size: number): string | undefined =>
  t ? imageUrl(t, 'Primary', { maxWidth: size, maxHeight: size, quality: 85 }) : undefined

/** Main artist of the track, for linking to the artist page. */
export const trackArtistId = (t: BaseItemDto): string | undefined =>
  t.ArtistItems?.[0]?.Id ?? t.AlbumArtists?.[0]?.Id ?? undefined

/** Keeps only what the player and its persistence need (ids, names, image tags, duration). */
export function leanTrack(t: BaseItemDto): BaseItemDto {
  return {
    Id: t.Id,
    Type: t.Type ?? 'Audio',
    Name: t.Name,
    Album: t.Album,
    AlbumId: t.AlbumId,
    AlbumArtist: t.AlbumArtist,
    AlbumPrimaryImageTag: t.AlbumPrimaryImageTag,
    Artists: t.Artists,
    ArtistItems: t.ArtistItems?.slice(0, 2),
    AlbumArtists: t.AlbumArtists?.slice(0, 1),
    ImageTags: t.ImageTags?.['Primary'] ? { Primary: t.ImageTags['Primary'] } : undefined,
    RunTimeTicks: t.RunTimeTicks,
    ParentId: t.ParentId,
    IndexNumber: t.IndexNumber,
    ParentIndexNumber: t.ParentIndexNumber,
    MediaType: 'Audio',
    UserData: t.UserData ? { ...t.UserData } : undefined,
  } as BaseItemDto
}
