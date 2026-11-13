import { createIdleState, type CastMedia, type CastSessionState } from '@shared/cast'
import { errorMessage, log } from './log'
import { withTimeout, type CastTransport, type TransportUpdate } from './transport'

export interface SessionHooks {
  onState(state: CastSessionState): void
  onEnded(deviceId: string, media: CastMedia | null): void
  onError(deviceId: string, message: string): void
}

export interface SessionOptions {
  /** Delays before each reconnect attempt after an unexpected link loss. */
  reconnectDelaysMs?: number[]
  connectTimeoutMs?: number
  commandTimeoutMs?: number
}

const DEFAULTS: Required<SessionOptions> = {
  reconnectDelaysMs: [500, 2000, 5000],
  connectTimeoutMs: 10_000,
  commandTimeoutMs: 10_000,
}

/**
 * Per-device session: owns one transport, serializes commands, reduces transport updates into a
 * CastSessionState, emits "ended" once per track and reconnects when the link drops mid-playback.
 */
export class CastSession {
  private state: CastSessionState
  private transport: CastTransport | null = null
  private connected = false
  private disposed = false
  private chain: Promise<unknown> = Promise.resolve()
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private generation = 0
  private readonly opts: Required<SessionOptions>

  constructor(
    readonly deviceId: string,
    private readonly createTransport: () => CastTransport,
    private readonly hooks: SessionHooks,
    opts: SessionOptions = {},
  ) {
    this.state = createIdleState(deviceId)
    this.opts = { ...DEFAULTS, ...opts }
  }

  getState(): CastSessionState {
    return this.state
  }

  // --- commands ---------------------------------------------------------------------------------

  load(media: CastMedia): Promise<void> {
    return this.enqueue(async () => {
      this.generation++
      this.cancelReconnect()
      this.patch({
        status: 'connecting',
        media,
        positionSec: media.startPositionSec ?? 0,
        durationSec: media.durationSec ?? 0,
        error: undefined,
      })
      try {
        await this.ensureConnected()
        this.patch({ status: 'buffering' })
        await this.call(this.requireTransport().load(media), 'load')
      } catch (err) {
        this.fail(err)
        throw err
      }
    })
  }

  play(): Promise<void> {
    return this.command('play', (t) => t.play())
  }

  pause(): Promise<void> {
    return this.command('pause', async (t) => {
      await t.pause()
      this.patch({ status: 'paused' })
    })
  }

  seek(positionSec: number): Promise<void> {
    const pos = Math.max(0, positionSec)
    return this.command('seek', async (t) => {
      await t.seek(pos)
      this.patch({ positionSec: pos })
    })
  }

  stop(): Promise<void> {
    return this.enqueue(async () => {
      this.generation++
      this.cancelReconnect()
      const t = this.transport
      this.patch({ status: 'idle', media: null, positionSec: 0, durationSec: 0 })
      if (t && this.connected) {
        try {
          await this.call(t.stop(), 'stop')
        } catch (err) {
          log.warn(`stop failed on ${this.deviceId}:`, errorMessage(err))
        }
      }
    })
  }

  setVolume(volume: number): Promise<void> {
    const v = Math.min(1, Math.max(0, volume))
    return this.command('setVolume', async (t) => {
      await t.setVolume(v)
      this.patch({ volume: v })
    })
  }

  setMuted(muted: boolean): Promise<void> {
    return this.command('setMuted', async (t) => {
      await t.setMuted(muted)
      this.patch({ muted })
    })
  }

  /** Stops playback on the device (best effort), closes the transport and releases everything. */
  disconnect(): Promise<void> {
    return this.enqueue(async () => {
      this.generation++
      this.cancelReconnect()
      const t = this.transport
      if (t && this.connected && this.state.media) {
        try {
          await this.call(t.stop(), 'stop')
        } catch {
          // device already gone: nothing to do
        }
      }
      await this.closeTransport()
      this.patch({ status: 'idle', media: null, positionSec: 0, durationSec: 0, error: undefined })
    })
  }

  /** Synchronous-ish teardown for app quit: no further events are emitted. */
  async dispose(): Promise<void> {
    this.disposed = true
    this.cancelReconnect()
    await this.closeTransport()
  }

  // --- internals --------------------------------------------------------------------------------

  private enqueue<T>(fn: () => Promise<T>): Promise<T> {
    const run = this.chain.then(fn, fn)
    // The chain itself must never reject (no unhandled rejections); callers get the real promise.
    this.chain = run.catch(() => undefined)
    return run
  }

