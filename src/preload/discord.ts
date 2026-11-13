import { ipcRenderer, type IpcRendererEvent } from 'electron'
import { DISCORD_CHANNELS as C, type DiscordApi, type DiscordState } from '@shared/ipc/discord'

export const discordApi: DiscordApi = {
  getState: () => ipcRenderer.invoke(C.getState),
  setPresence: (presence) => ipcRenderer.invoke(C.setPresence, presence),
  setSettings: (settings) => ipcRenderer.invoke(C.setSettings, settings),
  onStateChange: (listener) => {
    const handler = (_e: IpcRendererEvent, state: DiscordState) => listener(state)
    ipcRenderer.on(C.stateChanged, handler)
    return () => {
      ipcRenderer.removeListener(C.stateChanged, handler)
    }
  },
}
