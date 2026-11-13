import { contextBridge } from 'electron'
import type { JellyraApi } from '@shared/ipc'
import { accountsApi } from './accounts'
import { castApi } from './cast'
import { windowApi } from './window'

const api: JellyraApi = {
  window: windowApi,
  accounts: accountsApi,
  cast: castApi,
  platform: process.platform,
}

contextBridge.exposeInMainWorld('jellyra', api)
