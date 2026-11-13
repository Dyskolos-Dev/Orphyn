import type { AccountsApi } from './accounts'
import type { AppApi } from './app'
import type { CastApi } from './cast'
import type { UpdaterApi } from './updater'
import type { WindowApi } from './window'

export * from './accounts'
export * from './app'
export * from './cast'
export * from './updater'
export * from './window'

export interface LumenApi {
  window: WindowApi
  accounts: AccountsApi
  app: AppApi
  cast: CastApi
  updater: UpdaterApi
  platform: string
}
