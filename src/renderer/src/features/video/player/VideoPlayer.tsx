import { useQueryClient } from '@tanstack/react-query'
import i18n from 'i18next'
import { Clapperboard, PictureInPicture2 } from 'lucide-react'
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react'
import { useTranslation } from 'react-i18next'
import { useLocation, useNavigate, useParams, useSearchParams } from 'react-router'
import { Button, EmptyState, IconButton, Spinner, Text, useWindowState } from '@renderer/design'
import { imageUrl, jfKeys, useEpisodes, useItem, useSession } from '@renderer/jellyfin'
import { useShellImmersive } from '@renderer/shell'
import { landscapeImage, playPath } from '../cards'
import { VideoPlaybackController } from '../controller'
import { nextEpisode } from '../format'
import { useVideoPlaybackPreferences } from '../preferences'
import { pickInitialAudio, pickInitialSubtitle } from '../tracks'
import { upNextState } from '../upNext'
import { PlayerControls } from './PlayerControls'
import { useVideoMediaControls } from './useVideoMediaControls'
import { useVideoPip } from './useVideoPip'
import { setActiveController } from '../active'
import { VideoCastButtonSlot, VideoCastScreenSlot, type VideoCastProps } from '../castSlot'

const HIDE_AFTER_MS = 3000
const SEEK_STEP = 10

