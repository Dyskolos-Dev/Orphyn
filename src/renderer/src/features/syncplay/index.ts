// Watch / listen together (Jellyfin SyncPlay). Public API of the feature; see docs/SYNCPLAY.md.
import { registerShellExtension } from '@renderer/shell'
import { createMusicBridge } from './music'
import { syncPlayManager } from './manager'
import { SyncPlayHost } from './ui'

syncPlayManager.setMusicBridge(
  createMusicBridge((ids, mode) => syncPlayManager.queueItems(ids, mode)),
)

// Always mounted: navigation for the group, toasts and the sheet.
registerShellExtension({ id: 'syncplay', background: SyncPlayHost })

export { syncPlayManager, useSyncPlay } from './manager'
export type { MediaKind, Phase, SyncPlayState } from './manager'
export { SyncPlayBadge, SyncPlayButton } from './ui'
export { useSyncPlayVideo } from './video'
export type { SyncPlayVideo } from './video'
