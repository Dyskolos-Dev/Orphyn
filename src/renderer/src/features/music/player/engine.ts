import { PlaybackReporter, secondsToTicks, type ReportTarget } from '@renderer/jellyfin'
import { savePlayer, loadPlayer } from './persistence'
import { currentItem, nextIndex } from './queue'
import { usePlayerStore, type PlayerState } from './store'
import { trackDuration } from './track'
import type { AudioOutput, OutputEvent } from './output'
import type { Track } from './types'

/** Event other media players (video, cast) fire when they start, so music pauses. */
export const EXCLUSIVE_PLAYBACK_EVENT = 'jellyra:exclusive-playback'

export interface ExclusivePlaybackDetail {
  source: string
}

/** Minimal reporter surface the engine needs (the real one is `PlaybackReporter`). */
export interface Reporter {
  start(): Promise<void>
  pause(): Promise<void>
  resume(): Promise<void>
  tick(): Promise<void>
  stop(): Promise<void>
}

export interface EngineDeps {
  output: AudioOutput
  /** Stream URL for a track, or null when none can be built (no session). */
  resolveUrl(track: Track): string | null
  createReporter?(
    track: Track,
    getState: () => { position: number; paused: boolean },
  ): Reporter | null
  /** Called when a track is skipped because it cannot be played. */
  onUnplayable?(track: Track): void
  /** Delays before each retry of a network failure. */
  retryDelaysMs?: number[]
  storage?: Storage
  /** Seconds of playback after which the next track is preloaded. */
  preloadAfter?: number
}

/** Default reporter: Jellyfin "now playing" + progress every 10 s through PlaybackReporter. */
export function createJellyfinReporter(
  track: Track,
  getState: () => { position: number; paused: boolean },
): Reporter | null {
  if (!track.Id) return null
  const id = track.Id
  const target: ReportTarget = {
    itemId: id,
    mediaSourceId: track.MediaSources?.[0]?.Id ?? id,
    playSessionId: `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 10)}`,
    playMethod: 'DirectStream',
  }
  try {
    return new PlaybackReporter(target, () => ({
      positionTicks: secondsToTicks(getState().position),
      isPaused: getState().paused,
    }))
  } catch {
    return null // no active session
  }
}

/**
 * Connects the player store to an AudioOutput: loads tracks, near-gapless preload of the next one,
 * retries, skipping of unplayable tracks, progress reporting, persistence and exclusive playback.
 */
export class PlayerEngine {
  private output: AudioOutput
  private unsubOutput: () => void = () => {}
  private readonly unsubs: (() => void)[] = []
  private loadedId: number | null = null
  private loadedUid: string | null = null
  private preloadedUid: string | null = null
  private retries = 0
  private failures = 0
  private retryTimer: ReturnType<typeof setTimeout> | null = null
  private reporter: Reporter | null = null
  private saveTimer: ReturnType<typeof setTimeout> | null = null
  private lastSaved = 0
  private announced = false
  private readonly retryDelays: number[]
  private readonly preloadAfter: number
  private readonly storage: Storage | undefined

  constructor(private readonly deps: EngineDeps) {
    this.output = deps.output
    this.retryDelays = deps.retryDelaysMs ?? [800, 2500, 6000]
    this.preloadAfter = deps.preloadAfter ?? 1
    this.storage = deps.storage
  }

  /** Starts listening. `restore` loads the saved queue (paused). */
  start({ restore = true }: { restore?: boolean } = {}): void {
    if (restore) this.restore()
    this.bindOutput()
    this.syncVolume()
    this.unsubs.push(usePlayerStore.subscribe((s, prev) => this.onState(s, prev)))
    const onExclusive = (e: Event) => {
      const source = (e as CustomEvent<ExclusivePlaybackDetail>).detail?.source
      if (source !== 'music') usePlayerStore.getState().pause()
    }
    window.addEventListener(EXCLUSIVE_PLAYBACK_EVENT, onExclusive)
    const flush = () => this.flushSave()
    window.addEventListener('pagehide', flush)
    this.unsubs.push(
      () => window.removeEventListener(EXCLUSIVE_PLAYBACK_EVENT, onExclusive),
      () => window.removeEventListener('pagehide', flush),
    )
  }

