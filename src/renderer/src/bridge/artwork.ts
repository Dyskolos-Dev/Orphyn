/**
 * Online artist images. The Rust command (`src-tauri/src/artwork/`) queries Deezer / TheAudioDB
 * because those APIs send no CORS headers. Only exists in the Tauri shell: in a plain browser
 * (and in tests) `available()` is false and nothing is requested.
 */
import { getShellInfo } from '@shared/webview'
import { call } from './tauri'

export const artworkApi = {
  /** True when the native bridge is there to answer. */
  available: (): boolean => getShellInfo() != null,
  /** An https image URL for the artist, or `null` when none reliable was found. */
  artistImage: (name: string): Promise<string | null> =>
    call<string | null>('artwork_artist_image', { name }),
}
