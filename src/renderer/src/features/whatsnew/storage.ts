export const LAST_SEEN_KEY = 'orphyn.lastSeenVersion'

export function readLastSeen(): string | null {
  try {
    return localStorage.getItem(LAST_SEEN_KEY)
  } catch {
    return null
  }
}

export function writeLastSeen(version: string): void {
  try {
    localStorage.setItem(LAST_SEEN_KEY, version)
  } catch {
    /* storage unavailable: the notes would show again next launch, which is harmless */
  }
}
