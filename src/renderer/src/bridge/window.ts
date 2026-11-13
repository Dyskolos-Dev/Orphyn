import {
  WINDOW_CHANNELS,
  type TitleBarOverlayColors,
  type WindowApi,
  type WindowState,
} from '@shared/ipc/window'
import { setCaptionColors } from './captionButtons'
import { call, subscribe } from './tauri'

const CSS_COLOR = /^(#[0-9a-f]{3,8}|rgba?\([\d\s.,%]+\)|transparent)$/i

export const windowApi: WindowApi = {
  minimize: () => call('window_minimize'),
  toggleMaximize: () => call('window_toggle_maximize'),
  close: () => call('window_close'),
  getState: () => call<WindowState>('window_get_state'),
  isFullscreen: async () => (await call<WindowState>('window_get_state')).isFullscreen,
  setFullscreen: (fullscreen) => call('window_set_fullscreen', { fullscreen: Boolean(fullscreen) }),
  // The Windows caption buttons are drawn by the bridge (see captionButtons.ts): recolour them.
  setTitleBarOverlay: async (colors: TitleBarOverlayColors) => {
    const color = String(colors?.color ?? '')
    const symbolColor = String(colors?.symbolColor ?? '')
    if (CSS_COLOR.test(color) && CSS_COLOR.test(symbolColor)) {
      setCaptionColors({ color, symbolColor })
    }
  },
  setAppearance: (appearance) => call('window_set_appearance', { appearance }),
  onStateChange: (listener) => subscribe<WindowState>(WINDOW_CHANNELS.stateChanged, listener),
}
