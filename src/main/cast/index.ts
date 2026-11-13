import { app, BrowserWindow, ipcMain } from 'electron'
import { CAST_CHANNELS, type CastEndedEvent, type CastErrorEvent } from '@shared/ipc/cast'
import type { CastMedia } from '@shared/cast'
import { errorMessage, log } from './log'
import type { CastManager } from './manager'
import { createCastService } from './service'

function broadcast(channel: string, payload: unknown): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed() && !win.webContents.isDestroyed()) win.webContents.send(channel, payload)
  }
}

function isMedia(value: unknown): value is CastMedia {
  if (!value || typeof value !== 'object') return false
  const m = value as Record<string, unknown>
  return (
    typeof m.url === 'string' &&
    /^https?:\/\//i.test(m.url) &&
    typeof m.mimeType === 'string' &&
    (m.kind === 'audio' || m.kind === 'video') &&
    typeof m.title === 'string'
  )
}

let manager: CastManager | null = null

/** Registers cast IPC handlers. Owner: cast agent. */
export function registerCastIpc(): void {
  if (manager) return
  const m = createCastService({
    onDevicesChanged: (devices) => broadcast(CAST_CHANNELS.devicesChanged, devices),
    onSessionChanged: (state) => broadcast(CAST_CHANNELS.sessionChanged, state),
    onEnded: (deviceId, media) =>
      broadcast(CAST_CHANNELS.ended, { deviceId, media } satisfies CastEndedEvent),
    onError: (deviceId, message) =>
      broadcast(CAST_CHANNELS.error, { deviceId, message } satisfies CastErrorEvent),
  })
  manager = m

  const handle = (channel: string, fn: (...args: never[]) => unknown): void => {
    ipcMain.handle(channel, async (_e, ...args: unknown[]) => {
      try {
        return await (fn as (...a: unknown[]) => unknown)(...args)
      } catch (err) {
        log.warn(`${channel} failed:`, errorMessage(err))
        throw new Error(errorMessage(err), { cause: err })
      }
    })
  }
  const str = (v: unknown): string => {
    if (typeof v !== 'string') throw new Error('Invalid argument')
    return v
  }
  const num = (v: unknown): number => {
    if (typeof v !== 'number' || !Number.isFinite(v)) throw new Error('Invalid argument')
    return v
  }

  handle(CAST_CHANNELS.startDiscovery, () => m.startDiscovery())
  handle(CAST_CHANNELS.stopDiscovery, () => m.stopDiscovery())
  handle(CAST_CHANNELS.listDevices, () => m.listDevices())
  handle(CAST_CHANNELS.getSession, (id: unknown) => m.getSession(str(id)))
  handle(CAST_CHANNELS.load, (id: unknown, media: unknown) => {
    if (!isMedia(media)) throw new Error('Invalid media')
    return m.load(str(id), media)
  })
  handle(CAST_CHANNELS.play, (id: unknown) => m.play(str(id)))
  handle(CAST_CHANNELS.pause, (id: unknown) => m.pause(str(id)))
  handle(CAST_CHANNELS.seek, (id: unknown, pos: unknown) => m.seek(str(id), num(pos)))
  handle(CAST_CHANNELS.stop, (id: unknown) => m.stop(str(id)))
  handle(CAST_CHANNELS.setVolume, (id: unknown, v: unknown) => m.setVolume(str(id), num(v)))
  handle(CAST_CHANNELS.setMuted, (id: unknown, v: unknown) => m.setMuted(str(id), v === true))
  handle(CAST_CHANNELS.disconnect, (id: unknown) => m.disconnect(str(id)))

  // Short startup scan so the first device list is already warm.
  m.startupScan()

  app.once('before-quit', () => {
    void m.dispose().catch((e: unknown) => log.warn('dispose failed:', errorMessage(e)))
  })
}
