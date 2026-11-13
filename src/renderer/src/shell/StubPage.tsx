import { Construction } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useLocation } from 'react-router'
import { EmptyState } from '@renderer/design'
import { ShellPage } from './ShellPage'

/** Temporary page used by every stub route; feature agents replace it. */
export function StubPage({ name }: { name: string }) {
  const { t } = useTranslation()
  const { pathname, search } = useLocation()
  return (
    <ShellPage title={name}>
      <EmptyState
        icon={<Construction />}
        title={t('stub', { name })}
        message={
          <code>
            {pathname}
            {search}
          </code>
        }
      />
    </ShellPage>
  )
}