  stop(): void {
    this.unsubs.splice(0).forEach((u) => u())
    this.unsubOutput()
    if (this.retryTimer) clearTimeout(this.retryTimer)
    if (this.saveTimer) clearTimeout(this.saveTimer)
    void this.finishReport()
    this.output.dispose()
  }

  /** Replaces the output (cast hand-over), moving the current track and position to it. */
  setOutput(next: AudioOutput): void {
    const s = usePlayerStore.getState()
    const position = this.output.position || s.position
    this.unsubOutput()
    this.output.pause()
    this.output = next
    this.bindOutput()
    this.syncVolume()
    this.loadedId = null
    if (currentItem(s) && s.status !== 'idle') {
      usePlayerStore.setState({ startAt: position, position })
      this.load(s.wantPlaying, position)
    }
  }

  getOutput(): AudioOutput {
    return this.output
  }

  // ------------------------------------------------------------------ restore / persist

  private restore(): void {
    const saved = loadPlayer(this.storage)
    if (saved) usePlayerStore.getState().restore(saved)
  }

  private scheduleSave(): void {
    if (this.saveTimer) return
    const wait = Math.max(0, 2000 - (Date.now() - this.lastSaved))
    this.saveTimer = setTimeout(() => {
      this.saveTimer = null
      this.flushSave()
    }, wait)
  }

  flushSave(): void {
    if (this.saveTimer) clearTimeout(this.saveTimer)
    this.saveTimer = null
    this.lastSaved = Date.now()
    const s = usePlayerStore.getState()
    if (s.queue.length === 0) return
    savePlayer(s, this.storage)
  }

  // ------------------------------------------------------------------ store -> output

  private onState(s: PlayerState, prev: PlayerState): void {
    if (s.volume !== prev.volume || s.muted !== prev.muted) this.syncVolume()
    if (s.loadId !== prev.loadId) {
      if (!currentItem(s)) this.unload()
      else this.load(s.autoplay, s.startAt)
    } else if (s.wantPlaying !== prev.wantPlaying) {
      this.applyIntent(s)
    }
    if (s.seekNonce !== prev.seekNonce && this.loadedId === s.loadId) {
      this.output.seek(s.position)
      void this.reporter?.tick()
    }
    if (s.queue !== prev.queue || s.index !== prev.index || s.repeat !== prev.repeat) {
      this.refreshPreload()
    }
    if (
      s.queue !== prev.queue ||
      s.index !== prev.index ||
      s.shuffle !== prev.shuffle ||
      s.repeat !== prev.repeat ||
      s.volume !== prev.volume ||
      s.muted !== prev.muted ||
      Math.floor(s.position / 5) !== Math.floor(prev.position / 5)
    ) {
      this.scheduleSave()
    }
  }

  private syncVolume(): void {
    const s = usePlayerStore.getState()
    this.output.setVolume(s.volume)
    this.output.setMuted(s.muted)
  }

  private applyIntent(s: PlayerState): void {
    if (!s.wantPlaying) {
      this.output.pause()
      return
    }
    if (this.loadedId !== s.loadId) {
      this.load(true, s.position) // restored session: first play
      return
    }
    void this.output.play()
  }

  private unload(): void {
    this.loadedId = null
    this.loadedUid = null
    this.preloadedUid = null
    this.output.pause()
    this.output.preload(null)
    void this.finishReport()
  }

  private load(autoplay: boolean, startAt: number): void {
    const s = usePlayerStore.getState()
    const entry = currentItem(s)
    if (!entry) return
    const url = this.deps.resolveUrl(entry.track)
    this.cancelRetry()
    void this.finishReport()
    this.retries = 0
    this.announced = false
    this.loadedId = s.loadId
    this.loadedUid = entry.uid
    this.preloadedUid = null // consumed by the output's gapless swap, or stale
    if (!url) {
      this.handleFailure()
      return
    }
    this.output.load(url, { startAt, autoplay })
    this.reporter =
      this.deps.createReporter?.(entry.track, () => ({
        position: this.output.position,
        paused: this.output.paused,
      })) ?? null
  }

