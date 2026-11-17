import { useEffect, type CSSProperties, type RefObject } from 'react'
import {
  tameAmbientColor,
  themeStore,
  useAmbientArtwork,
  useInterfaceStyle,
  useTheme,
  type AmbientPalette,
} from '@renderer/design'

export interface ShellAmbientProps {
  /** The page scroll container, where the visible artwork is looked for. */
  scrollRef: RefObject<HTMLElement | null>
  /** The now-playing bar: its artwork is used when the page shows none. */
  fallback?: HTMLElement | null
  hidden?: boolean
}

const channels = (c: AmbientPalette['primary']) => `${c[0]} ${c[1]} ${c[2]}`
const hex = (c: AmbientPalette['primary']) =>
  `#${c.map((v) => Math.round(v).toString(16).padStart(2, '0')).join('')}`

/**
 * The layer behind the sidebar and the content, and the artwork colour of the interface:
 * - Glass: a soft colour wash (something for the glass to refract);
 * - Artwork backdrop (Settings > Appearance, any style): the visible artwork, huge and blurred;
 * - Cinema: its palette (`--ambient-rgb`, `--ambient-rgb-2` on <html>) tints the interface;
 * - "Follow artwork colour": its dominant colour becomes the accent (themeStore).
 * The artwork is only sampled when one of these needs it.
 */
export function ShellAmbient({ scrollRef, fallback = null, hidden }: ShellAmbientProps) {
  const { style, appearance } = useInterfaceStyle()
  const { followArtwork } = useTheme()
  const cinema = style === 'cinema'
  const backdrop = appearance.backdrop !== 'off'
  const enabled = cinema || backdrop || followArtwork
  const { src, palette } = useAmbientArtwork(scrollRef, enabled, fallback)

  useEffect(() => {
    const root = document.documentElement.style
    if (cinema && palette) {
      root.setProperty('--ambient-rgb', channels(tameAmbientColor(palette.primary)))
      root.setProperty('--ambient-rgb-2', channels(tameAmbientColor(palette.secondary)))
    } else {
      root.removeProperty('--ambient-rgb')
      root.removeProperty('--ambient-rgb-2')
    }
  }, [cinema, palette])

  useEffect(() => {
    if (followArtwork && palette) themeStore.setArtworkColor(hex(palette.primary))
  }, [followArtwork, palette])

  const image =
    backdrop && src
      ? ({ '--ambient-image': `url(${JSON.stringify(src)})` } as CSSProperties)
      : undefined
  return (
    <div className="jr-shell__ambient" aria-hidden="true" hidden={hidden}>
      {/* Keyed by image so a new artwork fades in instead of swapping. */}
      {image && <div key={src} className="jr-shell__ambient-art" style={image} />}
    </div>
  )
}
