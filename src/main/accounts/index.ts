/** Registers accounts/servers IPC handlers. Owner: data-layer agent. */
import { join } from 'node:path'
import { app, ipcMain, safeStorage } from 'electron'
import { ACCOUNTS_CHANNELS as C } from '@shared/ipc/accounts'
import type { NewServerAccount } from '@shared/jellyfin'
import { AccountStore } from './store'

export { AccountStore } from './store'

let store: AccountStore | null = null

export function getAccountStore(): AccountStore {
  store ??= new AccountStore({
    filePath: join(app.getPath('userData'), 'accounts.json'),
    storage: safeStorage,
  })
  return store
}

export function registerAccountsIpc(): void {
  const s = getAccountStore()
  if (!s.encrypted) console.warn('[accounts]', s.getStatus().warning)
  ipcMain.handle(C.list, () => s.list())
  ipcMain.handle(C.add, (_e, input: NewServerAccount) => s.add(input))
  ipcMain.handle(C.rename, (_e, id: string, name: string) => s.rename(id, name))
  ipcMain.handle(C.remove, (_e, id: string) => s.remove(id))
  ipcMain.handle(C.setActive, (_e, id: string | null) => s.setActive(id))
  ipcMain.handle(C.getActive, () => s.getActive())
  ipcMain.handle(C.getToken, (_e, id: string) => s.getToken(id))
  ipcMain.handle(C.getDeviceId, () => s.getDeviceId())
  ipcMain.handle(C.getStatus, () => s.getStatus())
}
