/** Pure window-geometry helpers (no Electron import, unit-tested). */

export interface Rect {
  x: number
  y: number
  width: number
  height: number
}

/** What is persisted between launches. */
export interface SavedWindowState {
  bounds: Partial<Rect>
  isMaximized?: boolean
  isFullscreen?: boolean
}

export const DEFAULT_SIZE = { width: 1280, height: 820 } as const
export const MIN_SIZE = { width: 1000, height: 640 } as const

const isNum = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v)

/** Parses the persisted JSON defensively; returns null for anything unusable. */
export function parseSavedState(raw: unknown): SavedWindowState | null {
  if (!raw || typeof raw !== 'object') return null
  const r = raw as Record<string, unknown>
  const b = r['bounds']
  if (!b || typeof b !== 'object') return null
  const bb = b as Record<string, unknown>
  const bounds: Partial<Rect> = {}
  for (const k of ['x', 'y', 'width', 'height'] as const) {
    if (isNum(bb[k])) bounds[k] = Math.round(bb[k])
  }
  return {
    bounds,
    isMaximized: r['isMaximized'] === true,
    isFullscreen: r['isFullscreen'] === true,
  }
}

function intersectionArea(a: Rect, b: Rect): number {
  const w = Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x)
  const h = Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y)
  return w > 0 && h > 0 ? w * h : 0
}

/**
 * Turns saved bounds into bounds that are safe to open with: at least the minimum size, no larger
 * than the display, and on a display that still exists (else centred on the primary work area).
 */
export function resolveBounds(
  saved: Partial<Rect> | undefined,
  workAreas: Rect[],
  primary: Rect,
): Partial<Rect> & { width: number; height: number } {
  const fitSize = (area: Rect, w: number, h: number) => ({
    width: Math.min(Math.max(w, MIN_SIZE.width), Math.max(area.width, MIN_SIZE.width)),
    height: Math.min(Math.max(h, MIN_SIZE.height), Math.max(area.height, MIN_SIZE.height)),
  })
  const width = saved?.width ?? DEFAULT_SIZE.width
  const height = saved?.height ?? DEFAULT_SIZE.height

  if (saved && isNum(saved.x) && isNum(saved.y)) {
    const candidate: Rect = { x: saved.x, y: saved.y, width, height }
    let best: Rect | undefined
    let bestArea = 0
    for (const area of workAreas) {
      const a = intersectionArea(candidate, area)
      if (a > bestArea) {
        bestArea = a
        best = area
      }
    }
    // Require a meaningful part of the window (a title bar's worth) to be visible.
    if (best && bestArea >= Math.min(candidate.width, 200) * 40) {
      const size = fitSize(best, width, height)
      const x = Math.min(Math.max(candidate.x, best.x), best.x + best.width - size.width)
      const y = Math.min(Math.max(candidate.y, best.y), best.y + best.height - size.height)
      return { x, y, ...size }
    }
  }
  const size = fitSize(primary, width, height)
  return {
    x: Math.round(primary.x + (primary.width - size.width) / 2),
    y: Math.round(primary.y + (primary.height - size.height) / 2),
    ...size,
  }
}

/** Windows 11 (build 22000+) supports Mica; 22621+ supports `backgroundMaterial`. */
export function supportsMica(platform: string, osRelease: string): boolean {
  if (platform !== 'win32') return false
  const build = Number(osRelease.split('.')[2])
  return Number.isFinite(build) && build >= 22621
}
