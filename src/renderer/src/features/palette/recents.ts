/** Recently used commands (ids), most recent first, kept in localStorage. */
export const RECENT_COMMANDS_KEY = 'orphyn.palette.recent'
export const MAX_RECENT_COMMANDS = 6

export function pushRecent(
  list: readonly string[],
  id: string,
  max = MAX_RECENT_COMMANDS,
): string[] {
  return [id, ...list.filter((x) => x !== id)].slice(0, max)
}

export function readRecentCommands(): string[] {
  try {
    const raw = globalThis.localStorage?.getItem(RECENT_COMMANDS_KEY)
    const parsed: unknown = raw ? JSON.parse(raw) : []
    return Array.isArray(parsed)
      ? parsed.filter((x): x is string => typeof x === 'string').slice(0, MAX_RECENT_COMMANDS)
      : []
  } catch {
    return []
  }
}

export function recordRecentCommand(id: string): string[] {
  const next = pushRecent(readRecentCommands(), id)
  try {
    globalThis.localStorage?.setItem(RECENT_COMMANDS_KEY, JSON.stringify(next))
  } catch {
    // Storage disabled: recents just do not persist.
  }
  return next
}
