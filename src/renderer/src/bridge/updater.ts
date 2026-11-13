import { UPDATER_CHANNELS as C, type UpdaterApi, type UpdaterState } from '@shared/ipc/updater'
import { call, subscribe } from './tauri'

export const updaterApi: UpdaterApi = {
  getState: () => call('updater_get_state'),
  check: () => call('updater_check'),
  install: () => call('updater_install'),
  openRelease: () => call('updater_open_release'),
  setAutoUpdate: (enabled) => call('updater_set_auto_update', { enabled: Boolean(enabled) }),
  onStateChange: (listener) => subscribe<UpdaterState>(C.stateChanged, listener),
}
