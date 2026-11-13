import type { AccountsApi } from '@shared/ipc/accounts'
import { call } from './tauri'

export const accountsApi: AccountsApi = {
  list: () => call('accounts_list'),
  add: (input) => call('accounts_add', { input }),
  rename: (id, name) => call('accounts_rename', { id, name }),
  remove: (id) => call('accounts_remove', { id }),
  setActive: (id) => call('accounts_set_active', { id }),
  getActive: () => call('accounts_get_active'),
  getToken: (id) => call('accounts_get_token', { id }),
  getDeviceId: () => call('accounts_get_device_id'),
  getStatus: () => call('accounts_get_status'),
}