  private async finishReport(): Promise<void> {
    const r = this.reporter
    this.reporter = null
    await r?.stop()
  }

  /** Preloads (or cancels the preload of) the track that follows the current one. */
  private refreshPreload(): void {
    const s = usePlayerStore.getState()
    if (this.loadedId !== s.loadId || s.position < this.preloadAfter) return
    const i = nextIndex(s, 'auto')
    const entry = i === null ? undefined : s.queue[i]
    if (!entry || (entry.uid === this.loadedUid && s.repeat !== 'one')) {
      if (this.preloadedUid) this.output.preload(null)
      this.preloadedUid = null
      return
    }
    if (entry.uid === this.preloadedUid) return
    const url = this.deps.resolveUrl(entry.track)
    if (!url) return
    this.preloadedUid = entry.uid
    this.output.preload(url)
  }

  // ------------------------------------------------------------------ output -> store

  private bindOutput(): void {
    this.unsubOutput = this.output.subscribe((e) => this.onOutput(e))
  }

  private onOutput(e: OutputEvent): void {
    const store = usePlayerStore.getState()
    switch (e.type) {
      case 'time': {
        const entry = currentItem(store)
        const duration = e.duration || (entry ? trackDuration(entry.track) : 0)
        store.setProgress(e.position, duration)
        if (e.position >= this.preloadAfter && this.preloadedUid === null) this.refreshPreload()
        break
      }
      case 'state': {
        if (e.state === 'loading') {
          if (store.status !== 'loading') store.setStatus('loading')
        } else if (e.state === 'playing') {
          this.failures = 0
          this.retries = 0
          store.setStatus('playing')
          if (!usePlayerStore.getState().wantPlaying) usePlayerStore.setState({ wantPlaying: true })
          if (!this.announced) {
            this.announced = true
            void this.reporter?.start()
            window.dispatchEvent(
              new CustomEvent<ExclusivePlaybackDetail>(EXCLUSIVE_PLAYBACK_EVENT, {
                detail: { source: 'music' },
              }),
            )
          } else {
            void this.reporter?.resume()
          }
        } else {
          if (store.status !== 'idle') store.setStatus('paused')
          if (store.wantPlaying && this.output.paused && this.loadedId === store.loadId) {
            // The output refused to play (autoplay policy): reflect it.
            usePlayerStore.setState({ wantPlaying: false })
          }
          void this.reporter?.pause()
        }
        break
      }
      case 'ended':
        void this.finishReport()
        store.handleEnded()
        break
      case 'error':
        this.onError(e)
        break
    }
  }

  private onError(e: Extract<OutputEvent, { type: 'error' }>): void {
    const s = usePlayerStore.getState()
    const delay = this.retryDelays[this.retries]
    const entry = currentItem(s)
    if (e.network && delay !== undefined && entry) {
      this.retries++
      s.setStatus('loading')
      const loadId = s.loadId
      const resumeAt = this.output.position || s.position
      this.cancelRetry()
      this.retryTimer = setTimeout(() => {
        this.retryTimer = null
        const cur = usePlayerStore.getState()
        if (cur.loadId !== loadId) return
        const url = this.deps.resolveUrl(entry.track)
        if (url) this.output.load(url, { startAt: resumeAt, autoplay: cur.wantPlaying })
        else this.handleFailure()
      }, delay)
      return
    }
    this.handleFailure()
  }

  private handleFailure(): void {
    const s = usePlayerStore.getState()
    const entry = currentItem(s)
    if (entry) this.deps.onUnplayable?.(entry.track)
    this.failures++
    // Give up when every entry failed in a row (server down): do not loop through the queue.
    if (this.failures >= s.queue.length) {
      this.failures = 0
      usePlayerStore.setState({ wantPlaying: false, status: 'error' })
      this.output.pause()
      return
    }
    s.skipUnplayable()
  }

  private cancelRetry(): void {
    if (this.retryTimer) clearTimeout(this.retryTimer)
    this.retryTimer = null
  }
}
