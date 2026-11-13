import type { CastApi } from '@shared/ipc'

/** `window.lumen.cast`, or null where the bridge does not exist (tests, plain browser). */
export function getCastApi(): CastApi | null {
  return typeof window !== 'undefined' ? (window.lumen?.cast ?? null) : null
}
