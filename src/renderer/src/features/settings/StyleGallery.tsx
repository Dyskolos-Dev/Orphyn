import clsx from 'clsx'
import { Check } from 'lucide-react'
import { useLayoutEffect, useRef, useState, type CSSProperties } from 'react'
import { useTranslation } from 'react-i18next'
import { INTERFACE_STYLES, useInterfaceStyle, type InterfaceStyle } from '@renderer/design'
import './StyleGallery.css'

/** Size of the miniature window before it is scaled to the card. */
const MINI_WIDTH = 720

/**
 * A live miniature of the app in one style: a small window (sidebar, toolbar, title, a shelf, a
 * few track rows) laid out with the real tokens, on a subtree carrying `data-style`, so it shows
 * exactly what the style does with the current theme and accent.
 */
export function StylePreview({ style }: { style: InterfaceStyle }) {
  const frame = useRef<HTMLDivElement>(null)
  const [scale, setScale] = useState(0.4)

  useLayoutEffect(() => {
    const el = frame.current
    if (!el) return
    const measure = () => {
      const w = el.clientWidth
      if (w > 0) setScale(w / MINI_WIDTH)
    }
    measure()
    if (typeof ResizeObserver !== 'function') return
    const ro = new ResizeObserver(measure)
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  return (
    <div ref={frame} className="jr-style-preview" aria-hidden="true">
      <div
        className="jr-style-preview__window"
        data-style={style}
        style={{ '--jr-preview-scale': scale } as CSSProperties}
      >
        <div className="jr-style-preview__backdrop">
          <div className="jr-style-preview__art" />
        </div>
        <div className="jr-style-preview__sidebar">
          <span className="jr-style-preview__search" />
          {[0, 1, 2, 3, 4].map((i) => (
            <span key={i} className={clsx('jr-style-preview__nav', i === 1 && 'is-active')}>
              <i />
              <b style={{ width: `${46 + ((i * 17) % 34)}%` }} />
            </span>
          ))}
        </div>
        <div className="jr-style-preview__main">
          <div className="jr-style-preview__toolbar" />
          <div className="jr-style-preview__content">
            <div className="jr-style-preview__title">Aa</div>
            <div className="jr-style-preview__shelf">
              {[0, 1, 2, 3, 4, 5, 6, 7].map((i) => (
                <span key={i} className="jr-style-preview__card" data-tone={i % 4} />
              ))}
            </div>
            <div className="jr-style-preview__rows">
              {[0, 1, 2, 3].map((i) => (
                <span key={i} className={clsx('jr-style-preview__row', i === 1 && 'is-current')}>
                  <i />
                  <b />
                </span>
              ))}
            </div>
          </div>
          <div className="jr-style-preview__player">
            <span className="jr-style-preview__play" />
            <span className="jr-style-preview__lcd" />
          </div>
        </div>
      </div>
    </div>
  )
}

/** Settings > Appearance: the interface style picker, one live preview card per style. */
export function StyleGallery() {
  const { t } = useTranslation('settings')
  const { style: current, setStyle } = useInterfaceStyle()

  return (
    <div className="jr-style-gallery" role="radiogroup" aria-label={t('general.style.label')}>
      {INTERFACE_STYLES.map((style) => {
        const selected = style === current
        return (
          <button
            key={style}
            type="button"
            role="radio"
            aria-checked={selected}
            className={clsx('jr-style-card', selected && 'jr-style-card--selected')}
            onClick={() => setStyle(style)}
          >
            <StylePreview style={style} />
            <span className="jr-style-card__text">
              <span className="jr-style-card__name">
                {t(`general.style.styles.${style}.name`)}
                {selected && (
                  <span className="jr-style-card__check" title={t('general.style.selected')}>
                    <Check aria-hidden="true" strokeWidth={3} />
                  </span>
                )}
              </span>
              <span className="jr-style-card__description">
                {t(`general.style.styles.${style}.description`)}
              </span>
            </span>
          </button>
        )
      })}
    </div>
  )
}
