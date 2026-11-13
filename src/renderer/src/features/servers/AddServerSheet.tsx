import { useTranslation } from 'react-i18next'
import { Sheet } from '@renderer/design'
import { ConnectFlow } from './ConnectFlow'

/** The onboarding flow in a Sheet, to add another server. Closing it discards the draft. */
export function AddServerSheet({ open, onClose }: { open: boolean; onClose(): void }) {
  const { t } = useTranslation('servers')
  return (
    <Sheet
      open={open}
      onClose={onClose}
      size="small"
      title={t('add.title')}
      description={t('add.description')}
    >
      <ConnectFlow variant="sheet" onDone={onClose} />
    </Sheet>
  )
}