  private command(name: string, fn: (t: CastTransport) => Promise<void>): Promise<void> {
    return this.enqueue(async () => {
      if (!this.transport || !this.connected) throw new Error(`Not connected to ${this.deviceId}`)
      try {
        await this.call(fn(this.transport), name)
      } catch (err) {
        this.hooks.onError(this.deviceId, `${name} failed: ${errorMessage(err)}`)
        throw err
      }
    })
  }

  private call<T>(p: Promise<T>, what: string): Promise<T> {
    return withTimeout(p, this.opts.commandTimeoutMs, what)
  }

  private requireTransport(): CastTransport {
    if (!this.transport) throw new Error('No transport')
    return this.transport
  }

  private async ensureConnected(): Promise<void> {
    if (this.transport && this.connected) return
    await this.closeTransport()
    const t = this.createTransport()
    this.transport = t
    t.setHandlers({
      onUpdate: (u) => {
        if (this.transport === t) this.applyUpdate(u)
      },
      onClosed: (err) => {
        if (this.transport === t) this.handleClosed(err)
      },
    })
    await withTimeout(t.connect(), this.opts.connectTimeoutMs, 'connect')
    if (this.transport !== t || this.disposed) throw new Error('Session closed while connecting')
    this.connected = true
  }

  private async closeTransport(): Promise<void> {
    const t = this.transport
    this.transport = null
    this.connected = false
    if (t) {
      try {
        await t.close()
      } catch (err) {
        log.warn(`close failed on ${this.deviceId}:`, errorMessage(err))
      }
    }
  }

  private applyUpdate(u: TransportUpdate): void {
    // After stop() / before load() nothing should revive the session.
    if (!this.state.media) return
    if (u.error) {
      this.fail(new Error(u.error))
      return
    }
    if (u.ended) {
      const media = this.state.media
      this.patch({ status: 'idle', media: null, positionSec: 0, durationSec: 0 })
      if (!this.disposed) this.hooks.onEnded(this.deviceId, media)
      return
    }
    const next: Partial<CastSessionState> = {}
    if (u.status !== undefined) {
      // A transient 'idle' while a load is in flight is not a state change we want to show.
      if (!(u.status === 'idle' && this.state.status === 'buffering')) next.status = u.status
    }
    if (u.positionSec !== undefined) next.positionSec = u.positionSec
    if (u.durationSec !== undefined && u.durationSec > 0) next.durationSec = u.durationSec
    if (u.volume !== undefined) next.volume = u.volume
    if (u.muted !== undefined) next.muted = u.muted
    this.patch(next)
  }

  private handleClosed(err?: Error): void {
    const wasActive = this.state.media !== null && this.state.status !== 'idle'
    const media = this.state.media
    const resumePaused = this.state.status === 'paused'
    this.connected = false
    void this.closeTransport()
    if (!wasActive || !media) {
      this.patch({ status: 'idle' })
      return
    }
    log.warn(`link to ${this.deviceId} lost${err ? `: ${err.message}` : ''}, reconnecting`)
    this.patch({ status: 'connecting' })
    this.scheduleReconnect(
      0,
      this.generation,
      { ...media, startPositionSec: this.state.positionSec },
      resumePaused,
    )
  }

  private scheduleReconnect(
    attempt: number,
    gen: number,
    media: CastMedia,
    resumePaused: boolean,
  ): void {
    const delay = this.opts.reconnectDelaysMs[attempt]
    if (delay === undefined) {
      this.fail(new Error('Device unreachable'))
      return
    }
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null
      void this.enqueue(async () => {
        if (gen !== this.generation || this.disposed) return
        try {
          await this.ensureConnected()
          await this.call(this.requireTransport().load(media), 'load')
          if (resumePaused) await this.call(this.requireTransport().pause(), 'pause')
          this.patch({ status: resumePaused ? 'paused' : 'buffering' })
        } catch (err) {
          log.warn(`reconnect ${attempt + 1} to ${this.deviceId} failed:`, errorMessage(err))
          await this.closeTransport()
          if (gen === this.generation) this.scheduleReconnect(attempt + 1, gen, media, resumePaused)
        }
      })
    }, delay)
  }

  private cancelReconnect(): void {
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer)
    this.reconnectTimer = null
  }

  private fail(err: unknown): void {
    const message = errorMessage(err)
    log.error(`${this.deviceId}: ${message}`)
    this.patch({ status: 'error', error: message })
    if (!this.disposed) this.hooks.onError(this.deviceId, message)
  }

  private patch(p: Partial<CastSessionState>): void {
    const next = { ...this.state, ...p }
    if (JSON.stringify(next) === JSON.stringify(this.state)) return
    this.state = next
    if (!this.disposed) this.hooks.onState(next)
  }
}
