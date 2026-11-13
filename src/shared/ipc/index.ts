import type { AccountsApi } from './accounts'
import type { CastApi } from './cast'
import type { WindowApi } from './window'

export * from './accounts'
export * from './cast'
export * from './window'

export interface LumenApi {
  window: WindowApi
  accounts: AccountsApi
  cast: CastApi
  platform: string
}
