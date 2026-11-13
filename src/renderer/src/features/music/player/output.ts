/**
 * Audio output abstraction. The engine only talks to this interface, so the cast feature can swap
 * in a remote output (Chromecast / DLNA) later through `engine.setOutput()` without touching the
 * queue, the store or the UI.
 */

export type OutputEvent =
  | { type: 'time'; position: number; duration: number }
  /** `loading` = buffering / waiting for data, `playing` / `paused` = the real output state. */
  | { type: 'state'; state: 'loading' | 'playing' | 'paused' }
  | { type: 'ended' }
  | { type: 'error'; network: boolean; message?: string }

export type OutputListener = (event: OutputEvent) => void

export interface LoadOptions {
  /** Start position in seconds. */
  startAt?: number
  /** Start playing as soon as possible (default true). */
  autoplay?: boolean
}

export interface AudioOutput {
  /** Loads `src`. If it was handed to `preload()` before, it switches to it without a gap. */
  load(src: string, options?: LoadOptions): void
  /** Warms up the next track (or cancels with null). Local output uses a second element. */
  preload(src: string | null): void
  play(): Promise<void>
  pause(): void
  /** Seconds. */
  seek(seconds: number): void
  /** 0..1 */
  setVolume(volume: number): void
  setMuted(muted: boolean): void
  readonly position: number
  readonly duration: number
  readonly paused: boolean
  subscribe(listener: OutputListener): () => void
  dispose(): void
}

const MEDIA_ERR_ABORTED = 1
const MEDIA_ERR_NETWORK = 2

/** Local output: two HTMLAudioElements, the idle one preloading the next track. */
export class LocalAudioOutput implements AudioOutput {
  private active: HTMLAudioElement
  private idle: HTMLAudioElement
  private preloadedSrc: string | null = null
  private readonly listeners = new Set<OutputListener>()
  private volume = 1
  private muted = false
  private pendingStart = 0
  private cleanups: (() => void)[] = []

  constructor(factory: () => HTMLAudioElement = () => new Audio()) {
    this.active = this.setup(factory())
    this.idle = this.setup(factory())
  }

  private setup(el: HTMLAudioElement): HTMLAudioElement {
    el.preload = 'auto'
    const on = (name: string, fn: () => void) => {
      const handler = () => {
        if (el === this.active) fn()
      }
      el.addEventListener(name, handler)
      this.cleanups.push(() => el.removeEventListener(name, handler))
    }
    on('timeupdate', () =>
      this.emit({ type: 'time', position: el.currentTime, duration: this.dur(el) }),
    )
    on('durationchange', () =>
      this.emit({ type: 'time', position: el.currentTime, duration: this.dur(el) }),
    )
    on('playing', () => this.emit({ type: 'state', state: 'playing' }))
    on('pause', () => {
      if (!el.ended) this.emit({ type: 'state', state: 'paused' })
    })
    on('waiting', () => this.emit({ type: 'state', state: 'loading' }))
    on('ended', () => this.emit({ type: 'ended' }))
    on('loadedmetadata', () => {
      if (this.pendingStart > 0) {
        el.currentTime = this.pendingStart
        this.pendingStart = 0
      }
    })
    on('error', () => {
      const err = el.error
      if (!el.getAttribute('src') || !err || err.code === MEDIA_ERR_ABORTED) return
      this.emit({
        type: 'error',
        network: err.code === MEDIA_ERR_NETWORK,
        message: err.message || undefined,
      })
    })
    el.volume = this.volume
    return el
  }

  private dur(el: HTMLAudioElement): number {
    return Number.isFinite(el.duration) ? el.duration : 0
  }

  private emit(event: OutputEvent): void {
    for (const l of [...this.listeners]) l(event)
  }

  private clear(el: HTMLAudioElement): void {
    el.pause()
    el.removeAttribute('src')
  }

  load(src: string, { startAt = 0, autoplay = true }: LoadOptions = {}): void {
    this.pendingStart = 0
    if (this.preloadedSrc === src) {
      // Gapless path: the idle element already buffered this track.
      const old = this.active
      this.active = this.idle
      this.idle = old
      this.clear(old)
      this.preloadedSrc = null
      this.emit({ type: 'state', state: 'loading' })
      if (startAt > 0) this.active.currentTime = startAt
    } else {
      this.clear(this.active)
      this.active.src = src
      this.pendingStart = startAt
      this.emit({ type: 'state', state: 'loading' })
    }
    this.active.muted = this.muted
    if (autoplay) void this.play()
  }

  preload(src: string | null): void {
    if (src === this.preloadedSrc) return
    this.preloadedSrc = src
    if (src === null) {
      this.clear(this.idle)
      return
    }
    this.idle.src = src
    this.idle.load()
  }

  async play(): Promise<void> {
    try {
      await this.active.play()
    } catch (e) {
      // AbortError: a new load interrupted this play(). NotAllowedError: no user gesture yet.
      if (e instanceof DOMException && e.name === 'NotAllowedError') {
        this.emit({ type: 'state', state: 'paused' })
      }
    }
  }

  pause(): void {
    this.active.pause()
  }

  seek(seconds: number): void {
    const t = Math.max(0, seconds)
    if (this.active.readyState === 0) this.pendingStart = t
    else this.active.currentTime = t
  }

  setVolume(volume: number): void {
    this.volume = Math.min(1, Math.max(0, volume))
    this.active.volume = this.volume
    this.idle.volume = this.volume
  }

  setMuted(muted: boolean): void {
    this.muted = muted
    this.active.muted = muted
    this.idle.muted = muted
  }

  get position(): number {
    return this.active.currentTime
  }

  get duration(): number {
    return this.dur(this.active)
  }

  get paused(): boolean {
    return this.active.paused
  }

  subscribe(listener: OutputListener): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  dispose(): void {
    this.cleanups.forEach((c) => c())
    this.cleanups = []
    this.listeners.clear()
    this.clear(this.active)
    this.clear(this.idle)
  }
}
