/** Drag payload shared by track rows, album cards and the sidebar playlists (drop targets). */
export const DRAG_MIME = 'application/x-orphyn-items'

export interface DragPayload {
  /** Track or album ids (the server expands albums when adding to a playlist). */
  ids: string[]
  label: string
}

export function writeDragItems(dt: DataTransfer, payload: DragPayload): void {
  dt.setData(DRAG_MIME, JSON.stringify(payload))
  dt.setData('text/plain', payload.label)
  dt.effectAllowed = 'copyMove'
}

export const hasDragItems = (dt: DataTransfer | null): boolean =>
  !!dt && Array.from(dt.types).includes(DRAG_MIME)

export function readDragItems(dt: DataTransfer | null): DragPayload | null {
  const raw = dt?.getData(DRAG_MIME)
  if (!raw) return null
  try {
    const p = JSON.parse(raw) as DragPayload
    return Array.isArray(p.ids) && p.ids.length > 0 ? p : null
  } catch {
    return null
  }
}

/** Playlist id of a sidebar link (`…/music/playlist/<id>`), or null. */
export function playlistIdFromHref(href: string | null): string | null {
  const m = /\/music\/playlist\/([^/?#]+)\/?$/.exec(href ?? '')
  return m?.[1] ?? null
}

/** Index of the gap (0..rects.length) nearest to pointer `y` between rows with the given vertical bounds. */
export function gapAtY(rows: readonly { top: number; bottom: number }[], y: number): number {
  for (let i = 0; i < rows.length; i++) {
    const r = rows[i]
    if (r && y < (r.top + r.bottom) / 2) return i
  }
  return rows.length
}
