import { Radio } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { MenuItem } from '@renderer/design'
import { startRadio } from './radioPlayer'

/** The "Start Radio" menu entry for a song, album or artist id. */
export function useRadioMenuItem(): (id: string) => MenuItem {
  const { t } = useTranslation('mixes')
  return (id) => ({
    label: t('radio.start'),
    icon: <Radio />,
    onSelect: () => void startRadio(id),
  })
}
