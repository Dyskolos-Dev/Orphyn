/**
 * Entry of the PiP windows (`index.html?pip=music|video`). Mounted by `bridge/pip.ts` instead of
 * the whole app: a PiP window must stay light and must not start a second audio engine, Discord
 * sync or media session. It only needs the design tokens, translations, and for the video the
 * Jellyfin session.
 */
import '@renderer/styles'
import '@renderer/i18n'
import './pip.css'
import { createRoot } from 'react-dom/client'
import { applyStoredTheme, watchSystemTheme } from '@renderer/design/theme/theme'
import { applyStoredInterfaceStyle } from '@renderer/design/theme/interfaceStyle'
import { accountsApi } from '@renderer/bridge/accounts'
import type { PipKind } from '@shared/ipc/pip'
import { MusicPip } from './MusicPip'
import { VideoPip } from './VideoPip'

export function mountPip(kind: PipKind): void {
  const html = document.documentElement
  html.dataset['pip'] = kind
  // The Jellyfin session reads the accounts through `window.orphyn` (the main window gets the
  // whole bridge; a PiP window only needs this part).
  if (!window.orphyn) {
    Object.defineProperty(window, 'orphyn', {
      value: Object.freeze({ accounts: accountsApi, platform: '' }),
    })
  }
  // Theme and interface style are the main window's: they live in localStorage, which both
  // windows share, so a change made in Settings reaches the PiP through the `storage` event.
  const sync = () => {
    applyStoredTheme()
    applyStoredInterfaceStyle()
  }
  window.addEventListener('storage', sync)
  watchSystemTheme()
  const root = document.getElementById('root')
  if (!root) return
  createRoot(root).render(kind === 'music' ? <MusicPip /> : <VideoPip />)
}
