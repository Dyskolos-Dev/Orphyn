import type { CastDevice, CastMedia, CastSessionState } from '@shared/cast'
import { log } from './log'
import { CastSession, type SessionOptions } from './session'
import type { CastTransport } from './transport'

export interface ManagerEvents {
  onDevicesChanged(devices: CastDevice[]): void
  onSessionChanged(state: CastSessionState): void
  onEnded(deviceId: string, media: CastMedia | null): void
  onError(deviceId: string | null, message: string): void
}

export interface DiscoveryControl {
  start(): void
  stop(): void
}

export interface ManagerDeps {
  discovery: DiscoveryControl
  listDevices(): CastDevice[]
  getDevice(id: string): CastDevice | undefined
  /** Builds the protocol transport for a device (null when the device cannot be controlled). */
  createTransport(device: CastDevice): CastTransport | null
  events: ManagerEvents
  sessionOptions?: SessionOptions
}

/** Owns one CastSession per device and the discovery on/off policy. */
export class CastManager {
  private readonly sessions = new Map<string, CastSession>()
  private rendererWantsDiscovery = false
  private startupTimer: ReturnType<typeof setTimeout> | null = null
  private disposed = false

  constructor(private readonly deps: ManagerDeps) {}

  /** Short scan at app startup so the device list is warm; stops unless the renderer asked for more. */
  startupScan(durationMs = 8000): void {
    if (this.disposed) return
    this.deps.discovery.start()
    this.startupTimer = setTimeout(() => {
      this.startupTimer = null
      if (!this.rendererWantsDiscovery) this.deps.discovery.stop()
    }, durationMs)
  }

  startDiscovery(): void {
    if (this.disposed) return
    this.rendererWantsDiscovery = true
    this.deps.discovery.start()
  }

  stopDiscovery(): void {
    this.rendererWantsDiscovery = false
    // Keep running if the startup scan is still in progress; it stops itself.
    if (!this.startupTimer) this.deps.discovery.stop()
  }

  listDevices(): CastDevice[] {
    return this.deps.listDevices()
  }

  getSession(deviceId: string): CastSessionState | null {
    return this.sessions.get(deviceId)?.getState() ?? null
  }

  async load(deviceId: string, media: CastMedia): Promise<void> {
    const session = this.sessionFor(deviceId, true)
    const device = this.deps.getDevice(deviceId)
    if (device && media.kind === 'video' && !device.capabilities.video) {
      throw new Error(`${device.name} cannot play video`)
    }
    if (device && !device.capabilities.audio && !device.capabilities.video) {
      throw new Error(`${device.name} is not supported for playback`)
    }
    await session.load(media)
  }

  async play(deviceId: string): Promise<void> {
    await this.sessionFor(deviceId).play()
  }
  async pause(deviceId: string): Promise<void> {
    await this.sessionFor(deviceId).pause()
  }
  async seek(deviceId: string, positionSec: number): Promise<void> {
    await this.sessionFor(deviceId).seek(positionSec)
  }
  async stop(deviceId: string): Promise<void> {
    await this.sessionFor(deviceId).stop()
  }
  async setVolume(deviceId: string, volume: number): Promise<void> {
    await this.sessionFor(deviceId).setVolume(volume)
  }
  async setMuted(deviceId: string, muted: boolean): Promise<void> {
    await this.sessionFor(deviceId).setMuted(muted)
  }

  async disconnect(deviceId: string): Promise<void> {
    const s = this.sessions.get(deviceId)
    if (!s) return
    await s.disconnect()
    this.sessions.delete(deviceId)
  }

  /** App quit: stop discovery, close every transport, emit nothing more. */
  async dispose(): Promise<void> {
    this.disposed = true
    if (this.startupTimer) clearTimeout(this.startupTimer)
    this.startupTimer = null
    this.deps.discovery.stop()
    const all = [...this.sessions.values()]
    this.sessions.clear()
    await Promise.allSettled(all.map((s) => s.dispose()))
  }

  private sessionFor(deviceId: string, create = false): CastSession {
    if (this.disposed) throw new Error('Casting is shut down')
    const existing = this.sessions.get(deviceId)
    if (existing) return existing
    if (!create) throw new Error(`No active session for ${deviceId}`)
    const device = this.deps.getDevice(deviceId)
    if (!device) throw new Error(`Device ${deviceId} is not available`)
    const session = new CastSession(
      deviceId,
      () => {
        const t = this.deps.createTransport(device)
        if (!t) throw new Error(`${device.name} cannot be controlled`)
        return t
      },
      {
        onState: (s) => this.deps.events.onSessionChanged(s),
        onEnded: (id, media) => this.deps.events.onEnded(id, media),
        onError: (id, message) => this.deps.events.onError(id, message),
      },
      this.deps.sessionOptions,
    )
    this.sessions.set(deviceId, session)
    log.info(`session created for ${device.name} (${device.kind})`)
    return session
  }
}
