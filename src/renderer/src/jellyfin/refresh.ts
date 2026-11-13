import type { QueryClient } from '@tanstack/react-query'
import { jfKeys } from './keys'

/** Background time after which regaining focus refetches the library. */
export const REFRESH_AFTER_BLUR_MS = 5 * 60_000

/**
 * Marks every Jellyfin query of one server stale and refetches the mounted ones. Never touches the
 * server's own library scan (users are not admins): it only re-reads.
 */
export function refreshLibrary(client: QueryClient, accountId: string | null): Promise<void> {
  if (!accountId) return Promise.resolve()
  return client.invalidateQueries({ queryKey: jfKeys.all(accountId) })
}

/** True when the window was in the background long enough to warrant a refetch. */
export function shouldRefreshOnFocus(hiddenSince: number | null, now: number): boolean {
  return hiddenSince !== null && now - hiddenSince > REFRESH_AFTER_BLUR_MS
}
