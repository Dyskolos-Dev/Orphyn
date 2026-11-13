/**
 * Stable public API of the music player, for search, cast and other features:
 *
 *   import { musicPlayer, useMusicPlayer } from '@renderer/features/music'
 *   musicPlayer.playAlbum(id, { shuffle: true })
 */
import { toast } from '@renderer/design'
import i18n from 'i18next'
import { startPlayerEngine } from './instance'
import {
  fetchAlbumTracks,
  fetchArtistSongs,
  fetchInstantMix,
  fetchPlaylistTracks,
  fetchTracksOf,
} from './fetch'
import { currentItem } from './queue'
import { usePlayerStore, type PlayerState } from './store'
import type { AudioOutput } from './output'
import type { QueueItem, Track } from './types'

type PlayOptions = { shuffle?: boolean }

async function guarded(run: () => Promise<void>): Promise<void> {
  try {
    startPlayerEngine()
    await run()
  } catch {
    toast({ title: i18n.t('music:player.loadFailed'), tone: 'error' })
  }
}

export const musicPlayer = {
  /** Plays `tracks` from `index` (random first track when shuffling without an index). */
  play(tracks: readonly Track[], index?: number, opts?: PlayOptions): void {
    startPlayerEngine()
    usePlayerStore.getState().play(tracks, index, opts)
  },
  playAlbum: (id: string, opts?: PlayOptions) =>
    guarded(async () => {
      const tracks = await fetchAlbumTracks(id)
      musicPlayer.play(tracks, opts?.shuffle ? undefined : 0, opts)
    }),
  playPlaylist: (id: string, opts?: PlayOptions) =>
    guarded(async () => {
      const tracks = await fetchPlaylistTracks(id)
      musicPlayer.play(tracks, opts?.shuffle ? undefined : 0, opts)
    }),
  playArtist: (id: string, opts?: PlayOptions) =>
    guarded(async () => {
      const tracks = await fetchArtistSongs(id)
      musicPlayer.play(tracks, opts?.shuffle ? undefined : 0, opts)
    }),
  /** Plays a song, or an album / playlist / artist, from its item id. */
  playItem: (id: string, opts?: PlayOptions) =>
    guarded(async () => {
      const tracks = await fetchTracksOf(id)
      musicPlayer.play(tracks, opts?.shuffle ? undefined : 0, opts)
    }),
  /** "Play Next" / "Add to queue" for a song, album, playlist or artist id. */
  queueItem: (id: string, where: 'next' | 'last') =>
    guarded(async () => {
      const tracks = await fetchTracksOf(id)
      if (where === 'next') musicPlayer.playNext(tracks)
      else musicPlayer.addToQueue(tracks)
      toast({
        title: i18n.t(where === 'next' ? 'music:queue.addedNext' : 'music:queue.addedLast'),
      })
    }),
  playInstantMix: (id: string) =>
    guarded(async () => {
      const tracks = await fetchInstantMix(id)
      musicPlayer.play(tracks, 0, { shuffle: false })
    }),
  playNext(tracks: readonly Track[]): void {
    startPlayerEngine()
    usePlayerStore.getState().playNext(tracks)
  },
  addToQueue(tracks: readonly Track[]): void {
    startPlayerEngine()
    usePlayerStore.getState().addToQueue(tracks)
  },
  pause: () => usePlayerStore.getState().pause(),
  resume() {
    startPlayerEngine()
    usePlayerStore.getState().resume()
  },
  toggle() {
    startPlayerEngine()
    usePlayerStore.getState().toggle()
  },
  next: () => usePlayerStore.getState().next(),
  previous: () => usePlayerStore.getState().previous(),
  seek: (seconds: number) => usePlayerStore.getState().seek(seconds),
  setVolume: (v: number) => usePlayerStore.getState().setVolume(v),
  stop: () => usePlayerStore.getState().stop(),
  getState: (): PlayerState => usePlayerStore.getState(),
  subscribe: usePlayerStore.subscribe,
  getCurrentTrack: (): Track | undefined => currentItem(usePlayerStore.getState())?.track,
  getQueue: (): readonly QueueItem[] => usePlayerStore.getState().queue,
  /** Swaps the audio output (cast hand-over). */
  setOutput: (output: AudioOutput): void => startPlayerEngine().setOutput(output),
  getOutput: (): AudioOutput => startPlayerEngine().getOutput(),
}

/** React selector hook over the player state: `useMusicPlayer((s) => s.status)`. */
export const useMusicPlayer = usePlayerStore

/** The current track (undefined when the queue is empty). */
export function useCurrentTrack(): Track | undefined {
  return usePlayerStore((s) => s.queue[s.index]?.track)
}
