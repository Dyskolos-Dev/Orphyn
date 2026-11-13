import { ListMusic, MessageSquareQuote, Music } from 'lucide-react'
import i18n from 'i18next'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Artwork, BrandLogo, formatDuration, IconButton, Slider, Text } from '@renderer/design'
import { musicPlayer, useCurrentTrack, useMusicPlayer } from '../player/api'
import { useMusicOutputName } from '../player/outputLabel'
import { trackArt, trackArtist, trackTitle } from '../player/track'
import { CastButtonSlot } from './CastButtonSlot'
import { TransportButtons, useTrackDuration, VolumeControl } from './Transport'
import { useNowPlayingUi } from './ui-state'
import { PipButton } from '@renderer/features/pip/PipButton'
import { useMusicPipHost } from '@renderer/features/pip/useMusicPipHost'
import { CurrentTrackHeart } from '../player/CurrentTrackHeart'

/** Thin progress line at the bottom of the capsule; elapsed / remaining show on hover. */
function CapsuleProgress() {
  const { t } = useTranslation('music')
  const position = useMusicPlayer((s) => s.position)
  const duration = useTrackDuration()
  const loading = useMusicPlayer((s) => s.status === 'loading')
  const [scrub, setScrub] = useState<number | null>(null)
  const shown = Math.min(scrub ?? position, duration || 0)
  return (
    <>
      <div className="jrm-mini__times jr-tabular" aria-hidden="true">
        <span>{formatDuration(shown)}</span>
        <span>{duration ? `-${formatDuration(Math.max(0, duration - shown))}` : '--:--'}</span>
      </div>
      <div className="jrm-mini__progress" data-loading={loading || undefined}>
        <Slider
          aria-label={t('player.seek')}
          value={shown}
          max={duration || 1}
          step={1}
          disabled={!duration}
          formatValue={formatDuration}
          onChangeStart={() => setScrub(position)}
          onChange={setScrub}
          onChangeEnd={(v) => {
            musicPlayer.seek(v)
            setScrub(null)
          }}
        />
      </div>
    </>
  )
}

/**
 * Music player bar, registered as the shell's persistent `nowPlaying` extension so it
 * survives navigation. Transport on the left, an "LCD" capsule in the centre, volume / cast /
 * lyrics / queue on the right. With an empty queue the capsule shows "Not Playing".
 */
export function MiniPlayer() {
  const { t } = useTranslation('music')
  const track = useCurrentTrack()
  useMusicPipHost()
  const hasLyricsView = useNowPlayingUi((s) => s.open && s.view === 'lyrics')
  const queueView = useNowPlayingUi((s) => s.open && s.view === 'queue')
  const outputName = useMusicOutputName((s) => s.name)
  const open = () => useNowPlayingUi.getState().show('player')
  const subtitle = track ? [trackArtist(track), track.Album].filter(Boolean).join(' — ') : ''

  return (
    <div className="jrm-mini" role="region" aria-label={t('player.nowPlaying')}>
      <div className="jrm-mini__left">
        <TransportButtons />
      </div>

      <div className="jrm-mini__capsule">
        {track ? (
          <>
            <button
              type="button"
              className="jrm-mini__info"
              aria-label={t('player.openNowPlaying')}
              onClick={open}
            >
              <Artwork
                src={trackArt(track, 112)}
                ratio="1/1"
                shape="rounded"
                fallback={<Music />}
                className="jrm-mini__art"
              />
              <span className="jrm-mini__text">
                <Text variant="subheadline" truncate className="jrm-mini__title">
                  {trackTitle(track)}
                </Text>
                <Text variant="footnote" color="secondary" truncate className="jrm-mini__subtitle">
                  {outputName ? i18n.t('cast:playingOn', { device: outputName }) : subtitle}
                </Text>
              </span>
            </button>
            <CurrentTrackHeart
              size="small"
              style={{ flex: 'none', marginBottom: 8, marginRight: 4 }}
            />
            <CapsuleProgress />
          </>
        ) : (
          <div className="jrm-mini__info">
            <BrandLogo size={32} className="jrm-mini__logo" />
            <span className="jrm-mini__text">
              <Text variant="subheadline" color="secondary" truncate className="jrm-mini__title">
                {t('player.notPlaying')}
              </Text>
            </span>
          </div>
        )}
      </div>

      <div className="jrm-mini__right">
        <VolumeControl />
        <CastButtonSlot />
        <PipButton kind="music" />
        <IconButton
          icon={<MessageSquareQuote />}
          label={t('player.lyrics')}
          pressed={hasLyricsView}
          onClick={() => useNowPlayingUi.getState().toggleView('lyrics')}
        />
        <IconButton
          icon={<ListMusic />}
          label={t('player.queue')}
          pressed={queueView}
          onClick={() => useNowPlayingUi.getState().toggleView('queue')}
        />
      </div>
    </div>
  )
}
