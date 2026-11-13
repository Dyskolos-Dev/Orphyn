export const DISCORD_CHANNELS = {
  getState: 'lumen:discord:get-state',
  setPresence: 'lumen:discord:set-presence',
  setSettings: 'lumen:discord:set-settings',
  stateChanged: 'lumen:discord:state-changed',
} as const

/** What is playing, as the renderer sees it. Never contains a token: artwork URLs are bare. */
interface PresenceBase {
  paused: boolean
  title: string
  positionSec: number
  durationSec: number
  /** Artwork URL on the Jellyfin server (no api_key); the main process decides whether to use it. */
  artworkUrl?: string
}

export interface MusicPresence extends PresenceBase {
  kind: 'music'
  artist?: string
  album?: string
}

export interface VideoPresence extends PresenceBase {
  kind: 'video'
  /** Set for an episode: `title` is then the episode title. */
  series?: string
  season?: number
  episode?: number
  year?: number
}

export type DiscordPresence = MusicPresence | VideoPresence

export interface DiscordSettings {
  /** "Show what I am playing on Discord". */
  enabled: boolean
  /** Send the artwork link (it points to the server) instead of the Lumen logo. */
  showArtwork: boolean
}

/**
 * - `not-configured`: DISCORD_CLIENT_ID is still the placeholder, the feature is inactive.
 * - `disabled`: the user turned it off.
 * - `connected` / `unavailable` (Discord is not running; retried quietly).
 * - `idle`: enabled, no connection attempt made yet (nothing has played).
 */
export type DiscordStatus = 'not-configured' | 'disabled' | 'idle' | 'connected' | 'unavailable'

export interface DiscordState extends DiscordSettings {
  status: DiscordStatus
}

export interface DiscordApi {
  getState(): Promise<DiscordState>
  /** `null` clears the activity. */
  setPresence(presence: DiscordPresence | null): Promise<void>
  setSettings(settings: Partial<DiscordSettings>): Promise<void>
  onStateChange(listener: (state: DiscordState) => void): () => void
}
