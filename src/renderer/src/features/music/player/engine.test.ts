import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { EXCLUSIVE_PLAYBACK_EVENT, PlayerEngine, type Reporter } from './engine'
import type { AudioOutput, LoadOptions, OutputEvent, OutputListener } from './output'
import { savePlayer } from './persistence'
import { createQueue } from './queue'
import { initialPlayerState, usePlayerStore } from './store'
import type { Track } from './types'

class FakeOutput implements AudioOutput {
  loads: { src: string; opts?: LoadOptions }[] = []
  preloads: (string | null)[] = []
  position = 0
  duration = 200
  paused = true
  volume = 1
  muted = false
  seeks: number[] = []
  private ls = new Set<OutputListener>()
  load(src: string, opts?: LoadOptions) {
    this.loads.push({ src, opts })
    this.paused = opts?.autoplay === false
  }
  preload(src: string | null) {
    this.preloads.push(src)
  }
  async play() {
    this.paused = false
  }
  pause() {
    this.paused = true
  }
  seek(s: number) {
    this.seeks.push(s)
    this.position = s
  }
  setVolume(v: number) {
    this.volume = v
  }
  setMuted(m: boolean) {
    this.muted = m
  }
  subscribe(l: OutputListener) {
    this.ls.add(l)
    return () => this.ls.delete(l)
  }
  dispose() {}
  emit(e: OutputEvent) {
    this.ls.forEach((l) => l(e))
  }
}

const tracks = (n: number): Track[] =>
  Array.from({ length: n }, (_, i) => ({ Id: `t${i}`, Name: `Track ${i}`, RunTimeTicks: 2e9 }))

const store = usePlayerStore

function makeReporter() {
  return {
    start: vi.fn(async () => {}),
    pause: vi.fn(async () => {}),
    resume: vi.fn(async () => {}),
    tick: vi.fn(async () => {}),
    stop: vi.fn(async () => {}),
  } satisfies Reporter
}

