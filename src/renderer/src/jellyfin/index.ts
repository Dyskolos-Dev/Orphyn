// Jellyfin data layer barrel (owner: data-layer agent). Screens import only from here.
// See docs/JELLYFIN.md.
export { JellyfinProvider, queryClient, requireApi, useSession } from './session'
export type { SessionState, SessionStatus } from './session'
export { connectServer, login, logoutAccount } from './auth'
export type { ConnectParams, ConnectResult, LoginError, LoginResult } from './auth'
export { probeServer } from './probe'
export type { ProbeError, ProbeErrorKind, ProbeResult, ServerInfo } from './probe'
export { normalizeServerUrl, urlCandidates } from './url'
export { buildImageUrl, imageUrl } from './images'
export type { ImageOptions } from './images'
export { jfKeys } from './keys'
export { refreshLibrary, shouldRefreshOnFocus, REFRESH_AFTER_BLUR_MS } from './refresh'
export * from './hooks'
export * from './playlistLogic'
export type { RemovedEntry } from './playlistOps'
export * from './playback'
export { fetchMediaSegments, normalizeSegments, segmentAt } from './segments'
export type { MediaSegment, MediaSegmentType } from './segments'
export type {
  BaseItemDto,
  ImageKind,
  ItemsPage,
  ItemsParams,
  ListKind,
  ListParams,
  Lyrics,
  SearchResults,
  TimedLyricLine,
} from './types'
