/**
 * Ambient artwork: finds the artwork the user is looking at (hero backdrop, poster, album cover;
 * else the now-playing artwork) and extracts a small palette from it with a canvas sampler, cached
 * per image. The shell paints the image, blurred, behind the content (artwork backdrop), tints the
 * Cinema style with the palette, and gives it to the accent when it follows the artwork.
 */
import { useEffect, useState, type RefObject } from 'react'
import { rgbToHsl, hslToRgb, type Rgb } from './color'

export interface AmbientPalette {
  /** Dominant colour of the artwork. */
  primary: Rgb
  /** A second colour, distinct from the first (falls back to a darker primary). */
  secondary: Rgb
}

const SAMPLE = 32
const cache = new Map<string, Promise<AmbientPalette | null>>()

function luminance([r, g, b]: Rgb): number {
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255
}

function saturation([r, g, b]: Rgb): number {
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  return max === 0 ? 0 : (max - min) / max
}

function distance(a: Rgb, b: Rgb): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2])
}

/**
 * Dominant + secondary colours of RGBA pixels: colours are bucketed (4 bits per channel) and
 * weighted towards saturated mid-tones, so a poster's mood wins over its black bars or white text.
 */
export function paletteFromPixels(data: ArrayLike<number>): AmbientPalette | null {
  const buckets = new Map<number, { r: number; g: number; b: number; n: number; w: number }>()
  for (let i = 0; i + 3 < data.length; i += 4) {
    if ((data[i + 3] ?? 0) < 128) continue
    const px: Rgb = [data[i] ?? 0, data[i + 1] ?? 0, data[i + 2] ?? 0]
    const l = luminance(px)
    const extreme = l < 0.06 || l > 0.94
    const weight = extreme ? 0.08 : 1 + saturation(px) * 3
    const key = ((px[0] >> 4) << 8) | ((px[1] >> 4) << 4) | (px[2] >> 4)
    const b = buckets.get(key) ?? { r: 0, g: 0, b: 0, n: 0, w: 0 }
    b.r += px[0]
    b.g += px[1]
    b.b += px[2]
    b.n += 1
    b.w += weight
    buckets.set(key, b)
  }
  const ranked = [...buckets.values()]
    .sort((a, b) => b.w - a.w)
    .map((b) => [Math.round(b.r / b.n), Math.round(b.g / b.n), Math.round(b.b / b.n)] as const)
  const primary = ranked[0]
  if (!primary) return null
  const secondary = ranked.find((c) => distance(c, primary) > 64) ?? [
    Math.round(primary[0] * 0.55),
    Math.round(primary[1] * 0.55),
    Math.round(primary[2] * 0.55),
  ]
  return { primary, secondary }
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.crossOrigin = 'anonymous'
    img.decoding = 'async'
    img.onload = () => resolve(img)
    img.onerror = () => reject(new Error('artwork failed to load'))
    img.src = src
  })
}

/**
 * Palette of an image, sampled on a 32×32 canvas. Cached per URL; `null` when the image cannot be
 * read (no canvas, or a server that does not allow cross-origin reads).
 */
export function extractPalette(src: string): Promise<AmbientPalette | null> {
  const hit = cache.get(src)
  if (hit) return hit
  const job = (async () => {
    try {
      const canvas = document.createElement('canvas')
      canvas.width = SAMPLE
      canvas.height = SAMPLE
      const ctx = canvas.getContext('2d', { willReadFrequently: true })
      if (!ctx) return null
      const img = await loadImage(src)
      ctx.drawImage(img, 0, 0, SAMPLE, SAMPLE)
      return paletteFromPixels(ctx.getImageData(0, 0, SAMPLE, SAMPLE).data)
    } catch {
      return null
    }
  })()
  cache.set(src, job)
  return job
}

const MIN_AREA = 120 * 120

