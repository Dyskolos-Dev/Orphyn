import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { Button, Skeleton, Text, TrackRow } from '@renderer/design'
import type { BaseItemDto } from '@renderer/jellyfin'
import { FavoriteAccessory, useRowPlayback, useTrackMenuHost } from '../../components/TrackRows'
import { pickPopularSongs } from '../../library/popularSongs'
import { musicPlayer } from '../../player/api'
import { trackArt, trackArtist, trackArtistId, trackDuration, trackTitle } from '../../player/track'

const COLLAPSED = 5
const EXPANDED = 10

interface Props {
  /** Every song of the artist that is in the library. */
  songs: readonly BaseItemDto[]
  /** Online popularity ranking (track titles, best first); empty when off or unknown. */
  ranking: readonly string[]
  loading: boolean
}

/** "Titres populaires": top 5 (10 expanded), ranked online when possible, else by play count. */
export function PopularSongs({ songs, ranking, loading }: Props) {
  const { t } = useTranslation('music')
  const [expanded, setExpanded] = useState(false)
  const { currentId, playing } = useRowPlayback()
  const { show, menu } = useTrackMenuHost()
  const top = useMemo(() => pickPopularSongs(songs, ranking, EXPANDED), [songs, ranking])
  const shown = expanded ? top : top.slice(0, COLLAPSED)

  if (loading) {
    return (
      <section className="jrm-section jrm-artist__section" aria-busy="true">
        <Text variant="title-2" className="jrm-section__title">
          {t('artist.topSongs')}
        </Text>
        {Array.from({ length: COLLAPSED }, (_, i) => (
          <Skeleton key={i} height={52} radius="var(--radius-md)" />
        ))}
      </section>
    )
  }
  if (top.length === 0) return null

  return (
    <section className="jrm-section jrm-artist__section">
      <Text variant="title-2" className="jrm-section__title">
        {t('artist.topSongs')}
      </Text>
      <ol aria-label={t('artist.topSongs')} className="jr-track-list jrm-tracklist">
        {shown.map((track, i) => {
          const artistId = trackArtistId(track)
          const artist = trackArtist(track)
          const plays = track.UserData?.PlayCount ?? 0
          return (
            <TrackRow
              key={track.Id}
              index={i + 1}
              artwork={trackArt(track, 80)}
              title={trackTitle(track)}
              artist={
                artist && artistId ? (
                  <Link
                    to={`/music/artist/${artistId}`}
                    className="jrm-link"
                    onClick={(e) => e.stopPropagation()}
                  >
                    {artist}
                  </Link>
                ) : undefined
              }
              album={track.Album}
              duration={trackDuration(track)}
              isCurrent={track.Id === currentId}
              isPlaying={playing}
              accessory={
                <>
                  {plays > 0 && (
                    <Text variant="footnote" color="secondary" className="jrm-artist__plays">
                      {t('artist.plays', { count: plays })}
                    </Text>
                  )}
                  <FavoriteAccessory track={track} />
                </>
              }
              onPlay={() => {
                if (track.Id === currentId && !playing) musicPlayer.resume()
                else musicPlayer.play(top, i, { shuffle: false })
              }}
              onPause={() => musicPlayer.pause()}
              onMore={show(track)}
              onContextMenu={show(track)}
            />
          )
        })}
      </ol>
      {menu}
      {top.length > COLLAPSED && (
        <Button variant="plain" onClick={() => setExpanded((v) => !v)}>
          {expanded ? t('artist.showLess') : t('artist.showMore')}
        </Button>
      )}
    </section>
  )
}
