/**
 * Native bridge. Imported first by `main.tsx`: when the page runs inside the Tauri shell it
 * installs `window.orphyn` (`OrphynApi`), backed by Rust commands (`invoke`) and events
 * (`listen`). In a plain browser and in tests it does nothing. See `docs/TAURI.md`.
 */
import type { OrphynApi } from '@shared/ipc'
import type { WindowState } from '@shared/ipc/window'
import { getShellInfo } from '@shared/webview'
import { appApi, installAppShortcuts } from './app'
import { accountsApi } from './accounts'
import { installCaptionButtons, updateCaptionButtons } from './captionButtons'
import { castApi } from './cast'
import { discordApi } from './discord'
import { installDragRegions } from './dragRegions'
import { mediaControlsApi } from './mediaControls'
import { pipApi } from './pip'
import { updaterApi } from './updater'
import { windowApi } from './window'

export { getShellInfo } from '@shared/webview'

export function createTauriApi(platform: string): OrphynApi {
  return {
    window: windowApi,
    accounts: accountsApi,
    app: appApi,
    cast: castApi,
    discord: discordApi,
    updater: updaterApi,
    platform,
    pip: pipApi,
    mediaControls: mediaControlsApi,
  }
}

/** Installs the bridge when running in the Tauri shell. Returns true when it did. */
export function installTauriBridge(): boolean {
  const shell = getShellInfo()
  if (!shell || typeof window === 'undefined' || window.orphyn) return false
  const api = createTauriApi(shell.platform)
  Object.defineProperty(window, 'orphyn', { value: Object.freeze(api), enumerable: true })

  installDragRegions()
  installAppShortcuts(shell.platform)
  if (shell.platform === 'win32') {
    installCaptionButtons()
    const sync = (s: WindowState) => updateCaptionButtons(s)
    void api.window.getState().then(sync, () => undefined)
    api.window.onStateChange(sync)
  }
  return true
}

installTauriBridge()
