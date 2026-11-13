import { contextBridge } from 'electron'
import type { LumenApi } from '@shared/ipc'
import { accountsApi } from './accounts'
import { appApi } from './app'
import { castApi } from './cast'
import { discordApi } from './discord'
import { updaterApi } from './updater'
import { windowApi } from './window'

const api: LumenApi = {
  window: windowApi,
  accounts: accountsApi,
  app: appApi,
  cast: castApi,
  discord: discordApi,
  updater: updaterApi,
  platform: process.platform,
}

contextBridge.exposeInMainWorld('lumen', api)
