import { beforeEach, describe, expect, it, vi } from 'vitest'
import { installMediaSession } from './mediaSession'
import { initialPlayerState, usePlayerStore } from './store'

class FakeMetadata {
  constructor(public init: MediaMetadataInit) {}
  get title() {
    return this.init.title
  }
  get artist() {
    return this.init.artist
  }
}

function fakeSession() {
  const handlers = new Map<string, MediaSessionActionHandler | null>()
  return {
    handlers,
    metadata: null as MediaMetadata | null,
    playbackState: 'none' as MediaSessionPlaybackState,
    setActionHandler: (a: MediaSessionAction, h: MediaSessionActionHandler | null) =>
      void handlers.set(a, h),
    setPositionState: vi.fn(),
  }
}

const tracks = [
  { Id: 'a', Name: 'Song A', Artists: ['Artist'], Album: 'Album', RunTimeTicks: 1e9 },
  { Id: 'b', Name: 'Song B', Artists: ['Artist'], Album: 'Album', RunTimeTicks: 1e9 },
]

const install = (s: ReturnType<typeof fakeSession>) =>
  installMediaSession(s, FakeMetadata as unknown as typeof MediaMetadata)
const call = (s: ReturnType<typeof fakeSession>, action: string, details = {}) =>
  s.handlers.get(action)?.({ action, ...details } as MediaSessionActionDetails)

describe('media session', () => {
  beforeEach(() => usePlayerStore.setState({ ...initialPlayerState }))

  it('publishes metadata and playback state of the current track', () => {
    const s = fakeSession()
    install(s)
    expect(s.metadata).toBeNull()
    usePlayerStore.getState().play(tracks, 1)
    expect(s.metadata).toMatchObject({ title: 'Song B', artist: 'Artist' })
    usePlayerStore.getState().setStatus('playing')
    expect(s.playbackState).toBe('playing')
    usePlayerStore.getState().setStatus('paused')
    expect(s.playbackState).toBe('paused')
  })

  it('maps media keys to the player', () => {
    const s = fakeSession()
    install(s)
    const store = usePlayerStore.getState()
    store.play(tracks, 0)
    call(s, 'nexttrack')
    expect(usePlayerStore.getState().index).toBe(1)
    usePlayerStore.getState().setProgress(1, 100)
    call(s, 'previoustrack')
    expect(usePlayerStore.getState().index).toBe(0)
    call(s, 'pause')
    expect(usePlayerStore.getState().wantPlaying).toBe(false)
    call(s, 'play')
    expect(usePlayerStore.getState().wantPlaying).toBe(true)
    call(s, 'seekto', { seekTime: 42 })
    expect(usePlayerStore.getState().position).toBe(42)
    call(s, 'seekforward')
    expect(usePlayerStore.getState().position).toBe(52)
    call(s, 'seekbackward', { seekOffset: 5 })
    expect(usePlayerStore.getState().position).toBe(47)
  })

  it('reports the position once the duration is known', () => {
    const s = fakeSession()
    install(s)
    usePlayerStore.getState().play(tracks, 0)
    usePlayerStore.getState().setProgress(3, 100)
    expect(s.setPositionState).toHaveBeenLastCalledWith({
      duration: 100,
      position: 3,
      playbackRate: 1,
    })
  })

  it('removes its handlers when uninstalled', () => {
    const s = fakeSession()
    const uninstall = install(s)
    uninstall()
    expect(s.handlers.get('play')).toBeNull()
    expect(s.metadata).toBeNull()
  })
})
