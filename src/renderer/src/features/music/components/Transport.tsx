import {
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume1,
  Volume2,
  VolumeX,
} from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { formatDuration, IconButton, Slider } from '@renderer/design'
import { musicPlayer, useCurrentTrack, useMusicPlayer } from '../player/api'
import { trackDuration } from '../player/track'

interface TransportProps {
  /** Size of the play / skip buttons. */
  size?: 'medium' | 'large'
  /** Shuffle and repeat toggles around the main controls. */
  extras?: boolean
}

/** Shuffle, previous, play / pause, next, repeat. */
export function TransportButtons({ size = 'medium', extras = true }: TransportProps) {
  const { t } = useTranslation('music')
  const wantPlaying = useMusicPlayer((s) => s.wantPlaying)
  const shuffle = useMusicPlayer((s) => s.shuffle)
  const repeat = useMusicPlayer((s) => s.repeat)
  const hasQueue = useMusicPlayer((s) => s.queue.length > 0)
  const status = useMusicPlayer((s) => s.status)
  const playing = wantPlaying && status !== 'paused' && status !== 'error'
  const fill = { fill: 'currentColor' } as const
  return (
    <div className="jrm-transport" role="group" aria-label={t('player.controls')}>
      {extras && (
        <IconButton
          icon={<Shuffle />}
          label={t('player.shuffle')}
          pressed={shuffle}
          onClick={() => useMusicPlayer.getState().toggleShuffle()}
          className="jrm-toggle"
        />
      )}
      <IconButton
        icon={<SkipBack {...fill} />}
        label={t('player.previous')}
        size={size}
        disabled={!hasQueue}
        onClick={() => musicPlayer.previous()}
      />
      <IconButton
        icon={playing ? <Pause {...fill} /> : <Play {...fill} />}
        label={playing ? t('player.pause') : t('player.play')}
        size={size}
        disabled={!hasQueue}
        onClick={() => musicPlayer.toggle()}
      />
      <IconButton
        icon={<SkipForward {...fill} />}
        label={t('player.next')}
        size={size}
        disabled={!hasQueue}
        onClick={() => musicPlayer.next()}
      />
      {extras && (
        <IconButton
          icon={repeat === 'one' ? <Repeat1 /> : <Repeat />}
          label={t(`player.repeat.${repeat}`)}
          pressed={repeat !== 'off'}
          onClick={() => useMusicPlayer.getState().cycleRepeat()}
          className="jrm-toggle"
        />
      )}
    </div>
  )
}

/**
 * Duration of the current track: the loaded one, or the metadata's RunTimeTicks while a restored
 * (paused) track has not loaded yet, so the bar never shows "--:--" for a known track.
 */
export function useTrackDuration(): number {
  const duration = useMusicPlayer((s) => s.duration)
  const track = useCurrentTrack()
  return duration > 0 ? duration : track ? trackDuration(track) : 0
}

/** Scrubber with elapsed / remaining time. Dragging does not fight the playback position. */
export function SeekBar({ onMedia = false }: { onMedia?: boolean }) {
  const { t } = useTranslation('music')
  const position = useMusicPlayer((s) => s.position)
  const duration = useTrackDuration()
  const [scrub, setScrub] = useState<number | null>(null)
  const shown = scrub ?? position
  return (
    <div className="jrm-seek">
      <span className="jrm-seek__time jr-tabular">{formatDuration(shown)}</span>
      <Slider
        aria-label={t('player.seek')}
        value={Math.min(shown, duration || 0)}
        max={duration || 1}
        step={1}
        disabled={!duration}
        onMedia={onMedia}
        formatValue={formatDuration}
        onChangeStart={() => setScrub(position)}
        onChange={setScrub}
        onChangeEnd={(v) => {
          musicPlayer.seek(v)
          setScrub(null)
        }}
      />
      <span className="jrm-seek__time jr-tabular">
        {duration ? `-${formatDuration(Math.max(0, duration - shown))}` : '--:--'}
      </span>
    </div>
  )
}

export function VolumeControl({ onMedia = false }: { onMedia?: boolean }) {
  const { t } = useTranslation('music')
  const volume = useMusicPlayer((s) => s.volume)
  const muted = useMusicPlayer((s) => s.muted)
  const Icon = muted || volume === 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2
  return (
    <div className="jrm-volume">
      <IconButton
        icon={<Icon />}
        label={muted ? t('player.unmute') : t('player.mute')}
        size="small"
        onClick={() => useMusicPlayer.getState().toggleMute()}
      />
      <Slider
        aria-label={t('player.volume')}
        value={muted ? 0 : volume}
        max={1}
        step={0.05}
        variant="regular"
        onMedia={onMedia}
        onChange={(v) => useMusicPlayer.getState().setVolume(v)}
        formatValue={(v) => `${Math.round(v * 100)}%`}
      />
    </div>
  )
}
