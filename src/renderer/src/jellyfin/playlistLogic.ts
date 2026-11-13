/** Pure helpers behind playlist editing and optimistic cache updates (no React, no network). */
import type { BaseItemDto } from './types'

/** Returns a copy of `list` with the element at `from` moved so that it ends up at index `to`. */
export function moveInArray<T>(list: readonly T[], from: number, to: number): T[] {
  const out = [...list]
  if (from < 0 || from >= out.length) return out
  const target = Math.max(0, Math.min(out.length - 1, to))
  const [item] = out.splice(from, 1)
  out.splice(target, 0, item as T)
  return out
}

/**
 * Final index of a row dragged from `from` and dropped on the gap `gap` (0 = before the first row,
 * `length` = after the last one). Dropping next to where it already is returns `from` (no-op).
 */
export function dropTargetIndex(from: number, gap: number): number {
  return gap > from ? gap - 1 : gap
}

/** Drops the entries whose PlaylistItemId is in `entryIds`. */
export function withoutEntries(
  items: readonly BaseItemDto[],
  entryIds: readonly string[],
): BaseItemDto[] {
  const set = new Set(entryIds)
  return items.filter((i) => !(i.PlaylistItemId && set.has(i.PlaylistItemId)))
}

/** Position and track id of entries about to be removed, in playlist order (used by Undo). */
export function describeRemoval(
  items: readonly BaseItemDto[],
  entryIds: readonly string[],
): { itemId: string; index: number }[] {
  const set = new Set(entryIds)
  const out: { itemId: string; index: number }[] = []
  items.forEach((item, index) => {
    if (item.PlaylistItemId && set.has(item.PlaylistItemId) && item.Id) {
      out.push({ itemId: item.Id, index })
    }
  })
  return out
}

const isItem = (v: unknown): v is BaseItemDto =>
  typeof v === 'object' && v !== null && 'Id' in v && 'Type' in v

/**
 * Applies `patch` to every item with Id `id` found anywhere in a cached query value (arrays, pages,
 * infinite pages, search groups, single items). Untouched branches keep their identity.
 */
export function patchItemTree<T>(
  data: T,
  id: string,
  patch: (item: BaseItemDto) => BaseItemDto,
  depth = 0,
): T {
  if (depth > 4 || typeof data !== 'object' || data === null) return data
  if (Array.isArray(data)) {
    let changed = false
    const next = data.map((v) => {
      const n = patchItemTree(v, id, patch, depth + 1)
      if (n !== v) changed = true
      return n
    })
    return (changed ? next : data) as T
  }
  let cur = data as Record<string, unknown>
  if (isItem(data) && data.Id === id) cur = { ...patch(data) } as Record<string, unknown>
  let copied = cur !== (data as unknown)
  for (const [k, v] of Object.entries(cur)) {
    if (k === 'UserData' || k === 'MediaSources' || typeof v !== 'object' || v === null) continue
    const n = patchItemTree(v, id, patch, depth + 1)
    if (n !== v) {
      if (!copied) {
        cur = { ...cur }
        copied = true
      }
      cur[k] = n
    }
  }
  return cur as T
}

export const withFavorite = (value: boolean) => (item: BaseItemDto) =>
  ({ ...item, UserData: { ...item.UserData, IsFavorite: value } }) as BaseItemDto
