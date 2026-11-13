import { ipcRenderer } from 'electron'
import { APP_CHANNELS, type AppApi } from '@shared/ipc/app'

function on(channel: string, listener: () => void): () => void {
  const handler = (): void => listener()
  ipcRenderer.on(channel, handler)
  return () => {
    ipcRenderer.removeListener(channel, handler)
  }
}

export const appApi: AppApi = {
  onRefresh: (l) => on(APP_CHANNELS.refresh, l),
  onOpenSettings: (l) => on(APP_CHANNELS.openSettings, l),
}
