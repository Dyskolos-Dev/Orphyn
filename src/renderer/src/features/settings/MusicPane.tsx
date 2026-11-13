import { useTranslation } from 'react-i18next'
import { List, ListRow } from '@renderer/design'
import { AUDIO_BITRATES, usePlaybackPreferences } from './preferences'
import { ValueMenu } from './ValueMenu'

export function MusicPane() {
  const { t } = useTranslation('settings')
  const prefs = usePlaybackPreferences()

  return (
    <List variant="inset" header={t('music.header')} footer={t('music.qualityFooter')}>
      <ListRow
        title={t('music.crossfade')}
        subtitle={t('music.crossfadeDescription')}
        disabled
        trailing={<span className="jr-settings__value">{t('music.crossfadeOff')}</span>}
      />
      <ListRow
        title={t('music.quality')}
        trailing={
          <ValueMenu<string | number>
            label={t('music.quality')}
            value={prefs.audioQuality}
            onChange={(v) => prefs.update({ audioQuality: v as 'original' | number })}
            options={[
              { value: 'original', label: t('music.original') },
              ...AUDIO_BITRATES.map((b) => ({
                value: b as string | number,
                label: t('music.kbps', { value: b / 1000 }),
              })),
            ]}
          />
        }
      />
    </List>
  )
}
