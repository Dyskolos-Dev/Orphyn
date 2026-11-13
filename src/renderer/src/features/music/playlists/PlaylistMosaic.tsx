import { ListMusic } from 'lucide-react'
import { usePlaylistItems, type BaseItemDto } from '@renderer/jellyfin'
import { trackArt } from '../player/track'

/** Up to four distinct album covers of a playlist as a 2×2 mosaic (a single cover fills the tile). */
export function coversOf(tracks: readonly BaseItemDto[], size: number, max = 4): string[] {
  const seen = new Set<string>()
  const out: string[] = []
  for (const track of tracks) {
    const url = trackArt(track, size)
    const key = track.AlbumId ?? url
    if (!url || !key || seen.has(key)) continue
    seen.add(key)
    out.push(url)
    if (out.length === max) break
  }
  return out
}

/** Fallback artwork for playlists without their own image. */
export function PlaylistMosaic({ playlistId, size = 160 }: { playlistId: string; size?: number }) {
  const items = usePlaylistItems(playlistId)
  const covers = coversOf(items.data ?? [], size * 2)
  if (covers.length === 0) return <ListMusic />
  if (covers.length < 4) {
    return <img className="jrm-mosaic__single" src={covers[0]} alt="" draggable={false} />
  }
  return (
    <div className="jrm-mosaic" aria-hidden="true">
      {covers.map((src) => (
        <img key={src} src={src} alt="" draggable={false} />
      ))}
    </div>
  )
}
