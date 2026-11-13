/** Playback reporting (Jellyfin "now playing", watched state and resume position). */
import type { Api } from '@jellyfin/sdk'
import { jfRequest } from '../client'
import { requireApi } from '../session'
import type { PlaybackInfo, PlayMethod } from './info'

export interface ReportTarget {
  itemId: string
  mediaSourceId: string
  playSessionId: string
  playMethod: PlayMethod
}

export interface PlaybackState {
  positionTicks: number
  isPaused: boolean
  isMuted?: boolean
  /** 0-100 */
  volumeLevel?: number
  audioStreamIndex?: number
  subtitleStreamIndex?: number
}

export type ReportEvent =
  'pause' | 'unpause' | 'timeupdate' | 'volumechange' | 'audiotrackchange' | 'subtitletrackchange'

export const targetFromInfo = (info: PlaybackInfo): ReportTarget => ({
  itemId: info.itemId,
  mediaSourceId: info.mediaSourceId,
  playSessionId: info.playSessionId,
  playMethod: info.method,
})

function body(t: ReportTarget, s: PlaybackState, event?: ReportEvent) {
  return {
    ItemId: t.itemId,
    MediaSourceId: t.mediaSourceId,
    PlaySessionId: t.playSessionId || undefined,
    PlayMethod: t.playMethod,
    PositionTicks: Math.max(0, Math.round(s.positionTicks)),
    IsPaused: s.isPaused,
    IsMuted: s.isMuted ?? false,
    VolumeLevel: s.volumeLevel,
    AudioStreamIndex: s.audioStreamIndex,
    SubtitleStreamIndex: s.subtitleStreamIndex,
    CanSeek: true,
    EventName: event,
  }
}

export const reportPlaybackStart = (t: ReportTarget, s: PlaybackState, api: Api = requireApi()) =>
  jfRequest<void>(api, 'POST', '/Sessions/Playing', { data: body(t, s) })

export const reportPlaybackProgress = (
  t: ReportTarget,
  s: PlaybackState,
  event: ReportEvent = 'timeupdate',
  api: Api = requireApi(),
) => jfRequest<void>(api, 'POST', '/Sessions/Playing/Progress', { data: body(t, s, event) })

/** Reports a pause/resume right away (keeps the server's "paused" indicator in sync). */
export const reportPlaying = (t: ReportTarget, s: PlaybackState, api: Api = requireApi()) =>
  reportPlaybackProgress(t, s, s.isPaused ? 'pause' : 'unpause', api)

export const reportPlaybackStopped = (t: ReportTarget, s: PlaybackState, api: Api = requireApi()) =>
  jfRequest<void>(api, 'POST', '/Sessions/Playing/Stopped', { data: body(t, s) })

export interface ReporterOptions {
  /** Progress interval; Jellyfin expects roughly every 10 s. */
  intervalMs?: number
  /** Api to report to. Captured at creation so a server switch cannot misroute reports. */
  api?: Api
  onError?: (e: unknown) => void
}

/**
 * Headless reporter: `start()` on first play, `pause()/resume()` on toggles, `stop()` at the end.
 * While playing it posts progress every 10 s using `getState()` for the current position.
 * Reporting failures never throw into the player (they go to `onError`).
 */
export class PlaybackReporter {
  private timer: ReturnType<typeof setInterval> | null = null
  private started = false
  private readonly api: Api
  private readonly interval: number

  constructor(
    private readonly target: ReportTarget,
    private readonly getState: () => PlaybackState,
    private readonly opts: ReporterOptions = {},
  ) {
    this.api = opts.api ?? requireApi()
    this.interval = opts.intervalMs ?? 10_000
  }

  get running(): boolean {
    return this.started
  }

  async start(): Promise<void> {
    if (this.started) return
    this.started = true
    await this.safe(() => reportPlaybackStart(this.target, this.getState(), this.api))
    this.timer = setInterval(() => void this.tick(), this.interval)
  }

  /** Sends progress now (e.g. after a seek). */
  async tick(event: ReportEvent = 'timeupdate'): Promise<void> {
    if (!this.started) return
    await this.safe(() => reportPlaybackProgress(this.target, this.getState(), event, this.api))
  }

  async pause(): Promise<void> {
    if (!this.started) return
    await this.safe(() =>
      reportPlaying(this.target, { ...this.getState(), isPaused: true }, this.api),
    )
  }

  async resume(): Promise<void> {
    if (!this.started) return
    await this.safe(() =>
      reportPlaying(this.target, { ...this.getState(), isPaused: false }, this.api),
    )
  }

  async stop(): Promise<void> {
    if (!this.started) return
    this.started = false
    if (this.timer) clearInterval(this.timer)
    this.timer = null
    await this.safe(() => reportPlaybackStopped(this.target, this.getState(), this.api))
  }

  private async safe(run: () => Promise<unknown>): Promise<void> {
    try {
      await run()
    } catch (e) {
      this.opts.onError?.(e)
    }
  }
}
