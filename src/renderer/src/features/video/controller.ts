import { getPlaybackPreferences } from '@renderer/features/settings'
import Hls from 'hls.js'
import {
  PlaybackReporter,
  getPlaybackInfo,
  secondsToTicks,
  streamUrlForCast,
  targetFromInfo,
  ticksToSeconds,
  type MediaSegment,
  type PlaybackInfo,
  type PlaybackOptions,
  type PlaybackState,
  type PlaybackTracks,
  type PlayMethod,
} from '@renderer/jellyfin'
import { subtitleSwitchNeedsReload } from './tracks'

export type PlaybackStatus = 'idle' | 'loading' | 'playing' | 'paused' | 'ended' | 'error'

/** What the rest of the app (UI, cast agent) can observe about the video that is playing. */
export interface PlaybackSnapshot {
  itemId: string | null
  status: PlaybackStatus
  positionSeconds: number
  durationSeconds: number
  bufferedSeconds: number
  volume: number
  muted: boolean
  rate: number
  method?: PlayMethod
  tracks?: PlaybackTracks
  /** Intro / credits / recap ranges of the item; empty when the server has none. */
  segments?: MediaSegment[]
  /** Trailer playback: nothing is reported to the server, nothing is resumed. */
  trailer?: boolean
  audioIndex?: number
  /** Selected subtitle stream index; -1 / undefined = off. */
  subtitleIndex?: number
  error?: string
}

export interface PlayOptions {
  audioIndex?: number
  subtitleIndex?: number
  /** Play as a trailer: no progress reporting (start / stop / ticks). */
  trailer?: boolean
}

/**
 * Small playback controller interface. The `<video>` element logic stays behind it, so another
 * output (a Chromecast, see the cast feature) can implement it and take over.
 */
export interface PlaybackController {
  getSnapshot(): PlaybackSnapshot
  subscribe(listener: () => void): () => void
  /** Starts `itemId` at `startTicks` (Jellyfin ticks, default 0). */
  play(itemId: string, startTicks?: number, options?: PlayOptions): Promise<void>
  pause(): void
  resume(): void
  /** Seeks to an absolute position in seconds. */
  seek(seconds: number): void
  /** Stops playback and reports it to the server. */
  stop(): Promise<void>
}

interface HlsLike {
  loadSource(url: string): void
  attachMedia(el: HTMLMediaElement): void
  on(event: string, cb: (event: string, data: HlsErrorData) => void): void
  startLoad(): void
  recoverMediaError(): void
  destroy(): void
}
interface HlsErrorData {
  fatal?: boolean
  type?: string
}

interface ReporterLike {
  start(): Promise<void>
  stop(): Promise<void>
  pause(): Promise<void>
  resume(): Promise<void>
  tick(): Promise<void>
}

export interface ControllerDeps {
  getPlaybackInfo(itemId: string, opts: PlaybackOptions): Promise<PlaybackInfo>
  createReporter(info: PlaybackInfo, getState: () => PlaybackState): ReporterLike
  createHls(startPosition: number): HlsLike | null
  /** Transcoded HLS stream for an item when direct play fails. */
  transcodeFallback(info: PlaybackInfo): PlaybackInfo
  getMaxBitrate(): number | undefined
  /** Tells other players (music) to pause. */
  announceExclusive(): void
}

const HLS_NETWORK_RETRIES = 3

export const defaultControllerDeps: ControllerDeps = {
  getPlaybackInfo,
  createReporter: (info, getState) => new PlaybackReporter(targetFromInfo(info), getState),
  createHls: (startPosition) =>
    Hls.isSupported()
      ? (new Hls({
          startPosition,
          enableWorker: true,
          lowLatencyMode: false,
        }) as unknown as HlsLike)
      : null,
  transcodeFallback: (info) => ({
    ...info,
    url: streamUrlForCast(info.itemId, 'video', {
      transcode: true,
      mediaSourceId: info.mediaSourceId,
    }),
    isHls: true,
    method: 'Transcode',
  }),
  getMaxBitrate: () => getPlaybackPreferences().maxVideoBitrate ?? undefined,
  announceExclusive: () =>
    window.dispatchEvent(
      new CustomEvent('orphyn:exclusive-playback', { detail: { source: 'video' } }),
    ),
}

const IDLE: PlaybackSnapshot = {
  itemId: null,
  status: 'idle',
  positionSeconds: 0,
  durationSeconds: 0,
  bufferedSeconds: 0,
  volume: 1,
  muted: false,
  rate: 1,
}

/** Drives one `<video>` element: playback info, hls.js, resume, fallback, server reporting. */
export class VideoPlaybackController implements PlaybackController {
  private video: HTMLVideoElement | null = null
  private snapshot: PlaybackSnapshot = IDLE
  private readonly listeners = new Set<() => void>()
  private hls: HlsLike | null = null
  private reporter: ReporterLike | null = null
  private info: PlaybackInfo | null = null
  private generation = 0
  private fellBack = false
  private hlsRetries = 0
  private detach: (() => void) | null = null
  private beginListener: (() => void) | null = null
  private trailer = false

