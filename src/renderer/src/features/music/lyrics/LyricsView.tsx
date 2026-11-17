import { MicOff } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { EmptyState, Spinner } from '@renderer/design'
import { useLyrics, useOnlineLyrics, type OnlineLyrics, type TrackQuery } from '@renderer/jellyfin'
import { usePlaybackPreferences } from '@renderer/features/settings/preferences'
import { musicPlayer, useCurrentTrack, useMusicPlayer } from '../player/api'
import { findLineIndex } from './lines'
import './lyrics.css'

/** Pause auto-scroll this long after the user scrolls by hand. */
const MANUAL_SCROLL_GRACE_MS = 3500

/**
 * Lyrics: synced lines with the current one large and bright, the others dimmed,
 * smooth auto-scroll, click a line to seek. Unsynced lyrics are plain text.
 */
export function LyricsView({
  trackId,
  hasLyrics,
}: {
  trackId: string | undefined
  /** The track's `HasLyrics`: `false` shows the empty state without asking the server. */
  hasLyrics?: boolean | null
}) {
  const { t } = useTranslation('music')
  const server = useLyrics(trackId, hasLyrics)
  const { onlineLyrics } = usePlaybackPreferences()
  const current = useCurrentTrack()
  // The track being played, read from the player so callers only pass its id.
  const query: TrackQuery | undefined =
    current && current.Id === trackId
      ? {
          title: current.Name ?? '',
          artist: current.Artists?.[0] ?? current.AlbumArtist ?? '',
          album: current.Album ?? undefined,
          duration: current.RunTimeTicks ? current.RunTimeTicks / 10_000_000 : undefined,
        }
      : undefined
  const serverEmpty = !server.isLoading && !(server.data && server.data.lines.length > 0)
  const online = useOnlineLyrics(query, onlineLyrics && serverEmpty)
  const lyrics = server.data && server.data.lines.length > 0 ? server.data : online.data
  const searching = serverEmpty && online.isLoading
  const source = lyrics && 'source' in lyrics ? (lyrics as OnlineLyrics).source : null
  const synced = !!lyrics?.synced
  const active = useMusicPlayer((s) =>
    lyrics?.synced ? findLineIndex(lyrics.lines, s.position * 1000) : -1,
  )
  const scroller = useRef<HTMLDivElement>(null)
  const lineRefs = useRef<(HTMLButtonElement | null)[]>([])
  const lastManual = useRef(0)

  useEffect(() => {
    if (!synced || active < 0) return
    if (Date.now() - lastManual.current < MANUAL_SCROLL_GRACE_MS) return
    // Scroll only the lyrics box (never an ancestor): the active line sits at ~35 % of its height.
    const box = scroller.current
    const line = lineRefs.current[active]
    if (!box || !line) return
    const b = box.getBoundingClientRect()
    const l = line.getBoundingClientRect()
    box.scrollBy({ top: l.top + l.height / 2 - (b.top + b.height * 0.35), behavior: 'smooth' })
  }, [active, synced])

  // New track: start from the top.
  useEffect(() => {
    lastManual.current = 0
    scroller.current?.scrollTo({ top: 0 })
  }, [trackId])

  if (server.isLoading) {
    return (
      <div className="jrm-lyrics jrm-lyrics--empty">
        <Spinner />
      </div>
    )
  }

  if (!lyrics || lyrics.lines.length === 0) {
    return (
      <div className="jrm-lyrics jrm-lyrics--empty" aria-busy={searching || undefined}>
        {searching && <Spinner />}
        {/* While LRCLIB is asked, the empty state waits out of sight instead of flashing. */}
        <div className={searching ? 'jrm-lyrics-pending' : undefined}>
          <EmptyState
            size="inline"
            icon={<MicOff />}
            title={t('lyrics.none')}
            message={t('lyrics.noneHint')}
          />
        </div>
      </div>
    )
  }

  const markManual = () => {
    lastManual.current = Date.now()
  }

  return (
    <div
      ref={scroller}
      className={`jrm-lyrics ${synced ? 'jrm-lyrics--synced' : 'jrm-lyrics--plain'}`}
      onWheel={markManual}
      onTouchMove={markManual}
      aria-label={t('lyrics.title')}
    >
      {synced ? (
        lyrics.lines.map((line, i) => (
          <button
            key={i}
            ref={(el) => {
              lineRefs.current[i] = el
            }}
            type="button"
            className="jrm-lyric"
            data-state={i === active ? 'active' : i < active ? 'past' : 'future'}
            aria-current={i === active || undefined}
            onClick={() => {
              lastManual.current = 0
              musicPlayer.seek((line.startMs ?? 0) / 1000)
            }}
          >
            {line.text.trim() === '' ? '♪' : line.text}
          </button>
        ))
      ) : (
        <>
          {lyrics.lines.map((line, i) => (
            <p key={i} className="jrm-lyric-plain">
              {line.text}
            </p>
          ))}
        </>
      )}
      {source && <p className="jrm-lyrics-source">{t('lyrics.source', { source })}</p>}
    </div>
  )
}
