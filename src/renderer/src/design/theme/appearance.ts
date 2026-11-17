/**
 * Appearance options (Settings > Appearance) layered on top of the interface style: transparency,
 * corners, density, text size, sidebar, artwork backdrop, motion, video cards. Each style has its
 * own defaults (`STYLE_DEFAULTS`); the user's choices are stored as overrides, so "Reset to style
 * defaults" simply drops them. Every option maps to a data attribute and / or a CSS variable on
 * <html> (`appearanceTokens`), which the token layers in styles/tokens.css read. Pure: the store
 * lives in interfaceStyle.ts.
 */
import type { InterfaceStyle } from './interfaceStyle'

export const CORNER_OPTIONS = ['sharp', 'standard', 'round'] as const
export const DENSITY_OPTIONS = ['comfortable', 'standard', 'compact'] as const
export const TEXT_SIZE_OPTIONS = ['s', 'm', 'l', 'xl'] as const
export const SIDEBAR_OPTIONS = ['expanded', 'icons', 'auto-hide'] as const
export const BACKDROP_OPTIONS = ['off', 'subtle', 'immersive'] as const
export const MOTION_OPTIONS = ['full', 'reduced'] as const
export const VIDEO_CARD_OPTIONS = ['poster', 'landscape'] as const

export type Corners = (typeof CORNER_OPTIONS)[number]
export type Density = (typeof DENSITY_OPTIONS)[number]
export type TextSize = (typeof TEXT_SIZE_OPTIONS)[number]
export type SidebarMode = (typeof SIDEBAR_OPTIONS)[number]
export type Backdrop = (typeof BACKDROP_OPTIONS)[number]
export type Motion = (typeof MOTION_OPTIONS)[number]
export type VideoCards = (typeof VIDEO_CARD_OPTIONS)[number]

export interface Appearance {
  /** Translucency of materials, 0 (solid, no blur) to 100 (the style's full recipe). */
  transparency: number
  corners: Corners
  /** Spacing and row heights, on top of the style's own (Compact stays denser than Classic). */
  density: Density
  /** Scales the whole type ramp. */
  textSize: TextSize
  sidebar: SidebarMode
  /** Blurred artwork of the page behind the interface. */
  backdrop: Backdrop
  motion: Motion
  /** Card shape of the video shelves. */
  videoCards: VideoCards
}

export type AppearanceKey = keyof Appearance

const BASE: Appearance = {
  transparency: 100,
  corners: 'standard',
  density: 'standard',
  textSize: 'm',
  sidebar: 'expanded',
  backdrop: 'off',
  motion: 'full',
  videoCards: 'poster',
}

/** What each style looks like before the user changes anything. */
export const STYLE_DEFAULTS: Readonly<Record<InterfaceStyle, Readonly<Appearance>>> = {
  classic: BASE,
  glass: BASE,
  cinema: { ...BASE, sidebar: 'icons', backdrop: 'immersive' },
  // Minimal is solid by design: its materials ignore the transparency.
  minimal: { ...BASE, transparency: 0 },
  compact: BASE,
}

/** Styles whose materials are always opaque: the transparency slider does nothing there. */
export const SOLID_STYLES: readonly InterfaceStyle[] = ['minimal']

const CHOICES: { [K in Exclude<AppearanceKey, 'transparency'>]: readonly Appearance[K][] } = {
  corners: CORNER_OPTIONS,
  density: DENSITY_OPTIONS,
  textSize: TEXT_SIZE_OPTIONS,
  sidebar: SIDEBAR_OPTIONS,
  backdrop: BACKDROP_OPTIONS,
  motion: MOTION_OPTIONS,
  videoCards: VIDEO_CARD_OPTIONS,
}

export const APPEARANCE_KEYS = Object.keys(BASE) as AppearanceKey[]

/** Keeps the valid fields of a stored object (anything else is dropped, field by field). */
export function sanitizeOverrides(raw: unknown): Partial<Appearance> {
  if (raw === null || typeof raw !== 'object') return {}
  const o = raw as Record<string, unknown>
  const out: Partial<Appearance> = {}
  const t = o['transparency']
  if (typeof t === 'number' && Number.isFinite(t)) {
    out.transparency = Math.round(Math.min(100, Math.max(0, t)))
  }
  for (const key of Object.keys(CHOICES) as (keyof typeof CHOICES)[]) {
    const v = o[key]
    if ((CHOICES[key] as readonly unknown[]).includes(v)) {
      ;(out as Record<string, unknown>)[key] = v
    }
  }
  return out
}

export function resolveAppearance(
  style: InterfaceStyle,
  overrides: Partial<Appearance>,
): Appearance {
  return { ...STYLE_DEFAULTS[style], ...overrides }
}

export const RADIUS_SCALE: Record<Corners, number> = { sharp: 0.35, standard: 1, round: 1.45 }
export const TEXT_SCALE: Record<TextSize, number> = { s: 0.92, m: 1, l: 1.1, xl: 1.22 }
/** Gaps and gutters / row heights / cards / items added to a shelf, per density. */
export const DENSITY_SCALE: Record<
  Density,
  { space: number; rows: number; cards: number; items: number }
> = {
  comfortable: { space: 1.25, rows: 1.15, cards: 1.12, items: -1 },
  standard: { space: 1, rows: 1, cards: 1, items: 0 },
  compact: { space: 0.75, rows: 0.85, cards: 0.86, items: 1 },
}

export interface AppearanceTokens {
  /** `data-*` attributes for <html> (camelCase dataset keys); `null` removes the attribute. */
  attributes: Record<string, string | null>
  /** CSS custom properties for <html>. */
  properties: Record<string, string>
}

/** The data attributes and CSS variables that carry an appearance onto <html>. */
export function appearanceTokens(a: Appearance): AppearanceTokens {
  const density = DENSITY_SCALE[a.density]
  const solid = a.transparency <= 0
  return {
    attributes: {
      corners: a.corners,
      density: a.density,
      textSize: a.textSize,
      sidebar: a.sidebar,
      backdrop: a.backdrop,
      motion: a.motion,
      videoCards: a.videoCards,
      transparency: solid ? 'solid' : a.transparency >= 100 ? 'full' : 'partial',
      // Kept for the CSS that kills every backdrop-filter when the user wants solid surfaces.
      reduceTransparency: solid ? 'true' : null,
    },
    properties: {
      '--transparency': String(Math.min(100, Math.max(0, a.transparency)) / 100),
      '--radius-scale': String(RADIUS_SCALE[a.corners]),
      '--text-scale': String(TEXT_SCALE[a.textSize]),
      '--density': String(density.space),
      '--density-rows': String(density.rows),
      '--density-cards': String(density.cards),
      '--density-items': String(density.items),
    },
  }
}

const ATTRIBUTE_KEYS = Object.keys(appearanceTokens(BASE).attributes)
const PROPERTY_KEYS = Object.keys(appearanceTokens(BASE).properties)

/** Writes an appearance onto an element (<html> by default). */
export function applyAppearanceToElement(a: Appearance, el?: HTMLElement): void {
  const root = el ?? (typeof document !== 'undefined' ? document.documentElement : undefined)
  if (!root) return
  const { attributes, properties } = appearanceTokens(a)
  for (const key of ATTRIBUTE_KEYS) {
    const v = attributes[key]
    if (v == null) delete root.dataset[key]
    else root.dataset[key] = v
  }
  for (const key of PROPERTY_KEYS) root.style.setProperty(key, properties[key] ?? '')
}
