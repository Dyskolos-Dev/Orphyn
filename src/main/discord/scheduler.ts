/**
 * Rate limiter for presence updates: at most one `send` per `minIntervalMs`. Calls in between
 * collapse into one trailing update carrying the latest value (the last write wins).
 */
export class UpdateScheduler<T> {
  private last = -Infinity
  private pending: { value: T } | null = null
  private timer: ReturnType<typeof setTimeout> | null = null

  constructor(
    private readonly send: (value: T) => void,
    private readonly minIntervalMs: number,
    private readonly now: () => number = Date.now,
  ) {}

  push(value: T): void {
    this.pending = { value }
    if (this.timer) return
    const wait = this.last + this.minIntervalMs - this.now()
    if (wait <= 0) this.flush()
    else {
      this.timer = setTimeout(() => {
        this.timer = null
        this.flush()
      }, wait)
      this.timer.unref?.()
    }
  }

  /** Sends right away whatever is pending (and resets the window). */
  flush(): void {
    if (this.timer) {
      clearTimeout(this.timer)
      this.timer = null
    }
    if (!this.pending) return
    const { value } = this.pending
    this.pending = null
    this.last = this.now()
    this.send(value)
  }

  cancel(): void {
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    this.pending = null
  }
}
