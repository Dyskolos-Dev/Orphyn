import { ipcRenderer } from 'electron'
import { ACCOUNTS_CHANNELS as C, type AccountsApi } from '@shared/ipc/accounts'

export const accountsApi: AccountsApi = {
  list: () => ipcRenderer.invoke(C.list),
  add: (input) => ipcRenderer.invoke(C.add, input),
  rename: (id, name) => ipcRenderer.invoke(C.rename, id, name),
  remove: (id) => ipcRenderer.invoke(C.remove, id),
  setActive: (id) => ipcRenderer.invoke(C.setActive, id),
  getActive: () => ipcRenderer.invoke(C.getActive),
  getToken: (id) => ipcRenderer.invoke(C.getToken, id),
  getDeviceId: () => ipcRenderer.invoke(C.getDeviceId),
  getStatus: () => ipcRenderer.invoke(C.getStatus),
}
