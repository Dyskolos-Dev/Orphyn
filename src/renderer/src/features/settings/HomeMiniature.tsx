import { useTranslation } from 'react-i18next'
import { countOf, type HomeLayout, type HomeScope } from '@renderer/features/home'

/** A small, live drawing of the home: a banner for the hero, a strip of cards for each other row. */
export function HomeMiniature({ scope, layout }: { scope: HomeScope; layout: HomeLayout }) {
  const { t } = useTranslation('settings')
  const rows = layout.rows.filter((r) => !r.hidden)
  return (
    <figure className="jr-mini" aria-label={t('home.preview')}>
      <div className="jr-mini__screen" aria-hidden="true">
        {rows.map((row) =>
          row.type === 'hero' ? (
            <div key={row.id} className="jr-mini__hero" />
          ) : (
            <div key={row.id} className="jr-mini__row">
              <span className="jr-mini__label" />
              <span className="jr-mini__cards">
                {Array.from({ length: Math.min(countOf(row, scope), 6) }, (_, i) => (
                  <span key={i} className="jr-mini__card" />
                ))}
              </span>
            </div>
          ),
        )}
      </div>
      <figcaption className="jr-mini__caption">{t('home.previewCaption')}</figcaption>
    </figure>
  )
}
