import type { AccountsApi } from './accounts'
import type { AppApi } from './app'
import type { CastApi } from './cast'
import type { DiscordApi } from './discord'
import type { UpdaterApi } from './updater'
import type { WindowApi } from './window'

export * from './accounts'
export * from './app'
export * from './cast'
export * from './discord'
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
}
