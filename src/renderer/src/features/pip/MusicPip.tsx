import { Heart, Maximize2, Music, Pause, Play, SkipBack, SkipForward, X } from 'lucide-react'
import { useEffect, useReducer, useState, type PointerEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Window } from '@tauri-apps/api/window'
import { onPip, sendPip } from './events'
import {
  EMPTY_MUSIC_PIP,
  PIP_EVENTS,
  extrapolatePosition,
  musicPipReducer,
  type MusicPipCommand,
  type MusicPipState,
} from './protocol'

const command = (cmd: MusicPipCommand): void => sendPip(PIP_EVENTS.musicCommand, cmd)

/** Brings the main window back (it may be minimised), then closes the PiP. */
export async function backToMain(): Promise<void> {
  try {
    const main = await Window.getByLabel('main')
    await main?.unminimize()
    await main?.show()
    await main?.setFocus()
  } catch {
    /* the main window is gone: nothing to go back to */
  }
}

export const closePip = (): void => {
  void Window.getCurrent().close()
}

function Progress({ state, position }: { state: MusicPipState; position: number }) {
  const fraction = state.duration > 0 ? Math.min(1, position / state.duration) : 0
  const seek = (e: PointerEvent<HTMLDivElement>) => {
    if (!(state.duration > 0)) return
    const rect = e.currentTarget.getBoundingClientRect()
    const f = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width))
    command({ type: 'seek', positionSec: f * state.duration })
  }
  return (
    <div
      className="pip__progress"
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={Math.round(state.duration)}
      aria-valuenow={Math.round(position)}
      onPointerDown={seek}
    >
      <div className="pip__progress-fill" style={{ width: `${fraction * 100}%` }} />
    </div>
  )
}

/** Music Picture in Picture: artwork full-bleed over a blurred copy, controls on hover. */
export function MusicPip() {
  const { t } = useTranslation('pip')
  const [state, dispatch] = useReducer(musicPipReducer, EMPTY_MUSIC_PIP)
  const [now, setNow] = useState(() => Date.now())

  useEffect(() => {
    const off = onPip<MusicPipState>(PIP_EVENTS.musicState, (s) =>
      dispatch({ type: 'snapshot', state: s }),
    )
    // Ask for the current state once the listener is in place.
    sendPip(PIP_EVENTS.ready, { kind: 'music' })
    return off
  }, [])

  // The bar moves on its own between two snapshots.
  useEffect(() => {
    if (!state.playing) return
    const id = window.setInterval(() => setNow(Date.now()), 250)
    return () => window.clearInterval(id)
  }, [state.playing])

  // Esc closes, space toggles: the window is focused when the viewer interacts with it.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') closePip()
      else if (e.key === ' ') {
        e.preventDefault()
        command({ type: 'toggle' })
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  const position = extrapolatePosition(state, now)
  const art = state.artworkUrl
  return (
    <div className="pip pip--music" data-theme="dark" data-tauri-drag-region>
      {art && <div className="pip__bg" style={{ backgroundImage: `url("${art}")` }} />}
      {art ? (
        <img className="pip__art" src={art} alt="" draggable={false} data-tauri-drag-region />
      ) : (
        <div className="pip__art pip__art--empty" data-tauri-drag-region>
          <Music />
        </div>
      )}
      <div className="pip__shade" data-tauri-drag-region />

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

      <div className="pip__bottom" data-tauri-drag-region>
        <div className="pip__meta" data-tauri-drag-region>
          <div className="pip__title" data-tauri-drag-region>
            {state.hasTrack ? state.title : t('nothingPlaying')}
          </div>
          {state.hasTrack && (
            <div className="pip__artist" data-tauri-drag-region>
              {state.artist}
            </div>
          )}
        </div>
        <div className="pip__controls">
          <button
            type="button"
            className="pip__btn"
            aria-label={state.favorite ? t('unfavorite') : t('favorite')}
            aria-pressed={state.favorite}
            disabled={!state.hasTrack}
            data-on={state.favorite || undefined}
            onClick={() => command({ type: 'favorite' })}
          >
            <Heart fill={state.favorite ? 'currentColor' : 'none'} />
          </button>
          <button
            type="button"
            className="pip__btn"
            aria-label={t('previous')}
            disabled={!state.hasTrack}
            onClick={() => command({ type: 'previous' })}
          >
            <SkipBack fill="currentColor" />
          </button>
          <button
            type="button"
            className="pip__btn pip__btn--main"
            aria-label={state.playing ? t('pause') : t('play')}
            disabled={!state.hasTrack}
            onClick={() => command({ type: 'toggle' })}
          >
            {state.playing ? <Pause fill="currentColor" /> : <Play fill="currentColor" />}
          </button>
          <button
            type="button"
            className="pip__btn"
            aria-label={t('next')}
            disabled={!state.hasTrack}
            onClick={() => command({ type: 'next' })}
          >
            <SkipForward fill="currentColor" />
          </button>
        </div>
        <Progress state={state} position={position} />
      </div>
    </div>
  )
}
