/**
 * Server clock estimation for SyncPlay. Each `/GetUtcTime` round trip gives one NTP-style sample:
 *
 *   offset = ((received - requestSent) + (sent - responseReceived)) / 2   (server − local)
 *   delay  = (responseReceived - requestSent) - (sent - received)          (network round trip)
 *
 * Samples with the smallest delay are the most trustworthy (least queueing), so the estimate is
 * the median offset of the best ones. A burst of samples on start, then one now and then.
 */

export interface TimeSample {
  /** Server time minus local time (ms). */
  offset: number
  /** Round-trip network delay (ms). */
  delay: number
}

export function timeSample(
  requestSent: number,
  serverReceived: number,
  serverSent: number,
  responseReceived: number,
): TimeSample {
  return {
    offset: (serverReceived - requestSent + (serverSent - responseReceived)) / 2,
    delay: Math.max(0, responseReceived - requestSent - (serverSent - serverReceived)),
  }
}

export function median(values: readonly number[]): number {
  if (values.length === 0) return 0
  const sorted = [...values].sort((a, b) => a - b)
  const mid = sorted.length >> 1
  const hi = sorted[mid] ?? 0
  return sorted.length % 2 ? hi : ((sorted[mid - 1] ?? hi) + hi) / 2
}

export class ClockOffsetEstimator {
  private samples: TimeSample[] = []

  /**
   * @param maxSamples samples kept (oldest dropped)
   * @param bestFraction share of the lowest-delay samples the median is taken over
   */
  constructor(
    private readonly maxSamples = 16,
    private readonly bestFraction = 0.5,
  ) {}

  add(sample: TimeSample): void {
    if (!Number.isFinite(sample.offset) || !Number.isFinite(sample.delay)) return
    this.samples.push(sample)
    if (this.samples.length > this.maxSamples) this.samples.shift()
  }

  get count(): number {
    return this.samples.length
  }

  private best(): TimeSample[] {
    const n = Math.max(1, Math.ceil(this.samples.length * this.bestFraction))
    return [...this.samples].sort((a, b) => a.delay - b.delay).slice(0, n)
  }

  /** Estimated server − local offset (ms); 0 before the first sample. */
  get offset(): number {
    return this.samples.length ? median(this.best().map((s) => s.offset)) : 0
  }

  /** Typical round trip of the good samples (ms). */
  get delay(): number {
    return this.samples.length ? median(this.best().map((s) => s.delay)) : 0
  }

  reset(): void {
    this.samples = []
  }
}

export interface TimeSyncOptions {
  /** One `/GetUtcTime` request: when the server received it and sent the answer (server ms). */
  fetchUtc(): Promise<{ received: number; sent: number }>
  now?: () => number
  /** Samples taken right away, `burstGapMs` apart. */
  burst?: number
  burstGapMs?: number
  /** Then one sample every `refreshMs`. */
  refreshMs?: number
  onUpdate?(estimator: ClockOffsetEstimator): void
}

/** Keeps a `ClockOffsetEstimator` fed while started. */
export class TimeSync {
  readonly estimator = new ClockOffsetEstimator()
  private timer: ReturnType<typeof setTimeout> | null = null
  private running = false
  private readonly now: () => number

  constructor(private readonly opts: TimeSyncOptions) {
    this.now = opts.now ?? Date.now
  }

  get offset(): number {
    return this.estimator.offset
  }

  get ready(): boolean {
    return this.estimator.count > 0
  }

  /** Local time (ms) → server time. */
  toServer(localMs: number): number {
    return localMs + this.offset
  }

  /** Server time (ms) → local time. */
  toLocal(serverMs: number): number {
    return serverMs - this.offset
  }

  serverNow(): number {
    return this.toServer(this.now())
  }

  start(): void {
    if (this.running) return
    this.running = true
    this.run(this.opts.burst ?? 8)
  }

  stop(): void {
    this.running = false
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
  }

  /** Throws the old samples away and starts a new burst (after a reconnection). */
  restart(): void {
    this.stop()
    this.estimator.reset()
    this.start()
  }

  /** Takes one sample now. */
  async sample(): Promise<void> {
    const t0 = this.now()
    const { received, sent } = await this.opts.fetchUtc()
    const t3 = this.now()
    this.estimator.add(timeSample(t0, received, sent, t3))
    this.opts.onUpdate?.(this.estimator)
  }

  private run(burstLeft: number): void {
    if (!this.running) return
    void this.sample()
      .catch(() => undefined)
      .finally(() => {
        if (!this.running) return
        const gap = burstLeft > 1 ? (this.opts.burstGapMs ?? 300) : (this.opts.refreshMs ?? 30_000)
        this.timer = setTimeout(() => this.run(Math.max(0, burstLeft - 1)), gap)
      })
  }
}
