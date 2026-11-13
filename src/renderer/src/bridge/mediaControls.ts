/**
 * OS media controls bridge (Windows SMTC, macOS Now Playing, Linux MPRIS), backed by
 * `src-tauri/src/media_controls/`.
 *
 * The OS has ONE media session per app, but Orphyn has two players (music and video). The hub
 * below lets each player `claim` it: the latest claimer owns the session (its metadata and
 * playback are published, the OS buttons go to its handler); releasing hands it back to the
 * previous owner, which is asked to republish. That is what avoids two entries in the OS panel.
 */
import {
  MEDIA_CONTROLS_CHANNELS as C,
  type MediaControlAction,
  type MediaControlsApi,
  type MediaControlsMetadata,
  type MediaControlsPlayback,
} from '@shared/ipc/mediaControls'
import { call, subscribe } from './tauri'

export const mediaControlsApi: MediaControlsApi = {
  setMetadata: (metadata) => call('media_controls_set_metadata', { metadata }),
  setPlayback: (playback) => call('media_controls_set_playback', { playback }),
  clear: () => call('media_controls_clear'),
  onAction: (listener) => subscribe<MediaControlAction>(C.action, listener),
}

export interface MediaControlsOwner {
  /** Receives the OS buttons while this owner holds the session. */
  onAction(action: MediaControlAction): void
  /** Called when the session comes back to this owner: publish metadata and playback again. */
  refresh?(): void
}

export interface MediaControlsClaim {
  setMetadata(metadata: MediaControlsMetadata): void
  setPlayback(playback: MediaControlsPlayback): void
  /** Gives the session back (and clears it when nobody else holds it). */
  release(): void
}

/** The native controls of this shell, or undefined (Electron and the browser use `navigator.mediaSession`). */
export const nativeMediaControls = (): MediaControlsApi | undefined =>
  typeof window === 'undefined' ? undefined : window.orphyn?.mediaControls

const owners: MediaControlsOwner[] = []
let unsubscribe: (() => void) | null = null

const top = (): MediaControlsOwner | undefined => owners[owners.length - 1]

/** Takes the OS media session. Returns undefined when the shell has no native controls. */
export function claimMediaControls(
  owner: MediaControlsOwner,
  api: MediaControlsApi | undefined = nativeMediaControls(),
): MediaControlsClaim | undefined {
  if (!api) return undefined
  owners.push(owner)
  unsubscribe ??= api.onAction((action) => top()?.onAction(action))
  const mine = () => top() === owner
  return {
    setMetadata: (m) => {
      if (mine()) void api.setMetadata(m).catch(() => undefined)
    },
    setPlayback: (p) => {
      if (mine()) void api.setPlayback(p).catch(() => undefined)
    },
    release: () => {
      const i = owners.indexOf(owner)
      if (i < 0) return
      const wasTop = i === owners.length - 1
      owners.splice(i, 1)
      if (!wasTop) return
      const next = top()
      if (next?.refresh) next.refresh()
      else void api.clear().catch(() => undefined)
      if (!owners.length) {
        unsubscribe?.()
        unsubscribe = null
      }
    },
  }
}
