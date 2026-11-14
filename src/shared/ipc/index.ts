import type { AccountsApi } from './accounts'
import type { AppApi } from './app'
import type { CastApi } from './cast'
import type { DiscordApi } from './discord'
import type { MediaControlsApi } from './mediaControls'
import type { PipApi } from './pip'
import type { UpdaterApi } from './updater'
import type { WindowApi } from './window'

export * from './accounts'
export * from './app'
export * from './cast'
export * from './discord'
export * from './mediaControls'
export * from './pip'
export * from './updater'
export * from './window'

export interface OrphynApi {
  window: WindowApi
  accounts: AccountsApi
  app: AppApi
  cast: CastApi
  discord: DiscordApi
  updater: UpdaterApi
  platform: string
  /** Optional: absent outside the native shell (see `docs/TAURI.md`). */
  pip?: PipApi
  /** Optional: absent outside the native shell (see `docs/TAURI.md`). */
  mediaControls?: MediaControlsApi
}
