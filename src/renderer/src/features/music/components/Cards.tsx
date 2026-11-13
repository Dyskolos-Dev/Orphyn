import { Disc3, User } from 'lucide-react'
import { ContextMenu, RoundCard, SquareCard, Skeleton, Grid } from '@renderer/design'
import type { BaseItemDto } from '@renderer/jellyfin'
import { musicPlayer } from '../player/api'
import { writeDragItems } from '../playlists/dnd'
import { useContainerMenuWithPlaylists } from '../playlists/menus'
import { PlaylistMosaic } from '../playlists/PlaylistMosaic'
import { art } from './hooks'

export function AlbumCard({ album }: { album: BaseItemDto }) {
  const menu = useContainerMenuWithPlaylists()
  const id = album.Id ?? ''
  return (
    <ContextMenu items={menu(album)}>
      {/* The card's link starts the drag; the payload lets the sidebar playlists accept the album. */}
      <div
        className="jrm-draggable"
        onDragStart={(e) => writeDragItems(e.dataTransfer, { ids: [id], label: album.Name ?? '' })}
      >
        <SquareCard
          title={album.Name ?? ''}
          subtitle={[album.AlbumArtist, album.ProductionYear].filter(Boolean).join(' · ')}
          image={art(album)}
          fallback={<Disc3 />}
          to={`/music/album/${id}`}
          onPlay={() => void musicPlayer.playAlbum(id)}
        />
      </div>
    </ContextMenu>
  )
}

export function PlaylistCard({ playlist }: { playlist: BaseItemDto }) {
  const menu = useContainerMenuWithPlaylists()
  const id = playlist.Id ?? ''
  return (
    <ContextMenu items={menu(playlist)}>
      <SquareCard
        title={playlist.Name ?? ''}
        subtitle={playlist.ChildCount != null ? String(playlist.ChildCount) : undefined}
        image={art(playlist)}
        fallback={<PlaylistMosaic playlistId={id} />}
        to={`/music/playlist/${id}`}
        onPlay={() => void musicPlayer.playPlaylist(id)}
      />
    </ContextMenu>
  )
}

export function ArtistCard({ artist }: { artist: BaseItemDto }) {
  const menu = useContainerMenuWithPlaylists()
  const id = artist.Id ?? ''
  return (
    <ContextMenu items={menu(artist)}>
      <RoundCard
        title={artist.Name ?? ''}
        image={art(artist)}
        fallback={<User />}
        to={`/music/artist/${id}`}
      />
    </ContextMenu>
  )
}

/** Placeholder grid while a library page loads. */
export function GridSkeleton({ kind }: { kind: 'square' | 'round' }) {
  return (
    <Grid kind={kind} aria-label="…">
      {Array.from({ length: 12 }, (_, i) => (
        <div key={i}>
          <Skeleton
            aspectRatio="1 / 1"
            radius={kind === 'round' ? '50%' : 'var(--radius-artwork)'}
          />
          <Skeleton shape="text" lines={2} />
        </div>
      ))}
    </Grid>
  )
}
