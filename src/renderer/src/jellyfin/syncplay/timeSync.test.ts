import { afterEach, describe, expect, it, vi } from 'vitest'
import { ClockOffsetEstimator, TimeSync, median, timeSample } from './timeSync'

describe('timeSample', () => {
  it('computes offset and round trip like NTP', () => {
    // Server 5 s ahead, 40 ms each way, 2 ms of server processing.
    const s = timeSample(1000, 6040, 6042, 1082)
    expect(s.offset).toBe(5000)
    expect(s.delay).toBe(80)
  })

  it('splits an asymmetric path evenly (best effort)', () => {
    const s = timeSample(0, 1100, 1100, 120) // 100 ms out, 20 ms back, server 1000 ahead
    expect(s.offset).toBe(1040)
    expect(s.delay).toBe(120)
  })
})

describe('ClockOffsetEstimator', () => {
  it('is 0 before any sample', () => {
    expect(new ClockOffsetEstimator().offset).toBe(0)
  })

  it('takes the median offset of the lowest-delay samples', () => {
    const est = new ClockOffsetEstimator(16, 0.5)
    // Good samples (low delay) agree on ~200 ms; congested ones are way off.
    est.add({ offset: 198, delay: 20 })
    est.add({ offset: 450, delay: 900 })
    est.add({ offset: 202, delay: 22 })
    est.add({ offset: -300, delay: 700 })
    est.add({ offset: 200, delay: 25 })
    est.add({ offset: 610, delay: 1200 })
    expect(est.offset).toBe(200)
    expect(est.delay).toBe(22)
  })

  it('keeps only the newest samples and ignores junk', () => {
    const est = new ClockOffsetEstimator(3)
    est.add({ offset: 1, delay: 1 })
    est.add({ offset: NaN, delay: 1 })
    for (let i = 0; i < 3; i++) est.add({ offset: 100, delay: 50 })
    expect(est.count).toBe(3)
    expect(est.offset).toBe(100)
  })

  it('median', () => {
    expect(median([3, 1, 2])).toBe(2)
    expect(median([4, 1, 2, 3])).toBe(2.5)
    expect(median([])).toBe(0)
  })
})

describe('TimeSync', () => {
  afterEach(() => vi.useRealTimers())

  it('samples in a burst then periodically, and converts clocks', async () => {
    vi.useFakeTimers()
    let local = 10_000
    const fetchUtc = vi.fn(async () => {
      local += 30 // 30 ms one way
      const server = local + 2000
      local += 30
      return { received: server, sent: server }
    })
    const sync = new TimeSync({
      fetchUtc,
      now: () => local,
      burst: 3,
      burstGapMs: 100,
      refreshMs: 5000,
    })
    sync.start()
    await vi.advanceTimersByTimeAsync(0)
    expect(fetchUtc).toHaveBeenCalledTimes(1)
    await vi.advanceTimersByTimeAsync(200)
    expect(fetchUtc).toHaveBeenCalledTimes(3)
    await vi.advanceTimersByTimeAsync(4000)
    expect(fetchUtc).toHaveBeenCalledTimes(3)
    await vi.advanceTimersByTimeAsync(1100)
    expect(fetchUtc).toHaveBeenCalledTimes(4)
    expect(sync.offset).toBe(2000)
    expect(sync.toLocal(52_000)).toBe(50_000)
    expect(sync.toServer(50_000)).toBe(52_000)
    sync.stop()
    await vi.advanceTimersByTimeAsync(20_000)
    expect(fetchUtc).toHaveBeenCalledTimes(4)
  })

  it('keeps going after a failed request', async () => {
    vi.useFakeTimers()
    const fetchUtc = vi
      .fn<() => Promise<{ received: number; sent: number }>>()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValue({ received: 500, sent: 500 })
    const sync = new TimeSync({ fetchUtc, now: () => 0, burst: 2, burstGapMs: 10 })
    sync.start()
    await vi.advanceTimersByTimeAsync(50)
    expect(sync.ready).toBe(true)
    expect(sync.offset).toBe(500)
    sync.stop()
  })
})
