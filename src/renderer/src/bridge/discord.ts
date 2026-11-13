/** Discord bridge: the Rust commands are stubs until the Discord migration task fills `src-tauri/src/discord/`. */
import { DISCORD_CHANNELS as C, type DiscordApi, type DiscordState } from '@shared/ipc/discord'
import { call, subscribe } from './tauri'

export const discordApi: DiscordApi = {
  getState: () => call('discord_get_state'),
  setPresence: (presence) => call('discord_set_presence', { presence }),
  setSettings: (settings) => call('discord_set_settings', { settings }),
  onStateChange: (listener) => subscribe<DiscordState>(C.stateChanged, listener),
}
