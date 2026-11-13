/** Picture-in-picture bridge: the Rust commands are stubs until the PiP task fills `src-tauri/src/pip/`. */
import { PIP_CHANNELS as C, type PipApi, type PipState } from '@shared/ipc/pip'
import { call, subscribe } from './tauri'

export const pipApi: PipApi = {
  getState: () => call('pip_get_state'),
  open: (kind) => call('pip_open', { kind }),
  close: () => call('pip_close'),
  onStateChange: (listener) => subscribe<PipState>(C.stateChanged, listener),
}
