import { PictureInPicture2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { IconButton, type IconButtonProps } from '@renderer/design'
import type { PipKind } from '@shared/ipc/pip'
import { pipAvailable, togglePip, usePipState } from './events'

type Props = Omit<IconButtonProps, 'icon' | 'label' | 'onClick' | 'pressed'> & { kind: PipKind }

/** Opens (or closes) the music PiP window. Renders nothing where the shell has no PiP. */
export function PipButton({ kind, ...rest }: Props) {
  const { t } = useTranslation('pip')
  const state = usePipState()
  if (!pipAvailable()) return null
  const showing = state.open && state.kind === kind
  return (
    <IconButton
      {...rest}
      icon={<PictureInPicture2 />}
      label={showing ? t('close') : t(kind === 'music' ? 'openMusic' : 'openVideo')}
      pressed={showing}
      onClick={() => void togglePip(kind, state)}
    />
  )
}
