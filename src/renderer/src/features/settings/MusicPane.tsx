import { useTranslation } from 'react-i18next'
import { List, ListRow, Slider, Toggle } from '@renderer/design'
import {
  AUDIO_BITRATES,
  CROSSFADE_SECONDS_MAX,
  CROSSFADE_SECONDS_MIN,
  usePlaybackPreferences,
} from './preferences'
import { ValueMenu } from './ValueMenu'

export function MusicPane() {
  const { t } = useTranslation('settings')
  const prefs = usePlaybackPreferences()

  return (
    <List variant="inset" header={t('music.header')} footer={t('music.qualityFooter')}>
      <ListRow
        title={
          <Toggle
            label={t('music.crossfade')}
            description={t('music.crossfadeDescription')}
            checked={prefs.crossfade}
            onChange={(v) => prefs.update({ crossfade: v })}
          />
        }
      />
      {prefs.crossfade && (
        <>
          <ListRow
            title={t('music.crossfadeDuration')}
            trailing={
              <div className="jr-appearance__slider">
                <Slider
                  variant="regular"
                  aria-label={t('music.crossfadeDuration')}
                  min={CROSSFADE_SECONDS_MIN}
                  max={CROSSFADE_SECONDS_MAX}
                  step={1}
                  value={prefs.crossfadeSeconds}
                  onChange={(v) => prefs.update({ crossfadeSeconds: Math.round(v) })}
                  formatValue={(v) => t('music.seconds', { value: Math.round(v) })}
                />
                <span className="jr-music__value jr-tabular" aria-hidden="true">
                  {t('music.seconds', { value: prefs.crossfadeSeconds })}
                </span>
              </div>
            }
          />
          <ListRow
            title={
              <Toggle
                label={t('music.crossfadeSkipAlbum')}
                description={t('music.crossfadeSkipAlbumDescription')}
                checked={prefs.crossfadeSkipAlbum}
                onChange={(v) => prefs.update({ crossfadeSkipAlbum: v })}
              />
            }
          />
        </>
      )}
      <ListRow
        title={
          <Toggle
            label={t('music.groupArtists')}
            description={t('music.groupArtistsDescription')}
            checked={prefs.groupArtists}
            onChange={(v) => prefs.update({ groupArtists: v })}
          />
        }
      />
      <ListRow
        title={
          <Toggle
            label={t('music.artistImages')}
            description={t('music.artistImagesDescription')}
            checked={prefs.artistImages}
            onChange={(v) => prefs.update({ artistImages: v })}
          />
        }
      />
      <ListRow
        title={
          <Toggle
            label={t('music.onlineLyrics')}
            description={t('music.onlineLyricsDescription')}
            checked={prefs.onlineLyrics}
            onChange={(v) => prefs.update({ onlineLyrics: v })}
          />
        }
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
