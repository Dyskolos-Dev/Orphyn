import { Heart, Play, Radio, Shuffle, User } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Artwork, Button, IconButton, Text } from '@renderer/design'
import { useToggleFavorite, type BaseItemDto } from '@renderer/jellyfin'
import { startRadio } from '@renderer/features/mixes'
import { useArtistImage } from '../../library/useArtistImage'

interface Props {
  artist: BaseItemDto
  onPlay(): void
  onShuffle(): void
}

/** Artist header: large round picture, name, genres and play / shuffle / radio / favourite. */
export function ArtistHeader({ artist, onPlay, onShuffle }: Props) {
  const { t } = useTranslation('music')
  const { t: tr } = useTranslation('mixes')
  const fav = useToggleFavorite()
  const { src } = useArtistImage(artist, { maxWidth: 480, eager: true })
  const id = artist.Id ?? ''
  const favorite = !!artist.UserData?.IsFavorite
  return (
    <header className="jrm-detail jrm-artist__header">
      <Artwork
        src={src}
        ratio="1/1"
        shape="circle"
        shadow
        priority
        fallback={<User />}
        className="jrm-detail__art jrm-artist__photo"
      />
      <div className="jrm-detail__info">
        <Text variant="callout" color="secondary">
          {t('artist.eyebrow')}
        </Text>
        <Text variant="title-1" className="jrm-detail__title" data-page-title="">
          {artist.Name}
        </Text>
        {artist.Genres && artist.Genres.length > 0 && (
          <Text variant="footnote" color="secondary" className="jrm-detail__meta">
            {artist.Genres.slice(0, 3).join(' · ')}
          </Text>
        )}
        <div className="jrm-detail__actions">
          <Button icon={<Play fill="currentColor" />} onClick={onPlay}>
            {t('actions.play')}
          </Button>
          <Button variant="tinted" icon={<Shuffle />} onClick={onShuffle}>
            {t('actions.shuffle')}
          </Button>
          <Button variant="tinted" icon={<Radio />} onClick={() => void startRadio(id)}>
            {tr('radio.start')}
          </Button>
          <IconButton
            icon={<Heart fill={favorite ? 'currentColor' : 'none'} />}
            label={favorite ? t('actions.unfavorite') : t('actions.favorite')}
            variant="gray"
            pressed={favorite}
            onClick={() => fav.mutate({ id, value: !favorite })}
          />
        </div>
      </div>
    </header>
  )
}
