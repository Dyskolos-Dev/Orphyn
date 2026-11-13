export const APP_CHANNELS = {
  refresh: 'orphyn:app:refresh',
  openSettings: 'orphyn:app:open-settings',
} as const

/** Actions triggered from the native application menu. */
export interface AppApi {
  onRefresh(listener: () => void): () => void
  onOpenSettings(listener: () => void): () => void
}
