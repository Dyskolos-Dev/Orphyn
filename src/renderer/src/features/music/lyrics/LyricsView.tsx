import { MicOff } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { EmptyState, Spinner } from '@renderer/design'
import { useLyrics } from '@renderer/jellyfin'
import { musicPlayer, useMusicPlayer } from '../player/api'
import { findLineIndex } from './lines'

/** Pause auto-scroll this long after the user scrolls by hand. */
const MANUAL_SCROLL_GRACE_MS = 3500

/**
 * Lyrics like Apple Music: synced lines with the current one large and bright, the others dimmed,
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
  const query = useLyrics(trackId, hasLyrics)
  const lyrics = query.data
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
    lineRefs.current[active]?.scrollIntoView({ block: 'center', behavior: 'smooth' })
  }, [active, synced])

  // New track: start from the top.
  useEffect(() => {
    lastManual.current = 0
    scroller.current?.scrollTo({ top: 0 })
  }, [trackId])

  if (query.isLoading) {
    return (
      <div className="jrm-lyrics jrm-lyrics--empty">
        <Spinner />
      </div>
    )
  }

  if (!lyrics || lyrics.lines.length === 0) {
    return (
      <div className="jrm-lyrics jrm-lyrics--empty">
        <EmptyState
          size="inline"
          icon={<MicOff />}
          title={t('lyrics.none')}
          message={t('lyrics.noneHint')}
        />
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
    </div>
  )
}
