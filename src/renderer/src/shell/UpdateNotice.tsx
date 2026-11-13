import { RefreshCw } from 'lucide-react'
import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { dismissToast, toast } from '@renderer/design'
import { useUpdater } from '@renderer/features/settings/useUpdater'

/** Apple-style toast: "Lumen X is ready. Restart to update" or "Lumen X is available". */
export function UpdateNotice() {
  const { t } = useTranslation('shell')
  const state = useUpdater()
  const status = state?.status
  const version = state?.version

  useEffect(() => {
    if (!version) return
    const updater = window.lumen.updater
    let id: number | null = null
    if (status === 'ready') {
      id = toast({
        icon: <RefreshCw size={18} />,
        title: t('update.ready', { version }),
        description: t('update.readyHint'),
        duration: Infinity,
        action: { label: t('update.restart'), onClick: () => void updater.install() },
        dismissLabel: t('update.later'),
      })
    } else if (status === 'available') {
      id = toast({
        icon: <RefreshCw size={18} />,
        title: t('update.available', { version }),
        duration: Infinity,
        action: { label: t('update.download'), onClick: () => void updater.openRelease() },
        dismissLabel: t('update.later'),
      })
    }
    return () => {
      if (id !== null) dismissToast(id)
    }
  }, [status, version, t])
  return null
}
