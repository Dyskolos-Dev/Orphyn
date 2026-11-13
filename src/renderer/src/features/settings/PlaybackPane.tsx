import { useTranslation } from 'react-i18next'
import { List, ListRow, Button, Toggle } from '@renderer/design'
import { LANGUAGES, VIDEO_BITRATES, usePlaybackPreferences } from './preferences'
import { ValueMenu } from './ValueMenu'

export function PlaybackPane() {
  const { t } = useTranslation('settings')
  const prefs = usePlaybackPreferences()

  const languages = (none: string) => [
    { value: null, label: none },
    ...LANGUAGES.map((l) => ({ value: l as string | null, label: t(`playback.languages.${l}`) })),
  ]

  return (
    <>
      <List variant="inset" header={t('playback.videoHeader')}>
        {/* The hint belongs to this row: as the list footer it ended up under the autoplay row. */}
        <ListRow
          title={t('playback.maxBitrate')}
          subtitle={t('playback.maxBitrateFooter')}
          trailing={
            <ValueMenu
              label={t('playback.maxBitrate')}
              value={prefs.maxVideoBitrate}
              onChange={(v) => prefs.update({ maxVideoBitrate: v })}
              options={[
                { value: null, label: t('playback.auto') },
                ...VIDEO_BITRATES.map((b) => ({
                  value: b as number | null,
                  label: t('playback.mbps', { value: b / 1_000_000 }),
                })),
              ]}
            />
          }
        />
        <ListRow
          title={t('playback.audioLanguage')}
          trailing={
            <ValueMenu
              label={t('playback.audioLanguage')}
              value={prefs.preferredAudioLanguage}
              onChange={(v) => prefs.update({ preferredAudioLanguage: v })}
              options={languages(t('playback.defaultTrack'))}
            />
          }
        />
        <ListRow
          title={t('playback.subtitleLanguage')}
          trailing={
            <ValueMenu
              label={t('playback.subtitleLanguage')}
              value={prefs.preferredSubtitleLanguage}
              onChange={(v) => prefs.update({ preferredSubtitleLanguage: v })}
              options={languages(t('playback.subtitlesOff'))}
            />
          }
        />
        <ListRow
          title={
            <Toggle
              label={t('playback.autoPlayNext')}
              description={t('playback.autoPlayNextDescription')}
              checked={prefs.autoPlayNext}
              onChange={(v) => prefs.update({ autoPlayNext: v })}
            />
          }
        />
      </List>
      <div>
        <Button variant="gray" size="small" onClick={prefs.reset}>
          {t('playback.reset')}
        </Button>
      </div>
    </>
  )
}
