import { useTranslation } from 'react-i18next'
import { List, ListRow, Toggle } from '@renderer/design'
import { useDiscord } from './useDiscord'

export function DiscordPane() {
  const { t } = useTranslation('settings')
  const state = useDiscord()
  if (!state)
    return (
      <List variant="inset" header={t('discord.header')}>
        <ListRow title={t('discord.unavailable')} />
      </List>
    )
  const configured = state.status !== 'not-configured'
  const set = (patch: { enabled?: boolean; showArtwork?: boolean }) =>
    void window.lumen.discord.setSettings(patch)

  return (
    <>
      <List variant="inset" header={t('discord.header')} footer={t('discord.footer')}>
        <ListRow
          title={t('discord.enabled')}
          subtitle={configured ? t('discord.enabledDescription') : t('discord.notConfiguredNote')}
          trailing={
            <Toggle
              aria-label={t('discord.enabled')}
              checked={state.enabled}
              onChange={(on) => set({ enabled: on })}
            />
          }
        />
        <ListRow
          title={t('discord.artwork')}
          subtitle={t('discord.artworkDescription')}
          disabled={!state.enabled}
          trailing={
            <Toggle
              aria-label={t('discord.artwork')}
              checked={state.showArtwork}
              disabled={!state.enabled}
              onChange={(on) => set({ showArtwork: on })}
            />
          }
        />
        <ListRow
          title={t('discord.status')}
          trailing={
            <span className="jr-settings__value" role="status">
              {t(`discord.statuses.${state.status}`)}
            </span>
          }
        />
      </List>
    </>
  )
}
