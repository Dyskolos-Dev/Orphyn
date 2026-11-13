/** OS media controls bridge: stubs until the media-controls task fills `src-tauri/src/media_controls/`. */
import {
  MEDIA_CONTROLS_CHANNELS as C,
  type MediaControlAction,
  type MediaControlsApi,
} from '@shared/ipc/mediaControls'
import { call, subscribe } from './tauri'

export const mediaControlsApi: MediaControlsApi = {
  setMetadata: (metadata) => call('media_controls_set_metadata', { metadata }),
  setPlayback: (playback) => call('media_controls_set_playback', { playback }),
  clear: () => call('media_controls_clear'),
  onAction: (listener) => subscribe<MediaControlAction>(C.action, listener),
}
