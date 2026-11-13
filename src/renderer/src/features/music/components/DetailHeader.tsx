import { Heart, Music, Play, Shuffle } from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Artwork, Button, IconButton, MenuButton, Text, type MenuItem } from '@renderer/design'
import { useToggleFavorite, type BaseItemDto } from '@renderer/jellyfin'
import { art, isFavorite } from './hooks'

interface DetailHeaderProps {
  item: BaseItemDto
  /** Replaces the plain title (e.g. an inline-editable name). */
  title?: ReactNode
  /** Artwork shown when the item has no image. */
  fallback?: ReactNode
  /** Line under the title, e.g. the artist link. */
  subtitle?: ReactNode
  meta?: ReactNode
  description?: string | null
  onPlay(): void
  onShuffle(): void
  menu?: readonly MenuItem[]
}

/** Album / playlist header: big artwork with a soft shadow, title, buttons. */
export function DetailHeader({
  item,
  title,
  fallback,
  subtitle,
  meta,
  description,
  onPlay,
  onShuffle,
  menu,
}: DetailHeaderProps) {
  const { t } = useTranslation('music')
  const fav = useToggleFavorite()
  const favorite = isFavorite(item)
  return (
    <header className="jrm-detail">
      <Artwork
        src={art(item, 232)}
        ratio="1/1"
        shape="rounded"
        shadow
        priority
        fallback={fallback ?? <Music />}
        className="jrm-detail__art"
      />
      <div className="jrm-detail__info">
        <Text variant="title-1" className="jrm-detail__title" data-page-title="">
          {title ?? item.Name}
        </Text>
        {subtitle && (
          <Text variant="title-3" className="jrm-detail__subtitle">
            {subtitle}
          </Text>
        )}
        {meta && (
          <Text variant="footnote" color="secondary" className="jrm-detail__meta">
            {meta}
          </Text>
        )}
        {description && (
          <Text variant="callout" color="secondary" className="jrm-detail__desc">
            {description}
          </Text>
        )}
        <div className="jrm-detail__actions">
          <Button icon={<Play fill="currentColor" />} onClick={onPlay}>
            {t('actions.play')}
          </Button>
          <Button variant="tinted" icon={<Shuffle />} onClick={onShuffle}>
            {t('actions.shuffle')}
          </Button>
          <IconButton
            icon={<Heart fill={favorite ? 'currentColor' : 'none'} />}
            label={favorite ? t('actions.unfavorite') : t('actions.favorite')}
            variant="gray"
            pressed={favorite}
            onClick={() => fav.mutate({ id: item.Id ?? '', value: !favorite })}
          />
          {menu && <MenuButton label={t('actions.more')} variant="gray" items={menu} />}
        </div>
      </div>
    </header>
  )
}
