/**
 * Which web engine renders Orphyn, and what it can play. WebView2 (Windows) is Chromium;
 * the app uses WKWebView on macOS and WebKitGTK (GStreamer) on Linux. The
 * Jellyfin device profile (`src/renderer/src/jellyfin/playback/profile.ts`) must follow it.
 */

export type WebviewEngine = 'chromium' | 'webview2' | 'wkwebview' | 'webkitgtk'

/** Injected by the Tauri shell before the page loads (`src-tauri/src/window.rs`). */
export interface ShellInfo {
  platform: 'win32' | 'darwin' | 'linux'
  webview: Exclude<WebviewEngine, 'chromium'>
}

declare global {
  var __ORPHYN_SHELL__: ShellInfo | undefined
}

/** Shell info when running inside the app, null in a plain browser or tests. */
export function getShellInfo(): ShellInfo | null {
  return globalThis.__ORPHYN_SHELL__ ?? null
}

export function currentWebviewEngine(): WebviewEngine {
  return getShellInfo()?.webview ?? 'chromium'
}

/** Same shape as `CodecSupport` in the renderer's playback profile. */
export interface WebviewCodecSupport {
  h264: boolean
  hevc: boolean
  vp9: boolean
  av1: boolean
  ac3: boolean
}

export interface WebviewMediaPolicy {
  engine: WebviewEngine
  /** DeviceProfile `Name`. */
  profileName: string
  /**
   * Upper bound on what direct play may use: AND it with `canPlayType` detection (a webview may
   * answer "maybe" for a codec it then fails to play in that container).
   */
  allow: WebviewCodecSupport
  /** Audio codecs accepted inside MP4 for direct play. */
  mp4AudioCodecs: string[]
  /** WebM / Ogg / Opus containers can be direct played. */
  webm: boolean
  /** `<video>` plays HLS itself: give it the .m3u8 URL instead of going through hls.js. */
  nativeHls: boolean
  /** Prefer server transcoding to h264/aac over direct play of anything else. */
  preferTranscode: boolean
  /** Bitrate cap to advertise, when lower than the user's choice. */
  maxStreamingBitrate?: number
}

const ALL: WebviewCodecSupport = { h264: true, hevc: true, vp9: true, av1: true, ac3: true }

export function mediaPolicyFor(engine: WebviewEngine): WebviewMediaPolicy {
  switch (engine) {
    case 'wkwebview':
      // AVFoundation: H.264 / HEVC, AAC / ALAC / AC-3 / E-AC-3, HLS natively. No VP9 or Opus in MP4.
      return {
        engine,
        profileName: 'Orphyn (macOS)',
        allow: { h264: true, hevc: true, vp9: false, av1: false, ac3: true },
        mp4AudioCodecs: ['aac', 'mp3', 'alac', 'flac', 'ac3', 'eac3'],
        webm: false,
        nativeHls: true,
        preferTranscode: false,
      }
    case 'webkitgtk':
      // GStreamer: H.264 / AAC are the safe pair; anything else is transcoded.
      return {
        engine,
        profileName: 'Orphyn (Linux)',
        allow: { h264: true, hevc: false, vp9: false, av1: false, ac3: false },
        mp4AudioCodecs: ['aac', 'mp3'],
        webm: false,
        nativeHls: false,
        preferTranscode: true,
        maxStreamingBitrate: 40_000_000,
      }
    case 'webview2':
    case 'chromium':
      return {
        engine,
        profileName: engine === 'webview2' ? 'Orphyn (Windows)' : 'Orphyn',
        allow: { ...ALL },
        mp4AudioCodecs: ['aac', 'mp3', 'opus', 'flac', 'vorbis'],
        webm: true,
        nativeHls: false,
        preferTranscode: false,
      }
  }
}

/** `detected` (from `canPlayType`) restricted to what the engine's policy allows. */
export function restrictSupport(
  detected: WebviewCodecSupport,
  policy: WebviewMediaPolicy,
): WebviewCodecSupport {
  return {
    h264: detected.h264 && policy.allow.h264,
    hevc: detected.hevc && policy.allow.hevc,
    vp9: detected.vp9 && policy.allow.vp9,
    av1: detected.av1 && policy.allow.av1,
    ac3: detected.ac3 && policy.allow.ac3,
  }
}
