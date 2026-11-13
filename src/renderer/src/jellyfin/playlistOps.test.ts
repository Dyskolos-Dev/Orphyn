import { QueryClient } from '@tanstack/react-query'
import { describe, expect, it } from 'vitest'
import { createJellyfinApi } from './client'
import { jfKeys } from './keys'
import {
  addToPlaylistOp,
  createPlaylistOp,
  deletePlaylistOp,
  movePlaylistItemOp,
  removeFromPlaylistOp,
  renamePlaylistOp,
  restoreToPlaylistOp,
  setFavoriteOp,
  type OpsCtx,
} from './playlistOps'
import {
  describeRemoval,
  dropTargetIndex,
  moveInArray,
  patchItemTree,
  withFavorite,
} from './playlistLogic'
import { fakeHttp, type FakeHandler } from './test-utils'
import type { BaseItemDto } from './types'

const ACC = 'acc'
const track = (id: string, entry?: string, fav = false): BaseItemDto =>
  ({ Id: id, Type: 'Audio', PlaylistItemId: entry, UserData: { IsFavorite: fav } }) as BaseItemDto

function setup(handler: FakeHandler = () => ({ status: 204 })) {
  const { http, calls } = fakeHttp(handler)
  const api = createJellyfinApi({
    baseUrl: 'http://s',
    deviceId: 'd',
    accessToken: 't',
    http,
  })
  const qc = new QueryClient()
  const ctx: OpsCtx = { api, userId: 'u1', acc: ACC, qc }
  return { ctx, calls, qc }
}

describe('playlist logic', () => {
  it('moves and clamps', () => {
    expect(moveInArray(['a', 'b', 'c', 'd'], 0, 2)).toEqual(['b', 'c', 'a', 'd'])
    expect(moveInArray(['a', 'b', 'c', 'd'], 3, 0)).toEqual(['d', 'a', 'b', 'c'])
    expect(moveInArray(['a', 'b'], 0, 9)).toEqual(['b', 'a'])
    expect(moveInArray(['a', 'b'], 5, 0)).toEqual(['a', 'b'])
  })

  it('turns a drop gap into a final index', () => {
    expect(dropTargetIndex(1, 4)).toBe(3)
    expect(dropTargetIndex(3, 1)).toBe(1)
    expect(dropTargetIndex(2, 2)).toBe(2)
    expect(dropTargetIndex(2, 3)).toBe(2)
  })

  it('describes removed entries in playlist order', () => {
    const items = [track('a', 'e1'), track('b', 'e2'), track('a', 'e3')]
    expect(describeRemoval(items, ['e3', 'e1'])).toEqual([
      { itemId: 'a', index: 0 },
      { itemId: 'a', index: 2 },
    ])
  })

  it('patches one item everywhere and keeps untouched branches', () => {
    const other = track('z')
    const data = {
      pages: [{ items: [track('a'), other], total: 2 }],
      single: track('a'),
      artist: { Id: 'a', Name: 'only-pairs-lack-Type' },
    }
    const out = patchItemTree(data, 'a', withFavorite(true))
    expect(out.pages[0]?.items[0]?.UserData?.IsFavorite).toBe(true)
    expect(out.pages[0]?.items[1]).toBe(other)
    expect(out.single.UserData?.IsFavorite).toBe(true)
    expect(out.artist).toBe(data.artist)
    expect(patchItemTree(data, 'nope', withFavorite(true))).toBe(data)
  })
})

