/** Placeholder application id: while it is set, Rich Presence stays inactive. */
export const DISCORD_CLIENT_ID_PLACEHOLDER = '000000000000000000'

// Public application id of the Discord application named Lumen (see docs/DISCORD.md). Empty or the
// placeholder above keeps Rich Presence inactive (Settings shows 'Not configured').
export const DISCORD_CLIENT_ID = '1570792208716537866'

export const isDiscordConfigured = (id: string = DISCORD_CLIENT_ID): boolean =>
  /^\d{17,20}$/.test(id) && id !== DISCORD_CLIENT_ID_PLACEHOLDER

/** Art asset keys uploaded to the Discord application. */
export const ASSET_LOGO = 'lumen'
export const ASSET_PAUSE = 'pause'

/** Discord allows 5 updates / 20 s: stay well under it. */
export const MIN_UPDATE_INTERVAL_MS = 2000
export const RECONNECT_INTERVAL_MS = 30_000
