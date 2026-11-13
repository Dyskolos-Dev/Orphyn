import { Cast, Pause, Play, X } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button, IconButton, Slider, Text, toast } from '@renderer/design'
import { formatClock } from '@renderer/features/video/format'
import type { VideoCastProps } from '@renderer/features/video'
import type { CastDevice } from '@shared/cast'
import { getCastApi } from './api'
import { CastPicker } from './CastPicker'
import { useCastSession } from './hooks'
import { currentMusicDeviceId, switchMusicOutput } from './musicCast'
import { useCastState } from './store'
import { startVideoCast, stopVideoCast } from './videoCast'

/** Music: the AirPlay-like button of the mini player and of Now Playing. */
export function MusicCastButton() {
  const { t } = useTranslation('cast')
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const target = useCastState((s) => (s.target?.scope === 'music' ? s.target : null))
  return (
    <>
      <IconButton
        ref={anchor}
        icon={<Cast />}
        label={t('button')}
        pressed={Boolean(target)}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      />
      <CastPicker
        scope="music"
        open={open}
        onClose={() => setOpen(false)}
        anchor={anchor}
        currentId={target?.deviceId ?? currentMusicDeviceId()}
        onSelect={switchMusicOutput}
      />
    </>
  )
}

/** Returns local playback to where the device was (or the end when it finished). */
function backToLocal(props: VideoCastProps, positionSec: number | null): void {
  if (positionSec == null) return
  props.controller.seek(positionSec)
  props.controller.resume()
}

/** Video: cast button of the player controls (video-capable devices only). */
export function VideoCastButton(props: VideoCastProps) {
  const { t } = useTranslation('cast')
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const target = useCastState((s) => (s.target?.scope === 'video' ? s.target : null))
  const { onHoldChange } = props
  const set = useCallback(
    (v: boolean) => {
      setOpen(v)
      onHoldChange?.(v)
    },
    [onHoldChange],
  )

  const onSelect = async (device: CastDevice | null) => {
    if (!device) {
      backToLocal(props, await stopVideoCast())
      return
    }
    const snap = props.controller.getSnapshot()
    await startVideoCast(
      device,
      {
        itemId: props.itemId,
        title: props.title,
        subtitle: props.subtitle,
        artworkUrl: props.artworkUrl,
        startSec: snap.positionSeconds,
        durationSec: snap.durationSeconds,
      },
      {
        onEnded: (end) => backToLocal(props, end ? end : null),
        onFailure: (message) => {
          toast({
            title: t('deviceFailed', { device: device.name }),
            description: message,
            tone: 'error',
          })
          void stopVideoCast().then((p) => backToLocal(props, p))
        },
      },
    )
    props.controller.pause()
    set(false)
  }

  return (
    <>
      <IconButton
        ref={anchor}
        variant="glass"
        icon={<Cast />}
        label={t('button')}
        pressed={Boolean(target)}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => set(!open)}
      />
      <CastPicker
        scope="video"
        open={open}
        onClose={() => set(false)}
        anchor={anchor}
        currentId={target?.deviceId ?? null}
        onSelect={onSelect}
      />
    </>
  )
}

/** Video: "Playing on <device>" screen with artwork and remote controls. */
export function VideoCastScreen(props: VideoCastProps) {
  const { t } = useTranslation('cast')
  const target = useCastState((s) => (s.target?.scope === 'video' ? s.target : null))
  const session = useCastSession(target?.deviceId ?? null)
  const [scrub, setScrub] = useState<number | null>(null)

  // Leaving the player while casting stops the cast.
  useEffect(() => () => void stopVideoCast(), [])

  if (!target) return null
  const cast = getCastApi()
  const playing = session?.status === 'playing' || session?.status === 'buffering'
  const duration = session?.durationSec || 0
  const position = scrub ?? session?.positionSec ?? 0
  const swallow = () => undefined

  return (
    <div
      className="jr-cast-screen"
      data-theme="dark"
      role="region"
      aria-label={t('playingOn', { device: target.name })}
    >
      {props.artworkUrl && (
        <img className="jr-cast-screen__art" src={props.artworkUrl} alt="" draggable={false} />
      )}
      <Text variant="title-2">{t('playingOn', { device: target.name })}</Text>
      <Text variant="headline">{props.title}</Text>
      {props.subtitle && (
        <Text variant="footnote" color="secondary">
          {props.subtitle}
        </Text>
      )}
      <div className="jr-cast-screen__scrubber">
        <Text variant="footnote" tabular>
          {formatClock(position)}
        </Text>
        <Slider
          aria-label={t('seek')}
          onMedia
          value={position}
          max={duration || 1}
          step={1}
          disabled={duration <= 0}
          formatValue={formatClock}
          onChangeStart={() => setScrub(session?.positionSec ?? 0)}
          onChange={setScrub}
          onChangeEnd={(v) => {
            setScrub(null)
            void cast?.seek(target.deviceId, v).catch(swallow)
          }}
        />
        <Text variant="footnote" tabular>
          -{formatClock(Math.max(0, duration - position))}
        </Text>
      </div>
      <div className="jr-cast-screen__controls">
        <IconButton
          variant="glass"
          size="large"
          icon={playing ? <Pause fill="currentColor" /> : <Play fill="currentColor" />}
          label={playing ? t('pause') : t('play')}
          onClick={() =>
            void (playing ? cast?.pause(target.deviceId) : cast?.play(target.deviceId))?.catch(
              swallow,
            )
          }
        />
        <Button
          variant="gray"
          onClick={() => void stopVideoCast().then((p) => backToLocal(props, p))}
        >
          <X aria-hidden="true" /> {t('stop')}
        </Button>
      </div>
    </div>
  )
}
