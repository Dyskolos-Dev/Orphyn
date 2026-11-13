import { useEffect, type ReactNode } from 'react'
import { Navigate, useLocation } from 'react-router'
import { Spinner } from '@renderer/design'
import { useSession } from '@renderer/jellyfin'
import './connect.css'

/**
 * Wraps the whole shell (registered as its `provider`, see register.tsx):
 * - boots the session store when nobody did,
 * - no saved server → every route goes to /onboarding,
 * - a saved server while on /onboarding → /video,
 * - the active server needs a new login → Settings › Servers (everything else is blocked).
 * Outside Electron (no `window.jellyra.accounts`) it renders its children untouched.
 */
export function ServerGuard({ children }: { children: ReactNode }) {
  const hasBridge = typeof window !== 'undefined' && Boolean(window.jellyra?.accounts)
  const status = useSession((s) => s.status)
  const { pathname } = useLocation()

  useEffect(() => {
    if (hasBridge && useSession.getState().status === 'idle') {
      void useSession
        .getState()
        .init()
        .catch(() => undefined)
    }
  }, [hasBridge])

  if (!hasBridge) return <>{children}</>
  if (status === 'idle' || status === 'loading') {
    return (
      <div className="jr-guard__splash">
        <Spinner size="large" />
      </div>
    )
  }
  const onboarding = pathname === '/onboarding'
  if (status === 'no-server' && !onboarding) return <Navigate to="/onboarding" replace />
  if (status === 'ready' && onboarding) return <Navigate to="/video" replace />
  if (status === 'needs-login' && !onboarding && !pathname.startsWith('/settings')) {
    return <Navigate to="/settings?pane=servers" replace />
  }
  return <>{children}</>
}
