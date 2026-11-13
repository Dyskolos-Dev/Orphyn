/** Window-control IPC contract (owner: window agent). */
export const WINDOW_CHANNELS = {
  minimize: 'window:minimize',
  toggleMaximize: 'window:toggle-maximize',
  close: 'window:close',
  getState: 'window:get-state',
  setFullscreen: 'window:set-fullscreen',
  setTitleBarOverlay: 'window:set-title-bar-overlay',
  setAppearance: 'window:set-appearance',
  /** Main -> renderer event, payload: {@link WindowState}. */
  stateChanged: 'window:state-changed',
} as const

/**
 * Native material behind the window: macOS vibrancy, Windows 11 Mica, or none (opaque window:
 * Linux, Windows 10). When it is not `none`, the renderer leaves the sidebar transparent.
 */
export type WindowMaterial = 'vibrancy' | 'mica' | 'none'

/** Theme appearance pushed to the native side (macOS vibrancy, Mica, native menus follow it). */
export type WindowAppearance = 'system' | 'light' | 'dark'

/** Snapshot of the native window state the renderer cares about. */
export interface WindowState {
  isFullscreen: boolean
  isMaximized: boolean
  isFocused: boolean
  material: WindowMaterial
}

/** Colours of the Windows title bar overlay (the native min/max/close buttons). */
export interface TitleBarOverlayColors {
  /** Background behind the native buttons, CSS colour. Use a transparent colour over Mica. */
  color: string
  /** Glyph colour of the native buttons, CSS colour. */
  symbolColor: string
}

export interface WindowApi {
  minimize(): Promise<void>
  toggleMaximize(): Promise<void>
  close(): Promise<void>
  getState(): Promise<WindowState>
  isFullscreen(): Promise<boolean>
  /** Enter or leave native fullscreen (used by the video player). */
  setFullscreen(fullscreen: boolean): Promise<void>
  /** Windows only (no-op elsewhere): recolour the native caption buttons to match the theme. */
  setTitleBarOverlay(colors: TitleBarOverlayColors): Promise<void>
  /** Make native chrome (vibrancy, Mica, menus) follow the app theme. */
  setAppearance(appearance: WindowAppearance): Promise<void>
  /** Subscribe to fullscreen / maximize / focus changes. Returns an unsubscribe function. */
  onStateChange(listener: (state: WindowState) => void): () => void
}

/** Height of the draggable title-bar / toolbar area, shared by main (overlay height) and renderer. */
export const TITLE_BAR_HEIGHT = 52
