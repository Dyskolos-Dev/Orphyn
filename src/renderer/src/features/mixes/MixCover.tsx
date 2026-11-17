import { useMemo, type CSSProperties } from 'react'
import { trackArt } from '../music/player/track'
import type { Mix } from './types'
import './mixes.css'

/** Up to four different album covers of the mix. */
function coversOf(mix: Mix): string[] {
  const seen = new Set<string>()
  const out: string[] = []
  for (const t of mix.tracks) {
    const key = t.AlbumId ?? ''
    const url = trackArt(t, 240)
    if (!key || !url || seen.has(key)) continue
    seen.add(key)
    out.push(url)
    if (out.length === 4) break
  }
  return out
}

/**
 * Generated cover: a hue gradient with a mosaic of album covers behind the name. Pure CSS, sized
 * by its container (container query units), so the lettering is crisp at every size.
 */
export function MixCover({
  mix,
  name,
  covers: given,
}: {
  mix: Mix
  name: string
  covers?: string[]
}) {
  const covers = useMemo(() => given ?? coversOf(mix), [mix, given])
  const style = { '--mix-hue': mix.hue } as CSSProperties
  return (
    <div className="jrx-cover" style={style} role="img" aria-label={name}>
      {covers.length > 0 && (
        <div className="jrx-cover__mosaic" data-count={covers.length} aria-hidden="true">
          {covers.map((src) => (
            <img key={src} src={src} alt="" draggable={false} loading="lazy" />
          ))}
        </div>
      )}
      <div className="jrx-cover__tint" aria-hidden="true" />
      <div className="jrx-cover__text" aria-hidden="true">
        <span className="jrx-cover__name">{name}</span>
      </div>
    </div>
  )
}