/** /video/play/:itemId?start=ticks: full-window player. */
export function VideoPlayer() {
  const { t } = useTranslation('video')
  const { itemId = '' } = useParams()
  const [search] = useSearchParams()
  const navigate = useNavigate()
  const location = useLocation()
  const queryClient = useQueryClient()
  const accountId = useSession((s) => s.account?.id)
  const prefs = useVideoPlaybackPreferences()
  const { isFullscreen, setFullscreen } = useWindowState()
  useShellImmersive()

  const controller = useMemo(() => new VideoPlaybackController(), [])
  const snapshot = useSyncExternalStore(controller.subscribe, controller.getSnapshot)
  const videoRef = useRef<HTMLVideoElement>(null)
  const startRef = useRef(Number(search.get('start')) || 0)
  const [controlsVisible, setControlsVisible] = useState(true)
  const [held, setHeld] = useState(false)
  const [dismissedFor, setDismissedFor] = useState<string | null>(null)
  const hideTimer = useRef<number | undefined>(undefined)
  const enteredFullscreen = useRef(false)

  const item = useItem(itemId)
  const episodes = useEpisodes(item.data?.SeriesId ?? undefined)
  const next = item.data?.Type === 'Episode' ? nextEpisode(episodes.data ?? [], itemId) : undefined

  // --- lifecycle: bind the element, start playback, stop + report on leave
  useEffect(() => {
    controller.attach(videoRef.current)
    const release = setActiveController(controller)
    return () => {
      release()
      void controller.dispose()
      if (accountId) void queryClient.invalidateQueries({ queryKey: jfKeys.all(accountId) })
    }
  }, [controller, queryClient, accountId])

  useEffect(() => {
    if (!itemId) return
    void controller.play(itemId, startRef.current)
    startRef.current = 0
    setDismissedFor(null)
  }, [controller, itemId])

  // --- preferred audio / subtitle language, once the tracks of a stream are known
  const appliedFor = useRef<string | null>(null)
  useEffect(() => {
    const tracks = snapshot.tracks
    if (!tracks || appliedFor.current === itemId) return
    appliedFor.current = itemId
    const audio = pickInitialAudio(tracks, prefs)
    const sub = pickInitialSubtitle(tracks, prefs)
    if (audio && audio.index !== snapshot.audioIndex) {
      void controller.switchTracks({ audioIndex: audio.index }).then(() => {
        if (sub) void controller.selectSubtitle(sub.index)
      })
    } else if (sub) {
      void controller.selectSubtitle(sub.index)
    }
  }, [snapshot.tracks, snapshot.audioIndex, itemId, prefs, controller])

  // --- text subtitles: only the selected VTT <track> is showing
  useEffect(() => {
    const video = videoRef.current
    if (!video) return
    for (const track of Array.from(video.textTracks)) {
      track.mode = track.id === `sub-${snapshot.subtitleIndex}` ? 'showing' : 'disabled'
    }
  }, [snapshot.tracks, snapshot.subtitleIndex])

  // --- overlay visibility: hide 3 s after the last interaction while playing
  const showControls = useCallback(() => {
    setControlsVisible(true)
    window.clearTimeout(hideTimer.current)
    hideTimer.current = window.setTimeout(() => setControlsVisible(false), HIDE_AFTER_MS)
  }, [])
  useEffect(() => {
    if (snapshot.status !== 'playing' || held) {
      window.clearTimeout(hideTimer.current)
      setControlsVisible(true)
    } else {
      showControls()
    }
    return () => window.clearTimeout(hideTimer.current)
  }, [snapshot.status, held, showControls])

  // --- actions
  const togglePlay = useCallback(() => {
    if (snapshot.status === 'playing' || snapshot.status === 'loading') controller.pause()
    else controller.resume()
  }, [controller, snapshot.status])
  const seekBy = useCallback(
    (delta: number) => controller.seek((videoRef.current?.currentTime ?? 0) + delta),
    [controller],
  )
  const setVolume = useCallback((v: number) => {
    const el = videoRef.current
    if (!el) return
    el.volume = Math.min(1, Math.max(0, v))
    el.muted = el.volume === 0
  }, [])
  const toggleMute = useCallback(() => {
    const el = videoRef.current
    if (el) el.muted = !el.muted
  }, [])
  const toggleFullscreen = useCallback(() => {
    enteredFullscreen.current = !isFullscreen
    void setFullscreen(!isFullscreen)
  }, [isFullscreen, setFullscreen])
  useEffect(
    () => () => {
      if (enteredFullscreen.current) void setFullscreen(false)
    },
    [setFullscreen],
  )
  const leave = useCallback(() => {
    if (location.key !== 'default') navigate(-1)
    else navigate(itemId ? `/video/item/${itemId}` : '/video', { replace: true })
  }, [location.key, navigate, itemId])
  const playNext = useCallback(() => {
    if (next?.Id) navigate(playPath(next.Id), { replace: true })
  }, [next, navigate])

  // --- keyboard: space, arrows, F, M, Esc
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null
      const role = target?.getAttribute('role')
      if (target && (role === 'slider' || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) {
        if (e.key !== 'Escape') return
      }
      if (e.metaKey || e.ctrlKey || e.altKey) return
      const handled = (fn: () => void) => {
        e.preventDefault()
        showControls()
        fn()
      }
      const el = videoRef.current
      switch (e.key) {
        case ' ':
        case 'k':
          if (target?.tagName === 'BUTTON') return
          return handled(togglePlay)
        case 'ArrowLeft':
          return handled(() => seekBy(-SEEK_STEP))
        case 'ArrowRight':
          return handled(() => seekBy(SEEK_STEP))
        case 'ArrowUp':
          return handled(() => setVolume((el?.volume ?? 1) + 0.1))
        case 'ArrowDown':
          return handled(() => setVolume((el?.volume ?? 1) - 0.1))
        case 'f':
        case 'F':
          return handled(toggleFullscreen)
        case 'm':
        case 'M':
          return handled(toggleMute)
        case 'Escape':
          return handled(() => {
            if (isFullscreen) {
              enteredFullscreen.current = false
              void setFullscreen(false)
            } else leave()
          })
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [
    togglePlay,
    seekBy,
    setVolume,
    toggleFullscreen,
    toggleMute,
    isFullscreen,
    setFullscreen,
    leave,
    showControls,
  ])

  // --- up next
  const dismissed = dismissedFor === itemId
  const up = upNextState({
    positionSeconds: snapshot.positionSeconds,
    durationSeconds: snapshot.durationSeconds,
    hasNext: Boolean(next),
    dismissed,
  })
  const ended = snapshot.status === 'ended'
  useEffect(() => {
    if (!ended) return
    if (next && prefs.autoPlayNext && !dismissed) playNext()
    else if (!next) leave()
  }, [ended, next, prefs.autoPlayNext, dismissed, playNext, leave])

  // --- media session (OS media keys / now playing)
  const data = item.data
  const nativeControls = useVideoMediaControls({
    controller,
    title: data?.SeriesName ?? data?.Name ?? '',
    artist: data?.SeriesName ? (data.Name ?? undefined) : undefined,
    album: data?.SeasonName ?? undefined,
    artworkUrl: data ? landscapeImage(data, 512) : undefined,
    snapshot,
    playNext: next ? playNext : undefined,
  })
  useEffect(() => {
    // The native OS controls (Tauri shell) replace navigator.mediaSession: one entry in the OS panel.
    if (nativeControls || !('mediaSession' in navigator) || !data) return
    const ms = navigator.mediaSession
    const art = landscapeImage(data, 512)
    ms.metadata = new MediaMetadata({
      title: data.Name ?? '',
      artist: data.SeriesName ?? '',
      album: data.SeasonName ?? '',
      artwork: art ? [{ src: art, sizes: '512x288' }] : [],
    })
    const set = (a: MediaSessionAction, h: MediaSessionActionHandler | null) => {
      try {
        ms.setActionHandler(a, h)
      } catch {
        /* unsupported action */
      }
    }
    set('play', () => controller.resume())
    set('pause', () => controller.pause())
    set('seekbackward', () => seekBy(-SEEK_STEP))
    set('seekforward', () => seekBy(SEEK_STEP))
    set('seekto', (d) => d.seekTime != null && controller.seek(d.seekTime))
    set('nexttrack', next ? playNext : null)
    return () => {
      ;(['play', 'pause', 'seekbackward', 'seekforward', 'seekto', 'nexttrack'] as const).forEach(
        (a) => set(a, null),
      )
      ms.metadata = null
    }
  }, [nativeControls, data, controller, seekBy, next, playNext])
  useEffect(() => {
    if (!nativeControls && 'mediaSession' in navigator) {
      navigator.mediaSession.playbackState = snapshot.status === 'playing' ? 'playing' : 'paused'
    }
  }, [nativeControls, snapshot.status])

  const subtitleTracks = (snapshot.tracks?.subtitles ?? []).filter((s) => s.vttUrl)
  const title = data?.SeriesName ?? data?.Name ?? ''
  const subtitle =
    data?.Type === 'Episode'
      ? [
          data.ParentIndexNumber != null && data.IndexNumber != null
            ? t('item.episodeLabel', { season: data.ParentIndexNumber, episode: data.IndexNumber })
            : undefined,
          data.Name,
        ]
          .filter(Boolean)
          .join(' · ')
      : undefined
  const pip = useVideoPip({ controller, videoRef, itemId, title, subtitle })
  const failed = snapshot.status === 'error'
  const loading = snapshot.status === 'loading'
  const showUpNext = up.visible || (ended && Boolean(next) && !prefs.autoPlayNext)

  const castProps: VideoCastProps = {
    controller,
    itemId,
    title,
    subtitle,
    artworkUrl: data ? imageUrl(data, 'Backdrop', { maxWidth: 1280 }) : undefined,
    onHoldChange: setHeld,
  }

  const onStageKey = (e: ReactKeyboardEvent) => e.stopPropagation()

  return (
    <div
      className="jr-video-player"
      data-controls={controlsVisible ? 'visible' : 'hidden'}
      onMouseMove={showControls}
      onKeyDown={onStageKey}
    >
      <video
        ref={videoRef}
        className="jr-video-player__video"
        // The page is served from the app's own origin: external <track> files (the server's
        // .vtt) only load in CORS mode, which Jellyfin answers with Access-Control-Allow-Origin.
        crossOrigin="anonymous"
        playsInline
        poster={data ? imageUrl(data, 'Backdrop', { maxWidth: 1280 }) : undefined}
        onClick={togglePlay}
        onDoubleClick={toggleFullscreen}
      >
        {subtitleTracks.map((s) => (
          <track
            key={s.index}
            id={`sub-${s.index}`}
            kind="subtitles"
            src={s.vttUrl}
            srcLang={s.language}
            label={s.title}
          />
        ))}
      </video>

      {loading && !failed && (
        <div className="jr-video-player__spinner" role="status" aria-label={t('player.loading')}>
          <Spinner size="large" />
        </div>
      )}

      {failed ? (
        <div className="jr-video-player__error" data-theme="dark" role="alert">
          <EmptyState
            icon={<Clapperboard />}
            title={t('player.errorTitle')}
            message={snapshot.error || t('player.errorMessage')}
            action={
              <>
                <Button variant="contrast" onClick={() => void controller.retry()}>
                  {t('player.retry')}
                </Button>
                <Button variant="gray" onClick={leave}>
                  {t('player.back')}
                </Button>
              </>
            }
          />
        </div>
      ) : (
        <PlayerControls
          snapshot={snapshot}
          title={title}
          subtitle={subtitle}
          isFullscreen={isFullscreen}
          onBack={leave}
          onTogglePlay={togglePlay}
          onSeekBy={seekBy}
          onSeek={(s) => controller.seek(s)}
          onVolume={setVolume}
          onToggleMute={toggleMute}
          onRate={(r) => {
            if (videoRef.current) videoRef.current.playbackRate = r
          }}
          onToggleFullscreen={toggleFullscreen}
          onSelectAudio={(a) => void controller.switchTracks({ audioIndex: a.index })}
          onSelectSubtitle={(s) => void controller.selectSubtitle(s ? s.index : -1)}
          onHoldChange={setHeld}
          castButton={<VideoCastButtonSlot {...castProps} />}
          pipButton={
            pip.available ? (
              <IconButton
                variant="glass"
                icon={<PictureInPicture2 />}
                label={i18n.t('pip:openVideo')}
                pressed={pip.active}
                onClick={() => void pip.toggle()}
              />
            ) : undefined
          }
        />
      )}

      <VideoCastScreenSlot {...castProps} />

      {showUpNext && next && (
        <div className="jr-video-upnext" data-theme="dark">
          <div className="jr-video-upnext__thumb">
            {landscapeImage(next, 320) && (
              <img src={landscapeImage(next, 320)} alt="" draggable={false} />
            )}
          </div>
          <div className="jr-video-upnext__body">
            <Text variant="caption" color="secondary">
              {t('player.upNext')}
            </Text>
            <Text variant="headline" truncate>
              {next.Name}
            </Text>
            {prefs.autoPlayNext && !ended && (
              <Text variant="footnote" color="secondary" tabular>
                {t('player.startsIn', { seconds: up.secondsLeft })}
              </Text>
            )}
            <div className="jr-video-upnext__actions">
              <Button variant="contrast" size="small" onClick={playNext}>
                {t('player.playNow')}
              </Button>
              <Button variant="gray" size="small" onClick={() => setDismissedFor(itemId)}>
                {t('player.cancel')}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
