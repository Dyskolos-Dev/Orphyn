import { useEffect, type CSSProperties, type RefObject } from 'react'
import { useAmbientArtwork, useInterfaceStyle, type AmbientPalette } from '@renderer/design'

export interface ShellAmbientProps {
  /** The page scroll container, where the visible artwork is looked for. */
  scrollRef: RefObject<HTMLElement | null>
  hidden?: boolean
}

const channels = (c: AmbientPalette['primary']) => `${c[0]} ${c[1]} ${c[2]}`

/**
 * The layer behind the sidebar and the content. Invisible in most styles; a soft colour wash in
 * Glass (something for the glass to refract); in Cinema, the visible artwork, huge and blurred,
 * whose palette (`--ambient-rgb`, `--ambient-rgb-2` on <html>) tints the whole interface.
 */
export function ShellAmbient({ scrollRef, hidden }: ShellAmbientProps) {
  const { style } = useInterfaceStyle()
  const cinema = style === 'cinema'
  const { src, palette } = useAmbientArtwork(scrollRef, cinema)

  useEffect(() => {
    const root = document.documentElement.style
    if (cinema && palette) {
      root.setProperty('--ambient-rgb', channels(palette.primary))
      root.setProperty('--ambient-rgb-2', channels(palette.secondary))
    } else {
      root.removeProperty('--ambient-rgb')
      root.removeProperty('--ambient-rgb-2')
    }
  }, [cinema, palette])

  const image =
    cinema && src
      ? ({ '--ambient-image': `url(${JSON.stringify(src)})` } as CSSProperties)
      : undefined
  return (
    <div className="jr-shell__ambient" aria-hidden="true" hidden={hidden}>
      {/* Keyed by image so a new artwork fades in instead of swapping. */}
      {image && <div key={src} className="jr-shell__ambient-art" style={image} />}
    </div>
  )
}
