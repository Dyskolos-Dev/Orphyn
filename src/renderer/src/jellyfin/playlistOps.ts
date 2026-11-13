/**
 * Playlist and favourite mutations with their cache handling, as plain async functions so they can be
 * tested against a mocked API. `usePlaylistActions` / `useToggleFavorite` (hooks.ts) wrap them.
 */
import type { QueryClient } from '@tanstack/react-query'
import type { Api } from '@jellyfin/sdk'
import * as ep from './endpoints'
import { jfKeys } from './keys'
import {
  describeRemoval,
  moveInArray,
  patchItemTree,
  withFavorite,
  withoutEntries,
} from './playlistLogic'
import type { BaseItemDto } from './types'

export interface OpsCtx {
  api: Api
  userId: string
  acc: string
  qc: QueryClient
}

type Snapshot = [readonly unknown[], unknown][]

const snapshot = (c: OpsCtx, key: readonly unknown[]): Snapshot =>
  c.qc.getQueriesData({ queryKey: key }) as Snapshot

const restore = (c: OpsCtx, snap: Snapshot) => snap.forEach(([k, d]) => c.qc.setQueryData(k, d))

const refreshPlaylistLists = (c: OpsCtx) =>
  Promise.all([
    c.qc.invalidateQueries({ queryKey: jfKeys.userPlaylists(c.acc) }),
    c.qc.invalidateQueries({ queryKey: ['jf', c.acc, 'list', 'playlists'] }),
    c.qc.invalidateQueries({ queryKey: ['jf', c.acc, 'items'] }),
  ])

/** Sets the heart on every cached copy of the item (lists, details, search, favourites). */
export async function setFavoriteOp(c: OpsCtx, id: string, value: boolean): Promise<void> {
  const all = jfKeys.all(c.acc)
  await c.qc.cancelQueries({ queryKey: all })
  const snap = snapshot(c, all)
  c.qc.setQueriesData({ queryKey: all }, (d: unknown) => patchItemTree(d, id, withFavorite(value)))
  try {
    await ep.setFavorite(c.api, c.userId, id, value)
  } catch (e) {
    restore(c, snap)
    throw e
  } finally {
    // Favourite lists gain / lose rows: refetch them (the flag itself is already correct).
    void c.qc.invalidateQueries({ queryKey: ['jf', c.acc, 'favorites'] })
  }
}

export async function createPlaylistOp(
  c: OpsCtx,
  name: string,
  itemIds: readonly string[] = [],
  opts: { isPublic?: boolean; description?: string } = {},
): Promise<string> {
  const id = await ep.createPlaylist(c.api, c.userId, name.trim(), itemIds, opts)
  const description = opts.description?.trim()
  if (description) await ep.setPlaylistDescription(c.api, c.userId, id, description)
  await refreshPlaylistLists(c)
  return id
}

export async function renamePlaylistOp(c: OpsCtx, playlistId: string, name: string): Promise<void> {
  const clean = name.trim()
  const snap = snapshot(c, jfKeys.all(c.acc))
  c.qc.setQueriesData({ queryKey: jfKeys.all(c.acc) }, (d: unknown) =>
    patchItemTree(d, playlistId, (i) => ({ ...i, Name: clean, SortName: clean })),
  )
  try {
    await ep.renamePlaylist(c.api, playlistId, clean)
  } catch (e) {
    restore(c, snap)
    throw e
  } finally {
    void refreshPlaylistLists(c)
  }
}

export async function deletePlaylistOp(c: OpsCtx, playlistId: string): Promise<void> {
  const key = jfKeys.userPlaylists(c.acc)
  const snap = snapshot(c, key)
  c.qc.setQueryData<BaseItemDto[]>(key, (d) => d?.filter((p) => p.Id !== playlistId))
  try {
    await ep.deletePlaylist(c.api, playlistId)
  } catch (e) {
    restore(c, snap)
    throw e
  } finally {
    c.qc.removeQueries({ queryKey: jfKeys.playlistItems(c.acc, playlistId) })
    c.qc.removeQueries({ queryKey: jfKeys.item(c.acc, playlistId) })
    void refreshPlaylistLists(c)
  }
}

export async function addToPlaylistOp(
  c: OpsCtx,
  playlistId: string,
  itemIds: readonly string[],
): Promise<void> {
  if (itemIds.length === 0) return
  await ep.addToPlaylist(c.api, c.userId, playlistId, itemIds)
  void c.qc.invalidateQueries({ queryKey: jfKeys.playlistItems(c.acc, playlistId) })
  void c.qc.invalidateQueries({ queryKey: jfKeys.item(c.acc, playlistId) })
  void refreshPlaylistLists(c)
}

export interface RemovedEntry {
  itemId: string
  index: number
}

/** Removes entries; resolves with what was removed (positions included) so the caller can offer Undo. */
export async function removeFromPlaylistOp(
  c: OpsCtx,
  playlistId: string,
  entryIds: readonly string[],
): Promise<RemovedEntry[]> {
  const key = jfKeys.playlistItems(c.acc, playlistId)
  const before = c.qc.getQueryData<BaseItemDto[]>(key) ?? []
  const removed = describeRemoval(before, entryIds)
  const snap = snapshot(c, key)
  c.qc.setQueryData<BaseItemDto[]>(key, (d) => (d ? withoutEntries(d, entryIds) : d))
  try {
    await ep.removeFromPlaylist(c.api, playlistId, entryIds)
  } catch (e) {
    restore(c, snap)
    throw e
  } finally {
    void c.qc.invalidateQueries({ queryKey: key })
    void c.qc.invalidateQueries({ queryKey: jfKeys.item(c.acc, playlistId) })
    void refreshPlaylistLists(c)
  }
  return removed
}

/** Undo of a removal: re-adds the tracks, then moves each back to where it was. */
export async function restoreToPlaylistOp(
  c: OpsCtx,
  playlistId: string,
  removed: readonly RemovedEntry[],
): Promise<void> {
  if (removed.length === 0) return
  await ep.addToPlaylist(
    c.api,
    c.userId,
    playlistId,
    removed.map((r) => r.itemId),
  )
  const items = await ep.fetchPlaylistItems(c.api, c.userId, playlistId)
  // The re-added entries are the last ones, in the order they were sent.
  const base = items.length - removed.length
  for (let i = 0; i < removed.length; i++) {
    const entry = items[base + i]?.PlaylistItemId
    const target = removed[i]
    if (entry && target && target.index < base + i) {
      await ep.movePlaylistItem(c.api, playlistId, entry, target.index)
    }
  }
  await c.qc.invalidateQueries({ queryKey: jfKeys.playlistItems(c.acc, playlistId) })
  void refreshPlaylistLists(c)
}

/** Reorders one entry: instant in the cache, rolled back if the server refuses. */
export async function movePlaylistItemOp(
  c: OpsCtx,
  playlistId: string,
  entryId: string,
  newIndex: number,
): Promise<void> {
  const key = jfKeys.playlistItems(c.acc, playlistId)
  await c.qc.cancelQueries({ queryKey: key })
  const snap = snapshot(c, key)
  c.qc.setQueryData<BaseItemDto[]>(key, (d) => {
    if (!d) return d
    const from = d.findIndex((i) => i.PlaylistItemId === entryId)
    return from < 0 ? d : moveInArray(d, from, newIndex)
  })
  try {
    await ep.movePlaylistItem(c.api, playlistId, entryId, newIndex)
  } catch (e) {
    restore(c, snap)
    throw e
  } finally {
    void c.qc.invalidateQueries({ queryKey: key })
  }
}
