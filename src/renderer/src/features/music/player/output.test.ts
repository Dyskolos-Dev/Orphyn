import { describe, expect, it, vi } from 'vitest'
import { LocalAudioOutput, type OutputEvent } from './output'

/** Just enough of an HTMLAudioElement: an EventTarget with the members the output touches. */
function fakeAudio() {
  const el = new EventTarget() as EventTarget & Record<string, unknown>
  let src: string | null = null
  Object.assign(el, {
    currentTime: 0,
    duration: NaN,
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

function setup() {
  const els: HTMLAudioElement[] = []
  const out = new LocalAudioOutput(() => {
    const e = fakeAudio()
    els.push(e)
    return e
  })
  const events: OutputEvent[] = []
  out.subscribe((e) => events.push(e))
  return { out, els: els as [HTMLAudioElement, HTMLAudioElement], events }
}

describe('LocalAudioOutput', () => {
  it('plays a loaded source on the active element', () => {
    const { out, els } = setup()
    out.load('http://x/a')
    expect(els[0].src).toBe('http://x/a')
    expect(els[0].play).toHaveBeenCalled()
  })

  it('preloads on the idle element and switches to it without reloading (gapless)', () => {
    const { out, els } = setup()
    out.load('http://x/a')
    out.preload('http://x/b')
    expect(els[1].src).toBe('http://x/b')
    expect(els[1].load).toHaveBeenCalledTimes(1)

    out.load('http://x/b')
    // The preloaded element took over: its source was not reset, and it plays.
    expect(els[1].src).toBe('http://x/b')
    expect(els[1].load).toHaveBeenCalledTimes(1)
    expect(els[1].play).toHaveBeenCalled()
    expect(els[0].pause).toHaveBeenCalled()
    expect(els[0].getAttribute('src')).toBeNull()
  })

  it('only forwards events of the active element', () => {
    const { out, els, events } = setup()
    out.load('http://x/a')
    out.preload('http://x/b')
    events.length = 0
    els[1].dispatchEvent(new Event('ended'))
    expect(events).toEqual([])
    els[0].dispatchEvent(new Event('ended'))
    expect(events).toEqual([{ type: 'ended' }])
  })

  it('reports network errors as retryable and ignores aborted ones', () => {
    const { out, els, events } = setup()
    out.load('http://x/a')
    events.length = 0
    Object.assign(els[0], { error: { code: 2, message: 'net' } })
    els[0].dispatchEvent(new Event('error'))
    Object.assign(els[0], { error: { code: 1, message: '' } })
    els[0].dispatchEvent(new Event('error'))
    Object.assign(els[0], { error: { code: 4, message: 'bad' } })
    els[0].dispatchEvent(new Event('error'))
    expect(events).toEqual([
      { type: 'error', network: true, message: 'net' },
      { type: 'error', network: false, message: 'bad' },
    ])
  })

  it('applies volume and mute to both elements', () => {
    const { out, els } = setup()
    out.setVolume(0.3)
    out.setMuted(true)
    expect(els.map((e) => e.volume)).toEqual([0.3, 0.3])
    expect(els.map((e) => e.muted)).toEqual([true, true])
  })
})
