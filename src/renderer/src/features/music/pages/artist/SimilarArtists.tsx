import { useTranslation } from 'react-i18next'
import { Shelf, Skeleton } from '@renderer/design'
import type { BaseItemDto } from '@renderer/jellyfin'
import { ArtistCard } from '../../components/Cards'

interface Props {
  artists: readonly BaseItemDto[]
  loading: boolean
}

/** "Artistes similaires": round images, clickable; skeleton while loading, hidden when empty. */
export function SimilarArtists({ artists, loading }: Props) {
  const { t } = useTranslation('music')
  if (loading) {
    return (
      <div className="jrm-bleed jrm-artist__shelf" aria-busy="true">
        <Shelf title={t('artist.similar')} kind="round">
          {Array.from({ length: 8 }, (_, i) => (
            <div key={i}>
              <Skeleton aspectRatio="1 / 1" radius="50%" />
              <Skeleton shape="text" />
            </div>
          ))}
        </Shelf>
      </div>
    )
  }
  if (artists.length === 0) return null
  return (
    <div className="jrm-bleed jrm-artist__shelf">
      <Shelf title={t('artist.similar')} kind="round">
        {artists.map((a) => (
          <ArtistCard key={a.Id} artist={a} />
        ))}
      </Shelf>
    </div>
  )
}