/** The largest loaded artwork visible in `root` (a hero backdrop wins), or null. */
export function findVisibleArtwork(root: HTMLElement): string | null {
  const view = root.getBoundingClientRect()
  let best: { src: string; score: number } | null = null
  const images = root.querySelectorAll<HTMLImageElement>('.jr-hero__backdrop img, .jr-artwork__img')
  for (const img of images) {
    const src = img.currentSrc || img.src
    if (!src || !img.complete || img.naturalWidth === 0) continue
    const r = img.getBoundingClientRect()
    const w = Math.min(r.right, view.right) - Math.max(r.left, view.left)
    const h = Math.min(r.bottom, view.bottom) - Math.max(r.top, view.top)
    if (w <= 0 || h <= 0) continue
    const area = w * h
    if (area < MIN_AREA) continue
    const score = img.closest('.jr-hero__backdrop') ? area * 4 : area
    if (!best || score > best.score) best = { src, score }
  }
  return best?.src ?? null
}

/**
 * The colour kept in a calm, readable range: mid lightness, no neon. The Cinema surfaces mix it
 * with near-black or near-white, and the labels must keep their contrast over the result.
 */
export function tameAmbientColor(c: Rgb): Rgb {
  const hsl = rgbToHsl(c)
  const [r, g, b] = hslToRgb({
    h: hsl.h,
    s: Math.min(hsl.s, 0.72),
    l: Math.min(0.58, Math.max(0.32, hsl.l)),
  })
  return [Math.round(r), Math.round(g), Math.round(b)]
}

/** The first loaded artwork in `root`, whatever its size (the now-playing bar's cover). */
export function findAnyArtwork(root: HTMLElement | null): string | null {
  if (!root) return null
  for (const img of root.querySelectorAll<HTMLImageElement>('.jr-artwork__img, img')) {
    const src = img.currentSrc || img.src
    if (src && img.complete && img.naturalWidth > 0) return src
  }
  return null
}

export interface AmbientArtwork {
  src: string | null
  palette: AmbientPalette | null
}

/**
 * Follows the artwork visible in the scroll container (route changes, images loading, scrolling)
 * while `enabled`; when the page shows none, the artwork in `fallback` (the now-playing bar). Keeps
 * the last artwork when there is none at all, so the mood does not flicker.
 */
export function useAmbientArtwork(
  scrollRef: RefObject<HTMLElement | null>,
  enabled: boolean,
  fallback: HTMLElement | null = null,
): AmbientArtwork {
  const [src, setSrc] = useState<string | null>(null)
  const [palette, setPalette] = useState<AmbientPalette | null>(null)

  useEffect(() => {
    const root = scrollRef.current
    if (!enabled || !root) return
    let timer: ReturnType<typeof setTimeout> | undefined
    const scan = () => {
      timer = undefined
      const found = findVisibleArtwork(root) ?? findAnyArtwork(fallback)
      if (found) setSrc(found)
    }
    const schedule = () => {
      if (timer === undefined) timer = setTimeout(scan, 250)
    }
    scan()
    const observer =
      typeof MutationObserver === 'function' ? new MutationObserver(schedule) : undefined
    for (const target of fallback ? [root, fallback] : [root]) {
      observer?.observe(target, {
        subtree: true,
        childList: true,
        attributes: true,
        attributeFilter: ['src', 'class'],
      })
    }
    fallback?.addEventListener('load', schedule, true)
    root.addEventListener('load', schedule, true)
    root.addEventListener('scroll', schedule, { passive: true })
    return () => {
      if (timer !== undefined) clearTimeout(timer)
      observer?.disconnect()
      root.removeEventListener('load', schedule, true)
      root.removeEventListener('scroll', schedule)
      fallback?.removeEventListener('load', schedule, true)
    }
  }, [scrollRef, enabled, fallback])

  useEffect(() => {
    if (!enabled || !src) return
    let live = true
    void extractPalette(src).then((value) => {
      if (live) setPalette(value)
    })
    return () => {
      live = false
    }
  }, [src, enabled])

  if (!enabled) return { src: null, palette: null }
  // The previous palette stays until the new one is ready: the tint glides instead of flashing.
  return { src, palette }
}
