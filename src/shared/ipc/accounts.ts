/** Accounts / servers IPC contract (owner: data-layer agent). Tokens only leave main via getToken. */
import type { AccountsStatus, NewServerAccount, ServerAccount } from '../jellyfin'

export const ACCOUNTS_CHANNELS = {
  list: 'accounts:list',
  add: 'accounts:add',
  rename: 'accounts:rename',
  remove: 'accounts:remove',
  setActive: 'accounts:set-active',
  getActive: 'accounts:get-active',
  getToken: 'accounts:get-token',
  getDeviceId: 'accounts:get-device-id',
  getStatus: 'accounts:get-status',
} as const

export interface AccountsApi {
  list(): Promise<ServerAccount[]>
  /** Adds (or, for the same server+user, refreshes) an account and returns it. Never stores passwords. */
  add(input: NewServerAccount): Promise<ServerAccount>
  rename(id: string, name: string): Promise<ServerAccount | null>
  remove(id: string): Promise<void>
  setActive(id: string | null): Promise<ServerAccount | null>
  getActive(): Promise<ServerAccount | null>
  getToken(id: string): Promise<string | null>
  /** Stable per-install device id. */
  getDeviceId(): Promise<string>
  getStatus(): Promise<AccountsStatus>
}
