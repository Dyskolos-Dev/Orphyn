/**
 * Picture-in-picture bridge (`src-tauri/src/pip/`): opens and closes the PiP windows.
 *
 * A PiP window is the same renderer loaded with `?pip=music|video`. `bridge/` is the first thing
 * `main.tsx` imports, so this file is also where such a window takes over: it mounts the light
 * PiP UI and then keeps the module graph pending, which means the rest of the app (`App`, the
 * audio engine, Discord sync, media session) never starts in that window.
 */
import { PIP_CHANNELS as C, type PipApi, type PipKind, type PipState } from '@shared/ipc/pip'
import { call, subscribe } from './tauri'

export const pipApi: PipApi = {
  getState: () => call('pip_get_state'),
  open: (kind) => call('pip_open', { kind }),
  close: () => call('pip_close'),
  onStateChange: (listener) => subscribe<PipState>(C.stateChanged, listener),
}

/** The PiP kind this window was opened for, or null for the main window. */
export function pipKindOfWindow(
  search: string = globalThis.location?.search ?? '',
): PipKind | null {
  const kind = new URLSearchParams(search).get('pip')
  return kind === 'music' || kind === 'video' ? kind : null
}

const pipKind =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window ? pipKindOfWindow() : null
if (pipKind) {
  const { mountPip } = await import('@renderer/features/pip/entry')
  mountPip(pipKind)
  // Never resolves: the app below this import must not load in a PiP window.
  await new Promise<never>(() => undefined)
}
