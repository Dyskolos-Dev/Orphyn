import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { Client } from '@xhayper/discord-rpc'
import { app, BrowserWindow, ipcMain } from 'electron'
import {
  DISCORD_CHANNELS as C,
  type DiscordPresence,
  type DiscordSettings,
} from '@shared/ipc/discord'
import type { Activity } from './activity'
import { DISCORD_CLIENT_ID, isDiscordConfigured } from './config'
import { DiscordService, type RpcClientLike } from './service'

const SETTINGS_FILE = 'discord.json'
const settingsPath = (): string => join(app.getPath('userData'), SETTINGS_FILE)

function loadSettings(): DiscordSettings {
  try {
    const raw = JSON.parse(readFileSync(settingsPath(), 'utf8')) as Partial<DiscordSettings>
    return { enabled: raw.enabled !== false, showArtwork: raw.showArtwork !== false }
  } catch {
    return { enabled: true, showArtwork: true }
  }
}

function saveSettings(settings: DiscordSettings): void {
  try {
    writeFileSync(settingsPath(), JSON.stringify(settings))
  } catch {
    // Read-only profile: the choice applies for this session.
  }
}

function createClient(clientId: string): RpcClientLike {
  const client = new Client({ clientId })
  return {
    login: () => client.login(),
    setActivity: async (activity: Activity) => {
      await client.user?.setActivity({
        ...activity,
        type: activity.type,
        startTimestamp: activity.startTimestamp,
        endTimestamp: activity.endTimestamp,
      })
    },
    clearActivity: async () => {
      await client.user?.clearActivity()
    },
    destroy: () => client.destroy(),
    on: (event, listener) => {
      client.on(event, listener)
    },
  }
}

const isPresence = (v: unknown): v is DiscordPresence =>
  typeof v === 'object' &&
  v !== null &&
  ((v as { kind?: unknown }).kind === 'music' || (v as { kind?: unknown }).kind === 'video') &&
  typeof (v as { title?: unknown }).title === 'string'

/** Sets up Discord Rich Presence and its IPC. Call once, after `app` is ready. */
export function registerDiscord(): DiscordService {
  const service = new DiscordService({
    clientId: DISCORD_CLIENT_ID,
    configured: isDiscordConfigured(DISCORD_CLIENT_ID),
    createClient,
    loadSettings,
    saveSettings,
  })
  ipcMain.handle(C.getState, () => service.getState())
  ipcMain.handle(C.setPresence, (_e, presence: unknown) =>
    service.setPresence(isPresence(presence) ? presence : null),
  )
  ipcMain.handle(C.setSettings, (_e, patch: unknown) => {
    const p = (patch ?? {}) as Partial<DiscordSettings>
    service.setSettings({
      enabled: typeof p.enabled === 'boolean' ? p.enabled : undefined,
      showArtwork: typeof p.showArtwork === 'boolean' ? p.showArtwork : undefined,
    })
  })
  service.onChange((state) => {
    for (const win of BrowserWindow.getAllWindows()) {
      if (!win.isDestroyed()) win.webContents.send(C.stateChanged, state)
    }
  })
  app.on('before-quit', () => void service.dispose())
  return service
}