  constructor(private readonly deps: ControllerDeps = defaultControllerDeps) {}

  // ----------------------------------------------------------------- observation

  getSnapshot = (): PlaybackSnapshot => this.snapshot

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  private patch(next: Partial<PlaybackSnapshot>): void {
    this.snapshot = { ...this.snapshot, ...next }
    this.listeners.forEach((l) => l())
  }

  /** The element the controller drives (null until attached). */
  get element(): HTMLVideoElement | null {
    return this.video
  }

  // ----------------------------------------------------------------- element

  /** Binds the `<video>` element (call with null on unmount). */
  attach(video: HTMLVideoElement | null): void {
    this.detach?.()
    this.detach = null
    this.video = video
    if (!video) return
    const on = <K extends keyof HTMLMediaElementEventMap>(name: K, fn: () => void) => {
      video.addEventListener(name, fn)
      return () => video.removeEventListener(name, fn)
    }
    const offs = [
      on('timeupdate', () => this.syncTime()),
      on('durationchange', () => this.syncTime()),
      on('progress', () => this.syncTime()),
      on('loadedmetadata', () => this.syncTime()),
      on('play', () => {
        if (this.snapshot.itemId) void this.reporter?.resume()
      }),
      on('playing', () => this.patch({ status: 'playing', error: undefined })),
      on('pause', () => {
        if (video.ended || this.snapshot.status === 'loading') return
        this.patch({ status: 'paused' })
        void this.reporter?.pause()
      }),
      on('waiting', () => {
        if (this.snapshot.status === 'playing') this.patch({ status: 'loading' })
      }),
      on('seeked', () => void this.reporter?.tick()),
      on('ended', () => this.patch({ status: 'ended' })),
      on('volumechange', () => this.patch({ volume: video.volume, muted: video.muted })),
      on('ratechange', () => this.patch({ rate: video.playbackRate })),
      on('error', () => this.onMediaError()),
    ]
    this.detach = () => offs.forEach((off) => off())
    this.patch({ volume: video.volume, muted: video.muted, rate: video.playbackRate })
  }

  private syncTime(): void {
    const v = this.video
    if (!v) return
    const buffered = v.buffered
    const bufferedEnd = buffered && buffered.length > 0 ? buffered.end(buffered.length - 1) : 0
    this.patch({
      positionSeconds: v.currentTime || 0,
      durationSeconds:
        Number.isFinite(v.duration) && v.duration > 0 ? v.duration : this.knownDuration(),
      bufferedSeconds: bufferedEnd,
    })
  }

  private knownDuration(): number {
    const ticks = this.info?.mediaSource.RunTimeTicks
    return ticks ? ticksToSeconds(ticks) : 0
  }

  // ----------------------------------------------------------------- commands

  async play(itemId: string, startTicks = 0, options: PlayOptions = {}): Promise<void> {
    const gen = ++this.generation
    this.deps.announceExclusive()
    void this.closeReporter()
    this.fellBack = false
    this.hlsRetries = 0
    this.trailer = options.trailer === true
    const startSeconds = this.trailer ? 0 : ticksToSeconds(startTicks)
    this.patch({
      itemId,
      status: 'loading',
      trailer: this.trailer,
      segments: undefined,
      error: undefined,
      positionSeconds: startSeconds,
      durationSeconds: 0,
      bufferedSeconds: 0,
    })
    try {
      const info = await this.deps.getPlaybackInfo(itemId, {
        startTimeTicks: startTicks,
        audioStreamIndex: options.audioIndex,
        subtitleStreamIndex: options.subtitleIndex,
        maxBitrate: this.deps.getMaxBitrate(),
      })
      if (gen !== this.generation) return
      this.patch({
        tracks: info.tracks,
        segments: this.trailer ? [] : (info.segments ?? []),
        audioIndex: options.audioIndex ?? info.tracks.defaultAudioIndex,
        subtitleIndex: options.subtitleIndex,
      })
      this.load(info, startSeconds, gen)
    } catch (e) {
      if (gen !== this.generation) return
      this.patch({ status: 'error', error: e instanceof Error ? e.message : String(e) })
    }
  }

  private load(info: PlaybackInfo, startSeconds: number, gen: number): void {
    const video = this.video
    if (!video) {
      this.patch({ status: 'error', error: 'No video element' })
      return
    }
    this.info = info
    this.destroyHls()
    this.patch({ method: info.method })

    if (this.beginListener) video.removeEventListener('loadedmetadata', this.beginListener)
    const hlsPlayer = info.isHls ? this.deps.createHls(startSeconds) : null
    const begin = () => {
      if (gen !== this.generation) return
      // hls.js already starts at `startPosition`; only plain sources need the explicit seek.
      if (!hlsPlayer && startSeconds > 0 && Math.abs(video.currentTime - startSeconds) > 1) {
        video.currentTime = startSeconds
      }
      void video.play()?.catch(() => this.patch({ status: 'paused' }))
    }
    this.beginListener = begin
    video.addEventListener('loadedmetadata', begin, { once: true })

    const hls = hlsPlayer
    if (hls) {
      this.hls = hls
      hls.on('hlsError', (_e, data) => this.onHlsError(data))
      hls.loadSource(info.url)
      hls.attachMedia(video)
    } else {
      video.src = info.url
      video.load()
    }

    if (!this.reporter && !this.trailer) {
      this.reporter = this.deps.createReporter(info, () => this.reportState())
      void this.reporter.start()
    }
  }

