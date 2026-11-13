import { useTranslation } from 'react-i18next'
import { useLocation } from 'react-router'

/** Temporary page used by every stub route; feature agents replace it. */
export function StubPage({ name }: { name: string }) {
  const { t } = useTranslation()
  const { pathname, search } = useLocation()
  return (
    <section>
      <h1>{name}</h1>
      <p>{t('stub', { name })}</p>
      <code>
        {pathname}
        {search}
      </code>
    </section>
  )
}
