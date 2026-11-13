import { isAbsolute } from 'node:path'
import { app } from 'electron'

/**
 * Chromium features behind the OS media integration: media keys and the Windows SMTC flyout /
 * macOS Now Playing / Linux MPRIS read `navigator.mediaSession` through them. They are on by
 * default in current Electron, but enabling them explicitly keeps them on if a later Electron
 * or a command-line `--disable-features` default changes.
 */
export const MEDIA_FEATURES = ['HardwareMediaKeyHandling', 'MediaSessionService'] as const

/** Merges `features` into a comma list, without duplicates, keeping existing entries first. */
export function mergeFeatureList(current: string, features: readonly string[]): string {
  const list = current
    .split(',')
    .map((f) => f.trim())
    .filter(Boolean)
  for (const f of features) if (!list.includes(f)) list.push(f)
  return list.join(',')
}

/**
 * Optional user data override, used by the end-to-end suite to start from a clean profile:
 * `LUMEN_USER_DATA_DIR` must be an absolute path, otherwise it is ignored.
 */
export function userDataOverride(env: NodeJS.ProcessEnv = process.env): string | null {
  const dir = env['LUMEN_USER_DATA_DIR']?.trim()
  return dir && isAbsolute(dir) ? dir : null
}

export function applyStartupSwitches(): void {
  const dir = userDataOverride()
  if (dir) app.setPath('userData', dir)

  const current = app.commandLine.getSwitchValue('enable-features')
  app.commandLine.appendSwitch('enable-features', mergeFeatureList(current, MEDIA_FEATURES))
}
