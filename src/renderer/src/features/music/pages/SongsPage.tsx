import { ArrowDown, ArrowUp, Music } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button, EmptyState, Skeleton, Text, TrackRow } from '@renderer/design'
import { flattenPages, useSongs } from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { useWindowedRows } from '../components/hooks'
import { FavoriteAccessory, useRowPlayback, useTrackMenuHost } from '../components/TrackRows'
import { useTrackListInteractions } from '../playlists/interactions'
import { musicPlayer } from '../player/api'
import { trackArt, trackArtist, trackDuration, trackTitle } from '../player/track'

/** Fixed row height (px): required by windowing, matches the `.jrm-songs` CSS. */
const ROW_HEIGHT = 52
const PAGE_SIZE = 100

type SortKey = 'name' | 'artist' | 'album' | 'time'
const SORT_FIELDS: Record<SortKey, string[]> = {
  name: ['SortName'],
  artist: ['Artist', 'Album', 'SortName'],
  album: ['Album', 'ParentIndexNumber', 'IndexNumber'],
  time: ['Runtime', 'SortName'],
}

/**
 * Every song of the library in a virtualised table: only the visible rows (plus a margin) are in
 * the DOM, pages of 100 are fetched as the window nears the end, and the scrollbar already
 * reflects the full size of the library.
 */
export function SongsPage() {
  const { t } = useTranslation('music')
  const [sort, setSort] = useState<{ key: SortKey; order: 'Ascending' | 'Descending' }>({
    key: 'name',
    order: 'Ascending',
  })
  const query = useSongs({
    sortBy: SORT_FIELDS[sort.key],
    sortOrder: sort.order,
    pageSize: PAGE_SIZE,
  })
  const songs = useMemo(() => flattenPages(query.data), [query.data])
  const total = query.data?.pages[0]?.total ?? 0
  const { listRef, start, end, paddingTop, paddingBottom } = useWindowedRows(total, ROW_HEIGHT)
  const { currentId, playing } = useRowPlayback()
  const { show, menu } = useTrackMenuHost()
  const ui = useTrackListInteractions({ tracks: songs, firstIndex: start })

  // Load more when the window gets close to the end of what is loaded.
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = query
  useEffect(() => {
    if (hasNextPage && !isFetchingNextPage && end > songs.length - 30) void fetchNextPage()
  }, [end, songs.length, hasNextPage, isFetchingNextPage, fetchNextPage])

  const toggleSort = (key: SortKey) =>
    setSort((s) =>
      s.key === key
        ? { key, order: s.order === 'Ascending' ? 'Descending' : 'Ascending' }
        : { key, order: 'Ascending' as const },
    )

  const header = (key: SortKey, label: string, className: string) => (
    <button
      type="button"
      className={`jrm-songs__col ${className}`}
      aria-sort={
        sort.key === key ? (sort.order === 'Ascending' ? 'ascending' : 'descending') : 'none'
      }
      onClick={() => toggleSort(key)}
    >
      {label}
      {sort.key === key &&
        (sort.order === 'Ascending' ? <ArrowUp size={12} /> : <ArrowDown size={12} />)}
    </button>
  )

  const rows = []
  for (let i = start; i < end; i++) {
    const song = songs[i]
    rows.push(
      song ? (
        <TrackRow
          key={song.Id}
          artwork={trackArt(song, 80)}
          title={trackTitle(song)}
          artist={trackArtist(song)}
          album={song.Album}
          duration={trackDuration(song)}
          className={ui.rowClass(i)}
          selected={ui.isSelected(i)}
          accessory={<FavoriteAccessory track={song} />}
          isCurrent={song.Id === currentId}
          isPlaying={playing}
          onPlay={() => {
            if (song.Id === currentId && !playing) musicPlayer.resume()
            // The queue is the loaded part of the list, starting at the chosen song.
            else musicPlayer.play(songs, i, { shuffle: false })
          }}
          onPause={() => musicPlayer.pause()}
          onMore={show(song)}
          onContextMenu={show(song)}
        />
      ) : (
        <li key={`sk-${i}`} className="jrm-songs__placeholder" aria-hidden="true">
          <Skeleton shape="text" lines={1} />
        </li>
      ),
    )
  }

  return (
    <ShellPage
      title={t('nav.songs')}
      subtitle={total ? t('library.songCount', { count: total }) : undefined}
      gutter
    >
      {query.isError ? (
        <EmptyState
          title={t('library.error')}
          action={<Button onClick={() => void query.refetch()}>{t('library.retry')}</Button>}
        />
      ) : !query.isLoading && total === 0 ? (
        <EmptyState icon={<Music />} title={t('songs.empty')} message={t('songs.emptyHint')} />
      ) : (
        <div className="jrm-songs" ref={listRef}>
          <div className="jrm-songs__head" role="row">
            <span className="jrm-songs__lead" />
            <span className="jrm-songs__main">
              {header('name', t('songs.title'), '')}
              {header('artist', t('songs.artist'), '')}
            </span>
            {header('album', t('songs.album'), 'jrm-songs__album')}
            {header('time', t('songs.time'), 'jrm-songs__time')}
            <span className="jrm-songs__more" />
          </div>
          {query.isLoading ? (
            <Text variant="footnote" color="secondary" className="jrm-songs__loading">
              {t('library.loading')}
            </Text>
          ) : (
            <div style={{ paddingTop, paddingBottom }}>
              <ol
                ref={ui.listRef}
                aria-label={t('nav.songs')}
                className="jr-track-list"
                {...ui.listProps}
              >
                {rows}
              </ol>
            </div>
          )}
        </div>
      )}
      {menu}
      {ui.bar}
    </ShellPage>
  )
}
