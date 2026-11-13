import { ipcRenderer } from 'electron'
import { WINDOW_CHANNELS, type WindowApi } from '@shared/ipc/window'

export const windowApi: WindowApi = {
  minimize: () => ipcRenderer.invoke(WINDOW_CHANNELS.minimize),
  toggleMaximize: () => ipcRenderer.invoke(WINDOW_CHANNELS.toggleMaximize),
  close: () => ipcRenderer.invoke(WINDOW_CHANNELS.close),
}
