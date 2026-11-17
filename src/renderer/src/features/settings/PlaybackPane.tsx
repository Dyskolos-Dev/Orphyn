import { useTranslation } from 'react-i18next'
import { List, ListRow, Button, Toggle } from '@renderer/design'
import { LANGUAGES, VIDEO_BITRATES, usePlaybackPreferences } from './preferences'
import { ValueMenu } from './ValueMenu'
import {
  revealDeviceLabels,
  setFallbackLabel,
  useAudioDevices,
} from '@renderer/features/music/player/audioDevice'

export function PlaybackPane() {
  const { t } = useTranslation('settings')
  const prefs = usePlaybackPreferences()

  const languages = (none: string) => [
    { value: null, label: none },
    ...LANGUAGES.map((l) => ({ value: l as string | null, label: t(`playback.languages.${l}`) })),
  ]

  const audio = useAudioDevices()
  setFallbackLabel((n) => t('playback.outputGeneric', { n }))
  const outputOptions = [
    { value: null as string | null, label: t('playback.outputDefault') },
    ...(audio.devices ?? []).map((d) => ({ value: d.deviceId as string | null, label: d.label })),
  ]
  // A saved device that is unplugged stays selectable-looking, so the choice is not lost.
  if (prefs.audioOutputId && !outputOptions.some((o) => o.value === prefs.audioOutputId)) {
    outputOptions.push({ value: prefs.audioOutputId, label: t('playback.outputMissing') })
  }

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
      {audio.supported && (
        <List variant="inset" header={t('playback.outputHeader')}>
          <ListRow
            title={t('playback.output')}
            subtitle={t('playback.outputFooter')}
            trailing={
              <ValueMenu
                label={t('playback.output')}
                value={prefs.audioOutputId}
                onChange={(v) => prefs.update({ audioOutputId: v })}
                options={outputOptions}
              />
            }
          />
          {audio.labelsHidden && (
            <ListRow
              title={t('playback.outputNames')}
              trailing={
                <Button variant="gray" size="small" onClick={() => void revealDeviceLabels()}>
                  {t('playback.outputNamesAction')}
                </Button>
              }
            />
          )}
        </List>
      )}
      <div>
        <Button variant="gray" size="small" onClick={prefs.reset}>
          {t('playback.reset')}
        </Button>
      </div>
    </>
  )
}
