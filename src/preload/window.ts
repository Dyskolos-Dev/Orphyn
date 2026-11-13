import { ipcRenderer, type IpcRendererEvent } from 'electron'
import { WINDOW_CHANNELS, type WindowApi, type WindowState } from '@shared/ipc/window'

export const windowApi: WindowApi = {
  minimize: () => ipcRenderer.invoke(WINDOW_CHANNELS.minimize),
  toggleMaximize: () => ipcRenderer.invoke(WINDOW_CHANNELS.toggleMaximize),
  close: () => ipcRenderer.invoke(WINDOW_CHANNELS.close),
  getState: () => ipcRenderer.invoke(WINDOW_CHANNELS.getState),
  isFullscreen: async () =>
    ((await ipcRenderer.invoke(WINDOW_CHANNELS.getState)) as WindowState).isFullscreen,
  setFullscreen: (fullscreen) =>
    ipcRenderer.invoke(WINDOW_CHANNELS.setFullscreen, Boolean(fullscreen)),
  setTitleBarOverlay: (colors) =>
    ipcRenderer.invoke(WINDOW_CHANNELS.setTitleBarOverlay, {
      color: String(colors.color),
      symbolColor: String(colors.symbolColor),
    }),
  setAppearance: (appearance) => ipcRenderer.invoke(WINDOW_CHANNELS.setAppearance, appearance),
  onStateChange: (listener) => {
    const handler = (_event: IpcRendererEvent, state: WindowState) => listener(state)
    ipcRenderer.on(WINDOW_CHANNELS.stateChanged, handler)
    return () => {
      ipcRenderer.removeListener(WINDOW_CHANNELS.stateChanged, handler)
    }
  },
}
