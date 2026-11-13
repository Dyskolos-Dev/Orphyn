import type { CastApi } from '@shared/ipc'

/** `window.orphyn.cast`, or null where the bridge does not exist (tests, plain browser). */
export function getCastApi(): CastApi | null {
  return typeof window !== 'undefined' ? (window.orphyn?.cast ?? null) : null
}
