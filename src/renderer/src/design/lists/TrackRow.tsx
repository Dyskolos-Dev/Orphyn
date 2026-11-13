import clsx from 'clsx'
import { Ellipsis, Pause, Play } from 'lucide-react'
import type { KeyboardEvent, MouseEvent, ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Artwork } from '../media/Artwork'
import './lists.css'

/** "3:07", "1:02:03". Negative / NaN give "--:--". */
export function formatDuration(totalSeconds: number): string {
  if (!Number.isFinite(totalSeconds) || totalSeconds < 0) return '--:--'
  const s = Math.floor(totalSeconds)
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = String(s % 60).padStart(2, '0')
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
}

export interface TrackListProps {
  children: ReactNode
  className?: string
  'aria-label'?: string
}

/**
 * Container for TrackRow (an <ol>). Arrow Up / Down move between rows, Enter plays.
 */
export function TrackList({ children, className, ...rest }: TrackListProps) {
  return (
    <ol className={clsx('jr-track-list', className)} aria-label={rest['aria-label']}>
      {children}
    </ol>
  )
}

export interface TrackRowProps {
  /** Number shown in the leading column (track or position). */
  index?: number
  title: string
  artist?: ReactNode
  album?: ReactNode
  /** Seconds. */
  duration?: number
  /** Small artwork instead of the number (playlists, songs list). */
  artwork?: string | null
  /** This is the current track. */
  isCurrent?: boolean
  /** Current track and playing (animated equalizer), else paused. */
  isPlaying?: boolean
  explicit?: boolean
  /** Play this track (double-click, Enter, or the hover play button). */
  onPlay(): void
  /** Pause, when this is the current playing track. */
  onPause?(): void
  /** "More" button (open a Menu anchored on the event target). */
  onMore?(e: MouseEvent<HTMLButtonElement>): void
  onContextMenu?(e: MouseEvent): void
  /** Trailing accessory before the duration (e.g. a favourite heart). */
  accessory?: ReactNode
  selected?: boolean
  onSelect?(): void
  className?: string
}

/** Music track row: number / play, title + artist, album, duration, more. */
export function TrackRow({
  index,
  title,
  artist,
  album,
  duration,
  artwork,
  isCurrent = false,
  isPlaying = false,
  explicit = false,
  onPlay,
  onPause,
  onMore,
  onContextMenu,
  accessory,
  selected = false,
  onSelect,
  className,
}: TrackRowProps) {
  const { t } = useTranslation('shell')
  const playingNow = isCurrent && isPlaying

  const onKeyDown = (e: KeyboardEvent<HTMLLIElement>) => {
    if (e.target !== e.currentTarget) return
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      if (playingNow && onPause) onPause()
      else onPlay()
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      const sibling =
        e.key === 'ArrowDown'
          ? e.currentTarget.nextElementSibling
          : e.currentTarget.previousElementSibling
      if (sibling instanceof HTMLElement) {
        e.preventDefault()
        sibling.focus()
      }
    }
  }

  const label = [
    title,
    typeof artist === 'string' ? artist : null,
    duration != null ? formatDuration(duration) : null,
  ]
    .filter(Boolean)
    .join(', ')

  return (
    <li
      tabIndex={0}
      aria-label={label}
      aria-current={isCurrent || undefined}
      className={clsx(
        'jr-track',
        isCurrent && 'jr-track--current',
        selected && 'jr-track--selected',
        artwork !== undefined && 'jr-track--with-art',
        album != null && 'jr-track--with-album',
        className,
      )}
      onClick={onSelect}
      onDoubleClick={onPlay}
      onKeyDown={onKeyDown}
      onContextMenu={onContextMenu}
    >
      <span className="jr-track__lead">
        {artwork !== undefined ? (
          <Artwork src={artwork} ratio="1/1" className="jr-track__art" />
        ) : (
          <span className="jr-track__number">
            {playingNow ? <Equalizer /> : index != null ? index : null}
          </span>
        )}
        <button
          type="button"
          tabIndex={-1}
          className="jr-track__play"
          aria-label={playingNow ? t('design.nowPlaying') : t('design.playItem', { title })}
          onClick={(e) => {
            e.stopPropagation()
            if (playingNow && onPause) onPause()
            else onPlay()
          }}
          onDoubleClick={(e) => e.stopPropagation()}
        >
          {playingNow ? (
            <Pause size={14} fill="currentColor" aria-hidden="true" />
          ) : (
            <Play size={14} fill="currentColor" aria-hidden="true" />
          )}
        </button>
      </span>
      <span className="jr-track__main">
        <span className="jr-track__title">
          <span className="jr-track__title-text">{title}</span>
          {explicit && (
            <span className="jr-track__explicit" title={t('design.explicit')}>
              E
            </span>
          )}
        </span>
        {artist && <span className="jr-track__artist">{artist}</span>}
      </span>
      {album != null && <span className="jr-track__album">{album}</span>}
      {accessory && <span className="jr-track__accessory">{accessory}</span>}
      <span className="jr-track__duration">{duration != null ? formatDuration(duration) : ''}</span>
      <span className="jr-track__more-slot">
        {onMore && (
          <button
            type="button"
            tabIndex={-1}
            className="jr-track__more"
            aria-label={t('design.moreFor', { title })}
            onClick={(e) => {
              e.stopPropagation()
              onMore(e)
            }}
          >
            <Ellipsis size={16} aria-hidden="true" />
          </button>
        )}
      </span>
    </li>
  )
}

function Equalizer() {
  return (
    <span className="jr-equalizer" aria-hidden="true">
      <span />
      <span />
      <span />
      <span />
    </span>
  )
}
