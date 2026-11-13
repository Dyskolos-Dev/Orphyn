import clsx from 'clsx'
import { ImageOff } from 'lucide-react'
import { useState, type CSSProperties, type ReactNode } from 'react'
import './media.css'

/** Common artwork ratios: poster 2:3, landscape 16:9, square 1:1. Any CSS ratio string works. */
export type ArtworkRatio = '2/3' | '16/9' | '1/1' | (string & {})

export interface ArtworkProps {
  /** Image URL. `null` / `undefined` shows the fallback directly. */
  src?: string | null
  srcSet?: string
  sizes?: string
  /** Alt text. Decorative artwork next to a visible title should use "" (the default). */
  alt?: string
  ratio?: ArtworkRatio
  /** `rounded` (artwork radius), `poster` (larger radius), `circle` (artists), `none`. */
  shape?: 'rounded' | 'poster' | 'circle' | 'none'
  /** Shown when there is no image or it fails: an icon or initials. */
  fallback?: ReactNode
  /** Load immediately (above the fold, e.g. hero) instead of lazily. */
  priority?: boolean
  /** Adds the subtle artwork shadow. */
  shadow?: boolean
  /** CSS object-position, e.g. "top" for backdrops. */
  objectPosition?: string
  /** Fill the parent instead of sizing by ratio (parent must be positioned). */
  fill?: boolean
  className?: string
  style?: CSSProperties
}

type Status = 'loading' | 'loaded' | 'error'

/**
 * Lazy image with a neutral placeholder, a fade-in once decoded, and a graceful fallback (icon or
 * initials) when the URL is missing or fails.
 */
export function Artwork({
  src,
  srcSet,
  sizes,
  alt = '',
  ratio = '1/1',
  shape = 'rounded',
  fallback,
  priority = false,
  shadow = false,
  objectPosition,
  fill = false,
  className,
  style,
}: ArtworkProps) {
  // Status is keyed by URL, so a new src restarts at "loading" without an effect.
  const [status, setStatus] = useState<{ src: string | null | undefined; value: Status }>({
    src,
    value: 'loading',
  })
  const current: Status = !src ? 'error' : status.src === src ? status.value : 'loading'
  const set = (value: Status) => setStatus({ src, value })

  return (
    <div
      className={clsx(
        'jr-artwork',
        `jr-artwork--${shape}`,
        `jr-artwork--${current}`,
        shadow && 'jr-artwork--shadow',
        fill && 'jr-artwork--fill',
        className,
      )}
      style={{ aspectRatio: fill ? undefined : ratio.replace('/', ' / '), ...style }}
      data-status={current}
    >
      {current === 'error' ? (
        <div
          className="jr-artwork__fallback"
          role={alt ? 'img' : undefined}
          aria-label={alt || undefined}
          aria-hidden={alt ? undefined : true}
        >
          {fallback ?? <ImageOff strokeWidth={1.5} />}
        </div>
      ) : (
        <img
          className="jr-artwork__img"
          src={src ?? undefined}
          srcSet={srcSet}
          sizes={sizes}
          alt={alt}
          loading={priority ? 'eager' : 'lazy'}
          decoding="async"
          draggable={false}
          style={objectPosition ? { objectPosition } : undefined}
          onLoad={() => set('loaded')}
          onError={() => set('error')}
        />
      )}
    </div>
  )
}

/** Initials for a name ("Daft Punk" -> "DP"), handy as an Artwork fallback. */
export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean)
  const first = parts[0]?.[0] ?? ''
  const last = parts.length > 1 ? (parts[parts.length - 1]?.[0] ?? '') : ''
  return (first + last).toUpperCase()
}
