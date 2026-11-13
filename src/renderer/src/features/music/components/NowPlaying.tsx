import { AnimatePresence, motion } from 'framer-motion'
import { ChevronDown, Music } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { Artwork, IconButton, SegmentedControl, springs, Text } from '@renderer/design'
import { LyricsView } from '../lyrics/LyricsView'
import { useCurrentTrack } from '../player/api'
import { trackArt, trackArtist, trackArtistId, trackTitle } from '../player/track'
import { CastButtonSlot } from './CastButtonSlot'
import { QueueView } from './QueueView'
import { SeekBar, TransportButtons, VolumeControl } from './Transport'
import { useNowPlayingUi, type NowPlayingView } from './ui-state'

/**
 * Full-window Now Playing sheet: blurred, slowly drifting artwork background, big artwork and
 * controls, with a Lyrics or Up Next panel beside them. Opened from the mini player.
 */
export function NowPlaying() {
  const open = useNowPlayingUi((s) => s.open)
  const track = useCurrentTrack()
  // Nothing to show once the queue is gone.
  useEffect(() => {
    if (open && !track) useNowPlayingUi.getState().close()
  }, [open, track])
  return createPortal(
    <AnimatePresence>{open && track && <Sheet key="np" />}</AnimatePresence>,
    document.body,
  )
}

function Sheet() {
  const { t } = useTranslation('music')
  const view = useNowPlayingUi((s) => s.view)
  const track = useCurrentTrack()
  const closeRef = useRef<HTMLButtonElement>(null)
  const art = trackArt(track, 640)
  const bg = trackArt(track, 160)

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null
    closeRef.current?.focus()
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') useNowPlayingUi.getState().close()
    }
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('keydown', onKey)
      previous?.focus?.()
    }
  }, [])

  if (!track) return null
  const artistId = trackArtistId(track)

  return (
    <motion.div
      className="jrm-np"
      data-theme="dark"
      data-view={view}
      role="dialog"
      aria-modal="true"
      aria-label={t('player.nowPlaying')}
      initial={{ y: '100%' }}
      animate={{ y: 0 }}
      exit={{ y: '100%' }}
      transition={springs.smooth}
    >
      <div className="jrm-np__bg" aria-hidden="true">
        {bg && (
          <div
            className="jrm-np__blob jrm-np__blob--a"
            style={{ backgroundImage: `url("${bg}")` }}
          />
        )}
        {bg && (
          <div
            className="jrm-np__blob jrm-np__blob--b"
            style={{ backgroundImage: `url("${bg}")` }}
          />
        )}
        <div className="jrm-np__scrim" />
      </div>

      <header className="jrm-np__header jr-drag">
        <IconButton
          ref={closeRef}
          icon={<ChevronDown />}
          label={t('player.close')}
          variant="glass"
          onClick={() => useNowPlayingUi.getState().close()}
        />
        <SegmentedControl<NowPlayingView>
          aria-label={t('player.view')}
          value={view}
          onChange={(v) => useNowPlayingUi.getState().show(v)}
          options={[
            { value: 'player', label: t('player.nowPlaying') },
            { value: 'lyrics', label: t('player.lyrics') },
            { value: 'queue', label: t('player.queue') },
          ]}
        />
        <div className="jrm-np__header-end">
          <CastButtonSlot />
        </div>
      </header>

      <div className="jrm-np__body">
        <section className="jrm-np__main" aria-label={t('player.nowPlaying')}>
          <Artwork
            src={art}
            ratio="1/1"
            shape="rounded"
            shadow
            priority
            fallback={<Music />}
            className="jrm-np__art"
          />
          <div className="jrm-np__meta">
            <Text variant="title-2" truncate className="jrm-np__title">
              {trackTitle(track)}
            </Text>
            <Text variant="body" color="secondary" truncate>
              {artistId ? (
                <Link
                  to={`/music/artist/${artistId}`}
                  onClick={() => useNowPlayingUi.getState().close()}
                  className="jrm-link"
                >
                  {trackArtist(track)}
                </Link>
              ) : (
                trackArtist(track)
              )}
              {track.Album && track.AlbumId ? (
                <>
                  {' — '}
                  <Link
                    to={`/music/album/${track.AlbumId}`}
                    onClick={() => useNowPlayingUi.getState().close()}
                    className="jrm-link"
                  >
                    {track.Album}
                  </Link>
                </>
              ) : null}
            </Text>
          </div>
          <SeekBar onMedia />
          <TransportButtons size="large" />
          <VolumeControl onMedia />
        </section>

        {view !== 'player' && (
          <section
            className="jrm-np__panel"
            aria-label={view === 'lyrics' ? t('player.lyrics') : t('player.queue')}
          >
            {view === 'lyrics' ? <LyricsView trackId={track.Id ?? undefined} /> : <QueueView />}
          </section>
        )}
      </div>
    </motion.div>
  )
}
