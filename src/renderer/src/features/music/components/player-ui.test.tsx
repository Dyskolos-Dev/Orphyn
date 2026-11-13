import { act, fireEvent, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { renderWithRouter } from '@renderer/design/test-utils'
import { LyricsView } from '../lyrics/LyricsView'
import { initialPlayerState, usePlayerStore } from '../player/store'
import { MiniPlayer } from './MiniPlayer'
import { NowPlaying } from './NowPlaying'
import { QueueView } from './QueueView'
import { useNowPlayingUi } from './ui-state'

const lyricsMock = vi.hoisted(() => ({
  data: undefined as unknown,
}))
vi.mock('@renderer/jellyfin', async (orig) => ({
  ...(await orig<typeof import('@renderer/jellyfin')>()),
  useLyrics: () => ({ data: lyricsMock.data, isLoading: false }),
}))

// The real engine would try to load tracks (no session in tests): keep it out.
vi.mock('../player/instance', () => ({ startPlayerEngine: vi.fn(), getPlayerEngine: () => null }))

Element.prototype.scrollIntoView = vi.fn()
Element.prototype.scrollTo = vi.fn()

const tracks = [
  { Id: 'a', Name: 'Song A', Artists: ['Artist'], Album: 'Album', RunTimeTicks: 2e9 },
  { Id: 'b', Name: 'Song B', Artists: ['Artist'], Album: 'Album', RunTimeTicks: 2e9 },
  { Id: 'c', Name: 'Song C', Artists: ['Artist'], Album: 'Album', RunTimeTicks: 2e9 },
]

describe('player UI', () => {
  beforeEach(() => {
    usePlayerStore.setState({ ...initialPlayerState })
    useNowPlayingUi.setState({ open: false, view: 'player' })
    lyricsMock.data = undefined
  })

  it('shows nothing while the queue is empty', () => {
    const { container } = renderWithRouter(<MiniPlayer />)
    expect(container).toBeEmptyDOMElement()
  })

  it('mini player shows the track length before a restored track has loaded', () => {
    usePlayerStore.getState().play(tracks, 0)
    usePlayerStore.getState().pause()
    usePlayerStore.getState().setProgress(0, 0)
    renderWithRouter(<MiniPlayer />)
    expect(screen.getByText('-3:20')).toBeInTheDocument()
    expect(screen.queryByText('--:--')).not.toBeInTheDocument()
  })

  it('mini player shows the track and drives the player', () => {
    usePlayerStore.getState().play(tracks, 0)
    renderWithRouter(<MiniPlayer />)
    expect(screen.getByText('Song A')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Pause' }))
    expect(usePlayerStore.getState().wantPlaying).toBe(false)
    fireEvent.click(screen.getByRole('button', { name: 'Next' }))
    expect(usePlayerStore.getState().index).toBe(1)
    expect(screen.getByText('Song B')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Shuffle' }))
    expect(usePlayerStore.getState().shuffle).toBe(true)
  })

  it('opens the Now Playing sheet and closes it with Escape', async () => {
    usePlayerStore.getState().play(tracks, 0)
    renderWithRouter(
      <>
        <MiniPlayer />
        <NowPlaying />
      </>,
    )
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Lyrics' }))
    expect(await screen.findByRole('dialog', { name: 'Now Playing' })).toBeInTheDocument()
    expect(useNowPlayingUi.getState().view).toBe('lyrics')
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(useNowPlayingUi.getState().open).toBe(false)
  })

  it('synced lyrics highlight the current line and seek on click', () => {
    lyricsMock.data = {
      synced: true,
      lines: [
        { text: 'first', startMs: 1000 },
        { text: 'second', startMs: 5000 },
        { text: 'third', startMs: 9000 },
      ],
    }
    usePlayerStore.getState().play(tracks, 0)
    renderWithRouter(<LyricsView trackId="a" />)
    expect(document.querySelector('[aria-current="true"]')).toBeNull()
    act(() => usePlayerStore.getState().setProgress(5.5, 100))
    expect(screen.getByRole('button', { name: 'second' })).toHaveAttribute('aria-current', 'true')
    act(() => usePlayerStore.getState().setProgress(9.1, 100))
    expect(screen.getByRole('button', { name: 'third' })).toHaveAttribute('aria-current', 'true')
    expect(screen.getByRole('button', { name: 'second' })).not.toHaveAttribute('aria-current')
    fireEvent.click(screen.getByRole('button', { name: 'first' }))
    expect(usePlayerStore.getState().position).toBe(1)
  })

  it('shows unsynced lyrics as plain text and a state when there are none', () => {
    lyricsMock.data = { synced: false, lines: [{ text: 'la la', startMs: null }] }
    const { unmount } = renderWithRouter(<LyricsView trackId="a" />)
    expect(screen.getByText('la la')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'la la' })).not.toBeInTheDocument()
    unmount()
    lyricsMock.data = null
    renderWithRouter(<LyricsView trackId="a" />)
    expect(screen.getByText('No lyrics')).toBeInTheDocument()
  })

  it('queue view reorders by drag and drop and removes entries', () => {
    usePlayerStore.getState().play(tracks, 0)
    renderWithRouter(<QueueView />)
    const items = screen.getAllByRole('listitem')
    const data = new Map<string, string>()
    const dataTransfer = {
      setData: (k: string, v: string) => data.set(k, v),
      getData: (k: string) => data.get(k) ?? '',
      effectAllowed: '',
      dropEffect: '',
    }
    fireEvent.dragStart(items[2] as HTMLElement, { dataTransfer })
    fireEvent.dragOver(items[0] as HTMLElement, { dataTransfer })
    fireEvent.drop(items[0] as HTMLElement, { dataTransfer })
    expect(usePlayerStore.getState().queue.map((q) => q.track.Id)).toEqual(['c', 'a', 'b'])
    expect(usePlayerStore.getState().index).toBe(1) // still on Song A
    fireEvent.click(screen.getByRole('button', { name: 'Remove Song B' }))
    expect(usePlayerStore.getState().queue.map((q) => q.track.Id)).toEqual(['c', 'a'])
  })
})
