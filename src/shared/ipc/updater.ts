export const UPDATER_CHANNELS = {
  getState: 'orphyn:updater:get-state',
  check: 'orphyn:updater:check',
  install: 'orphyn:updater:install',
  openRelease: 'orphyn:updater:open-release',
  setAutoUpdate: 'orphyn:updater:set-auto-update',
  stateChanged: 'orphyn:updater:state-changed',
} as const

/**
 * - `idle`: nothing checked yet. `up-to-date`: last check found nothing newer.
 * - `downloading`: in-app update (NSIS / AppImage) being fetched in the background, `percent` 0-100.
 * - `ready`: downloaded; "Restart" installs it (it also installs on quit).
 * - `available`: a newer release exists but cannot be applied in-app (macOS unsigned, .deb):
 *   the user opens the release page.
 */
export type UpdaterStatus =
  'idle' | 'checking' | 'up-to-date' | 'available' | 'downloading' | 'ready' | 'error'

export interface UpdaterState {
  status: UpdaterStatus
  currentVersion: string
  /** Version of the newer release, when known. */
  version: string | null
  percent: number
  error: string | null
  autoUpdate: boolean
  /** Updates are only checked in packaged builds. */
  supported: boolean
}

export interface UpdaterApi {
  getState(): Promise<UpdaterState>
  check(): Promise<void>
  /** Quit and install the downloaded update. */
  install(): Promise<void>
  /** Open the GitHub release page in the browser. */
  openRelease(): Promise<void>
  setAutoUpdate(enabled: boolean): Promise<void>
  onStateChange(listener: (state: UpdaterState) => void): () => void
}
