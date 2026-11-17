/**
 * Colour maths for the theme: WCAG relative luminance and contrast, and lightness adjustments that
 * keep the hue. Used to derive accessible accent tokens from any colour (presets, the custom
 * picker, the artwork) and to keep the ambient artwork colour in a readable range.
 */

export type Rgb = readonly [number, number, number]

const HEX = /^#?([0-9a-f]{6})$/i

export function parseHex(hex: string): Rgb | null {
  const m = HEX.exec(hex.trim())
  if (!m?.[1]) return null
  const n = parseInt(m[1], 16)
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255]
}

export function toHex([r, g, b]: Rgb): string {
  const c = (v: number) =>
    Math.round(Math.min(255, Math.max(0, v)))
      .toString(16)
      .padStart(2, '0')
  return `#${c(r)}${c(g)}${c(b)}`
}

/** WCAG 2 relative luminance, 0 (black) to 1 (white). */
export function luminance([r, g, b]: Rgb): number {
  const lin = (c: number) => {
    const s = c / 255
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/** WCAG 2 contrast ratio between two colours, 1 to 21. */
export function contrast(a: Rgb, b: Rgb): number {
  const la = luminance(a)
  const lb = luminance(b)
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05)
}

export interface Hsl {
  h: number
  s: number
  l: number
}

export function rgbToHsl([r, g, b]: Rgb): Hsl {
  const rn = r / 255
  const gn = g / 255
  const bn = b / 255
  const max = Math.max(rn, gn, bn)
  const min = Math.min(rn, gn, bn)
  const l = (max + min) / 2
  if (max === min) return { h: 0, s: 0, l }
  const d = max - min
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
  let h: number
  if (max === rn) h = (gn - bn) / d + (gn < bn ? 6 : 0)
  else if (max === gn) h = (bn - rn) / d + 2
  else h = (rn - gn) / d + 4
  return { h: h * 60, s, l }
}

export function hslToRgb({ h, s, l }: Hsl): Rgb {
  if (s === 0) return [l * 255, l * 255, l * 255]
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s
  const p = 2 * l - q
  const hue = (t: number) => {
    let x = t
    if (x < 0) x += 1
    if (x > 1) x -= 1
    if (x < 1 / 6) return p + (q - p) * 6 * x
    if (x < 1 / 2) return q
    if (x < 2 / 3) return p + (q - p) * (2 / 3 - x) * 6
    return p
  }
  const hn = h / 360
  return [hue(hn + 1 / 3) * 255, hue(hn) * 255, hue(hn - 1 / 3) * 255]
}

/** The colour with its HSL lightness replaced (rounded to whole channels). */
export function withLightness(c: Rgb, l: number): Rgb {
  const hsl = rgbToHsl(c)
  const [r, g, b] = hslToRgb({ ...hsl, l: Math.min(1, Math.max(0, l)) })
  return [Math.round(r), Math.round(g), Math.round(b)]
}

/**
 * The closest colour of the same hue whose contrast with `against` reaches `target`, moving the
 * lightness towards black (`darken`) or white. Returns the colour itself when it already passes.
 */
export function ensureContrast(c: Rgb, against: Rgb, target: number, darken: boolean): Rgb {
  if (contrast(c, against) >= target) return c
  const start = rgbToHsl(c).l
  // Binary search on lightness between the colour and the far end.
  let lo = darken ? 0 : start
  let hi = darken ? start : 1
  let best: Rgb = darken ? [0, 0, 0] : [255, 255, 255]
  for (let i = 0; i < 24; i++) {
    const mid = (lo + hi) / 2
    const candidate = withLightness(c, mid)
    const ok = contrast(candidate, against) >= target
    if (ok) {
      best = candidate
      if (darken) lo = mid
      else hi = mid
    } else if (darken) hi = mid
    else lo = mid
  }
  return best
}
