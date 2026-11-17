/**
 * Audio output abstraction. The engine only talks to this interface, so the cast feature can swap
 * in a remote output (Chromecast / DLNA) later through `engine.setOutput()` without touching the
 * queue, the store or the UI.
 */

import { createWebAudioGraph, fadeGains, type FadeGraph } from './crossfade'

export type OutputEvent =
  | { type: 'time'; position: number; duration: number }
  /** `loading` = buffering / waiting for data, `playing` / `paused` = the real output state. */
  | { type: 'state'; state: 'loading' | 'playing' | 'paused' }
  | { type: 'ended' }
  /** The next track started fading in: it is now the current one (the output already switched). */
  | { type: 'crossfade' }
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
  /**
   * Crossfade length in seconds for the preloaded next track (0 = plain gapless switch). Outputs
   * that cannot mix two streams (cast) leave it out, which means "never crossfade".
   */
  setCrossfade?(seconds: number): void
  readonly position: number
  readonly duration: number
  readonly paused: boolean
  subscribe(listener: OutputListener): () => void
  dispose(): void
}

const MEDIA_ERR_ABORTED = 1
const MEDIA_ERR_NETWORK = 2

interface FadeState {
  /** The element fading out. */
  out: HTMLAudioElement
  /** Source of the element fading in (the new current track). */
  src: string
  /** Length of the whole fade, and of the segment currently scheduled. */
  total: number
  length: number
  /** Progress (0..1) at which the current segment starts. */
  from: number
  startedAt: number
  timer: ReturnType<typeof setTimeout> | null
  frozen: boolean
}

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
  private graph: FadeGraph | null
  private readonly graphInjected: boolean
  private fadeSeconds = 0
  private fade: FadeState | null = null
  /** A preload asked for while the idle element was fading out; applied when the fade ends. */
  private pendingPreload: string | null | undefined

  constructor(
    factory: () => HTMLAudioElement = () => new Audio(),
    graph: FadeGraph | null | undefined = undefined,
  ) {
    this.graph = graph ?? null
    this.graphInjected = graph !== undefined
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
    on('timeupdate', () => {
      this.emit({ type: 'time', position: el.currentTime, duration: this.dur(el) })
      this.maybeStartFade()
    })
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
    if (this.fade && this.fade.src === src && startAt === 0) {
      // The crossfade already started this track: keep it going instead of reloading.
      queueMicrotask(() =>
        this.emit({ type: 'state', state: this.active.paused ? 'paused' : 'playing' }),
      )
      return
    }
    this.cancelFade()
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
      this.wire(this.active)
      this.active.src = src
      this.pendingStart = startAt
      this.emit({ type: 'state', state: 'loading' })
    }
    this.active.muted = this.muted
    if (autoplay) void this.play()
  }

  preload(src: string | null): void {
    if (this.fade) {
      this.pendingPreload = src // the idle element is still fading out
      return
    }
    if (src === this.preloadedSrc) return
    this.preloadedSrc = src
    if (src === null) {
      this.clear(this.idle)
      return
    }
    this.wire(this.idle)
    this.idle.src = src
    this.idle.load()
  }

  async play(): Promise<void> {
    try {
      // Only wait for the audio clock when this element is routed through it.
      if (this.graph?.isAttached(this.active)) await this.graph.resume()
      if (this.fade?.frozen) this.resumeFade()
      await this.active.play()
    } catch (e) {
      // AbortError: a new load interrupted this play(). NotAllowedError: no user gesture yet.
      if (e instanceof DOMException && e.name === 'NotAllowedError') {
        this.emit({ type: 'state', state: 'paused' })
      }
    }
  }

  pause(): void {
    this.freezeFade()
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

  setCrossfade(seconds: number): void {
    this.fadeSeconds = Number.isFinite(seconds) ? Math.max(0, seconds) : 0
    if (this.fadeSeconds > 0 && !this.graphInjected && !this.graph) {
      this.graph = createWebAudioGraph()
    }
    // Wire the idle element now (it has no source yet), so the next preload can be mixed.
    if (!this.fade) this.wire(this.idle)
  }

  /** Routes an element through the shared graph. Only done while it has no source. */
  private wire(el: HTMLAudioElement): void {
    if (this.fadeSeconds > 0 && this.graph && !el.getAttribute('src')) this.graph.attach(el)
  }

  private maybeStartFade(): void {
    const g = this.graph
    const el = this.active
    if (!g || this.fade || this.fadeSeconds <= 0 || !this.preloadedSrc || el.paused) return
    if (!g.isAttached(el) || !g.isAttached(this.idle) || this.idle.readyState < 2) return
    const remaining = this.dur(el) - el.currentTime
    if (!(remaining > 0) || remaining > this.fadeSeconds) return
    this.startFade(Math.min(this.fadeSeconds, remaining))
  }

  private startFade(length: number): void {
    const g = this.graph
    if (!g || !this.preloadedSrc) return
    const out = this.active
    const incoming = this.idle
    this.active = incoming
    this.idle = out
    this.fade = {
      out,
      src: this.preloadedSrc,
      total: length,
      length,
      from: 0,
      startedAt: g.now(),
      timer: setTimeout(() => this.endFade(), length * 1000),
      frozen: false,
    }
    this.preloadedSrc = null
    this.pendingPreload = undefined
    g.setGain(incoming, 0)
    g.setGain(out, 1)
    g.ramp(out, incoming, 0, length)
    incoming.muted = this.muted
    void incoming.play()
    this.emit({ type: 'crossfade' })
  }

  private endFade(): void {
    const f = this.fade
    if (!f) return
    this.dropFade(f)
    const pending = this.pendingPreload
    this.pendingPreload = undefined
    if (pending !== undefined) this.preload(pending)
  }

  /** Abandons a fade in progress (a different track was requested): the outgoing one is dropped. */
  private cancelFade(): void {
    const f = this.fade
    if (!f) return
    this.dropFade(f)
    this.pendingPreload = undefined
  }

  private dropFade(f: FadeState): void {
    if (f.timer) clearTimeout(f.timer)
    this.fade = null
    this.clear(f.out)
    this.graph?.setGain(f.out, 1)
    this.graph?.setGain(this.active, 1)
  }

  /** Pause during a fade: both elements stop, the gains stay where they were. */
  private freezeFade(): void {
    const f = this.fade
    if (!f || f.frozen || !this.graph) return
    const done = Math.min(1, Math.max(0, (this.graph.now() - f.startedAt) / f.length))
    f.from = f.from + (1 - f.from) * done
    f.frozen = true
    if (f.timer) clearTimeout(f.timer)
    f.timer = null
    f.out.pause()
    const g = fadeGains(f.from)
    this.graph.setGain(f.out, g.out)
    this.graph.setGain(this.active, g.in)
  }

  private resumeFade(): void {
    const f = this.fade
    const g = this.graph
    if (!f || !g) return
    f.frozen = false
    f.length = Math.max(0.05, f.total * (1 - f.from))
    f.startedAt = g.now()
    g.ramp(f.out, this.active, f.from, f.length)
    f.timer = setTimeout(() => this.endFade(), f.length * 1000)
    void f.out.play()
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
    this.cancelFade()
    this.cleanups.forEach((c) => c())
    this.cleanups = []
    this.listeners.clear()
    this.clear(this.active)
    this.clear(this.idle)
  }
}
