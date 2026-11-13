/**
 * Session store: which server is active, and the SDK `Api` instance bound to it.
 * Switching servers swaps the Api and clears the TanStack Query cache; a 401 flags the account as
 * needing a new login. Wrap the app once in <JellyfinProvider>.
 */
import { useEffect, type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Api } from '@jellyfin/sdk'
import { create } from 'zustand'
import type { ServerAccount } from '@shared/jellyfin'
import { logoutAccount } from './auth'
import { JellyfinHttpError, createJellyfinApi, setUnauthorizedHandler } from './client'

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      gcTime: 5 * 60_000,
      refetchOnWindowFocus: false,
      retry: (count, error) => {
        const status = error instanceof JellyfinHttpError ? error.status : undefined
        if (status !== undefined && status >= 400 && status < 500) return false
        return count < 2
      },
    },
  },
})

export type SessionStatus = 'idle' | 'loading' | 'ready' | 'no-server' | 'needs-login'

export interface SessionState {
  status: SessionStatus
  accounts: ServerAccount[]
  /** Active account (null when none). */
  account: ServerAccount | null
  /** SDK Api for the active account, ready to use; null when none or login required. */
  api: Api | null
  deviceId: string | null
  /** Account ids whose token was rejected (401) or is missing. */
  needsLogin: Record<string, true>

  /** Load accounts from main and activate the stored active one. Call once at startup. */
  init: () => Promise<void>
  refreshAccounts: () => Promise<void>
  /** Make `id` the active server (persisted) and reset all cached data. */
  switchServer: (id: string) => Promise<void>
  /** Make a just-added account active (after connectServer). */
  activate: (account: ServerAccount) => Promise<void>
  /** Revoke the session on the server and forget the account. */
  signOut: (id: string) => Promise<void>
  markNeedsLogin: (id: string) => void
}

const accountsApi = () => window.jellyra.accounts

export const useSession = create<SessionState>((set, get) => {
  /** Monotonic token so a slow activation cannot overwrite a newer one. */
  let activation = 0

  async function applyActive(account: ServerAccount | null): Promise<void> {
    const mine = ++activation
    if (!account) {
      await queryClient.cancelQueries()
      queryClient.clear()
      set({ account: null, api: null, status: 'no-server' })
      return
    }
    const [token, deviceId] = await Promise.all([
      accountsApi().getToken(account.id),
      get().deviceId ?? accountsApi().getDeviceId(),
    ])
    if (mine !== activation) return
    await queryClient.cancelQueries()
    queryClient.clear()
    if (!token) {
      set((s) => ({
        account,
        api: null,
        deviceId,
        status: 'needs-login',
        needsLogin: { ...s.needsLogin, [account.id]: true },
      }))
      return
    }
    set((s) => {
      const needsLogin = { ...s.needsLogin }
      delete needsLogin[account.id]
      return {
        account,
        deviceId,
        api: createJellyfinApi({ baseUrl: account.url, deviceId, accessToken: token }),
        status: 'ready',
        needsLogin,
      }
    })
  }

  return {
    status: 'idle',
    accounts: [],
    account: null,
    api: null,
    deviceId: null,
    needsLogin: {},

    async init() {
      set({ status: 'loading' })
      const [accounts, active, deviceId] = await Promise.all([
        accountsApi().list(),
        accountsApi().getActive(),
        accountsApi().getDeviceId(),
      ])
      set({ accounts, deviceId })
      await applyActive(active)
    },

    async refreshAccounts() {
      set({ accounts: await accountsApi().list() })
    },

    async switchServer(id) {
      const account = await accountsApi().setActive(id)
      await get().refreshAccounts()
      await applyActive(account)
    },

    async activate(account) {
      await accountsApi().setActive(account.id)
      await get().refreshAccounts()
      await applyActive(account)
    },

    async signOut(id) {
      const account = get().accounts.find((a) => a.id === id)
      if (account) await logoutAccount(account)
      else await accountsApi().remove(id)
      await get().refreshAccounts()
      if (get().account?.id === id) await applyActive(await accountsApi().getActive())
    },

    markNeedsLogin(id) {
      set((s) => ({
        needsLogin: { ...s.needsLogin, [id]: true },
        ...(s.account?.id === id ? { status: 'needs-login' as const } : {}),
      }))
    },
  }
})

// A 401 from the active server's Api marks that account (ignores stale Api instances).
setUnauthorizedHandler((api) => {
  const { account, api: current, markNeedsLogin } = useSession.getState()
  if (account && current === api) markNeedsLogin(account.id)
})

/** Active Api or throws; for imperative helpers (playback, reporting). */
export function requireApi(): Api {
  const { api } = useSession.getState()
  if (!api) throw new Error('No active Jellyfin session')
  return api
}

/** Provides the query client and boots the session store once. Wrap the app with it. */
export function JellyfinProvider({ children }: { children: ReactNode }) {
  useEffect(() => {
    if (useSession.getState().status === 'idle') void useSession.getState().init()
  }, [])
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
}
