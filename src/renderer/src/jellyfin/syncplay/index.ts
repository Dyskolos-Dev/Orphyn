// SyncPlay protocol layer (server socket, REST requests, messages, clock). See docs/SYNCPLAY.md.
import type { Api } from '@jellyfin/sdk'
import { jfGet } from '../client'
import { DEFAULT_FIELDS } from '../endpoints'
import type { BaseItemDto } from '../types'
import { normalizeGuid } from './protocol'

export * from './protocol'
export * from './socket'
export * from './timeSync'

/**
 * The items of a group's queue, as this user sees them. An id missing from the result is an item
 * the user cannot access (another library, parental rating): the caller says so.
 */
export async function fetchItemsByIds(
  api: Api,
  userId: string,
  ids: readonly string[],
): Promise<Map<string, BaseItemDto>> {
  const found = new Map<string, BaseItemDto>()
  // Long queues (a whole artist) go in chunks so the URL stays reasonable.
  for (let i = 0; i < ids.length; i += 100) {
    const chunk = ids.slice(i, i + 100)
    const res = await jfGet<{ Items?: BaseItemDto[] }>(api, '/Items', {
      userId,
      ids: chunk,
      fields: [...DEFAULT_FIELDS, 'MediaSources'],
      enableUserData: true,
    })
    for (const item of res.Items ?? []) if (item.Id) found.set(normalizeGuid(item.Id), item)
  }
  return found
}

/** The signed-in user's name and avatar tag. */
export async function fetchSelf(api: Api): Promise<{ name: string; imageTag: string | undefined }> {
  const me = await jfGet<{ Name?: string | null; PrimaryImageTag?: string | null }>(
    api,
    '/Users/Me',
  )
  return { name: me.Name ?? '', imageTag: me.PrimaryImageTag ?? undefined }
}

/** Avatar of a user (by id), or undefined when they have none. */
export function userAvatarUrl(
  api: Api,
  userId: string,
  tag: string | null | undefined,
  size = 96,
): string | undefined {
  if (!tag) return undefined
  return `${api.basePath}/Users/${userId}/Images/Primary?tag=${encodeURIComponent(tag)}&maxWidth=${size}&quality=90`
}
