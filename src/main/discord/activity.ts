import type { DiscordPresence, DiscordSettings } from '@shared/ipc/discord'
import { ASSET_LOGO, ASSET_PAUSE, ORPHYN_WEBSITE_URL } from './config'

/** Discord activity types (discord-api-types `ActivityType`). */
export const ACTIVITY_LISTENING = 2
export const ACTIVITY_WATCHING = 3

/** The subset of the RPC `SetActivity` payload Orphyn uses. */
export interface Activity {
  type: number
  details: string
  state?: string
  startTimestamp?: number
  endTimestamp?: number
  largeImageKey: string
  largeImageText?: string
  smallImageKey: string
  smallImageText?: string
  largeImageUrl?: string
  smallImageUrl?: string
}

const SECRET_PARAMS = ['api_key', 'apikey', 'x-emby-token', 'x-mediabrowser-token', 'token']

/** Hosts a Discord client cannot fetch from the internet: localhost, LAN, link-local, .local. */
export function isPublicHost(hostname: string): boolean {
  const host = hostname.toLowerCase().replace(/^\[|\]$/g, '')
  if (!host || host === 'localhost' || host.endsWith('.localhost')) return false
  if (host.endsWith('.local') || host.endsWith('.lan') || host.endsWith('.internal')) return false
  if (!host.includes('.') && !host.includes(':')) return false // bare machine name
  const v4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(host)
  if (v4) {
    const [a, b] = [Number(v4[1]), Number(v4[2])]
    if (a === 10 || a === 127 || a === 0 || a >= 224) return false
    if (a === 169 && b === 254) return false
    if (a === 172 && b >= 16 && b <= 31) return false
    if (a === 192 && b === 168) return false
    if (a === 100 && b >= 64 && b <= 127) return false // CGNAT / Tailscale
    return true
  }
  if (host.includes(':')) {
    // IPv6: only global unicast (2000::/3) counts as public.
    return /^[23]/.test(host)
  }
  return true
}

/**
 * The artwork URL that may be sent to Discord, or undefined. It must be https on a public host
 * (Discord's servers fetch it), and never carries credentials: secret query params are dropped,
 * and a URL with user info is refused.
 */
export function safeArtworkUrl(raw: string | undefined, showArtwork: boolean): string | undefined {
  if (!showArtwork || !raw) return undefined
  let url: URL
  try {
    url = new URL(raw)
  } catch {
    return undefined
  }
  if (url.protocol !== 'https:' || url.username || url.password) return undefined
  if (!isPublicHost(url.hostname)) return undefined
  for (const key of [...url.searchParams.keys()]) {
    if (SECRET_PARAMS.includes(key.toLowerCase())) url.searchParams.delete(key)
  }
  url.hash = ''
  const out = url.toString()
  // Discord limits image strings to 256 characters.
  return out.length <= 256 ? out : undefined
}

const clip = (s: string, max = 128): string => (s.length > max ? `${s.slice(0, max - 1)}…` : s)

const episodeCode = (season?: number, episode?: number): string => {
  const s = season !== undefined ? `S${season}` : ''
  const e = episode !== undefined ? `E${episode}` : ''
  return `${s}${e}`
}

/** Builds the Discord activity for what is playing; null when nothing is. */
export function buildActivity(
  presence: DiscordPresence | null,
  settings: Pick<DiscordSettings, 'showArtwork'>,
  now: number = Date.now(),
): Activity | null {
  if (!presence || !presence.title.trim()) return null

  let details: string
  let state: string | undefined
  let largeText: string
  if (presence.kind === 'music') {
    details = presence.title
    state = [presence.artist, presence.album].filter(Boolean).join(' — ') || undefined
    largeText = presence.album || presence.title
  } else if (presence.series) {
    const code = episodeCode(presence.season, presence.episode)
    details = `${presence.series} · ${code ? `${code} — ` : ''}${presence.title}`
    state = presence.series
    largeText = presence.series
  } else {
    details = presence.title
    state = presence.year ? String(presence.year) : undefined
    largeText = presence.title
  }

  const art = safeArtworkUrl(presence.artworkUrl, settings.showArtwork)
  const activity: Activity = {
    type: presence.kind === 'music' ? ACTIVITY_LISTENING : ACTIVITY_WATCHING,
    details: clip(details),
    state: state ? clip(state) : undefined,
    largeImageKey: art ?? ASSET_LOGO,
    largeImageText: clip(largeText),
    smallImageKey: ASSET_LOGO,
    smallImageText: 'Orphyn',
    largeImageUrl: ORPHYN_WEBSITE_URL,
    smallImageUrl: ORPHYN_WEBSITE_URL,
  }

  if (presence.paused) {
    activity.state = clip(state ? `Paused · ${state}` : 'Paused')
    activity.smallImageKey = ASSET_PAUSE
    activity.smallImageText = 'Paused'
  } else {
    const start = now - Math.max(0, presence.positionSec) * 1000
    activity.startTimestamp = Math.round(start)
    if (presence.durationSec > 0) {
      activity.endTimestamp = Math.round(start + presence.durationSec * 1000)
    }
  }
  return activity
}
