/**
 * What SyncPlay needs from a player. The video player and the music player each provide one
 * (see video.ts and music.ts); the players themselves are not changed.
 */
export interface SyncPlayer {
  readonly kind: 'video' | 'audio'
  /** Jellyfin id of the item loaded (normalised, no dashes), or null. */
  itemId(): string | null
  /** Current position (s). */
  position(): number
  /** Audio / picture is moving. */
  isPlaying(): boolean
  /** Enough data to start or keep playing at the current position. */
  isReady(): boolean
  play(): void
  pause(): void
  seek(seconds: number): void
  /** Speed control for gentle drift correction; players without it only seek. */
  setRate?(rate: number): void
  /** Called on any change of the above. */
  subscribe(listener: () => void): () => void
}

export type Notice =
  | { kind: 'userJoined'; name: string }
  | { kind: 'userLeft'; name: string }
  | { kind: 'paused'; by: string | null }
  | { kind: 'resumed'; by: string | null }
  | { kind: 'seeked'; by: string | null }
  | { kind: 'nowPlaying'; title: string; by: string | null }
  | { kind: 'waiting'; name: string | null }
  | { kind: 'joined'; group: string }
  | { kind: 'left' }
  | { kind: 'groupClosed' }
  | { kind: 'connectionLost' }
  | { kind: 'reconnected' }
  | { kind: 'itemUnavailable'; title: string | null }
  | { kind: 'libraryDenied' }
  | { kind: 'joinDenied' }
  | { kind: 'createDenied' }
  | { kind: 'error' }
