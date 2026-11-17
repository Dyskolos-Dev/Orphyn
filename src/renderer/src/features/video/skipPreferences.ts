import { useSyncExternalStore } from 'react'

/** Local, per-device preferences of the skip pills (same pattern as the Settings preferences). */
export interface SkipPreferences {
  /** Jump over intros and recaps without asking. Off by default. */
  skipIntroAuto: boolean
}

export const SKIP_PREFERENCES_KEY = 'orphyn.video.skip'
export const DEFAULT_SKIP_PREFERENCES: SkipPreferences = { skipIntroAuto: false }

function read(): SkipPreferences {
  try {
    const raw = globalThis.localStorage?.getItem(SKIP_PREFERENCES_KEY)
    const o = raw ? (JSON.parse(raw) as Record<string, unknown>) : {}
    return { skipIntroAuto: o?.['skipIntroAuto'] === true }
  } catch {
    return { ...DEFAULT_SKIP_PREFERENCES }
  }
}

let current = read()
const listeners = new Set<() => void>()

export const getSkipPreferences = (): SkipPreferences => current

export function updateSkipPreferences(patch: Partial<SkipPreferences>): void {
  current = {
    ...current,
    ...patch,
    skipIntroAuto: (patch.skipIntroAuto ?? current.skipIntroAuto) === true,
  }
  try {
    globalThis.localStorage?.setItem(SKIP_PREFERENCES_KEY, JSON.stringify(current))
  } catch {
    // Storage unavailable: the choice still applies for this session.
  }
  listeners.forEach((l) => l())
}

/** Re-reads storage (tests, or another window changed it). */
export function reloadSkipPreferences(): void {
  current = read()
  listeners.forEach((l) => l())
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  const onStorage = (e: StorageEvent) => {
    if (e.key === SKIP_PREFERENCES_KEY || e.key === null) reloadSkipPreferences()
  }
  window.addEventListener('storage', onStorage)
  return () => {
    listeners.delete(listener)
    window.removeEventListener('storage', onStorage)
  }
}

export function useSkipPreferences(): SkipPreferences & {
  update(patch: Partial<SkipPreferences>): void
} {
  const prefs = useSyncExternalStore(subscribe, getSkipPreferences, getSkipPreferences)
  return { ...prefs, update: updateSkipPreferences }
}
