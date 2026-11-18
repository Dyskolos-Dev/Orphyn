import type { BaseItemDto } from '@renderer/jellyfin'

export type ReleaseType = 'album' | 'ep' | 'compilation'

/** Releases with this many tracks or fewer are singles / EPs. */
export const MAX_SINGLE_TRACKS = 6

const COMPILATION =
  /\b(?:compilations?|best of|greatest hits|the very best|anthology|collection|essentials?|hits|retrospective|rarities|b-sides)\b/i
const SINGLE_OR_EP = /\b(?:single|ep|maxi)\b/i
const VARIOUS = /^(?:various artists?|artistes? vari[ée]s?|va)$/i

/**
 * Classifies a Jellyfin album. The server exposes no release type, so: a title or credit that says
 * "compilation" / "greatest hits" / "various artists" is a compilation; a title that says
 * "single" / "EP", or a short track count, is an EP / single; everything else is an album.
 */
export function classifyRelease(album: BaseItemDto): ReleaseType {
  const name = album.Name ?? ''
  if (COMPILATION.test(name) || VARIOUS.test((album.AlbumArtist ?? '').trim())) return 'compilation'
  if (SINGLE_OR_EP.test(name)) return 'ep'
  const n = album.ChildCount
  if (n != null && n > 0 && n <= MAX_SINGLE_TRACKS) return 'ep'
  return 'album'
}

/** Newest first; undated releases last, ties by name. */
export function sortNewestFirst(albums: readonly BaseItemDto[]): BaseItemDto[] {
  return [...albums].sort(
    (a, b) =>
      (b.ProductionYear ?? 0) - (a.ProductionYear ?? 0) ||
      (a.Name ?? '').localeCompare(b.Name ?? ''),
  )
}

export interface Discography {
  albums: BaseItemDto[]
  eps: BaseItemDto[]
  compilations: BaseItemDto[]
  appearsOn: BaseItemDto[]
}

/**
 * Splits an artist's albums. `ids` are every Jellyfin id of the (possibly merged) artist: an album
 * credited to one of them is the artist's own, any other album only holds a track of theirs.
 */
export function buildDiscography(
  albums: readonly BaseItemDto[],
  ids: readonly string[],
): Discography {
  const own: BaseItemDto[] = []
  const appearsOn: BaseItemDto[] = []
  for (const a of albums) {
    if (a.AlbumArtists?.some((x) => !!x.Id && ids.includes(x.Id))) own.push(a)
    else appearsOn.push(a)
  }
  const by = (t: ReleaseType) => sortNewestFirst(own.filter((a) => classifyRelease(a) === t))
  return {
    albums: by('album'),
    eps: by('ep'),
    compilations: by('compilation'),
    appearsOn: sortNewestFirst(appearsOn),
  }
}
