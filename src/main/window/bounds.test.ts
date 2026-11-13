import { describe, expect, it } from 'vitest'
import { MIN_SIZE, parseSavedState, resolveBounds, supportsMica } from './bounds'

const primary = { x: 0, y: 25, width: 1920, height: 1055 }

describe('window bounds', () => {
  it('centres a default-size window on the primary display without saved state', () => {
    const b = resolveBounds(undefined, [primary], primary)
    expect(b.width).toBe(1280)
    expect(b.x).toBe(320)
  })

  it('restores saved bounds that are on a display', () => {
    const b = resolveBounds({ x: 100, y: 100, width: 1200, height: 700 }, [primary], primary)
    expect(b).toEqual({ x: 100, y: 100, width: 1200, height: 700 })
  })

  it('enforces the minimum size', () => {
    const b = resolveBounds({ x: 10, y: 30, width: 300, height: 200 }, [primary], primary)
    expect(b.width).toBe(MIN_SIZE.width)
    expect(b.height).toBe(MIN_SIZE.height)
  })

  it('moves a window from a disconnected display back to the primary one', () => {
    const b = resolveBounds({ x: 4000, y: 100, width: 1200, height: 700 }, [primary], primary)
    expect(b.x).toBe(360)
  })

  it('pulls a partly off-screen window back inside the work area', () => {
    const b = resolveBounds({ x: 1500, y: 100, width: 1200, height: 700 }, [primary], primary)
    expect(b.x).toBe(1920 - 1200)
  })

  it('parses persisted state defensively', () => {
    expect(parseSavedState(null)).toBeNull()
    expect(parseSavedState({ bounds: { x: 'a', width: 1100.4 }, isMaximized: true })).toEqual({
      bounds: { width: 1100 },
      isMaximized: true,
      isFullscreen: false,
    })
  })

  it('detects Mica support from the Windows build number', () => {
    expect(supportsMica('win32', '10.0.26200')).toBe(true)
    expect(supportsMica('win32', '10.0.19045')).toBe(false)
    expect(supportsMica('darwin', '24.0.0')).toBe(false)
  })
})
