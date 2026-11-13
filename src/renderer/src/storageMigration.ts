/**
 * One-time rename of the localStorage keys from the pre-1.1 `lumen.*` prefix to `orphyn.*`:
 * the old value is copied (unless the new key already exists), then the old key is deleted.
 */
const LEGACY_PREFIX = 'lumen.'
const PREFIX = 'orphyn.'

export function migrateLegacyStorage(storage: Storage): void {
  try {
    const legacyKeys: string[] = []
    for (let i = 0; i < storage.length; i += 1) {
      const key = storage.key(i)
      if (key?.startsWith(LEGACY_PREFIX)) legacyKeys.push(key)
    }
    for (const key of legacyKeys) {
      const next = PREFIX + key.slice(LEGACY_PREFIX.length)
      const value = storage.getItem(key)
      if (value !== null && storage.getItem(next) === null) storage.setItem(next, value)
      storage.removeItem(key)
    }
  } catch {
    /* storage unavailable: nothing to migrate */
  }
}

if (typeof localStorage !== 'undefined') migrateLegacyStorage(localStorage)
