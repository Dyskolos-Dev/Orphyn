/** Registers accounts/servers IPC handlers. Owner: data-layer agent. */
import { join } from 'node:path'
import { app, ipcMain, safeStorage } from 'electron'
import { ACCOUNTS_CHANNELS as C } from '@shared/ipc/accounts'
import type { NewServerAccount } from '@shared/jellyfin'
import { AccountStore } from './store'

export { AccountStore } from './store'

let store: Promise<AccountStore> | null = null

/** Opens the store once (decrypting tokens is asynchronous). Call after `app` is ready. */
export function getAccountStore(): Promise<AccountStore> {
  store ??= AccountStore.open({
    filePath: join(app.getPath('userData'), 'accounts.json'),
    storage: safeStorage,
  })
  return store
}

/** Resolves once the store is open and every handler is registered: await it before loading a window. */
export async function registerAccountsIpc(): Promise<void> {
  const s = await getAccountStore()
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
