import { Heart } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { IconButton, type IconButtonProps } from '@renderer/design'
import { useIsFavorite, useToggleFavorite } from '@renderer/jellyfin'
import { useCurrentTrack } from './api'

/**
 * Favourite heart of the track that is playing (mini player, Now Playing). The toggle is
 * optimistic: `useToggleFavorite` patches every cached copy of the item before the server answers.
 */
export function CurrentTrackHeart(props: Omit<IconButtonProps, 'icon' | 'label' | 'onClick'>) {
  const { t } = useTranslation('music')
  const track = useCurrentTrack()
  const id = track?.Id ?? undefined
  const favorite = useIsFavorite(id)
  const toggle = useToggleFavorite()
  if (!id) return null
  return (
    <IconButton
      {...props}
      icon={<Heart fill={favorite ? 'currentColor' : 'none'} />}
      label={favorite ? t('actions.unfavorite') : t('actions.favorite')}
      pressed={favorite}
      style={favorite ? { color: 'var(--accent)', ...props.style } : props.style}
      onClick={() => toggle.mutate({ id, value: !favorite })}
    />
  )
}
