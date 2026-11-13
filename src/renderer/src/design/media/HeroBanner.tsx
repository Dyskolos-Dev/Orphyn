import clsx from 'clsx'
import { useState, type ReactNode } from 'react'
import { Artwork } from './Artwork'
import './media.css'

export interface HeroBannerProps {
  /** Wide backdrop image (16:9 or wider). */
  backdrop?: string | null
  /** Transparent title logo; the text title is used when it is missing or fails to load. */
  logo?: string | null
  title: string
  /** Small uppercase line above the title, e.g. "Up Next" / "Continue Watching". */
  eyebrow?: ReactNode
  /** One-line metadata, e.g. "2024 · Drama · 2 h 10 min · 4K". */
  meta?: ReactNode
  /** Synopsis, clamped to 3 lines. */
  description?: ReactNode
  /** Buttons, usually `<Button variant="contrast" size="large" icon={<Play/>}>`. */
  actions?: ReactNode
  /** 0..1 progress, drawn under the actions with a remaining-time caption. */
  progress?: number
  progressCaption?: ReactNode
  /** `tall` ~ 70 % of the viewport (home), `medium` ~ 52 % (detail pages). */
  size?: 'tall' | 'medium'
  className?: string
}

/**
 * "Up Next" hero: full-bleed backdrop, bottom + side gradients, logo or large title,
 * metadata, actions. Always rendered in the dark theme (white text over imagery).
 */
export function HeroBanner({
  backdrop,
  logo,
  title,
  eyebrow,
  meta,
  description,
  actions,
  progress,
  progressCaption,
  size = 'tall',
  className,
}: HeroBannerProps) {
  const [logoFailed, setLogoFailed] = useState<string | null>(null)
  const showLogo = Boolean(logo) && logoFailed !== logo
  return (
    <section
      className={clsx('jr-hero', `jr-hero--${size}`, className)}
      data-theme="dark"
      aria-label={title}
    >
      <div className="jr-hero__backdrop">
        <Artwork src={backdrop} fill shape="none" priority objectPosition="center top" />
      </div>
      <div className="jr-hero__scrim" aria-hidden="true" />
      <div className="jr-hero__content">
        {eyebrow && <p className="jr-hero__eyebrow">{eyebrow}</p>}
        {showLogo ? (
          <h1 className="jr-hero__logo-heading">
            <img
              className="jr-hero__logo"
              src={logo ?? undefined}
              alt={title}
              draggable={false}
              onError={() => setLogoFailed(logo ?? null)}
            />
          </h1>
        ) : (
          <h1 className="jr-hero__title">{title}</h1>
        )}
        {meta && <p className="jr-hero__meta">{meta}</p>}
        {description && <p className="jr-hero__description">{description}</p>}
        {(actions || progress != null) && (
          <div className="jr-hero__footer">
            {actions && <div className="jr-hero__actions">{actions}</div>}
            {progress != null && (
              <div className="jr-hero__progress">
                <span className="jr-hero__progress-track" aria-hidden="true">
                  <span
                    className="jr-hero__progress-fill"
                    style={{ width: `${Math.round(Math.min(1, Math.max(0, progress)) * 100)}%` }}
                  />
                </span>
                {progressCaption && (
                  <span className="jr-hero__progress-caption">{progressCaption}</span>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </section>
  )
}
