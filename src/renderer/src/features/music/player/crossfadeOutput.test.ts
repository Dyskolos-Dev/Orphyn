import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { FadeGraph } from './crossfade'
import { LocalAudioOutput, type OutputEvent } from './output'

function fakeAudio() {
  const el = new EventTarget() as EventTarget & Record<string, unknown>
  let src: string | null = null
  Object.assign(el, {
    currentTime: 0,
    duration: 100,
    paused: true,
    ended: false,
    readyState: 4,
    volume: 1,
    muted: false,
    error: null,
    preload: '',
    play: vi.fn(async () => {
      el.paused = false
    }),
    pause: vi.fn(() => {
      el.paused = true
    }),
    load: vi.fn(),
    getAttribute: (n: string) => (n === 'src' ? src : null),
    removeAttribute: (n: string) => {
      if (n === 'src') src = null
    },
  })
  Object.defineProperty(el, 'src', {
    get: () => src ?? '',
    set: (v: string) => {
      src = v
    },
  })
  return el as unknown as HTMLAudioElement
}

function fakeGraph() {
  const attached = new Set<HTMLAudioElement>()
  const gains = new Map<HTMLAudioElement, number>()
  const ramps: { from: number; seconds: number }[] = []
  let clock = 0
  const graph: FadeGraph = {
    attach: (el) => (attached.add(el), true),
    isAttached: (el) => attached.has(el),
    now: () => clock,
    resume: async () => {},
    setGain: (el, g) => void gains.set(el, g),
    ramp: (_o, _i, from, seconds) => void ramps.push({ from, seconds }),
  }
  return { graph, gains, ramps, advance: (s: number) => (clock += s) }
}

function setup(withGraph = true) {
  const els: HTMLAudioElement[] = []
  const g = fakeGraph()
  const out = new LocalAudioOutput(
    () => {
      const e = fakeAudio()
      els.push(e)
      return e
    },
    withGraph ? g.graph : null,
  )
  const events: OutputEvent[] = []
  out.subscribe((e) => events.push(e))
  return { out, els: els as [HTMLAudioElement, HTMLAudioElement], events, ...g }
}

/** Plays `a`, preloads `b` and moves `a` to `at` seconds. */
async function playing(s: ReturnType<typeof setup>, fade: number, at: number) {
  s.out.setCrossfade(fade)
  s.out.load('http://x/a')
  s.out.preload('http://x/b')
  await Promise.resolve()
  s.els[0].currentTime = at
  s.els[0].dispatchEvent(new Event('timeupdate'))
}

describe('LocalAudioOutput crossfade', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  it('starts the fade `duration` seconds before the end', async () => {
    const s = setup()
    await playing(s, 6, 93)
    expect(s.events.some((e) => e.type === 'crossfade')).toBe(false)
    s.els[0].currentTime = 94
    s.els[0].dispatchEvent(new Event('timeupdate'))
    expect(s.events.filter((e) => e.type === 'crossfade')).toHaveLength(1)
    expect(s.ramps).toEqual([{ from: 0, seconds: 6 }])
    expect(s.els[1].play).toHaveBeenCalled()
    expect(s.gains.get(s.els[1])).toBe(0)
    // The incoming element is now the current one.
    expect(s.out.position).toBe(s.els[1].currentTime)
  })

  it('shortens the fade when started late (seek near the end)', async () => {
    const s = setup()
    await playing(s, 6, 98)
    expect(s.ramps).toEqual([{ from: 0, seconds: 2 }])
  })

  it('adopts the faded-in track on load() instead of reloading it, then cleans up', async () => {
    const s = setup()
    await playing(s, 6, 95)
    s.events.length = 0
    s.out.load('http://x/b')
    await Promise.resolve()
    expect(s.els[1].load).toHaveBeenCalledTimes(1)
    expect(s.events).toEqual([{ type: 'state', state: 'playing' }])
    vi.advanceTimersByTime(5100)
    expect(s.els[0].getAttribute('src')).toBeNull()
    expect(s.gains.get(s.els[1])).toBe(1)
  })

  it('does not fade without crossfade, preload, or a wired element', async () => {
    const off = setup()
    await playing(off, 0, 99)
    expect(off.ramps).toEqual([])
    const noGraph = setup(false)
    await playing(noGraph, 6, 99)
    expect(noGraph.events.some((e) => e.type === 'crossfade')).toBe(false)
    const none = setup()
    none.out.setCrossfade(6)
    none.out.load('http://x/a')
    none.els[0].currentTime = 99
    none.els[0].dispatchEvent(new Event('timeupdate'))
    expect(none.events.some((e) => e.type === 'crossfade')).toBe(false)
  })

  it('pauses both elements during a fade and finishes the remaining part on resume', async () => {
    const s = setup()
    await playing(s, 6, 94)
    s.advance(3)
    s.out.pause()
    expect(s.els[0].pause).toHaveBeenCalled()
    expect(s.els[1].pause).toHaveBeenCalled()
    expect(s.gains.get(s.els[0])).toBeCloseTo(Math.SQRT1_2)
    expect(s.gains.get(s.els[1])).toBeCloseTo(Math.SQRT1_2)
    vi.advanceTimersByTime(10_000) // the paused fade does not end by itself
    expect(s.els[0].getAttribute('src')).toBe('http://x/a')
    await s.out.play()
    expect(s.ramps.at(-1)).toEqual({ from: 0.5, seconds: 3 })
    expect(s.els[0].play).toHaveBeenCalledTimes(2)
    vi.advanceTimersByTime(3100)
    expect(s.els[0].getAttribute('src')).toBeNull()
  })

  it('drops the outgoing track when another one is requested mid-fade', async () => {
    const s = setup()
    await playing(s, 6, 94)
    s.out.load('http://x/c')
    expect(s.els[0].getAttribute('src')).toBeNull()
    expect(s.els[1].src).toBe('http://x/c')
    expect(s.gains.get(s.els[1])).toBe(1)
  })

  it('holds a preload asked for during the fade until it ends', async () => {
    const s = setup()
    await playing(s, 6, 94)
    s.out.preload('http://x/c')
    expect(s.els[0].src).toBe('http://x/a')
    vi.advanceTimersByTime(6100)
    expect(s.els[0].src).toBe('http://x/c')
  })
})