describe('PlayerEngine', () => {
  let out: FakeOutput
  let engine: PlayerEngine
  let onUnplayable: Mock<(track: Track) => void>
  let reporter: ReturnType<typeof makeReporter>

  beforeEach(() => {
    localStorage.clear()
    vi.useFakeTimers()
    store.setState({ ...initialPlayerState })
    out = new FakeOutput()
    onUnplayable = vi.fn<(track: Track) => void>()
    reporter = makeReporter()
    engine = new PlayerEngine({
      output: out,
      resolveUrl: (t) => `http://x/${t.Id}`,
      createReporter: () => reporter,
      onUnplayable,
      retryDelaysMs: [100, 200],
    })
    engine.start()
  })

  afterEach(() => {
    engine.stop()
    vi.useRealTimers()
  })

  it('loads and plays the chosen track', () => {
    store.getState().play(tracks(3), 1)
    expect(out.loads.at(-1)).toEqual({ src: 'http://x/t1', opts: { startAt: 0, autoplay: true } })
    out.emit({ type: 'state', state: 'playing' })
    expect(store.getState().status).toBe('playing')
    expect(reporter.start).toHaveBeenCalled()
  })

  it('announces exclusive playback and ignores its own event', () => {
    const seen = vi.fn()
    window.addEventListener(EXCLUSIVE_PLAYBACK_EVENT, seen)
    store.getState().play(tracks(2), 0)
    out.emit({ type: 'state', state: 'playing' })
    expect((seen.mock.calls[0]?.[0] as CustomEvent).detail).toEqual({ source: 'music' })
    expect(store.getState().wantPlaying).toBe(true)
    window.removeEventListener(EXCLUSIVE_PLAYBACK_EVENT, seen)
  })

  it('pauses when another source starts playing', () => {
    store.getState().play(tracks(2), 0)
    out.emit({ type: 'state', state: 'playing' })
    window.dispatchEvent(new CustomEvent(EXCLUSIVE_PLAYBACK_EVENT, { detail: { source: 'video' } }))
    expect(store.getState().wantPlaying).toBe(false)
    expect(out.paused).toBe(true)
  })

  it('goes to the next track when one ends, and stops at the end of the queue', () => {
    store.getState().play(tracks(2), 0)
    out.emit({ type: 'ended' })
    expect(out.loads.at(-1)?.src).toBe('http://x/t1')
    expect(store.getState().index).toBe(1)
    out.emit({ type: 'ended' })
    expect(store.getState().wantPlaying).toBe(false)
    expect(store.getState().index).toBe(1)
    expect(out.loads.at(-1)?.opts?.autoplay).toBe(false)
    expect(reporter.stop).toHaveBeenCalled()
  })

  it('repeat one loads the same track again', () => {
    store.getState().play(tracks(3), 1)
    store.getState().setRepeat('one')
    out.emit({ type: 'ended' })
    expect(store.getState().index).toBe(1)
    expect(out.loads.at(-1)?.src).toBe('http://x/t1')
    expect(out.loads).toHaveLength(2)
  })

  it('preloads the next track once playback is under way, and follows queue edits', () => {
    store.getState().play(tracks(3), 0)
    out.emit({ type: 'time', position: 0.2, duration: 200 })
    expect(out.preloads).toEqual([])
    out.emit({ type: 'time', position: 1.5, duration: 200 })
    expect(out.preloads.at(-1)).toBe('http://x/t1')
    store.getState().playNext([{ Id: 'new' }])
    expect(out.preloads.at(-1)).toBe('http://x/new')
  })

  it('does not preload at the end of the queue with repeat off', () => {
    store.getState().play(tracks(2), 1)
    out.emit({ type: 'time', position: 5, duration: 200 })
    expect(out.preloads.filter(Boolean)).toEqual([])
  })

  it('retries network failures at the same position, then gives up and skips', () => {
    store.getState().play(tracks(3), 0)
    out.emit({ type: 'state', state: 'playing' })
    out.position = 30
    out.emit({ type: 'error', network: true })
    expect(store.getState().status).toBe('loading')
    vi.advanceTimersByTime(100)
    expect(out.loads.at(-1)).toEqual({ src: 'http://x/t0', opts: { startAt: 30, autoplay: true } })
    out.emit({ type: 'error', network: true })
    vi.advanceTimersByTime(200)
    expect(out.loads).toHaveLength(3)
    out.emit({ type: 'error', network: true }) // retries exhausted
    expect(onUnplayable).toHaveBeenCalledTimes(1)
    expect(store.getState().index).toBe(1)
    expect(out.loads.at(-1)?.src).toBe('http://x/t1')
  })

  it('skips an undecodable track straight away and plays on', () => {
    store.getState().play(tracks(3), 0)
    out.emit({ type: 'error', network: false })
    expect(onUnplayable).toHaveBeenCalledWith(expect.objectContaining({ Id: 't0' }))
    expect(store.getState().index).toBe(1)
    expect(store.getState().wantPlaying).toBe(true)
  })

  it('stops instead of looping when every track fails', () => {
    store.getState().play(tracks(2), 0)
    out.emit({ type: 'error', network: false })
    out.emit({ type: 'error', network: false })
    expect(store.getState().status).toBe('error')
    expect(store.getState().wantPlaying).toBe(false)
  })

  it('pause and resume drive the output', () => {
    store.getState().play(tracks(2), 0)
    out.emit({ type: 'state', state: 'playing' })
    store.getState().pause()
    expect(out.paused).toBe(true)
    store.getState().resume()
    expect(out.paused).toBe(false)
  })

  it('seeks the output', () => {
    store.getState().play(tracks(2), 0)
    out.emit({ type: 'time', position: 1, duration: 200 })
    store.getState().seek(120)
    expect(out.seeks).toEqual([120])
  })

  it('syncs volume and mute', () => {
    store.getState().setVolume(0.4)
    expect(out.volume).toBe(0.4)
    store.getState().toggleMute()
    expect(out.muted).toBe(true)
  })

  it('restores a saved session without playing, then resumes from the saved position', () => {
    engine.stop()
    store.setState({ ...initialPlayerState })
    savePlayer({
      ...createQueue(tracks(3), 2, { shuffle: false, repeat: 'all' }),
      position: 61,
      volume: 0.5,
      muted: false,
    })
    out = new FakeOutput()
    engine = new PlayerEngine({ output: out, resolveUrl: (t) => `http://x/${t.Id}` })
    engine.start()
    expect(out.loads).toHaveLength(0)
    expect(store.getState()).toMatchObject({
      index: 2,
      status: 'paused',
      repeat: 'all',
      volume: 0.5,
    })
    expect(out.volume).toBe(0.5)
    store.getState().resume()
    expect(out.loads).toEqual([{ src: 'http://x/t2', opts: { startAt: 61, autoplay: true } }])
  })

  it('persists the queue while playing', () => {
    store.getState().play(tracks(3), 1)
    vi.advanceTimersByTime(2500)
    const saved = JSON.parse(localStorage.getItem('orphyn.music.player') ?? '{}')
    expect(saved.index).toBe(1)
    expect(saved.queue).toHaveLength(3)
  })

  it('hands the current track over when the output is swapped', () => {
    store.getState().play(tracks(2), 1)
    out.position = 44
    const next = new FakeOutput()
    engine.setOutput(next)
    expect(out.paused).toBe(true)
    expect(next.loads).toEqual([{ src: 'http://x/t1', opts: { startAt: 44, autoplay: true } }])
  })
})
