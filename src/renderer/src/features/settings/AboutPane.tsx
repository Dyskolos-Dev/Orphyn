import type { TFunction } from 'i18next'
import { ExternalLink } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button, List, ListRow, Text, Toggle } from '@renderer/design'
import { useActiveAccount } from '@renderer/features/servers'
import { version } from '../../../../../package.json'
import type { UpdaterState } from '@shared/ipc/updater'
import { getPlatform } from '@renderer/design/platform'
import { useAccountsStatus } from './useAccountsStatus'
import { useUpdater } from './useUpdater'

const JELLYFIN_URL = 'https://jellyfin.org'
const JELLYFIN_DOCS_URL = 'https://jellyfin.org/docs/'
const AUTHOR_URL = 'https://dyskolos.fr'

const open = (url: string) => window.open(url, '_blank', 'noopener,noreferrer')

export function AboutPane() {
  const { t } = useTranslation('settings')
  const account = useActiveAccount()
  const status = useAccountsStatus()
  const updater = useUpdater()

  return (
    <>
      <div className="jr-settings__about">
        <Text variant="title-1">{t('about.app')}</Text>
        <Text variant="callout" color="secondary" selectable>
          {t('about.version', { version })}
        </Text>
      </div>

      {updater && (
        <List variant="inset" header={t('about.updates')}>
          <ListRow
            title={t('about.checkForUpdates')}
            subtitle={updateStatusText(t, updater)}
            trailing={
              updater.status === 'ready' ? (
                <Button size="small" onClick={() => void window.lumen.updater.install()}>
                  {t('about.restart')}
                </Button>
              ) : updater.status === 'available' ? (
                <Button size="small" onClick={() => void window.lumen.updater.openRelease()}>
                  {t('about.download')}
                </Button>
              ) : (
                <Button
                  size="small"
                  variant="gray"
                  disabled={
                    !updater.supported ||
                    updater.status === 'checking' ||
                    updater.status === 'downloading'
                  }
                  onClick={() => void window.lumen.updater.check()}
                >
                  {t('about.checkForUpdates')}
                </Button>
              )
            }
          />
          <ListRow
            title={t('about.autoUpdate')}
            subtitle={t('about.autoUpdateDescription')}
            trailing={
              <Toggle
                aria-label={t('about.autoUpdate')}
                checked={updater.autoUpdate}
                onChange={(on) => void window.lumen.updater.setAutoUpdate(on)}
              />
            }
          />
        </List>
      )}

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
              <Value>
                {status.encrypted
                  ? t('about.encrypted')
                  : t(getPlatform() === 'linux' ? 'about.notEncryptedLinux' : 'about.notEncrypted')}
              </Value>
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

      <Text variant="footnote" color="secondary" className="jr-settings__credit">
        {t('about.credit.before')}
        <a
          href={AUTHOR_URL}
          onClick={(e) => {
            e.preventDefault()
            open(AUTHOR_URL)
          }}
        >
          {t('about.credit.author')}
        </a>
      </Text>
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

function updateStatusText(t: TFunction<'settings'>, u: UpdaterState): string {
  if (!u.supported) return t('about.status.unsupported')
  return t(`about.status.${u.status}`, {
    version: u.version ?? '',
    percent: u.percent,
    error: u.error ?? '',
  })
}
