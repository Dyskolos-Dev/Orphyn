import { Maximize2, Pause, Play, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'
import { VideoPlaybackController } from '@renderer/features/video/controller'
import { formatClock } from '@renderer/features/video/format'
import { useSession } from '@renderer/jellyfin'
import { onPip, sendPip } from './events'
import { backToMain, closePip } from './MusicPip'
import { PIP_EVENTS, type VideoPipProgress, type VideoPipStart } from './protocol'
import { handOverStart, toTicks } from './videoHandover'

/**
 * Custom video PiP window, for webviews without a native Picture in Picture (WebKitGTK). It plays
 * the same item with its own controller from the position the main player handed over, and
 * reports its progress back so the main window resumes where the PiP stopped.
 */
export function VideoPip() {
  const { t } = useTranslation('pip')
  const controller = useMemo(() => new VideoPlaybackController(), [])
  const snapshot = useSyncExternalStore(controller.subscribe, controller.getSnapshot)
  const videoRef = useRef<HTMLVideoElement>(null)
  const [title, setTitle] = useState('')
  const [scrub, setScrub] = useState<number | null>(null)

  useEffect(() => {
    controller.attach(videoRef.current)
    let off: (() => void) | undefined
    let cancelled = false
    const boot = async () => {
      if (useSession.getState().status === 'idle') await useSession.getState().init()
      if (cancelled) return
      off = onPip<VideoPipStart>(PIP_EVENTS.videoStart, (start) => {
        setTitle([start.title, start.subtitle].filter(Boolean).join(' · '))
        void controller.play(start.itemId, toTicks(handOverStart(start.positionSec, 0)), {
          audioIndex: start.audioIndex,
          subtitleIndex: start.subtitleIndex,
        })
      })
      sendPip(PIP_EVENTS.ready, { kind: 'video' })
    }
    void boot()
    return () => {
      cancelled = true
      off?.()
      void controller.dispose()
    }
  }, [controller])

  // Progress back to the main window, once a second and on every state change.
  useEffect(() => {
    const report = () => {
      const s = controller.getSnapshot()
      if (!s.itemId) return
      const progress: VideoPipProgress = {
        positionSec: s.positionSeconds,
        durationSec: s.durationSeconds,
        playing: s.status === 'playing' || s.status === 'loading',
        ended: s.status === 'ended',
      }
      sendPip(PIP_EVENTS.videoProgress, progress)
    }
    const id = window.setInterval(report, 1000)
    const unsub = controller.subscribe(() => {
      const st = controller.getSnapshot().status
      if (st !== 'playing') report()
    })
    window.addEventListener('pagehide', report)
    return () => {
      window.clearInterval(id)
      unsub()
      window.removeEventListener('pagehide', report)
    }
  }, [controller])

  const playing = snapshot.status === 'playing' || snapshot.status === 'loading'
  const duration = snapshot.durationSeconds
  const position = scrub ?? snapshot.positionSeconds
  const toggle = () => (playing ? controller.pause() : controller.resume())

  return (
    <div className="pip pip--video" data-theme="dark" data-tauri-drag-region>
      <video ref={videoRef} className="pip__video" playsInline onClick={toggle} />
      <div className="pip__shade pip__shade--video" data-tauri-drag-region />
      <div className="pip__top">
        <button
          type="button"
          className="pip__btn"
          aria-label={t('backToOrphyn')}
          title={t('backToOrphyn')}
          onClick={() => void backToMain()}
        >
          <Maximize2 />
        </button>
        <button
          type="button"
          className="pip__btn"
          aria-label={t('close')}
          title={t('close')}
          onClick={closePip}
        >
          <X />
        </button>
      </div>
      <div className="pip__bottom pip__bottom--video" data-tauri-drag-region>
        {title && (
          <div className="pip__title" data-tauri-drag-region>
            {title}
          </div>
        )}
        <div className="pip__scrubber">
          <button
            type="button"
            className="pip__btn pip__btn--main"
            aria-label={playing ? t('pause') : t('play')}
            onClick={toggle}
          >
            {playing ? <Pause fill="currentColor" /> : <Play fill="currentColor" />}
          </button>
          <span className="pip__time">{formatClock(position)}</span>
          <input
            type="range"
            className="pip__range"
            aria-label={t('seek')}
            min={0}
            max={duration || 1}
            step={1}
            value={Math.min(position, duration || 1)}
            disabled={!(duration > 0)}
            onChange={(e) => setScrub(Number(e.target.value))}
            onPointerUp={() => {
              if (scrub !== null) controller.seek(scrub)
              setScrub(null)
            }}
            onKeyUp={() => {
              if (scrub !== null) controller.seek(scrub)
              setScrub(null)
            }}
          />
          <span className="pip__time">-{formatClock(Math.max(0, duration - position))}</span>
        </div>
      </div>
    </div>
  )
}