describe('playlist requests', () => {
  it('creates with the expected body and refreshes the lists', async () => {
    const { ctx, calls, qc } = setup(() => ({ data: { Id: 'new1' } }))
    qc.setQueryData(jfKeys.userPlaylists(ACC), [])
    const id = await createPlaylistOp(ctx, '  Road trip ', ['t1', 'al1'], { isPublic: true })
    expect(id).toBe('new1')
    expect(calls[0]).toMatchObject({
      method: 'POST',
      url: 'http://s/Playlists',
      data: {
        Name: 'Road trip',
        Ids: ['t1', 'al1'],
        UserId: 'u1',
        MediaType: 'Audio',
        IsPublic: true,
      },
    })
    expect(qc.getQueryState(jfKeys.userPlaylists(ACC))?.isInvalidated).toBe(true)
  })

  it('adds, removes and moves with Jellyfin parameters', async () => {
    const { ctx, calls } = setup()
    await addToPlaylistOp(ctx, 'p1', ['a', 'b'])
    await removeFromPlaylistOp(ctx, 'p1', ['e1', 'e2'])
    await movePlaylistItemOp(ctx, 'p1', 'e9', 3)
    expect(calls.map((c) => `${c.method} ${c.url}`)).toEqual([
      'POST http://s/Playlists/p1/Items?ids=a%2Cb&userId=u1',
      'DELETE http://s/Playlists/p1/Items?entryIds=e1%2Ce2',
      'POST http://s/Playlists/p1/Items/e9/Move/3',
    ])
  })

  it('renames optimistically in every cached copy and rolls back on failure', async () => {
    const { ctx, qc } = setup(() => ({ status: 500 }))
    const list = [{ Id: 'p1', Type: 'Playlist', Name: 'Old' }] as BaseItemDto[]
    qc.setQueryData(jfKeys.userPlaylists(ACC), list)
    await expect(renamePlaylistOp(ctx, 'p1', 'New')).rejects.toThrow()
    expect(qc.getQueryData<BaseItemDto[]>(jfKeys.userPlaylists(ACC))?.[0]?.Name).toBe('Old')
  })

  it('renames through POST /Playlists/{id}', async () => {
    const { ctx, calls, qc } = setup()
    qc.setQueryData(jfKeys.item(ACC, 'p1'), { Id: 'p1', Type: 'Playlist', Name: 'Old' })
    await renamePlaylistOp(ctx, 'p1', 'New')
    expect(calls[0]).toMatchObject({
      method: 'POST',
      url: 'http://s/Playlists/p1',
      data: { Name: 'New' },
    })
    expect(qc.getQueryData<BaseItemDto>(jfKeys.item(ACC, 'p1'))?.Name).toBe('New')
  })

  it('deletes via /Items and drops it from the picker list', async () => {
    const { ctx, calls, qc } = setup()
    qc.setQueryData(jfKeys.userPlaylists(ACC), [
      { Id: 'p1', Type: 'Playlist' },
      { Id: 'p2', Type: 'Playlist' },
    ])
    await deletePlaylistOp(ctx, 'p1')
    expect(calls[0]).toMatchObject({ method: 'DELETE', url: 'http://s/Items/p1' })
    expect(qc.getQueryData<BaseItemDto[]>(jfKeys.userPlaylists(ACC))?.map((p) => p.Id)).toEqual([
      'p2',
    ])
  })

  it('removes entries from the cache at once and reports what was removed', async () => {
    const { ctx, qc } = setup()
    const key = jfKeys.playlistItems(ACC, 'p1')
    qc.setQueryData(key, [track('a', 'e1'), track('b', 'e2'), track('c', 'e3')])
    const removed = await removeFromPlaylistOp(ctx, 'p1', ['e2'])
    expect(removed).toEqual([{ itemId: 'b', index: 1 }])
    expect(qc.getQueryData<BaseItemDto[]>(key)?.map((t) => t.Id)).toEqual(['a', 'c'])
  })

  it('reorders optimistically and restores the order when the server refuses', async () => {
    const { ctx, qc } = setup(() => ({ status: 500 }))
    const key = jfKeys.playlistItems(ACC, 'p1')
    qc.setQueryData(key, [track('a', 'e1'), track('b', 'e2'), track('c', 'e3')])
    await expect(movePlaylistItemOp(ctx, 'p1', 'e1', 2)).rejects.toThrow()
    expect(qc.getQueryData<BaseItemDto[]>(key)?.map((t) => t.Id)).toEqual(['a', 'b', 'c'])
  })

  it('undo re-adds then moves the entry back to its position', async () => {
    const { ctx, calls } = setup((r) =>
      r.method === 'GET'
        ? { data: { Items: [track('a', 'e1'), track('c', 'e3'), track('b', 'n1')] } }
        : { status: 204 },
    )
    await restoreToPlaylistOp(ctx, 'p1', [{ itemId: 'b', index: 1 }])
    expect(calls.map((c) => `${c.method} ${c.url.split('?')[0]}`)).toEqual([
      'POST http://s/Playlists/p1/Items',
      'GET http://s/Playlists/p1/Items',
      'POST http://s/Playlists/p1/Items/n1/Move/1',
    ])
  })
})

describe('favourites', () => {
  it('flips every cached copy instantly and calls the right verb', async () => {
    const { ctx, calls, qc } = setup()
    qc.setQueryData(jfKeys.item(ACC, 'a'), track('a'))
    qc.setQueryData(jfKeys.list(ACC, 'songs', {}), { pages: [{ items: [track('a')] }] })
    await setFavoriteOp(ctx, 'a', true)
    expect(calls[0]).toMatchObject({
      method: 'POST',
      url: 'http://s/UserFavoriteItems/a?userId=u1',
    })
    expect(qc.getQueryData<BaseItemDto>(jfKeys.item(ACC, 'a'))?.UserData?.IsFavorite).toBe(true)
    const list = qc.getQueryData<{ pages: { items: BaseItemDto[] }[] }>(
      jfKeys.list(ACC, 'songs', {}),
    )
    expect(list?.pages[0]?.items[0]?.UserData?.IsFavorite).toBe(true)
    await setFavoriteOp(ctx, 'a', false)
    expect(calls[1]?.method).toBe('DELETE')
  })

  it('rolls back when the request fails', async () => {
    const { ctx, qc } = setup(() => ({ status: 500 }))
    qc.setQueryData(jfKeys.item(ACC, 'a'), track('a'))
    await expect(setFavoriteOp(ctx, 'a', true)).rejects.toThrow()
    expect(qc.getQueryData<BaseItemDto>(jfKeys.item(ACC, 'a'))?.UserData?.IsFavorite).toBe(false)
  })
})
