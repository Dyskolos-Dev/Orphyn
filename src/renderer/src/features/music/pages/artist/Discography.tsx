import { Disc3 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ContextMenu, Shelf, SquareCard } from '@renderer/design'
import type { BaseItemDto } from '@renderer/jellyfin'
import { art } from '../../components/hooks'
import type { Discography as DiscographyData } from '../../library/releaseType'
import { musicPlayer } from '../../player/api'
import { useContainerMenuWithPlaylists } from '../../playlists/menus'

/** Release card: the year sits under the cover (the artist is the page's own). */
function ReleaseCard({ album, showArtist }: { album: BaseItemDto; showArtist?: boolean }) {
  const menu = useContainerMenuWithPlaylists()
  const id = album.Id ?? ''
  return (
    <ContextMenu items={menu(album)}>
      <SquareCard
        title={album.Name ?? ''}
        subtitle={
          [showArtist ? album.AlbumArtist : undefined, album.ProductionYear]
            .filter(Boolean)
            .join(' · ') || undefined
        }
        image={art(album)}
        fallback={<Disc3 />}
        to={`/music/album/${id}`}
        onPlay={() => void musicPlayer.playAlbum(id)}
      />
    </ContextMenu>
  )
}

/** "Albums", "EP & singles", "Compilations", "Appears on": each newest first, hidden when empty. */
export function Discography({ data }: { data: DiscographyData }) {
  const { t } = useTranslation('music')
  const groups: Array<[string, BaseItemDto[], boolean]> = [
    [t('artist.albums'), data.albums, false],
    [t('artist.singles'), data.eps, false],
    [t('artist.compilations'), data.compilations, false],
    [t('artist.appearsOn'), data.appearsOn, true],
  ]
  return (
    <>
      {groups.map(
        ([title, list, showArtist]) =>
          list.length > 0 && (
            <div key={title} className="jrm-bleed jrm-artist__shelf">
              <Shelf title={title} kind="square">
                {list.map((a) => (
                  <ReleaseCard key={a.Id} album={a} showArtist={showArtist} />
                ))}
              </Shelf>
            </div>
          ),
      )}
    </>
  )
}
