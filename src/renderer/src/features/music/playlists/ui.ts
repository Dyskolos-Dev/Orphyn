/** Tiny store behind the playlist dialogs: any menu or button opens them, `PlaylistHost` renders them. */
import { create } from 'zustand'
import type { BaseItemDto } from '@renderer/jellyfin'

export interface PlaylistUiState {
  /** "New Playlist" sheet; `itemIds` are added to it right after creation. */
  sheet: { itemIds: string[] } | null
  /** "Add to playlist" picker menu. */
  picker: { ids: string[]; anchor: { x: number; y: number } } | null
  /** Playlist awaiting delete confirmation. */
  deleting: { playlist: BaseItemDto; onDeleted?: () => void } | null
}

export const usePlaylistUi = create<PlaylistUiState>(() => ({
  sheet: null,
  picker: null,
  deleting: null,
}))

let lastPointer = { x: 0, y: 0 }
let tracking = false

/** Remembers where the user last clicked, so a picker opened from a menu item appears right there. */
export function trackPointer(): () => void {
  if (tracking || typeof window === 'undefined') return () => undefined
  tracking = true
  const on = (e: PointerEvent) => {
    lastPointer = { x: e.clientX, y: e.clientY }
  }
  window.addEventListener('pointerdown', on, true)
  return () => {
    window.removeEventListener('pointerdown', on, true)
    tracking = false
  }
}

export const openNewPlaylist = (itemIds: string[] = []) =>
  usePlaylistUi.setState({ sheet: { itemIds } })

export const openPlaylistPicker = (ids: string[], anchor?: { x: number; y: number }) =>
  usePlaylistUi.setState({ picker: { ids, anchor: anchor ?? lastPointer } })

export const askDeletePlaylist = (playlist: BaseItemDto, onDeleted?: () => void) =>
  usePlaylistUi.setState({ deleting: { playlist, onDeleted } })
