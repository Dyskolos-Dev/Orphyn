/**
 * Windows caption buttons (minimize, maximize/restore, close). Electron drew them natively over
 * the page (`titleBarOverlay`); Tauri has no such overlay, so the bridge adds them in a fixed
 * box at the top right. They fill the 140px the shell already reserves on Windows
 * (`--titlebar-inset-right`), are `TITLE_BAR_HEIGHT` tall, hide in fullscreen and dim when the
 * window loses focus, like the native ones.
 */
import { TITLE_BAR_HEIGHT, type TitleBarOverlayColors, type WindowState } from '@shared/ipc/window'
import { call } from './tauri'

const ROOT_ID = 'orphyn-caption-buttons'

/** Segoe Fluent Icons / MDL2 glyphs. */
const GLYPH = { minimize: '', maximize: '', restore: '', close: '' }

const STYLE = `
#${ROOT_ID} {
  position: fixed; top: 0; right: 0; z-index: 2147483000; display: flex;
  height: ${TITLE_BAR_HEIGHT}px; align-items: flex-start;
  --caption-bg: transparent; --caption-fg: #000;
  background: var(--caption-bg); color: var(--caption-fg);
}
#${ROOT_ID}[hidden] { display: none; }
#${ROOT_ID} button {
  all: unset; box-sizing: border-box; width: 46px; height: 32px;
  display: flex; align-items: center; justify-content: center;
  font: 10px 'Segoe Fluent Icons', 'Segoe MDL2 Assets', sans-serif; color: inherit;
  transition: background-color 80ms linear;
}
#${ROOT_ID}[data-focused='false'] button { opacity: 0.45; }
#${ROOT_ID} button:hover { background: color-mix(in srgb, currentColor 9%, transparent); opacity: 1; }
#${ROOT_ID} button:active { background: color-mix(in srgb, currentColor 6%, transparent); }
#${ROOT_ID} button[data-action='close']:hover { background: #c42b1c; color: #fff; }
#${ROOT_ID} button[data-action='close']:active { background: #c84031; color: #fff; }
`

let root: HTMLElement | null = null
let pendingColors: TitleBarOverlayColors | null = null

function button(action: string, glyph: string, label: string): HTMLButtonElement {
  const b = document.createElement('button')
  b.type = 'button'
  b.dataset['action'] = action
  b.textContent = glyph
  b.setAttribute('aria-label', label)
  b.title = label
  b.tabIndex = -1
  return b
}

export function setCaptionColors(colors: TitleBarOverlayColors): void {
  pendingColors = colors
  if (!root) return
  root.style.setProperty('--caption-bg', colors.color)
  root.style.setProperty('--caption-fg', colors.symbolColor)
}

export function updateCaptionButtons(state: WindowState): void {
  if (!root) return
  root.hidden = state.isFullscreen
  root.dataset['focused'] = String(state.isFocused)
  const max = root.querySelector<HTMLButtonElement>("button[data-action='maximize']")
  if (max) {
    max.textContent = state.isMaximized ? GLYPH.restore : GLYPH.maximize
    const label = state.isMaximized ? 'Restore' : 'Maximize'
    max.setAttribute('aria-label', label)
    max.title = label
  }
}

/** Adds the buttons once the body exists. Windows only. */
export function installCaptionButtons(): void {
  const mount = () => {
    if (root || document.getElementById(ROOT_ID)) return
    const style = document.createElement('style')
    style.textContent = STYLE
    document.head.appendChild(style)
    root = document.createElement('div')
    root.id = ROOT_ID
    root.setAttribute('role', 'group')
    root.setAttribute('aria-label', 'Window')
    root.append(
      button('minimize', GLYPH.minimize, 'Minimize'),
      button('maximize', GLYPH.maximize, 'Maximize'),
      button('close', GLYPH.close, 'Close'),
    )
    root.addEventListener('click', (e) => {
      const action = (e.target as HTMLElement).closest('button')?.dataset['action']
      if (action === 'minimize') void call('window_minimize')
      else if (action === 'maximize') void call('window_toggle_maximize')
      else if (action === 'close') void call('window_close')
    })
    // A press on the buttons must not start a window drag.
    root.addEventListener('mousedown', (e) => e.stopPropagation())
    root.addEventListener('dblclick', (e) => e.stopPropagation())
    document.body.appendChild(root)
    if (pendingColors) setCaptionColors(pendingColors)
  }
  if (document.body) mount()
  else document.addEventListener('DOMContentLoaded', mount, { once: true })
}
