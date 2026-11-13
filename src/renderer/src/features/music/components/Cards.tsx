import { Disc3, ListMusic, User } from 'lucide-react'
import { ContextMenu, RoundCard, SquareCard, Skeleton, Grid } from '@renderer/design'
import type { BaseItemDto } from '@renderer/jellyfin'
import { musicPlayer } from '../player/api'
import { art, useContainerMenu } from './hooks'

export function AlbumCard({ album }: { album: BaseItemDto }) {
  const menu = useContainerMenu()
  const id = album.Id ?? ''
  return (
    <ContextMenu items={menu(album)}>
      <SquareCard
        title={album.Name ?? ''}
        subtitle={[album.AlbumArtist, album.ProductionYear].filter(Boolean).join(' · ')}
        image={art(album)}
        fallback={<Disc3 />}
        to={`/music/album/${id}`}
        onPlay={() => void musicPlayer.playAlbum(id)}
      />
    </ContextMenu>
  )
}

export function PlaylistCard({ playlist }: { playlist: BaseItemDto }) {
  const menu = useContainerMenu()
  const id = playlist.Id ?? ''
  return (
    <ContextMenu items={menu(playlist)}>
      <SquareCard
        title={playlist.Name ?? ''}
        subtitle={playlist.ChildCount != null ? String(playlist.ChildCount) : undefined}
        image={art(playlist)}
        fallback={<ListMusic />}
        to={`/music/playlist/${id}`}
        onPlay={() => void musicPlayer.playPlaylist(id)}
      />
    </ContextMenu>
  )
}

export function ArtistCard({ artist }: { artist: BaseItemDto }) {
  const menu = useContainerMenu()
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
