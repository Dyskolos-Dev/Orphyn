import type {
  DiscordPresence,
  DiscordSettings,
  DiscordState,
  DiscordStatus,
} from '@shared/ipc/discord'
import { buildActivity, type Activity } from './activity'
import { MIN_UPDATE_INTERVAL_MS, RECONNECT_INTERVAL_MS } from './config'
import { UpdateScheduler } from './scheduler'

/** The slice of the RPC client the service needs (the real one is @xhayper/discord-rpc). */
export interface RpcClientLike {
  login(): Promise<unknown>
  setActivity(activity: Activity): Promise<unknown>
  clearActivity(): Promise<unknown>
  destroy(): Promise<void>
  on(event: 'disconnected', listener: () => void): void
}

export interface DiscordServiceDeps {
  clientId: string
  configured: boolean
  createClient(clientId: string): RpcClientLike
  loadSettings(): DiscordSettings
  saveSettings(settings: DiscordSettings): void
  now?: () => number
}

/**
 * Rich Presence manager. Connects lazily (first presence, or when the state is read), retries every
 * 30 s while Discord is not running, rate-limits updates, and never throws or logs when Discord is
 * absent.
 */
export class DiscordService {
  private settings: DiscordSettings
  private client: RpcClientLike | null = null
  private connected = false
  private attempted = false
  private connecting: Promise<void> | null = null
  private reconnectTimer: ReturnType<typeof setInterval> | null = null
  private presence: DiscordPresence | null = null
  private readonly listeners = new Set<(state: DiscordState) => void>()
  private readonly scheduler: UpdateScheduler<Activity | null>
  private lastStatus: DiscordStatus

  constructor(private readonly deps: DiscordServiceDeps) {
    this.settings = deps.loadSettings()
    this.scheduler = new UpdateScheduler<Activity | null>(
      (activity) => void this.apply(activity),
      MIN_UPDATE_INTERVAL_MS,
      deps.now,
    )
    this.lastStatus = this.status()
  }

  private get active(): boolean {
    return this.deps.configured && this.settings.enabled
  }

  status(): DiscordStatus {
    if (!this.deps.configured) return 'not-configured'
    if (!this.settings.enabled) return 'disabled'
    if (this.connected) return 'connected'
    return this.attempted ? 'unavailable' : 'idle'
  }

  getState(): DiscordState {
    // Reading the state is a reason to find out whether Discord is there.
    if (this.active && !this.attempted) void this.connect()
    return { ...this.settings, status: this.status() }
  }

  onChange(listener: (state: DiscordState) => void): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  setPresence(presence: DiscordPresence | null): void {
    this.presence = presence
    if (!this.active) return
    const activity = buildActivity(presence, this.settings, this.deps.now?.())
    if (!activity) {
      // Clearing is not rate limited the same way: do it now so it feels instant.
      this.scheduler.cancel()
      void this.apply(null)
      return
    }
    this.scheduler.push(activity)
    void this.connect()
  }

  setSettings(patch: Partial<DiscordSettings>): void {
    const next: DiscordSettings = {
      enabled: patch.enabled ?? this.settings.enabled,
      showArtwork: patch.showArtwork ?? this.settings.showArtwork,
    }
    const wasEnabled = this.settings.enabled
    this.settings = next
    this.deps.saveSettings(next)
    if (!this.deps.configured) return this.emit()
    if (!next.enabled) {
      this.scheduler.cancel()
      void this.apply(null)
      this.stopReconnect()
    } else if (!wasEnabled || patch.showArtwork !== undefined) {
      // Rebuild the activity with the new settings, immediately.
      this.scheduler.cancel()
      const activity = buildActivity(this.presence, next, this.deps.now?.())
      if (activity) {
        void this.connect().then(() => this.apply(activity))
      }
    }
    this.emit()
  }

  async dispose(): Promise<void> {
    this.scheduler.cancel()
    this.stopReconnect()
    const client = this.client
    this.client = null
    this.connected = false
    try {
      await client?.clearActivity()
      await client?.destroy()
    } catch {
      // Discord already gone.
    }
  }

  private emit(): void {
    const state = this.getStateQuiet()
    this.lastStatus = state.status
    for (const l of this.listeners) l(state)
  }

  private getStateQuiet(): DiscordState {
    return { ...this.settings, status: this.status() }
  }

  private async apply(activity: Activity | null): Promise<void> {
    if (!this.connected || !this.client) return
    try {
      if (activity) await this.client.setActivity(activity)
      else await this.client.clearActivity()
    } catch {
      this.markDisconnected()
    }
  }

  private connect(): Promise<void> {
    if (!this.active || this.connected) return Promise.resolve()
    if (this.connecting) return this.connecting
    this.attempted = true
    this.connecting = (async () => {
      const client = this.deps.createClient(this.deps.clientId)
      try {
        client.on('disconnected', () => {
          if (this.client === client) this.markDisconnected()
        })
        await client.login()
        this.client = client
        this.connected = true
        this.stopReconnect()
        // Show whatever is playing now.
        const activity = buildActivity(this.presence, this.settings, this.deps.now?.())
        if (activity && this.active) await this.apply(activity)
      } catch {
        // Discord is not running (or refused): stay quiet and try again later.
        void client.destroy().catch(() => undefined)
        this.connected = false
        this.startReconnect()
      } finally {
        this.connecting = null
        if (this.status() !== this.lastStatus) this.emit()
      }
    })()
    return this.connecting
  }

  private markDisconnected(): void {
    const client = this.client
    this.client = null
    this.connected = false
    void client?.destroy().catch(() => undefined)
    if (this.active) this.startReconnect()
    this.emit()
  }

  private startReconnect(): void {
    if (this.reconnectTimer || !this.active) return
    this.reconnectTimer = setInterval(() => void this.connect(), RECONNECT_INTERVAL_MS)
    this.reconnectTimer.unref?.()
  }

  private stopReconnect(): void {
    if (this.reconnectTimer) clearInterval(this.reconnectTimer)
    this.reconnectTimer = null
  }
}

export { RECONNECT_INTERVAL_MS }
