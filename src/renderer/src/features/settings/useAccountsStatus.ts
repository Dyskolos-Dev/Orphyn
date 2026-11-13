import { useEffect, useState } from 'react'
import type { AccountsStatus } from '@shared/jellyfin'

/** Whether tokens are encrypted on disk (null while loading / outside Electron). */
export function useAccountsStatus(): AccountsStatus | null {
  const [status, setStatus] = useState<AccountsStatus | null>(null)
  useEffect(() => {
    let alive = true
    window.orphyn?.accounts
      ?.getStatus()
      .then((s) => alive && setStatus(s))
      .catch(() => undefined)
    return () => {
      alive = false
    }
  }, [])
  return status
}
