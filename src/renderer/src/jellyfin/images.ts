import { buildQuery } from './client'
import { useSession } from './session'
import type { BaseItemDto, ImageKind } from './types'

export interface ImageOptions {
  maxWidth?: number
  maxHeight?: number
  /** JPEG/WebP quality 1-100 (default 90). */
  quality?: number
  /** Override the cache-busting tag (normally taken from the item). */
  tag?: string
  /** Override the server base URL (defaults to the active server). */
  baseUrl?: string
}

interface Source {
  id: string
  tag: string | undefined
  /** Backdrop index, when relevant. */
  index?: number
  /** Image type actually served (Thumb may fall back to Primary). */
  type?: ImageKind
}

const asType = (s: Source | undefined, type: ImageKind): Source | undefined => s && { ...s, type }

/** Picks which item/tag provides `type`, following the fallbacks (series for episodes, album for tracks). */
function resolveSource(item: BaseItemDto, type: ImageKind): Source | undefined {
  const id = item.Id
  if (!id) return undefined
  const own = (tag: string | null | undefined, itemId = id): Source | undefined =>
    tag ? { id: itemId, tag } : undefined

  switch (type) {
    case 'Primary':
      return (
        own(item.ImageTags?.['Primary']) ??
        (item.Type === 'Episode' || item.Type === 'Season'
          ? own(item.SeriesPrimaryImageTag, item.SeriesId ?? undefined)
          : undefined) ??
        (item.Type === 'Audio'
          ? own(item.AlbumPrimaryImageTag, item.AlbumId ?? undefined)
          : undefined) ??
        own(item.ParentPrimaryImageTag, item.ParentPrimaryImageItemId ?? undefined)
      )
    case 'Backdrop': {
      const tag = item.BackdropImageTags?.[0]
      if (tag) return { id, tag, index: 0 }
      const parentTag = item.ParentBackdropImageTags?.[0]
      if (parentTag && item.ParentBackdropItemId) {
        return { id: item.ParentBackdropItemId, tag: parentTag, index: 0 }
      }
      return undefined
    }
    case 'Logo':
      return (
        own(item.ImageTags?.['Logo']) ??
        own(item.ParentLogoImageTag, item.ParentLogoItemId ?? undefined)
      )
    case 'Thumb':
      return (
        own(item.ImageTags?.['Thumb']) ??
        own(item.ParentThumbImageTag, item.ParentThumbItemId ?? undefined) ??
        // An episode's own still is the natural thumbnail when no Thumb exists.
        asType(resolveSource(item, 'Primary'), 'Primary')
      )
  }
}

/** Pure builder: image URL for `item` on the server at `baseUrl`, or undefined when no image exists. */
export function buildImageUrl(
  baseUrl: string,
  item: BaseItemDto,
  type: ImageKind = 'Primary',
  opts: ImageOptions = {},
): string | undefined {
  const src = resolveSource(item, type)
  if (!src) return undefined
  const kind = src.type ?? type
  const path =
    src.index !== undefined
      ? `/Items/${src.id}/Images/${kind}/${src.index}`
      : `/Items/${src.id}/Images/${kind}`
  const query = buildQuery({
    maxWidth: opts.maxWidth,
    maxHeight: opts.maxHeight,
    quality: opts.quality ?? 90,
    tag: opts.tag ?? src.tag,
  })
  return `${baseUrl.replace(/\/+$/, '')}${path}?${query}`
}

/** Image URL on the active server; undefined when there is no image or no active server. */
export function imageUrl(
  item: BaseItemDto,
  type: ImageKind = 'Primary',
  opts: ImageOptions = {},
): string | undefined {
  const base = opts.baseUrl ?? useSession.getState().api?.basePath
  return base ? buildImageUrl(base, item, type, opts) : undefined
}
