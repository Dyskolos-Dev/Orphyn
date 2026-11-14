/** Jellyfin types and constants shared by the renderer. */

export const CLIENT_NAME = 'Orphyn'
/** Oldest Jellyfin server version the app supports (lyrics API, modern auth). */
export const MIN_SERVER_VERSION = '10.9.0'
/** Device name sent to the server (shown in the server's Devices dashboard). */
export const DEVICE_NAME = 'Orphyn Desktop'

/** A saved server account as seen by the renderer: never contains the access token. */
export interface ServerAccount {
  /** Local id (uuid), stable across renames. */
  id: string
  /** User-chosen display name. */
  name: string
  /** Normalised base URL, no trailing slash (may include a base path such as /jellyfin). */
  url: string
  /** Jellyfin server id (System/Info/Public Id). */
  serverId: string
  serverName: string
  version: string
  userId: string
  userName: string
  /** ISO timestamps. */
  addedAt: string
  lastUsedAt: string
  /** False when the token was not persisted (no secure storage) or was lost: login needed. */
  hasToken: boolean
}

/** Input of `accounts.add`. The token comes from AuthenticateByName; passwords are never passed. */
export interface NewServerAccount {
  /** Display name; empty keeps the name of an existing entry for this server and user. */
  name: string
  url: string
  serverId: string
  serverName: string
  version: string
  userId: string
  userName: string
  accessToken: string
}

export interface AccountsStatus {
  /** True when tokens are encrypted on disk in the OS keychain. */
  encrypted: boolean
  /** Set when encryption is unavailable: tokens are kept in memory only. */
  warning?: string
}

/** Compare dotted versions ("10.9.11", "10.10.0-rc1"). Returns <0, 0, >0. */
export function compareVersions(a: string, b: string): number {
  const parse = (v: string): number[] =>
    v
      .split(/[-+]/)[0]!
      .split('.')
      .map((p) => Number.parseInt(p, 10) || 0)
  const pa = parse(a)
  const pb = parse(b)
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0)
    if (d !== 0) return d
  }
  return 0
}
