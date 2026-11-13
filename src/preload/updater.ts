import { ipcRenderer, type IpcRendererEvent } from 'electron'
import { UPDATER_CHANNELS as C, type UpdaterApi, type UpdaterState } from '@shared/ipc/updater'

export const updaterApi: UpdaterApi = {
  getState: () => ipcRenderer.invoke(C.getState),
  check: () => ipcRenderer.invoke(C.check),
  install: () => ipcRenderer.invoke(C.install),
  openRelease: () => ipcRenderer.invoke(C.openRelease),
  setAutoUpdate: (enabled) => ipcRenderer.invoke(C.setAutoUpdate, Boolean(enabled)),
  onStateChange: (listener) => {
    const handler = (_e: IpcRendererEvent, state: UpdaterState) => listener(state)
    ipcRenderer.on(C.stateChanged, handler)
    return () => {
      ipcRenderer.removeListener(C.stateChanged, handler)
    }
  },
}
