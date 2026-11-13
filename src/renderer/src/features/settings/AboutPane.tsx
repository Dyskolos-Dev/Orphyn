import { ExternalLink } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { List, ListRow, Text } from '@renderer/design'
import { useActiveAccount } from '@renderer/features/servers'
import { version } from '../../../../../package.json'
import { useAccountsStatus } from './useAccountsStatus'

const JELLYFIN_URL = 'https://jellyfin.org'
const JELLYFIN_DOCS_URL = 'https://jellyfin.org/docs/'

const open = (url: string) => window.open(url, '_blank', 'noopener,noreferrer')

export function AboutPane() {
  const { t } = useTranslation('settings')
  const account = useActiveAccount()
  const status = useAccountsStatus()

  return (
    <>
      <div className="jr-settings__about">
        <Text variant="title-1">{t('about.app')}</Text>
        <Text variant="callout" color="secondary" selectable>
          {t('about.version', { version })}
        </Text>
      </div>

      <List variant="inset" header={t('about.server')}>
        {account ? (
          <>
            <ListRow title={t('about.serverName')} trailing={<Value>{account.serverName}</Value>} />
            <ListRow title={t('about.serverVersion')} trailing={<Value>{account.version}</Value>} />
            <ListRow title={t('about.serverUrl')} trailing={<Value>{account.url}</Value>} />
            <ListRow title={t('about.user')} trailing={<Value>{account.userName}</Value>} />
          </>
        ) : (
          <ListRow title={t('about.noServer')} />
        )}
        {status && (
          <ListRow
            title={t('about.tokenStorage')}
            trailing={
              <Value>{status.encrypted ? t('about.encrypted') : t('about.notEncrypted')}</Value>
            }
          />
        )}
      </List>

      <List variant="inset" header={t('about.links')}>
        <ListRow
          title={t('about.jellyfin')}
          subtitle={t('about.jellyfinDescription')}
          trailing={<ExternalLink className="jr-settings__link-icon" aria-hidden="true" />}
          onClick={() => open(JELLYFIN_URL)}
        />
        <ListRow
          title={t('about.docs')}
          trailing={<ExternalLink className="jr-settings__link-icon" aria-hidden="true" />}
          onClick={() => open(JELLYFIN_DOCS_URL)}
        />
      </List>
    </>
  )
}

function Value({ children }: { children: string }) {
  return (
    <Text variant="callout" color="secondary" selectable>
      {children}
    </Text>
  )
}
