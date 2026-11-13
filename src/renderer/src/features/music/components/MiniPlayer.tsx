import { ListMusic, MessageSquareQuote, Music } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Artwork, IconButton, Text } from '@renderer/design'
import { useCurrentTrack, useMusicPlayer } from '../player/api'
import { trackArt, trackArtist, trackTitle } from '../player/track'
import { CastButtonSlot } from './CastButtonSlot'
import { SeekBar, TransportButtons, VolumeControl } from './Transport'
import { useNowPlayingUi } from './ui-state'

/**
 * Apple Music style player bar, registered as the shell's persistent `nowPlaying` extension so it
 * survives navigation. Renders nothing (the bar stays hidden) while the queue is empty.
 */
export function MiniPlayer() {
  const { t } = useTranslation('music')
  const track = useCurrentTrack()
  const hasLyricsView = useNowPlayingUi((s) => s.open && s.view === 'lyrics')
  const queueView = useNowPlayingUi((s) => s.open && s.view === 'queue')
  const status = useMusicPlayer((s) => s.status)
  if (!track) return null

  return (
    <div className="jrm-mini" role="region" aria-label={t('player.nowPlaying')}>
      <div className="jrm-mini__left">
        <TransportButtons />
      </div>

      <div className="jrm-mini__center jr-material-regular">
        <button
          type="button"
          className="jrm-mini__info"
          aria-label={t('player.openNowPlaying')}
          onClick={() => useNowPlayingUi.getState().show('player')}
        >
          <Artwork
            src={trackArt(track, 96)}
            ratio="1/1"
            shape="rounded"
            fallback={<Music />}
            className="jrm-mini__art"
          />
          <span className="jrm-mini__text">
            <Text variant="subheadline" truncate className="jrm-mini__title">
              {trackTitle(track)}
            </Text>
            <Text variant="footnote" color="secondary" truncate>
              {[trackArtist(track), track.Album].filter(Boolean).join(' — ')}
            </Text>
          </span>
        </button>
        <div className="jrm-mini__seek" data-loading={status === 'loading' || undefined}>
          <SeekBar />
        </div>
      </div>

      <div className="jrm-mini__right">
        <VolumeControl />
        <CastButtonSlot />
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
