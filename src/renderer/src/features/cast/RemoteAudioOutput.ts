import type { CastMedia, CastSessionState } from '@shared/cast'
import type { CastApi } from '@shared/ipc'
import type {
  AudioOutput,
  LoadOptions,
  OutputEvent,
  OutputListener,
} from '@renderer/features/music'

export interface RemoteOutputDeps {
  cast: CastApi
  deviceId: string
  /** Media (cast URL + metadata + artwork) of what the engine is loading right now. */
  describe(src: string): CastMedia | null
  /**
   * The device failed (refused the media, link lost for good). When given, the failure is handed
   * here instead of being emitted as an output error, so the engine does not skip tracks because a
   * device is unreachable.
   */
  onFailure?(message: string): void
}

const message = (e: unknown): string => (e instanceof Error ? e.message : String(e))

/**
 * AudioOutput on top of a cast session: the music engine keeps owning the queue, the store and the
 * Jellyfin reporting; this class only turns its calls into cast commands and the session events
 * (position, state, "ended") back into OutputEvents. No gapless preload on a remote device.
 */
export class RemoteAudioOutput implements AudioOutput {
  private readonly listeners = new Set<OutputListener>()
  private readonly offs: (() => void)[] = []
  private url: string | null = null
  private gen = 0
  private _position = 0
  private _duration = 0
  private _paused = true
  private lastStatus: CastSessionState['status'] | null = null

  constructor(private readonly deps: RemoteOutputDeps) {
    const { cast, deviceId } = deps
    this.offs.push(
      cast.onSessionChanged((s) => {
        if (s.deviceId === deviceId) this.onSession(s)
      }),
      cast.onEnded((e) => {
        if (e.deviceId !== deviceId || !this.url) return
        if (e.media && e.media.url !== this.url) return // a previous track
        this._paused = true
        this.emit({ type: 'ended' })
      }),
    )
  }

  get deviceId(): string {
    return this.deps.deviceId
  }

  private emit(event: OutputEvent): void {
    for (const l of [...this.listeners]) l(event)
  }

  private onSession(s: CastSessionState): void {
    if (!this.url) return
    // Ignore state still describing the previous media while a load is in flight.
    if (s.media && s.media.url !== this.url) return
    if (s.status === 'idle') return
    if (s.status === 'error') {
      this.fail(s.error ?? 'Cast error')
      return
    }
    if (s.status === 'playing' || s.status === 'paused') {
      this._position = s.positionSec
      if (s.durationSec > 0) this._duration = s.durationSec
      this.emit({ type: 'time', position: this._position, duration: this._duration })
    }
    if (s.status === this.lastStatus) return
    this.lastStatus = s.status
    if (s.status === 'playing') {
      this._paused = false
      this.emit({ type: 'state', state: 'playing' })
    } else if (s.status === 'paused') {
      this._paused = true
      this.emit({ type: 'state', state: 'paused' })
    } else {
      this.emit({ type: 'state', state: 'loading' })
    }
  }

  private fail(e: unknown): void {
    const text = message(e)
    if (this.deps.onFailure) this.deps.onFailure(text)
    else this.emit({ type: 'error', network: false, message: text })
  }

  load(src: string, { startAt = 0, autoplay = true }: LoadOptions = {}): void {
    const media = this.deps.describe(src)
    if (!media) {
      this.fail(new Error('No media'))
      return
    }
    const gen = ++this.gen
    this.url = media.url
    this._position = startAt
    this._duration = media.durationSec ?? 0
    this._paused = !autoplay
    this.lastStatus = null
    this.emit({ type: 'state', state: 'loading' })
    const { cast, deviceId } = this.deps
    cast
      .load(deviceId, { ...media, startPositionSec: startAt })
      .then(() => (gen === this.gen && !autoplay ? cast.pause(deviceId) : undefined))
      .catch((e) => gen === this.gen && this.fail(e))
  }

  preload(): void {
    /* no gapless on a remote device */
  }

  async play(): Promise<void> {
    this._paused = false
    try {
      await this.deps.cast.play(this.deps.deviceId)
    } catch (e) {
      this._paused = true
      this.fail(e)
    }
  }

  pause(): void {
    this._paused = true
    this.deps.cast.pause(this.deps.deviceId).catch((e) => this.fail(e))
  }

  seek(seconds: number): void {
    this._position = Math.max(0, seconds)
    this.deps.cast.seek(this.deps.deviceId, this._position).catch((e) => this.fail(e))
  }

  setVolume(volume: number): void {
    // Some receivers (AirPlay) have no remote volume: not an error worth showing.
    this.deps.cast.setVolume(this.deps.deviceId, volume).catch(() => undefined)
  }

  setMuted(muted: boolean): void {
    this.deps.cast.setMuted(this.deps.deviceId, muted).catch(() => undefined)
  }

  get position(): number {
    return this._position
  }
  get duration(): number {
    return this._duration
  }
  get paused(): boolean {
    return this._paused
  }

  subscribe(listener: OutputListener): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  /** Releases the listeners; does not stop the device (use `stopRemote`). */
  dispose(): void {
    this.offs.splice(0).forEach((o) => o())
    this.listeners.clear()
    this.gen++
  }

  /** Stops playback on the device and releases the connection. */
  async stopRemote(): Promise<void> {
    this.gen++
    await this.deps.cast.disconnect(this.deps.deviceId).catch(() => undefined)
  }
}
