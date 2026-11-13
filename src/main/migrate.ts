import { cpSync, existsSync, readdirSync, statSync } from 'node:fs'
import { dirname, join } from 'node:path'

/** Folder names the app data lived in before the rename to Orphyn (productName / package name). */
export const LEGACY_DATA_DIR_NAMES = ['Lumen', 'lumen'] as const

/** Profile folders holding the renderer's localStorage and IndexedDB. */
const PROFILE_DIRS = ['Local Storage', 'IndexedDB']

/**
 * One-time move of the user data from the pre-1.1 "Lumen" folder to the "Orphyn" one, so saved
 * servers, window state, settings and the renderer storage survive the rename. Runs only when
 * the new folder has no `accounts.json` and a sibling legacy folder has one. Nothing is
 * overwritten and the old folder is left untouched. safeStorage tokens stay valid because they
 * are bound to the OS user, not to the folder. Must run before the first BrowserWindow exists.
 *
 * @returns the legacy folder copied from, or null when nothing was migrated.
 */
export function migrateLegacyUserData(
  userDataDir: string,
  log: (message: string) => void = () => {},
): string | null {
  if (existsSync(join(userDataDir, 'accounts.json'))) return null
  const parent = dirname(userDataDir)
  const legacy = LEGACY_DATA_DIR_NAMES.map((name) => join(parent, name)).find(
    (dir) => dir !== userDataDir && existsSync(join(dir, 'accounts.json')),
  )
  if (!legacy) return null

  try {
    for (const entry of readdirSync(legacy)) {
      const from = join(legacy, entry)
      const to = join(userDataDir, entry)
      if (existsSync(to)) continue
      const isFile = statSync(from).isFile()
      if (isFile || PROFILE_DIRS.includes(entry)) cpSync(from, to, { recursive: true })
    }
    log(`[orphyn] migrated user data from ${legacy} to ${userDataDir}`)
    return legacy
  } catch (error) {
    log(`[orphyn] user data migration from ${legacy} failed: ${String(error)}`)
    return null
  }
}
