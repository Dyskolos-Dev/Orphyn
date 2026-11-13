import { ipcRenderer, type IpcRendererEvent } from 'electron'
import { CAST_CHANNELS, type CastApi } from '@shared/ipc/cast'

function subscribe<T>(channel: string, cb: (payload: T) => void): () => void {
  const listener = (_e: IpcRendererEvent, payload: T): void => cb(payload)
  ipcRenderer.on(channel, listener)
  return () => {
    ipcRenderer.removeListener(channel, listener)
  }
}

export const castApi: CastApi = {
  startDiscovery: () => ipcRenderer.invoke(CAST_CHANNELS.startDiscovery),
  stopDiscovery: () => ipcRenderer.invoke(CAST_CHANNELS.stopDiscovery),
  listDevices: () => ipcRenderer.invoke(CAST_CHANNELS.listDevices),
  getSession: (deviceId) => ipcRenderer.invoke(CAST_CHANNELS.getSession, deviceId),
  load: (deviceId, media) => ipcRenderer.invoke(CAST_CHANNELS.load, deviceId, media),
  play: (deviceId) => ipcRenderer.invoke(CAST_CHANNELS.play, deviceId),
  pause: (deviceId) => ipcRenderer.invoke(CAST_CHANNELS.pause, deviceId),
  seek: (deviceId, positionSec) => ipcRenderer.invoke(CAST_CHANNELS.seek, deviceId, positionSec),
  stop: (deviceId) => ipcRenderer.invoke(CAST_CHANNELS.stop, deviceId),
  setVolume: (deviceId, volume) => ipcRenderer.invoke(CAST_CHANNELS.setVolume, deviceId, volume),
  setMuted: (deviceId, muted) => ipcRenderer.invoke(CAST_CHANNELS.setMuted, deviceId, muted),
  disconnect: (deviceId) => ipcRenderer.invoke(CAST_CHANNELS.disconnect, deviceId),
  onDevicesChanged: (cb) => subscribe(CAST_CHANNELS.devicesChanged, cb),
  onSessionChanged: (cb) => subscribe(CAST_CHANNELS.sessionChanged, cb),
  onEnded: (cb) => subscribe(CAST_CHANNELS.ended, cb),
  onError: (cb) => subscribe(CAST_CHANNELS.error, cb),
}
