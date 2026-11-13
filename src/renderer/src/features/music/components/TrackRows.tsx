import { Heart } from 'lucide-react'
import { Fragment, useState, type MouseEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { Menu, Text, TrackRow } from '@renderer/design'
import { useToggleFavorite, type BaseItemDto } from '@renderer/jellyfin'
import { musicPlayer, useMusicPlayer } from '../player/api'
import { trackArt, trackArtist, trackArtistId, trackDuration, trackTitle } from '../player/track'
import { useTrackListInteractions } from '../playlists/interactions'
import { useTrackMenuWithPlaylists } from '../playlists/menus'
import { isFavorite } from './hooks'

/** Which track is current and whether the player is running, for the "now playing" row state. */
export function useRowPlayback() {
  const currentId = useMusicPlayer((s) => s.queue[s.index]?.track.Id)
  const status = useMusicPlayer((s) => s.status)
  const wantPlaying = useMusicPlayer((s) => s.wantPlaying)
  return { currentId, playing: wantPlaying && status !== 'paused' && status !== 'error' }
}

/** One "more" / right-click menu shared by all rows of a list. */
export function useTrackMenuHost(opts?: { onRemove?: (track: BaseItemDto) => void }) {
  const buildItems = useTrackMenuWithPlaylists(opts)
  const [open, setOpen] = useState<{ track: BaseItemDto; x: number; y: number } | null>(null)
  const show = (track: BaseItemDto) => (e: MouseEvent) => {
    e.preventDefault()
    const target = e.currentTarget as HTMLElement
    const rect = target.getBoundingClientRect()
    const anchored = e.type === 'click'
    setOpen({ track, x: anchored ? rect.right : e.clientX, y: anchored ? rect.bottom : e.clientY })
  }
  const menu = open && (
    <Menu
      open
      anchor={{ x: open.x, y: open.y }}
      items={buildItems(open.track)}
      onClose={() => setOpen(null)}
    />
  )
  return { show, menu }
}

export function FavoriteAccessory({ track }: { track: BaseItemDto }) {
  const { t } = useTranslation('music')
  const toggle = useToggleFavorite()
  const favorite = isFavorite(track)
  return (
    <button
      type="button"
      tabIndex={-1}
      className={favorite ? 'jrm-heart' : 'jrm-heart jrm-heart--off'}
      aria-label={favorite ? t('actions.unfavorite') : t('actions.favorite')}
      aria-pressed={favorite}
      onClick={(e) => {
        e.stopPropagation()
        toggle.mutate({ id: track.Id ?? '', value: !favorite })
      }}
    >
      <Heart size={14} fill={favorite ? 'currentColor' : 'none'} aria-hidden="true" />
    </button>
  )
}

export interface SongRowsProps {
  tracks: readonly BaseItemDto[]
  /** Leading column: the track number, or a thumbnail (playlists, top songs). */
  lead: 'number' | 'artwork'
  /** Show the artist line even when it equals `hideArtist`. */
  hideArtist?: string
  showAlbum?: boolean
  /** Group by disc when the tracks span several discs. */
  groupByDisc?: boolean
  /** Set on a playlist page: drag to reorder, Delete / menu to remove. */
  playlist?: BaseItemDto
  'aria-label': string
}

/** Track list for albums, playlists and artist top songs: clicking a row plays the whole list from it. */
export function SongRows({
  tracks,
  lead,
  hideArtist,
  showAlbum,
  groupByDisc,
  playlist,
  ...rest
}: SongRowsProps) {
  const { t } = useTranslation('music')
  const { currentId, playing } = useRowPlayback()
  const ui = useTrackListInteractions({ tracks, playlist })
  const { show, menu } = useTrackMenuHost(
    playlist
      ? {
          onRemove: (track) => ui.removeTrack(tracks.indexOf(track)),
        }
      : undefined,
  )

  const discs = groupByDisc
    ? new Set(tracks.map((x) => x.ParentIndexNumber ?? 1))
    : new Set<number>()
  const multiDisc = discs.size > 1

  return (
    <>
      <ol
        ref={ui.listRef}
        aria-label={rest['aria-label']}
        className="jr-track-list jrm-tracklist"
        {...ui.listProps}
      >
        {tracks.map((track, i) => {
          const artist = trackArtist(track)
          const artistId = trackArtistId(track)
          const newDisc =
            multiDisc &&
            (i === 0 || (track.ParentIndexNumber ?? 1) !== (tracks[i - 1]?.ParentIndexNumber ?? 1))
          return (
            <Fragment key={`${track.Id}-${i}`}>
              {newDisc && (
                <li className="jrm-disc" aria-hidden="true">
                  <Text variant="footnote" color="secondary">
                    {t('album.disc', { n: track.ParentIndexNumber ?? 1 })}
                  </Text>
                </li>
              )}
              <TrackRow
                index={track.IndexNumber ?? i + 1}
                artwork={lead === 'artwork' ? trackArt(track, 80) : undefined}
                title={trackTitle(track)}
                artist={
                  artist && artist !== hideArtist ? (
                    artistId ? (
                      <Link
                        to={`/music/artist/${artistId}`}
                        className="jrm-link"
                        onClick={(e) => e.stopPropagation()}
                      >
                        {artist}
                      </Link>
                    ) : (
                      artist
                    )
                  ) : undefined
                }
                album={showAlbum ? track.Album : undefined}
                duration={trackDuration(track)}
                className={ui.rowClass(i)}
                selected={ui.isSelected(i)}
                isCurrent={track.Id === currentId}
                isPlaying={playing}
                accessory={<FavoriteAccessory track={track} />}
                onPlay={() => {
                  if (track.Id === currentId && !playing) musicPlayer.resume()
                  else musicPlayer.play(tracks, i, { shuffle: false })
                }}
                onPause={() => musicPlayer.pause()}
                onMore={show(track)}
                onContextMenu={show(track)}
              />
            </Fragment>
          )
        })}
      </ol>
      {menu}
      {ui.bar}
    </>
  )
}
