import { contextBridge } from 'electron'
import type { LumenApi } from '@shared/ipc'
import { accountsApi } from './accounts'
import { castApi } from './cast'
import { windowApi } from './window'

const api: LumenApi = {
  window: windowApi,
  accounts: accountsApi,
  cast: castApi,
  platform: process.platform,
}

contextBridge.exposeInMainWorld('lumen', api)
