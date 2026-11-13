import { Heart } from 'lucide-react'
import { Fragment, useState, type MouseEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { Menu, Text, TrackList, TrackRow } from '@renderer/design'
import { useToggleFavorite, type BaseItemDto } from '@renderer/jellyfin'
import { musicPlayer, useMusicPlayer } from '../player/api'
import { trackArt, trackArtist, trackArtistId, trackDuration, trackTitle } from '../player/track'
import { isFavorite, useTrackMenu } from './hooks'

/** Which track is current and whether the player is running, for the "now playing" row state. */
export function useRowPlayback() {
  const currentId = useMusicPlayer((s) => s.queue[s.index]?.track.Id)
  const status = useMusicPlayer((s) => s.status)
  const wantPlaying = useMusicPlayer((s) => s.wantPlaying)
  return { currentId, playing: wantPlaying && status !== 'paused' && status !== 'error' }
}

/** One "more" / right-click menu shared by all rows of a list. */
export function useTrackMenuHost() {
  const buildItems = useTrackMenu()
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

function FavoriteAccessory({ track }: { track: BaseItemDto }) {
  const { t } = useTranslation('music')
  const toggle = useToggleFavorite()
  if (!isFavorite(track)) return null
  return (
    <button
      type="button"
      tabIndex={-1}
      className="jrm-heart"
      aria-label={t('actions.unfavorite')}
      onClick={(e) => {
        e.stopPropagation()
        toggle.mutate({ id: track.Id ?? '', value: false })
      }}
    >
      <Heart size={14} fill="currentColor" aria-hidden="true" />
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
  'aria-label': string
}

/** Track list for albums, playlists and artist top songs: clicking a row plays the whole list from it. */
export function SongRows({
  tracks,
  lead,
  hideArtist,
  showAlbum,
  groupByDisc,
  ...rest
}: SongRowsProps) {
  const { t } = useTranslation('music')
  const { currentId, playing } = useRowPlayback()
  const { show, menu } = useTrackMenuHost()

  const discs = groupByDisc
    ? new Set(tracks.map((x) => x.ParentIndexNumber ?? 1))
    : new Set<number>()
  const multiDisc = discs.size > 1

  return (
    <>
      <TrackList aria-label={rest['aria-label']} className="jrm-tracklist">
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
      </TrackList>
      {menu}
    </>
  )
}