  private reportState(): PlaybackState {
    const v = this.video
    return {
      positionTicks: secondsToTicks(v?.currentTime ?? this.snapshot.positionSeconds),
      isPaused: v?.paused ?? true,
      isMuted: v?.muted,
      volumeLevel: v ? Math.round(v.volume * 100) : undefined,
      audioStreamIndex: this.snapshot.audioIndex,
      subtitleStreamIndex: this.snapshot.subtitleIndex,
    }
  }

  pause(): void {
    this.video?.pause()
  }

  resume(): void {
    void this.video?.play()?.catch(() => undefined)
    this.deps.announceExclusive()
  }

  seek(seconds: number): void {
    const v = this.video
    if (!v) return
    const duration = this.snapshot.durationSeconds || v.duration || Infinity
    const target = Math.min(Math.max(0, seconds), Number.isFinite(duration) ? duration : seconds)
    v.currentTime = target
    this.patch({ positionSeconds: target })
  }

  /** Re-requests the stream at the current position with other tracks (audio, burned-in subtitles). */
  async switchTracks(options: PlayOptions): Promise<void> {
    const id = this.snapshot.itemId
    if (!id) return
    await this.play(id, secondsToTicks(this.video?.currentTime ?? this.snapshot.positionSeconds), {
      audioIndex: options.audioIndex ?? this.snapshot.audioIndex,
      subtitleIndex: options.subtitleIndex ?? this.snapshot.subtitleIndex,
      trailer: this.trailer,
    })
  }

  /**
   * Selects a subtitle stream (-1 / undefined = off). Text subtitles switch instantly (the UI
   * shows the matching VTT `<track>`); image subtitles are burned in by the server, which needs a
   * new stream request at the current position.
   */
  async selectSubtitle(index: number | undefined): Promise<void> {
    const tracks = this.snapshot.tracks
    const from = this.snapshot.subtitleIndex
    if (!tracks || !subtitleSwitchNeedsReload(tracks, from, index)) {
      this.patch({ subtitleIndex: index })
      return
    }
    const burn =
      index != null && tracks.subtitles.find((t) => t.index === index)?.burnInRequired === true
    await this.switchTracks({ subtitleIndex: burn ? index : -1 })
    if (!burn) this.patch({ subtitleIndex: index })
  }

  /** Reloads the current item at the current position (error overlay "Retry"). */
  async retry(): Promise<void> {
    await this.switchTracks({})
  }

  /**
   * Stops playback. Everything touching the element / state happens synchronously (so a
   * re-mount that plays again right away is never undone); only the server report is awaited.
   */
  async stop(): Promise<void> {
    this.generation++
    // Starts the "stopped" report now, while the element still has its final position.
    const reported = this.closeReporter()
    this.destroyHls()
    const v = this.video
    if (v) {
      v.pause()
      v.removeAttribute('src')
      v.load()
    }
    this.info = null
    this.patch({ ...IDLE, volume: this.snapshot.volume, muted: this.snapshot.muted })
    await reported
  }

  /** Stops, unbinds the element and drops every listener. Call on unmount. */
  async dispose(): Promise<void> {
    const stopped = this.stop()
    this.attach(null)
    await stopped
  }

  // ----------------------------------------------------------------- errors

  /** Detaches the reporter and starts its stop request synchronously; resolves when it is sent. */
  private closeReporter(): Promise<void> {
    const r = this.reporter
    this.reporter = null
    return r ? r.stop() : Promise.resolve()
  }

  private destroyHls(): void {
    this.hls?.destroy()
    this.hls = null
  }

  private onHlsError(data: HlsErrorData): void {
    if (!data.fatal) return
    if (data.type === 'networkError' && this.hlsRetries < HLS_NETWORK_RETRIES) {
      this.hlsRetries++
      this.hls?.startLoad()
    } else if (data.type === 'mediaError' && this.hlsRetries < HLS_NETWORK_RETRIES) {
      this.hlsRetries++
      this.hls?.recoverMediaError()
    } else {
      this.fail()
    }
  }

  private onMediaError(): void {
    // hls.js reports its own errors; a raw element error matters for direct play / remux.
    if (this.hls) return
    if (!this.video?.getAttribute('src')) return
    this.fail()
  }

  private fail(): void {
    const info = this.info
    if (info && !this.fellBack && info.method !== 'Transcode') {
      this.fellBack = true
      const position = this.video?.currentTime ?? this.snapshot.positionSeconds
      this.load(this.deps.transcodeFallback(info), position, this.generation)
      return
    }
    this.patch({ status: 'error', error: this.video?.error?.message || 'Playback failed' })
  }
}
