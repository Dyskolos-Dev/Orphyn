import { toast } from '@renderer/design'
import i18n from 'i18next'
import { musicPlayer } from '../music/player/api'
import { fetchItem } from '../music/player/fetch'
import { usePlayerStore } from '../music/player/store'
import { fetchRadioBatch } from './api'
import { createRadioController, pickRefill, type RadioController, type RadioSeed } from './radio'
import type { Track } from './types'

let controller: RadioController | null = null

function radio(): RadioController {
  controller ??= createRadioController({
    player: {
      getState: () => usePlayerStore.getState(),
      subscribe: (fn) => usePlayerStore.subscribe(fn),
      addToQueue: (tracks) => musicPlayer.addToQueue(tracks),
    },
    fetchMore: fetchRadioBatch,
  })
  return controller
}

export const stopRadio = (): void => controller?.stop()

/** Starts a radio from a song, an album or an artist (its item id). */
export async function startRadio(id: string): Promise<void> {
  try {
    const item = await fetchItem(id)
    const kind: RadioSeed['kind'] =
      item.Type === 'MusicArtist' ? 'artist' : item.Type === 'MusicAlbum' ? 'album' : 'song'
    const batch = await fetchRadioBatch(id)
    const first: Track[] = kind === 'song' ? [item] : []
    const queue = pickRefill([...first, ...batch], new Set(), undefined, 30)
    if (queue.length === 0) throw new Error('empty radio')
    musicPlayer.play(queue, 0, { shuffle: false })
    radio().start({ id, name: item.Name ?? '', kind }, queue)
    toast({ title: i18n.t('mixes:radio.started', { name: item.Name ?? '' }) })
  } catch {
    toast({ title: i18n.t('mixes:radio.failed'), tone: 'error' })
  }
}
