import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { BaseItemDto } from '@renderer/jellyfin'
import {
  reloadPlaybackPreferences,
  updatePlaybackPreferences,
} from '@renderer/features/settings/preferences'
import { resetArtistImageState } from './artistImages'
import { useArtistImage } from './useArtistImage'

const bridge = vi.hoisted(() => ({
  available: vi.fn(() => true),
  artistImage: vi.fn<(name: string) => Promise<string | null>>(),
}))
vi.mock('@renderer/bridge/artwork', () => ({ artworkApi: bridge }))
vi.mock('@renderer/jellyfin', async (orig) => ({
  ...(await orig<typeof import('@renderer/jellyfin')>()),
  imageUrl: (item: BaseItemDto) =>
    item.ImageTags?.['Primary'] ? `https://server/Items/${item.Id}/Images/Primary` : undefined,
}))

const artist = (over: Partial<BaseItemDto> = {}): BaseItemDto =>
  ({ Id: 'a1', Name: 'Daft Punk', ...over }) as BaseItemDto
const withServerImage = artist({ ImageTags: { Primary: 'tag' } })

beforeEach(() => {
  localStorage.clear()
  reloadPlaybackPreferences()
  resetArtistImageState()
  bridge.available.mockReturnValue(true)
  bridge.artistImage.mockReset()
  bridge.artistImage.mockResolvedValue('https://cdn.example/daft.jpg')
})

describe('useArtistImage fallback order', () => {
  it('uses the server image and never asks online when there is one', async () => {
    const { result } = renderHook(() => useArtistImage(withServerImage))
    expect(result.current.src).toBe('https://server/Items/a1/Images/Primary')
    await act(async () => {})
    expect(bridge.artistImage).not.toHaveBeenCalled()
  })

  it('falls back to the online image when the server has none', async () => {
    const { result } = renderHook(() => useArtistImage(artist()))
    expect(result.current.src).toBeUndefined()
    await waitFor(() => expect(result.current.src).toBe('https://cdn.example/daft.jpg'))
    expect(bridge.artistImage).toHaveBeenCalledWith('Daft Punk')
  })

  it('stays empty (caller fallback) when nothing reliable is found', async () => {
    bridge.artistImage.mockResolvedValue(null)
    const { result } = renderHook(() => useArtistImage(artist()))
    await waitFor(() => expect(bridge.artistImage).toHaveBeenCalled())
    expect(result.current.src).toBeUndefined()
  })

  it('makes no request when the setting is off', async () => {
    updatePlaybackPreferences({ artistImages: false })
    const { result } = renderHook(() => useArtistImage(artist()))
    await act(async () => {})
    expect(result.current.src).toBeUndefined()
    expect(bridge.artistImage).not.toHaveBeenCalled()
  })

  it('makes no request outside the native shell', async () => {
    bridge.available.mockReturnValue(false)
    renderHook(() => useArtistImage(artist()))
    await act(async () => {})
    expect(bridge.artistImage).not.toHaveBeenCalled()
  })

  it('answers repeated artists from the cache with a single lookup', async () => {
    const first = renderHook(() => useArtistImage(artist()))
    await waitFor(() => expect(first.result.current.src).toBeDefined())
    const second = renderHook(() => useArtistImage(artist({ Id: 'a2', Name: 'daft  punk' })))
    expect(second.result.current.src).toBe('https://cdn.example/daft.jpg')
    expect(bridge.artistImage).toHaveBeenCalledTimes(1)
  })
})
