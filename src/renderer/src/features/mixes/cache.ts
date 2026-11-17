import { leanTrack } from '../music/player/track'
import type { Mix } from './types'

export const mixesStorageKey = (serverId: string, userId: string): string =>
  `orphyn.mixes.${serverId}.${userId}`

/** Local calendar day, the unit of the daily refresh. */
export const dayOf = (ms: number): string => {
  const d = new Date(ms)
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

interface Stored {
  day: string
  mixes: Mix[]
}

/** The mixes saved today, or `null` (another day, nothing saved, unreadable). */
export function readMixes(
  key: string,
  now: number,
  storage: Pick<Storage, 'getItem'> = localStorage,
): Mix[] | null {
  try {
    const raw = storage.getItem(key)
    if (!raw) return null
    const stored = JSON.parse(raw) as Stored
    if (stored.day !== dayOf(now) || !Array.isArray(stored.mixes)) return null
    return stored.mixes
  } catch {
    return null
  }
}

export function writeMixes(
  key: string,
  mixes: readonly Mix[],
  now: number,
  storage: Pick<Storage, 'setItem'> = localStorage,
): void {
  const lean = mixes.map((m) => ({ ...m, tracks: m.tracks.map(leanTrack) }))
  try {
    storage.setItem(key, JSON.stringify({ day: dayOf(now), mixes: lean } satisfies Stored))
  } catch {
    // quota / private mode: the mixes are simply generated again next time
  }
}
