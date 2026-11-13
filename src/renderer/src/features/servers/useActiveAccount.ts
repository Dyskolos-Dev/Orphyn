import { useSession } from '@renderer/jellyfin'
import type { ServerAccount } from '@shared/jellyfin'

/**
 * The active account, taken from the fresh accounts list so a rename shows up immediately (the
 * session store keeps the object it activated with).
 */
export function useActiveAccount(): ServerAccount | null {
  const active = useSession((s) => s.account)
  const accounts = useSession((s) => s.accounts)
  return (active && accounts.find((a) => a.id === active.id)) || active
}
