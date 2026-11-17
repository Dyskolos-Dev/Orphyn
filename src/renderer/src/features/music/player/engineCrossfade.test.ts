import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { CrossfadeSettings } from './crossfade'
import { PlayerEngine } from './engine'
import type { AudioOutput, OutputEvent, OutputListener } from './output'
import { initialPlayerState, usePlayerStore } from './store'
import type { Track } from './types'

class FakeOutput implements AudioOutput {
  loads: string[] = []
  position = 0
  duration = 200
  paused = true
  fades: number[] = []
  private ls = new Set<OutputListener>()
  load(src: string) {
    this.loads.push(src)
  }
  preload() {}
  async play() {}
  pause() {}
  seek() {}
  setVolume() {}
  setMuted() {}
  subscribe(l: OutputListener) {
    this.ls.add(l)
    return () => this.ls.delete(l)
  }
  dispose() {}
  emit(e: OutputEvent) {
    this.ls.forEach((l) => l(e))
  }
}

/** A cast-like output: same thing without `setCrossfade`. */
class FadingOutput extends FakeOutput {
  setCrossfade(seconds: number) {
    this.fades.push(seconds)
  }
}

const album = (id: string, n: number, albumId: string): Track => ({
  Id: id,
  Name: id,
  AlbumId: albumId,
  IndexNumber: n,
  RunTimeTicks: 2e9,
})

const settings: CrossfadeSettings = { enabled: true, seconds: 6, skipSameAlbum: true }

describe('PlayerEngine crossfade', () => {
  let engine: PlayerEngine | null = null
  beforeEach(() => {
    localStorage.clear()
    usePlayerStore.setState({ ...initialPlayerState })
  })
  afterEach(() => engine?.stop())

  const start = (out: AudioOutput, getCrossfade = () => settings) => {
    engine = new PlayerEngine({
      output: out,
      resolveUrl: (t) => `http://x/${t.Id}`,
      getCrossfade,
    })
    engine.start({ restore: false })
  }

  const queue = [album('a1', 1, 'A'), album('a2', 2, 'A'), album('b1', 1, 'B')]

  it('asks the output for the planned fade, album rule included', () => {
    const out = new FadingOutput()
    start(out)
    usePlayerStore.getState().play(queue, 0)
    out.emit({ type: 'time', position: 3, duration: 200 })
    expect(out.fades.at(-1)).toBe(0) // a1 -> a2: same album, gapless
    usePlayerStore.getState().jumpTo(1)
    out.emit({ type: 'time', position: 3, duration: 200 })
    expect(out.fades.at(-1)).toBe(6) // a2 -> b1
  })

  it('applies preference changes live and never fades on repeat-one', () => {
    const out = new FadingOutput()
    let current = settings
    start(out, () => current)
    usePlayerStore.getState().play(queue, 1)
    out.emit({ type: 'time', position: 3, duration: 200 })
    expect(out.fades.at(-1)).toBe(6)
    current = { ...settings, seconds: 3 }
    out.emit({ type: 'time', position: 4, duration: 200 })
    expect(out.fades.at(-1)).toBe(3)
    usePlayerStore.getState().setRepeat('one')
    expect(out.fades.at(-1)).toBe(0)
    usePlayerStore.getState().setRepeat('off')
    current = { ...settings, enabled: false }
    out.emit({ type: 'time', position: 5, duration: 200 })
    expect(out.fades.at(-1)).toBe(0)
  })

  it('leaves an output without crossfade support (cast) alone and plain-switches', () => {
    const out = new FakeOutput()
    start(out)
    usePlayerStore.getState().play(queue, 1)
    out.emit({ type: 'time', position: 3, duration: 200 })
    expect(out.fades).toEqual([])
    out.emit({ type: 'ended' })
    expect(usePlayerStore.getState().index).toBe(2)
    expect(out.loads.at(-1)).toBe('http://x/b1')
  })

  it('makes the next track current when the fade starts', () => {
    const out = new FadingOutput()
    start(out)
    usePlayerStore.getState().play(queue, 1)
    out.emit({ type: 'crossfade' })
    const s = usePlayerStore.getState()
    expect(s.index).toBe(2)
    expect(s.wantPlaying).toBe(true)
    expect(out.loads.at(-1)).toBe('http://x/b1')
  })
})
